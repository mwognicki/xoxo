use std::collections::HashMap;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, ChildStderr, Command};
use tokio::time::{Duration, timeout};

/// Errors produced by a [`BashSession`].
#[derive(Debug)]
pub enum BashSessionError {
    /// Failed to spawn the bash child process.
    SpawnFailed(std::io::Error),
    /// I/O error while communicating with the bash process.
    IoError(std::io::Error),
    /// The command did not complete within the allotted time.
    Timeout,
}

impl std::fmt::Display for BashSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BashSessionError::SpawnFailed(e) => write!(f, "failed to spawn bash: {e}"),
            BashSessionError::IoError(e) => write!(f, "bash I/O error: {e}"),
            BashSessionError::Timeout => write!(f, "command timed out"),
        }
    }
}

impl std::error::Error for BashSessionError {}

/// Output captured from a single command execution.
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    /// Exit code of the command. `-1` if the session timed out.
    pub exit_code: i32,
    pub timed_out: bool,
}

/// Options controlling how a [`BashSession`] is spawned.
///
/// Passed to [`BashSession::spawn`] and [`AgentExecutionContext::new`].
/// Designed for extension: new privilege controls (chroot, allowed env vars,
/// cgroup limits) are added here without changing call sites that use
/// `BashOptions::default()`.
///
/// [`AgentExecutionContext::new`]: super::ToolExecutionContext::new
#[derive(Debug, Clone)]
pub struct BashOptions {
    /// Spawn a restricted shell (`rbash`) instead of a full bash session.
    ///
    /// Restricted mode disables: `cd`, changes to `PATH`/`SHELL`/`ENV`,
    /// output redirections (`>`), and executing commands by absolute path.
    /// Use for agents that must not be able to escape a controlled environment.
    ///
    /// Note: `rbash` does not provide a security boundary on its own — combine
    /// with OS-level controls (Kubernetes pod security context, seccomp,
    /// namespaces) for true isolation.
    pub restricted: bool,
    /// Whether to spawn a login shell (`bash --login`).
    ///
    /// When `true` (the default), bash is invoked with `--login` so it sources
    /// the standard login init files in order:
    /// 1. `/etc/profile` — on macOS runs `path_helper`, which reads
    ///    `/etc/paths` and `/etc/paths.d/` and adds entries like
    ///    `/usr/local/go/bin`.
    /// 2. `~/.bash_profile` (or `~/.bash_login` / `~/.profile`) — Rustup
    ///    injects `source "$HOME/.cargo/env"` here at install time, making
    ///    `cargo` and `rustc` available.
    ///
    /// Set to `false` for truly isolated or restricted agents that must not
    /// execute user-controlled init scripts.
    pub login: bool,
    /// Whether the bash session inherits the parent process environment.
    ///
    /// When `true` (the default), the child bash process inherits the full
    /// environment of the parent process — including `PATH`, `HOME`, `USER`,
    /// `KUBECONFIG`, and any other variables set at runtime. `env_vars`
    /// overrides are applied on top.
    ///
    /// When `false`, the session starts with a clean environment (`env_clear`)
    /// and only the variables in `env_vars` are set. Use this for agents that
    /// must not be able to see the parent's environment.
    pub inherit_env: bool,
    /// Extra environment variables applied to the bash session.
    ///
    /// When `inherit_env` is `true`, these override variables inherited from
    /// the parent. When `inherit_env` is `false`, these are the only variables
    /// available to the session.
    pub env_vars: HashMap<String, String>,
}

impl Default for BashOptions {
    fn default() -> Self {
        Self {
            restricted: false,
            login: true,
            inherit_env: true,
            env_vars: HashMap::new(),
        }
    }
}

/// Sentinel written to stdout after each command: `--DONE--{exit_code}--`
const STDOUT_SENTINEL_PREFIX: &str = "--DONE--";
/// Sentinel written to stderr after each command.
const STDERR_SENTINEL: &str = "--DONE--";

