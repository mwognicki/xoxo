use std::sync::Arc;

use serde::Deserialize;
use xoxo_core::tooling::{
    ErasedTool, Tool, ToolContext, ToolError, ToolExecutionContext, ToolRegistration, ToolSchema,
};

const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Maximum bytes kept per stream (stdout or stderr).
/// Output beyond this limit is truncated and a notice is appended.
const OUTPUT_CAP_BYTES: usize = 64 * 1024;
const SNIP_NO_FILTER_MARKER: &str = "snip: no filter for ";

#[derive(Deserialize)]
pub(crate) struct ExecInput {
    pub(crate) command: String,
    pub(crate) timeout_secs: Option<u64>,
    #[serde(default)]
    pub(crate) extended: bool,
}

/// Tool for running shell commands in the agent's persistent bash session.
///
/// Commands run in the same bash process across calls, so working directory,
/// environment variables, and shell functions persist between invocations.
///
/// The bash session is created at agent spawn time with an explicit env
/// allowlist — the parent process environment is never inherited.
///
/// Requires `ToolContext::execution_context` to be `Some`; returns an error
/// if the agent's blueprint does not grant shell access.
///
/// # Input
/// ```json
/// { "command": "kubectl get pods -n default", "timeout_secs": 10, "extended": false }
/// ```
///
/// # Output
/// ```json
/// { "stdout": "...", "stderr": "...", "exit_code": 0, "timed_out": false }
/// ```
pub struct ExecTool;

impl ExecTool {
    pub fn new() -> Self {
        Self
    }
}

impl Tool for ExecTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "exec".to_string(),
            description: "Run a shell command in the agent's persistent bash session. \
                Working directory and environment persist across calls. \
                Returns stdout, stderr, exit code, and whether the command timed out. \
                When extended=false (default), the tool uses snip when available for \
                single commands to reduce token-heavy output. Set extended=true to \
                always return the normal command output. \
                Prefer chaining multiple commands with `&&` in a single call \
                (e.g. `which cargo && which micromamba`) rather than making separate \
                tool calls for each command."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["command"],
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute. \
                            Chain multiple commands with `&&` instead of making separate calls."
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Timeout in seconds. Defaults to 30. \
                            If exceeded, the output collected so far is returned \
                            with timed_out set to true."
                    },
                    "extended": {
                        "type": "boolean",
                        "default": false,
                        "description": "When false (default), the tool uses snip when available for commands without `&&`. \
                            When true, the command is always run normally. Returned output is still subject to truncation caps."
                    }
                }
            }),
        }
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, ToolError> {
        let parsed: ExecInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(e.to_string()))?;

        let exec_ctx = ctx.execution_context.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed(
                "exec requires an execution context — agent blueprint does not grant shell access"
                    .to_string(),
            )
        })?;

        let timeout_secs = parsed.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS);
        let command = if parsed.extended || parsed.command.contains("&&") {
            parsed.command.clone()
        } else if let Some(snip_path) = detect_snip(exec_ctx, timeout_secs).await? {
            format!("{snip_path} {}", parsed.command)
        } else {
            parsed.command.clone()
        };

        let output = exec_ctx
            .bash
            .lock()
            .await
            .run_command(&command, timeout_secs)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let exit_code = output.exit_code;
        let timed_out = output.timed_out;
        let stdout = truncate(strip_snip_notice_lines(output.stdout), OUTPUT_CAP_BYTES);
        let stderr = truncate(strip_snip_notice_lines(output.stderr), OUTPUT_CAP_BYTES);

        Ok(serde_json::json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": exit_code,
            "timed_out": timed_out,
        }))
    }
}

async fn detect_snip(
    exec_ctx: &ToolExecutionContext,
    timeout_secs: u64,
) -> Result<Option<String>, ToolError> {
    let detection = exec_ctx
        .bash
        .lock()
        .await
        .run_command("which snip", timeout_secs)
        .await
        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

    if detection.timed_out || detection.exit_code != 0 {
        return Ok(None);
    }

    let path = detection.stdout.lines().next().unwrap_or("").trim();
    if path.is_empty() {
        Ok(None)
    } else {
        Ok(Some(path.to_string()))
    }
}

fn strip_snip_notice_lines(s: String) -> String {
    s.lines()
        .filter(|line| !line.contains(SNIP_NO_FILTER_MARKER))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Truncate `s` to at most `cap` bytes on a UTF-8 character boundary.
/// Appends a notice when truncation occurs.
fn truncate(s: String, cap: usize) -> String {
    if s.len() <= cap {
        return s;
    }
    // Find the largest char boundary at or before `cap`.
    let boundary = (0..=cap).rev().find(|&i| s.is_char_boundary(i)).unwrap_or(0);
    let mut result = s[..boundary].to_string();
    result.push_str("\n[output truncated]");
    result
}

inventory::submit! {
    ToolRegistration {
        name: "exec",
        factory: || Arc::new(ExecTool::new()) as Arc<dyn ErasedTool>,
    }
}
