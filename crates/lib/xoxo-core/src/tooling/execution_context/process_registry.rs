use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tokio::process::ChildStdin;
use tokio::sync::Mutex;

use crate::helpers::new_id;

// ---------------------------------------------------------------------------
// ManagedProcess
// ---------------------------------------------------------------------------

/// A child process owned by an agent's execution context.
///
/// stdout and stderr are streamed into in-memory buffers by background tasks.
/// Consumers read from those buffers at a byte offset via `poll` — reads are
/// non-destructive and idempotent. stdin is held in a `Mutex<Option<…>>` so
/// write operations can be serialised; it is set to `None` when the process
/// exits or stdin is explicitly closed.
pub struct ManagedProcess {
    pub process_id: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    /// OS PID, available from the moment the process is spawned.
    pub pid: Option<u32>,
    /// Write end of the child's stdin pipe.
    pub stdin: Mutex<Option<ChildStdin>>,
    /// Accumulated stdout output. Appended by a background reader task.
    pub stdout_buf: Arc<Mutex<String>>,
    /// Accumulated stderr output. Appended by a background reader task.
    pub stderr_buf: Arc<Mutex<String>>,
    /// `true` while the process is still running.
    pub running: Arc<AtomicBool>,
    /// Exit code set by the waiter task when the process terminates.
    pub exit_code: Arc<Mutex<Option<i32>>>,
    /// Signal name set by the waiter task (Unix only).
    pub signal_name: Arc<Mutex<Option<String>>>,
}

/// Serialisable summary returned to the LLM.
#[derive(Serialize)]
pub struct ProcessSummary {
    pub process_id: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub pid: Option<u32>,
    pub running: bool,
    pub exit_code: Option<i32>,
    pub signal: Option<String>,
}

impl ManagedProcess {
    pub async fn summary(&self) -> ProcessSummary {
        ProcessSummary {
            process_id: self.process_id.clone(),
            command: self.command.clone(),
            args: self.args.clone(),
            cwd: self.cwd.clone(),
            pid: self.pid,
            running: self.running.load(Ordering::SeqCst),
            exit_code: *self.exit_code.lock().await,
            signal: self.signal_name.lock().await.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// ProcessRegistry
// ---------------------------------------------------------------------------

/// Agent-scoped registry of managed child processes.
///
/// Processes are keyed by a UUID `process_id` assigned at spawn time.
/// The registry is held in `AgentExecutionContext` and shut down with the
/// agent — all running processes receive a best-effort `SIGTERM` on
/// `shutdown_all`.
pub struct ProcessRegistry {
    processes: Mutex<HashMap<String, Arc<ManagedProcess>>>,
}

impl ProcessRegistry {
    pub fn new() -> Self {
        Self { processes: Mutex::new(HashMap::new()) }
    }

    /// Spawn a new child process and register it.
    ///
    /// Merges `env_overrides` on top of the current process environment so
    /// callers can supply targeted overrides (e.g. `PORT=3000`) without
    /// having to re-specify `PATH` and other essentials.
    pub async fn start(
        &self,
        command: String,
        args: Vec<String>,
        cwd: Option<String>,
        env_overrides: HashMap<String, String>,
    ) -> Result<Arc<ManagedProcess>, String> {
        use std::process::Stdio;
        use tokio::io::AsyncReadExt;

        let mut cmd = tokio::process::Command::new(&command);
        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(std::env::vars())
            .envs(&env_overrides);

        if let Some(ref dir) = cwd {
            cmd.current_dir(dir);
        }

        let mut child = cmd.spawn().map_err(|e| format!("failed to spawn process: {e}"))?;

        let pid = child.id();
        let stdin = child.stdin.take().expect("stdin piped");
        let mut stdout_pipe = child.stdout.take().expect("stdout piped");
        let mut stderr_pipe = child.stderr.take().expect("stderr piped");

        let stdout_buf = Arc::new(Mutex::new(String::new()));
        let stderr_buf = Arc::new(Mutex::new(String::new()));
        let running = Arc::new(AtomicBool::new(true));
        let exit_code: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));
        let signal_name: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        // Background task: stream stdout into buffer.
        let stdout_buf_clone = Arc::clone(&stdout_buf);
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match stdout_pipe.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let chunk = String::from_utf8_lossy(&buf[..n]).into_owned();
                        stdout_buf_clone.lock().await.push_str(&chunk);
                    }
                }
            }
        });

        // Background task: stream stderr into buffer.
        let stderr_buf_clone = Arc::clone(&stderr_buf);
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match stderr_pipe.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let chunk = String::from_utf8_lossy(&buf[..n]).into_owned();
                        stderr_buf_clone.lock().await.push_str(&chunk);
                    }
                }
            }
        });

        // Background task: wait for process exit; update status fields.
        let running_clone = Arc::clone(&running);
        let exit_code_clone = Arc::clone(&exit_code);
        let signal_name_clone = Arc::clone(&signal_name);
        tokio::spawn(async move {
            match child.wait().await {
                Ok(status) => {
                    *exit_code_clone.lock().await = status.code();
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::ExitStatusExt;
                        if let Some(sig) = status.signal() {
                            *signal_name_clone.lock().await =
                                Some(signal_number_to_name(sig).to_string());
                        }
                    }
                    let _ = signal_name_clone; // suppress unused warning on non-unix
                }
                Err(_) => {}
            }
            running_clone.store(false, Ordering::SeqCst);
        });

        let process_id = new_id();
        let managed = Arc::new(ManagedProcess {
            process_id: process_id.clone(),
            command,
            args,
            cwd,
            pid,
            stdin: Mutex::new(Some(stdin)),
            stdout_buf,
            stderr_buf,
            running,
            exit_code,
            signal_name,
        });

        self.processes.lock().await.insert(process_id, Arc::clone(&managed));
        Ok(managed)
    }

    /// Look up a managed process by ID.
    pub async fn get(&self, process_id: &str) -> Option<Arc<ManagedProcess>> {
        self.processes.lock().await.get(process_id).cloned()
    }

    /// Return all managed processes.
    pub async fn list(&self) -> Vec<Arc<ManagedProcess>> {
        self.processes.lock().await.values().cloned().collect()
    }

    /// Terminate all running processes. Called on agent shutdown.
    pub async fn shutdown_all(&self) {
        let processes: Vec<Arc<ManagedProcess>> =
            self.processes.lock().await.values().cloned().collect();
        for p in processes {
            if p.running.load(Ordering::SeqCst) {
                if let Some(pid) = p.pid {
                    let _ = terminate_pid(pid, "SIGTERM");
                }
            }
            // Close stdin so the process sees EOF.
            *p.stdin.lock().await = None;
        }
    }

    /// Send a signal to a process by ID.
    ///
    /// Returns an error string if the process is not found or the signal name
    /// is not part of the portable supported subset.
    pub async fn send_signal(
        &self,
        process_id: &str,
        signal: &str,
    ) -> Result<Arc<ManagedProcess>, String> {
        let process = self
            .get(process_id)
            .await
            .ok_or_else(|| format!("unknown process: {process_id}"))?;

        if let Some(pid) = process.pid {
            terminate_pid(pid, signal)?;
        }

        Ok(process)
    }
}

