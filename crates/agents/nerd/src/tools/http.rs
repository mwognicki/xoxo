use std::collections::HashMap;
use std::sync::Arc;

use serde::Deserialize;
use xoxo_core::tooling::{ErasedTool, Tool, ToolContext, ToolError, ToolRegistration, ToolSchema};

const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Allowed HTTP methods. DELETE is explicitly excluded.
const ALLOWED_METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "HEAD", "OPTIONS"];

#[derive(Deserialize)]
struct HttpInput {
    url: String,
    method: String,
    #[serde(default)]
    headers: HashMap<String, String>,
    body: Option<String>,
    timeout_secs: Option<u64>,
}

/// Tool for making HTTP(S) requests via `curl` through the agent's bash session.
///
/// Supported methods: `GET`, `POST`, `PUT`, `PATCH`, `HEAD`, `OPTIONS`.
/// `DELETE` is explicitly disallowed.
///
/// Routing requests through `curl` in the bash session means network access can
/// be sandboxed at the OS level (network namespace, seccomp, etc.) alongside
/// `exec` — a single isolation boundary covers all outbound traffic.
///
/// Requires `execution_context` to be present — agents whose blueprint includes
/// this tool must also include a shell tool so that a bash session is spawned.
///
/// # Input
/// ```json
/// {
///   "url": "https://example.com",
///   "method": "POST",
///   "headers": { "Content-Type": "application/json" },
///   "body": "{\"key\": \"value\"}",
///   "timeout_secs": 10
/// }
/// ```
///
/// # Output
/// ```json
/// { "status": 200, "headers": { "content-type": "application/json" }, "body": "..." }
/// ```
pub struct HttpTool;

impl HttpTool {
    pub fn new() -> Self {
        Self
    }
}

impl Tool for HttpTool {


    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "http_request".to_string(),
            description: "Make an HTTP(S) request. Allowed methods: GET, POST, PUT, PATCH, HEAD, OPTIONS. DELETE is not permitted.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["url", "method"],
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The target URL."
                    },
                    "method": {
                        "type": "string",
                        "enum": ALLOWED_METHODS,
                        "description": "HTTP method."
                    },
                    "headers": {
                        "type": "object",
                        "description": "Optional request headers.",
                        "additionalProperties": { "type": "string" }
                    },
                    "body": {
                        "type": "string",
                        "description": "Optional request body."
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Request timeout in seconds. Defaults to 30."
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
        let parsed: HttpInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(e.to_string()))?;

        let method = parse_method(&parsed.method)?;
        let timeout = parsed.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS);

        let exec_ctx = ctx.execution_context.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed(
                "http_request requires a bash session; agent blueprint must include a shell tool"
                    .to_string(),
            )
        })?;

        let cmd = build_curl_command(
            &parsed.url,
            &method,
            &parsed.headers,
            parsed.body.as_deref(),
            timeout,
        );

        // Give bash a 5-second grace window beyond curl's own --max-time.
        let output = exec_ctx
            .bash
            .lock()
            .await
            .run_command(&cmd, timeout + 5)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        if output.timed_out {
            return Err(ToolError::ExecutionFailed("http_request timed out".to_string()));
        }

        if output.exit_code != 0 {
            return Err(ToolError::ExecutionFailed(format!(
                "curl exited with code {}: {}",
                output.exit_code,
                output.stderr.trim()
            )));
        }

        parse_curl_response(&output.stdout)
    }
}

/// Wrap a string in single quotes, escaping any embedded single quotes.
///
/// Safe for all user-supplied values in a shell command — URL, header
/// names/values, body. Prevents shell injection regardless of content.
pub(crate) fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Build a `curl -si` command from parsed HTTP input.
///
/// All user-supplied values are single-quote-escaped via [`shell_quote`].
pub(crate) fn build_curl_command(
    url: &str,
    method: &str,
    headers: &HashMap<String, String>,
    body: Option<&str>,
    timeout_secs: u64,
) -> String {
    let mut parts = vec![
        "curl".to_string(),
        "-si".to_string(),
        "--max-time".to_string(),
        timeout_secs.to_string(),
        "-X".to_string(),
        method.to_string(),
    ];

    for (key, value) in headers {
        parts.push("-H".to_string());
        parts.push(shell_quote(&format!("{key}: {value}")));
    }

    if let Some(b) = body {
        parts.push("--data-raw".to_string());
        parts.push(shell_quote(b));
    }

    parts.push(shell_quote(url));
    parts.join(" ")
}

/// Parse the combined header+body output produced by `curl -si`.
///
/// curl writes the HTTP status line and response headers before the body when
/// `-i` is used. This function splits on the blank line separator and
/// reconstructs `{ status, headers, body }`. Header names are lowercased for
/// consistent access.
pub(crate) fn parse_curl_response(raw: &str) -> Result<serde_json::Value, ToolError> {
    // Headers and body are separated by \r\n\r\n (or \n\n for lenient servers).
    let (header_section, body) = raw
        .split_once("\r\n\r\n")
        .or_else(|| raw.split_once("\n\n"))
        .ok_or_else(|| {
            ToolError::ExecutionFailed(
                "could not parse curl response: missing header/body separator".to_string(),
            )
        })?;

    let mut lines = header_section.lines();

    let status_line = lines.next().ok_or_else(|| {
        ToolError::ExecutionFailed("curl response is empty".to_string())
    })?;

    // Status line formats: "HTTP/1.1 200 OK" or "HTTP/2 200" (no reason phrase).
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| {
            ToolError::ExecutionFailed(format!(
                "could not parse HTTP status from: {status_line:?}"
            ))
        })?;

    let mut headers: HashMap<String, String> = HashMap::new();
    for line in lines {
        if let Some((key, value)) = line.split_once(": ") {
            headers.insert(
                key.to_lowercase(),
                value.trim_end_matches('\r').to_string(),
            );
        }
    }

    Ok(serde_json::json!({
        "status": status,
        "headers": headers,
        "body": body,
    }))
}

/// Parse and validate an HTTP method string.
///
/// Returns the uppercased method name on success.
/// Returns `ToolError::InvalidInput` for DELETE or any unrecognised method.
pub(crate) fn parse_method(method: &str) -> Result<String, ToolError> {
    match method.to_uppercase().as_str() {
        "DELETE" => Err(ToolError::InvalidInput(
            "DELETE method is not allowed".to_string(),
        )),
        m if ALLOWED_METHODS.contains(&m) => Ok(m.to_string()),
        other => Err(ToolError::InvalidInput(format!(
            "unsupported method: {other:?}; allowed: {}",
            ALLOWED_METHODS.join(", ")
        ))),
    }
}

inventory::submit! {
    ToolRegistration {
        name: "http_request",
        factory: || Arc::new(HttpTool::new()) as Arc<dyn ErasedTool>,
    }
}