/// A persistent bash process owned by a single agent instance.
///
/// Commands are executed sequentially on the same bash process, so shell
/// state (working directory, environment variables, shell functions) persists
/// across calls. Two agents never share a session.
///
/// # Sentinel protocol
///
/// After each user command, the session appends:
/// ```sh
/// __ec__=$?
/// printf '%s\n' "--DONE--$__ec__--"          # stdout sentinel with exit code
/// printf '%s\n' "--DONE--" >&2               # stderr sentinel
/// ```
/// Both stdout and stderr readers block until they see their respective
/// sentinel, enabling concurrent draining of both streams.
pub struct BashSession {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    stderr: BufReader<ChildStderr>,
}

impl BashSession {
    /// Spawn a new bash session with the given options.
    pub async fn spawn(options: BashOptions) -> Result<Self, BashSessionError> {
        let shell = if options.restricted { "rbash" } else { "bash" };
        let mut cmd = Command::new(shell);
        if options.login && !options.restricted {
            cmd.arg("--login");
        }
        cmd.arg("-s")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if !options.inherit_env {
            cmd.env_clear();
        }
        cmd.envs(&options.env_vars);
        let mut child = cmd.spawn().map_err(BashSessionError::SpawnFailed)?;

        let stdin = BufWriter::new(child.stdin.take().expect("stdin piped"));
        let stdout = BufReader::new(child.stdout.take().expect("stdout piped"));
        let stderr = BufReader::new(child.stderr.take().expect("stderr piped"));

        Ok(Self { child, stdin, stdout, stderr })
    }

    /// Run a shell command and return its output.
    ///
    /// The command runs on the persistent session — working directory, env
    /// vars, and shell functions carry over from previous calls.
    ///
    /// If `timeout_secs` elapses before the sentinels are received, returns
    /// `CommandOutput { timed_out: true, exit_code: -1, ... }`. The session
    /// is left in an undefined state after a timeout; callers should call
    /// [`kill`] and discard the session.
    ///
    /// [`kill`]: BashSession::kill
    pub async fn run_command(
        &mut self,
        cmd: &str,
        timeout_secs: u64,
    ) -> Result<CommandOutput, BashSessionError> {
        let script = format!(
            "{cmd}\n__ec__=$?\nprintf '%s\\n' \"--DONE--$__ec__--\"\nprintf '%s\\n' \"--DONE--\" >&2\n"
        );
        self.stdin
            .write_all(script.as_bytes())
            .await
            .map_err(BashSessionError::IoError)?;
        self.stdin.flush().await.map_err(BashSessionError::IoError)?;

        let mut stdout_buf = String::new();
        let mut stderr_buf = String::new();
        let mut exit_code = 0i32;
        let mut stdout_done = false;
        let mut stderr_done = false;
        let mut stdout_line = String::new();
        let mut stderr_line = String::new();

        let read_result = timeout(Duration::from_secs(timeout_secs), async {
            while !stdout_done || !stderr_done {
                tokio::select! {
                    result = self.stdout.read_line(&mut stdout_line), if !stdout_done => {
                        result.map_err(BashSessionError::IoError)?;
                        if stdout_line.starts_with(STDOUT_SENTINEL_PREFIX) {
                            let trimmed = stdout_line.trim();
                            if let Some(inner) = trimmed
                                .strip_prefix(STDOUT_SENTINEL_PREFIX)
                                .and_then(|s| s.strip_suffix("--"))
                            {
                                exit_code = inner.parse().unwrap_or(0);
                            }
                            stdout_done = true;
                        } else {
                            stdout_buf.push_str(&stdout_line);
                        }
                        stdout_line.clear();
                    }
                    result = self.stderr.read_line(&mut stderr_line), if !stderr_done => {
                        result.map_err(BashSessionError::IoError)?;
                        if stderr_line.trim() == STDERR_SENTINEL {
                            stderr_done = true;
                        } else {
                            stderr_buf.push_str(&stderr_line);
                        }
                        stderr_line.clear();
                    }
                }
            }
            Ok::<_, BashSessionError>(())
        })
        .await;

        match read_result {
            Ok(Ok(())) => Ok(CommandOutput {
                stdout: stdout_buf,
                stderr: stderr_buf,
                exit_code,
                timed_out: false,
            }),
            Ok(Err(e)) => Err(e),
            Err(_elapsed) => Ok(CommandOutput {
                stdout: stdout_buf,
                stderr: stderr_buf,
                exit_code: -1,
                timed_out: true,
            }),
        }
    }

    /// Kill the bash process and wait for it to exit.
    pub async fn kill(&mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}