// ---------------------------------------------------------------------------
// Signal helpers
// ---------------------------------------------------------------------------

/// Send `signal` to `pid` using the portable supported subset:
/// `SIGTERM`, `SIGKILL`, `SIGINT`.
fn terminate_pid(pid: u32, signal: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use nix::sys::signal::kill;
        use nix::unistd::Pid;

        let sig = parse_signal(signal)?;
        kill(Pid::from_raw(pid as i32), sig).map_err(|err| err.to_string())?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        let normalized = normalize_portable_signal(signal)?;
        let mut command = std::process::Command::new("taskkill");
        command.arg("/PID").arg(pid.to_string()).arg("/T");

        if normalized == "SIGKILL" {
            command.arg("/F");
        }

        let output = command
            .output()
            .map_err(|err| format!("failed to run taskkill: {err}"))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "taskkill failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }

    #[cfg(not(any(unix, target_os = "windows")))]
    {
        let _ = pid;
        let _ = normalize_portable_signal(signal)?;
        Err("process signaling is not implemented for this platform".to_string())
    }
}

#[cfg(unix)]
fn parse_signal(name: &str) -> Result<nix::sys::signal::Signal, String> {
    use nix::sys::signal::Signal;
    match normalize_portable_signal(name)?.as_str() {
        "SIGTERM" => Ok(Signal::SIGTERM),
        "SIGKILL" => Ok(Signal::SIGKILL),
        "SIGINT" => Ok(Signal::SIGINT),
        _ => Err(format!("unsupported signal: {name}")),
    }
}

fn normalize_portable_signal(name: &str) -> Result<String, String> {
    match name {
        "SIGTERM" | "15" => Ok("SIGTERM".to_string()),
        "SIGKILL" | "9" => Ok("SIGKILL".to_string()),
        "SIGINT" | "2" => Ok("SIGINT".to_string()),
        _ => Err(format!(
            "unsupported signal: {name}; portable supported signals: SIGTERM, SIGKILL, SIGINT"
        )),
    }
}

#[cfg(unix)]
fn signal_number_to_name(sig: i32) -> &'static str {
    match sig {
        1  => "SIGHUP",
        2  => "SIGINT",
        9  => "SIGKILL",
        10 => "SIGUSR1",
        12 => "SIGUSR2",
        15 => "SIGTERM",
        18 => "SIGCONT",
        19 => "SIGSTOP",
        _  => "UNKNOWN",
    }
}
