//! Per-tool output formatting.
//!
//! Each tool kind can customize how its `Started` / `Completed` / `Failed`
//! bus events are rendered in the conversation pane by implementing
//! [`ToolFormatter`]. The trait methods default to the generic rendering used
//! before this module existed, so tools that do not implement a custom
//! formatter keep today's behavior.
//!
//! [`format_started`], [`format_completed`], and [`format_failed`] are the
//! entry points used by `ui.rs`. They pick an implementation based on
//! `tool_name` and fall back to the default trait implementation when no
//! specialization is registered.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use xoxo_core::chat::structs::{ToolCallCompleted, ToolCallFailed, ToolCallStarted};

use crate::app::App;
use crate::ui::{default_tool_byline, default_tool_result_lines};

/// Maximum number of stdout lines the exec formatter will render.
const EXEC_STDOUT_PREVIEW_LINES: usize = 5;

/// Formats the three tool-call bus events for a single tool kind.
///
/// Every method has a default implementation that reproduces the generic
/// rendering (byline for `Started`, dimmed result preview for `Completed`,
/// red error preview for `Failed`). Concrete implementations override only
/// the methods whose output they want to customize.
pub trait ToolFormatter {
    fn format_started(&self, app: &App, started: &ToolCallStarted) -> Vec<Line<'static>> {
        vec![default_tool_byline(app, started)]
    }

    fn format_completed(
        &self,
        _app: &App,
        completed: &ToolCallCompleted,
    ) -> Vec<Line<'static>> {
        default_tool_result_lines(&completed.result_preview, false)
    }

    fn format_failed(&self, _app: &App, failed: &ToolCallFailed) -> Vec<Line<'static>> {
        default_tool_result_lines(&failed.message, true)
    }
}

/// Fallback formatter used when a tool name has no dedicated implementation.
///
/// Relies entirely on the default trait methods.
struct DefaultToolFormatter;

impl ToolFormatter for DefaultToolFormatter {}

/// Formatter for the `exec` tool.
///
/// `exec`'s raw result preview is a serialized JSON object —
/// `{"stdout": "...", "stderr": "...", "exit_code": N, "timed_out": bool}` —
/// which is noisy to read verbatim. This formatter parses the preview and
/// surfaces whichever stream is most useful:
///
/// - if `stderr` is non-empty, it is rendered in full (errors are short and
///   worth reading completely),
/// - otherwise the first [`EXEC_STDOUT_PREVIEW_LINES`] lines of `stdout` are
///   rendered,
/// - if neither stream has content, the formatter falls back to the generic
///   rendering so the user still sees *something*.
///
/// Embedded newlines in the JSON payload are honored rather than rendered as
/// literal `\n`.
struct ExecFormatter;

impl ToolFormatter for ExecFormatter {
    fn format_completed(
        &self,
        app: &App,
        completed: &ToolCallCompleted,
    ) -> Vec<Line<'static>> {
        let Some((stdout, stderr)) = parse_exec_streams(&completed.result_preview) else {
            return DefaultToolFormatter.format_completed(app, completed);
        };

        if !stderr.trim().is_empty() {
            return render_exec_stream(&stderr, None, true);
        }

        if !stdout.trim().is_empty() {
            return render_exec_stream(&stdout, Some(EXEC_STDOUT_PREVIEW_LINES), false);
        }

        DefaultToolFormatter.format_completed(app, completed)
    }
}

/// Extract `(stdout, stderr)` from an exec preview payload.
///
/// Returns `None` if the preview is not a JSON object or the fields have the
/// wrong shape, so the caller can fall back to the generic renderer instead
/// of displaying half-formatted output.
fn parse_exec_streams(preview: &str) -> Option<(String, String)> {
    let value: serde_json::Value = serde_json::from_str(preview).ok()?;
    let stdout = value.get("stdout")?.as_str()?.to_string();
    let stderr = value.get("stderr")?.as_str()?.to_string();
    Some((stdout, stderr))
}

/// Render an exec stream as dimmed (or red, for errors) lines, honoring
/// embedded newlines. The first content line gets the same `└ ` elbow the
/// generic renderer uses; continuation lines are indented to match.
fn render_exec_stream(
    content: &str,
    max_lines: Option<usize>,
    is_error: bool,
) -> Vec<Line<'static>> {
    let color = if is_error {
        Color::Indexed(160)
    } else {
        Color::DarkGray
    };
    let style = Style::default().fg(color);

    let stream_lines: Vec<&str> = content.lines().collect();
    let (visible, truncated) = match max_lines {
        Some(cap) if stream_lines.len() > cap => (&stream_lines[..cap], true),
        _ => (&stream_lines[..], false),
    };

    let mut lines = Vec::with_capacity(visible.len() + 1);

    for (index, stream_line) in visible.iter().enumerate() {
        let prefix = if index == 0 { "└ " } else { "  " };
        lines.push(Line::from(Span::styled(
            format!("{prefix}{stream_line}"),
            style,
        )));
    }

    if truncated {
        lines.push(Line::from(Span::styled("  …", style)));
    }

    lines
}

/// Resolves a `tool_name` to the formatter that should render its events.
///
/// New per-tool formatters are added as sibling types with their own match
/// arm; anything unregistered falls through to [`DefaultToolFormatter`].
fn formatter_for(tool_name: &str) -> Box<dyn ToolFormatter> {
    match tool_name {
        "exec" => Box::new(ExecFormatter),
        _ => Box::new(DefaultToolFormatter),
    }
}

/// Render a `ToolCallStarted` event using the formatter registered for its tool.
pub fn format_started(app: &App, started: &ToolCallStarted) -> Vec<Line<'static>> {
    formatter_for(&started.tool_name).format_started(app, started)
}

/// Render a `ToolCallCompleted` event using the formatter registered for its tool.
pub fn format_completed(app: &App, completed: &ToolCallCompleted) -> Vec<Line<'static>> {
    formatter_for(&completed.tool_name).format_completed(app, completed)
}

/// Render a `ToolCallFailed` event using the formatter registered for its tool.
pub fn format_failed(app: &App, failed: &ToolCallFailed) -> Vec<Line<'static>> {
    formatter_for(&failed.tool_name).format_failed(app, failed)
}
