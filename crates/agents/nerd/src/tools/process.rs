use std::collections::HashMap;
use std::sync::Arc;

use serde::Deserialize;
use tokio::io::AsyncWriteExt;
use agentix::tooling::{
    ErasedTool, ManagedProcess, Tool, ToolContext, ToolError, ToolExecutionContext,
    ToolRegistration, ToolSchema,
};
// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(crate) enum ProcessInput {
    Start(StartInput),
    List,
    Poll(PollInput),
    Write(WriteInput),
    SendKeys(WriteInput),
    Paste(WriteInput),
    Submit(SubmitInput),
    Terminate(TerminateInput),
}

#[derive(Deserialize)]
pub(crate) struct StartInput {
    pub(crate) command: String,
    #[serde(default)]
    pub(crate) args: Vec<String>,
    pub(crate) cwd: Option<String>,
    #[serde(default)]
    pub(crate) env: HashMap<String, String>,
}

#[derive(Deserialize)]
pub(crate) struct PollInput {
    pub(crate) process_id: String,
    #[serde(default)]
    pub(crate) stdout_offset: usize,
    #[serde(default)]
    pub(crate) stderr_offset: usize,
}

#[derive(Deserialize)]
pub(crate) struct WriteInput {
    pub(crate) process_id: String,
    pub(crate) data: String,
}

#[derive(Deserialize)]
pub(crate) struct SubmitInput {
    pub(crate) process_id: String,
    #[serde(default)]
    pub(crate) data: String,
}

#[derive(Deserialize)]
pub(crate) struct TerminateInput {
    pub(crate) process_id: String,
    pub(crate) signal: Option<String>,
}

// ---------------------------------------------------------------------------
// ProcessTool
// ---------------------------------------------------------------------------

/// Stateful tool for managing long-running child processes.
///
/// Unlike `exec` (which runs a command to completion), `process` keeps child
/// processes alive across tool calls. Agents can start a server, stream its
/// output via `poll`, write to its stdin, and terminate it when done.
///
/// All actions are dispatched through a single tool call via the `action`
/// field. Process state is held in `AgentExecutionContext::process_registry`
/// and torn down with the agent.
///
/// Requires `ToolContext::execution_context` to be `Some`.
///
/// # Environment
///
/// `start` merges the supplied `env` **on top of** the parent process
/// environment, so callers only need to specify overrides (e.g. `PORT=3000`).
///
/// # Actions
///
/// | action      | description                                    |
/// |-------------|------------------------------------------------|
/// | `start`     | Spawn a process; returns its `process_id`      |
/// | `list`      | List all managed processes                     |
/// | `poll`      | Read stdout/stderr from a byte offset          |
/// | `write`     | Write raw bytes to stdin                       |
/// | `send_keys` | Alias of `write`                               |
/// | `paste`     | Alias of `write`                               |
/// | `submit`    | Write `data\n` to stdin                        |
/// | `terminate` | Send a signal (default `SIGTERM`)              |
pub struct ProcessTool;

impl ProcessTool {
    pub fn new() -> Self {
        Self
    }
}

