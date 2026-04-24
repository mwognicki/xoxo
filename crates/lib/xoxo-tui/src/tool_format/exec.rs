use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use xoxo_core::chat::structs::ToolCallCompleted;

use crate::app::App;

use super::{DefaultToolFormatter, ToolFormatter};

/// Maximum number of stdout lines the exec formatter will render.
const EXEC_STDOUT_PREVIEW_LINES: usize = 5;

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
pub(super) struct ExecFormatter;

impl ToolFormatter for ExecFormatter {
    fn format_completed(
        &self,
        app: &App,
        completed: &ToolCallCompleted,
        viewport_width: u16,
    ) -> Vec<Line<'static>> {
        let Some((stdout, stderr)) = parse_exec_streams(&completed.result_preview) else {
            return DefaultToolFormatter.format_completed(app, completed, viewport_width);
        };

        if !stderr.trim().is_empty() {
            return render_exec_stream(&stderr, None, true);
        }

        if !stdout.trim().is_empty() {
            return render_exec_stream(&stdout, Some(EXEC_STDOUT_PREVIEW_LINES), false);
        }

        DefaultToolFormatter.format_completed(app, completed, viewport_width)
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
