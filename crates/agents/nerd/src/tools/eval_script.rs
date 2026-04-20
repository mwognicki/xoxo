
use std::collections::HashMap;
use std::sync::Arc;

use serde::Deserialize;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::{Duration, timeout};
use xoxo_core::helpers::new_id;
use xoxo_core::tooling::{ErasedTool, Tool, ToolContext, ToolError, ToolRegistration, ToolSchema};
use crate::types::ScriptLanguage;

const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Maximum bytes kept per stream (stdout or stderr).
const OUTPUT_CAP_BYTES: usize = 64 * 1024;

#[derive(Deserialize)]
pub(crate) struct EvalScriptInput {
    pub(crate) language: ScriptLanguage,
    pub(crate) code: String,
    pub(crate) timeout_secs: Option<u64>,
    #[serde(default)]
    pub(crate) permissions: Vec<String>,
    #[serde(default)]
    pub(crate) env_vars: HashMap<String, String>,
}

/// Tool for evaluating dynamic scripts written by agents at runtime.
///
/// Supported languages: `js_ts` (JavaScript/TypeScript via Deno),
/// `python` (Python 3), `shell` (Bash). Each invocation spawns a fresh,
/// isolated child process — no state persists between calls.
///
/// # Security model
///
/// - Parent process environment is **never** inherited (`env_clear`).
///   Only variables in `env_vars` are passed to the child.
/// - Hard timeout: process is killed if it exceeds `timeout_secs`.
/// - Output is capped at 64 KB per stream.
/// - For JS/TS, Deno capabilities must be explicitly listed in `permissions`
///   (e.g. `["net", "read"]`). Without permissions, scripts run fully
///   sandboxed with no I/O or network access.
/// - Scripts are leaves: they cannot call back into framework tools.
///
/// # Deno permissions (js_ts only)
///
/// | Permission | Deno flag         |
/// |------------|-------------------|
/// | `net`      | `--allow-net`     |
/// | `read`     | `--allow-read`    |
/// | `write`    | `--allow-write`   |
/// | `env`      | `--allow-env`     |
/// | `run`      | `--allow-run`     |
/// | `sys`      | `--allow-sys`     |
/// | `ffi`      | `--allow-ffi`     |
///
/// Unknown permission strings are rejected as `InvalidInput`.
/// `permissions` is ignored for `python` and `shell`.
///
/// # Input
/// ```json
/// {
///   "language": "js_ts",
///   "code": "console.log('hello')",
///   "timeout_secs": 10,
///   "permissions": ["net"],
///   "env_vars": { "MY_VAR": "value" }
/// }
/// ```
///
/// # Output
/// ```json
/// { "stdout": "hello\n", "stderr": "", "exit_code": 0, "timed_out": false }
/// ```
pub struct EvalScriptTool;

impl EvalScriptTool {
    pub fn new() -> Self {
        Self
    }
}

impl Tool for EvalScriptTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "eval_script".to_string(),
            description: "Evaluate a script in an isolated child process. \
                Supported languages: deno (JS/TS), python. \
                The process runs with a clean environment; \
                only explicitly listed env_vars are available. \
                For Deno, capabilities must be declared in permissions."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["language", "code"],
                "properties": {
                    "language": {
                        "type": "string",
                        "enum": ["js_ts", "python", "shell"],
                        "description": "Script language: js_ts for JavaScript/TypeScript (Deno), python for Python 3, shell for Bash."
                    },
                    "code": {
                        "type": "string",
                        "description": "Source code to execute."
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Hard timeout in seconds. Defaults to 30."
                    },
                    "permissions": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Deno capability allowlist (js_ts only). Accepted values: net, read, write, env, run, sys, ffi."
                    },
                    "env_vars": {
                        "type": "object",
                        "additionalProperties": { "type": "string" },
                        "description": "Environment variables passed to the script. Parent env is never inherited."
                    }
                }
            }),
        }
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, ToolError> {
        let parsed: EvalScriptInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(e.to_string()))?;

        match parsed.language {
            ScriptLanguage::JsTs => run_js_ts(parsed).await,
            ScriptLanguage::Python => run_python(parsed).await,
            ScriptLanguage::Shell => run_shell(parsed).await,
        }
    }
}

async fn run_js_ts(input: EvalScriptInput) -> Result<serde_json::Value, ToolError> {
    let permission_flags = build_deno_flags(&input.permissions)?;

    let path = write_temp_file(&input.code, "ts").await?;

    let mut cmd = Command::new("deno");
    cmd.arg("run").arg("--no-prompt");
    for flag in &permission_flags {
        cmd.arg(flag);
    }
    cmd.arg(&path);

    let result = run_process(cmd, input.env_vars, input.timeout_secs).await;
    tokio::fs::remove_file(&path).await.ok();
    result
}