impl Tool for ProcessTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "process".to_string(),
            description: "Manage long-running child processes. \
                Start a process and interact with it across multiple tool calls \
                via its process_id. Requires shell access (execution_context)."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["action"],
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["start", "list", "poll", "write", "send_keys", "paste", "submit", "terminate"],
                        "description": "Operation to perform."
                    },
                    "command": { "type": "string", "description": "start: executable to run." },
                    "args":    { "type": "array", "items": { "type": "string" }, "description": "start: arguments." },
                    "cwd":     { "type": "string", "description": "start: working directory." },
                    "env":     {
                        "type": "object",
                        "additionalProperties": { "type": "string" },
                        "description": "start: env overrides merged on top of parent env."
                    },
                    "process_id": { "type": "string", "description": "ID returned by start." },
                    "stdout_offset": { "type": "integer", "description": "poll: byte offset to read stdout from." },
                    "stderr_offset": { "type": "integer", "description": "poll: byte offset to read stderr from." },
                    "data":   { "type": "string", "description": "write/send_keys/paste/submit: data to write to stdin." },
                    "signal": { "type": "string", "description": "terminate: signal name (default SIGTERM). E.g. SIGKILL, SIGINT." }
                }
            }),
        }
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, ToolError> {
        let exec_ctx = ctx
            .execution_context
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionFailed(
                "process requires an execution context — agent blueprint does not grant shell access"
                    .to_string(),
            ))?;

        let parsed: ProcessInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(e.to_string()))?;

        match parsed {
            ProcessInput::Start(i) => {
                let p = exec_ctx
                    .process_registry
                    .start(i.command, i.args, i.cwd, i.env)
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(e))?;
                Ok(serde_json::json!({
                    "ok": true,
                    "action": "start",
                    "process": p.summary().await,
                }))
            }
            ProcessInput::List => {
                let summaries: Vec<_> = {
                    let procs = exec_ctx.process_registry.list().await;
                    let mut out = Vec::with_capacity(procs.len());
                    for p in procs {
                        out.push(p.summary().await);
                    }
                    out
                };
                Ok(serde_json::json!({
                    "ok": true,
                    "action": "list",
                    "processes": summaries,
                }))
            }
            ProcessInput::Poll(i) => {
                let p = get_process(exec_ctx, &i.process_id).await?;
                let stdout_full = p.stdout_buf.lock().await.clone();
                let stderr_full = p.stderr_buf.lock().await.clone();
                let stdout_slice = char_boundary_slice(&stdout_full, i.stdout_offset);
                let stderr_slice = char_boundary_slice(&stderr_full, i.stderr_offset);
                Ok(serde_json::json!({
                    "ok": true,
                    "action": "poll",
                    "process": p.summary().await,
                    "stdout": stdout_slice,
                    "stderr": stderr_slice,
                    "stdout_offset": stdout_full.len(),
                    "stderr_offset": stderr_full.len(),
                }))
            }
            ProcessInput::Write(i) | ProcessInput::SendKeys(i) | ProcessInput::Paste(i) => {
                let p = get_process(exec_ctx, &i.process_id).await?;
                let bytes = write_stdin(&p, i.data.as_bytes()).await?;
                Ok(serde_json::json!({
                    "ok": true,
                    "action": "write",
                    "process": p.summary().await,
                    "bytes_written": bytes,
                }))
            }
            ProcessInput::Submit(i) => {
                let p = get_process(exec_ctx, &i.process_id).await?;
                let payload = format!("{}\n", i.data);
                let bytes = write_stdin(&p, payload.as_bytes()).await?;
                Ok(serde_json::json!({
                    "ok": true,
                    "action": "submit",
                    "process": p.summary().await,
                    "bytes_written": bytes,
                }))
            }
            ProcessInput::Terminate(i) => {
                let signal = i.signal.as_deref().unwrap_or("SIGTERM");
                let p = exec_ctx
                    .process_registry
                    .send_signal(&i.process_id, signal)
                    .await
                    .map_err(ToolError::InvalidInput)?;
                Ok(serde_json::json!({
                    "ok": true,
                    "action": "terminate",
                    "process": p.summary().await,
                }))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn get_process(
    exec_ctx: &ToolExecutionContext,
    process_id: &str,
) -> Result<Arc<ManagedProcess>, ToolError> {
    exec_ctx
        .process_registry
        .get(process_id)
        .await
        .ok_or_else(|| ToolError::InvalidInput(format!("unknown process: {process_id}")))
}

async fn write_stdin(
    process: &ManagedProcess,
    data: &[u8],
) -> Result<usize, ToolError> {
    use std::sync::atomic::Ordering;

    if !process.running.load(Ordering::SeqCst) {
        return Err(ToolError::ExecutionFailed(format!(
            "process {} is not running",
            process.process_id
        )));
    }

    let mut guard = process.stdin.lock().await;
    let stdin = guard.as_mut().ok_or_else(|| {
        ToolError::ExecutionFailed(format!(
            "stdin is closed for process {}",
            process.process_id
        ))
    })?;

    stdin
        .write_all(data)
        .await
        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
    stdin
        .flush()
        .await
        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

    Ok(data.len())
}

/// Slice `s` from `offset` bytes, snapping to the nearest valid UTF-8 boundary.
fn char_boundary_slice(s: &str, offset: usize) -> &str {
    if offset >= s.len() {
        return "";
    }
    let safe = (offset..=s.len())
        .find(|&i| s.is_char_boundary(i))
        .unwrap_or(s.len());
    &s[safe..]
}

inventory::submit! {
    ToolRegistration {
        name: "process",
        factory: || Arc::new(ProcessTool::new()) as Arc<dyn ErasedTool>,
    }
}