async fn run_python(input: EvalScriptInput) -> Result<serde_json::Value, ToolError> {
    let path = write_temp_file(&input.code, "py").await?;

    let mut cmd = Command::new("python3");
    cmd.arg(&path);

    let result = run_process(cmd, input.env_vars, input.timeout_secs).await;
    tokio::fs::remove_file(&path).await.ok();
    result
}

async fn run_shell(input: EvalScriptInput) -> Result<serde_json::Value, ToolError> {
    let path = write_temp_file(&input.code, "sh").await?;

    let mut cmd = Command::new("bash");
    cmd.arg(&path);

    let result = run_process(cmd, input.env_vars, input.timeout_secs).await;
    tokio::fs::remove_file(&path).await.ok();
    result
}

/// Write `code` to a temp file with the given extension. Returns the file path.
async fn write_temp_file(code: &str, ext: &str) -> Result<std::path::PathBuf, ToolError> {
    let filename = format!("{}.{}", new_id(), ext);
    let path = std::env::temp_dir().join(filename);
    tokio::fs::write(&path, code.as_bytes())
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("failed to write temp file: {e}")))?;
    Ok(path)
}

/// Spawn the command with a clean env, wait with timeout, capture output.
async fn run_process(
    mut cmd: Command,
    env_vars: HashMap<String, String>,
    timeout_secs: Option<u64>,
) -> Result<serde_json::Value, ToolError> {
    use std::process::Stdio;

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .envs(&env_vars);

    let secs = timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS);

    let mut child = cmd
        .spawn()
        .map_err(|e| ToolError::ExecutionFailed(format!("failed to spawn process: {e}")))?;

    // Take the stdio handles before the timeout future so they can be moved
    // into the async block while `child` stays in scope for kill() on timeout.
    let mut stdout = child.stdout.take().expect("stdout piped");
    let mut stderr = child.stderr.take().expect("stderr piped");

    let read_result = timeout(Duration::from_secs(secs), async {
        let mut out = Vec::new();
        let mut err = Vec::new();
        tokio::try_join!(
            stdout.read_to_end(&mut out),
            stderr.read_to_end(&mut err),
        )?;
        Ok::<_, std::io::Error>((out, err))
    })
    .await;

    match read_result {
        Ok(Ok((out, err))) => {
            let status = child
                .wait()
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            Ok(serde_json::json!({
                "stdout": truncate(String::from_utf8_lossy(&out).into_owned(), OUTPUT_CAP_BYTES),
                "stderr": truncate(String::from_utf8_lossy(&err).into_owned(), OUTPUT_CAP_BYTES),
                "exit_code": status.code().unwrap_or(-1),
                "timed_out": false,
            }))
        }
        Ok(Err(e)) => Err(ToolError::ExecutionFailed(e.to_string())),
        Err(_elapsed) => {
            child.kill().await.ok();
            Ok(serde_json::json!({
                "stdout": "",
                "stderr": "",
                "exit_code": -1,
                "timed_out": true,
            }))
        }
    }
}

/// Map permission strings to `--allow-*` Deno flags.
pub(crate) fn build_deno_flags(permissions: &[String]) -> Result<Vec<String>, ToolError> {
    permissions
        .iter()
        .map(|p| match p.as_str() {
            "net"   => Ok("--allow-net".to_string()),
            "read"  => Ok("--allow-read".to_string()),
            "write" => Ok("--allow-write".to_string()),
            "env"   => Ok("--allow-env".to_string()),
            "run"   => Ok("--allow-run".to_string()),
            "sys"   => Ok("--allow-sys".to_string()),
            "ffi"   => Ok("--allow-ffi".to_string()),
            other   => Err(ToolError::InvalidInput(format!(
                "unknown permission {other:?}; accepted: net, read, write, env, run, sys, ffi"
            ))),
        })
        .collect()
}

/// Truncate `s` to at most `cap` bytes on a UTF-8 character boundary.
pub(crate) fn truncate(s: String, cap: usize) -> String {
    if s.len() <= cap {
        return s;
    }
    let boundary = (0..=cap).rev().find(|&i| s.is_char_boundary(i)).unwrap_or(0);
    let mut result = s[..boundary].to_string();
    result.push_str("\n[output truncated]");
    result
}

inventory::submit! {
    ToolRegistration {
        name: "eval_script",
        factory: || Arc::new(EvalScriptTool::new()) as Arc<dyn ErasedTool>,
    }
}