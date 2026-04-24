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

use ansi_to_tui::IntoText as _;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use unicode_width::{UnicodeWidthChar as _, UnicodeWidthStr as _};
use xoxo_core::chat::structs::{ToolCallCompleted, ToolCallFailed, ToolCallStarted};

use crate::app::App;
use crate::syntax_highlighter::highlight_syntax;
use crate::ui::{
    default_tool_byline_with_lookup, default_tool_result_lines, ToolOutcomeLookup,
};

/// Maximum number of stdout lines the exec formatter will render.
const EXEC_STDOUT_PREVIEW_LINES: usize = 5;
const PATCH_DIFF_PREFIX_WIDTH: usize = 2;
const PATCH_DIFF_DIVIDER_WIDTH: usize = 3;
const PATCH_DIFF_MIN_PANE_WIDTH: usize = 8;
const PATCH_DIFF_LINE_NUMBER_WIDTH: usize = 4;
const PATCH_DIFF_MARKER_WIDTH: usize = 2;
const PATCH_DIFF_CELL_OVERHEAD_WIDTH: usize =
    PATCH_DIFF_LINE_NUMBER_WIDTH + PATCH_DIFF_MARKER_WIDTH + 1;

#[derive(Debug)]
struct PatchFileDiffPreview {
    file_path: String,
    summary: String,
    stats: PatchFileDiffStats,
    rows: Vec<PatchFileDiffRow>,
}

#[derive(Debug)]
struct PatchFileDiffStats {
    added: usize,
    removed: usize,
    modified: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatchFileDiffChange {
    Added,
    Context,
    Omitted,
    Removed,
    Modified,
}

#[derive(Debug)]
struct PatchFileDiffRow {
    change: PatchFileDiffChange,
    left_line_number: Option<usize>,
    right_line_number: Option<usize>,
    left_content: String,
    right_content: String,
}

#[derive(Debug)]
struct WriteFilePreview {
    file_path: String,
    content: String,
}

/// Formats the three tool-call bus events for a single tool kind.
///
/// Every method has a default implementation that reproduces the generic
/// rendering (byline for `Started`, dimmed result preview for `Completed`,
/// red error preview for `Failed`). Concrete implementations override only
/// the methods whose output they want to customize.
pub trait ToolFormatter {
    fn format_started(
        &self,
        app: &App,
        tool_outcomes: &ToolOutcomeLookup<'_>,
        started: &ToolCallStarted,
    ) -> Vec<Line<'static>> {
        vec![default_tool_byline_with_lookup(app, tool_outcomes, started)]
    }

    fn format_completed(
        &self,
        _app: &App,
        completed: &ToolCallCompleted,
        _viewport_width: u16,
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

/// Formatter for `patch_file`.
///
/// The tool emits a structured JSON preview that contains the file path and a
/// compact list of changed rows. This formatter renders those rows as a
/// side-by-side diff with stable-width panes, line numbers, and colored
/// backgrounds for additions/removals.
struct PatchFileFormatter;

impl ToolFormatter for PatchFileFormatter {
    fn format_completed(
        &self,
        app: &App,
        completed: &ToolCallCompleted,
        viewport_width: u16,
    ) -> Vec<Line<'static>> {
        let Some(preview) = parse_patch_file_preview(&completed.result_preview) else {
            return DefaultToolFormatter.format_completed(app, completed, viewport_width);
        };

        render_patch_file_diff(&preview, usize::from(viewport_width))
    }
}

struct WriteFileFormatter;

impl ToolFormatter for WriteFileFormatter {
    fn format_completed(
        &self,
        app: &App,
        completed: &ToolCallCompleted,
        viewport_width: u16,
    ) -> Vec<Line<'static>> {
        let Some(preview) = parse_write_file_preview(&completed.result_preview) else {
            return DefaultToolFormatter.format_completed(app, completed, viewport_width);
        };

        render_write_file_preview(&preview)
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

fn parse_patch_file_preview(preview: &str) -> Option<PatchFileDiffPreview> {
    let value: serde_json::Value = serde_json::from_str(preview).ok()?;
    let kind = value.get("kind")?.as_str()?.to_string();
    if kind != "patch_file_diff" {
        return None;
    }

    let stats = value.get("stats")?;
    let rows = value.get("rows")?.as_array()?;
    let mut parsed_rows = Vec::with_capacity(rows.len());

    for row in rows {
        let change = match row.get("change")?.as_str()? {
            "added" => PatchFileDiffChange::Added,
            "context" => PatchFileDiffChange::Context,
            "omitted" => PatchFileDiffChange::Omitted,
            "removed" => PatchFileDiffChange::Removed,
            "modified" => PatchFileDiffChange::Modified,
            _ => return None,
        };
        parsed_rows.push(PatchFileDiffRow {
            change,
            left_line_number: row
                .get("left_line_number")
                .and_then(serde_json::Value::as_u64)
                .map(|value| value as usize),
            right_line_number: row
                .get("right_line_number")
                .and_then(serde_json::Value::as_u64)
                .map(|value| value as usize),
            left_content: row.get("left_content")?.as_str()?.to_string(),
            right_content: row.get("right_content")?.as_str()?.to_string(),
        });
    }

    Some(PatchFileDiffPreview {
        file_path: value.get("file_path")?.as_str()?.to_string(),
        summary: value.get("summary")?.as_str()?.to_string(),
        stats: PatchFileDiffStats {
            added: stats.get("added")?.as_u64()? as usize,
            removed: stats.get("removed")?.as_u64()? as usize,
            modified: stats.get("modified")?.as_u64()? as usize,
        },
        rows: parsed_rows,
    })
}

fn parse_write_file_preview(preview: &str) -> Option<WriteFilePreview> {
    let value: serde_json::Value = serde_json::from_str(preview).ok()?;
    let kind = value.get("kind")?.as_str()?.to_string();
    if kind != "write_file_preview" {
        return None;
    }

    Some(WriteFilePreview {
        file_path: value.get("file_path")?.as_str()?.to_string(),
        content: value.get("content")?.as_str()?.to_string(),
    })
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

fn render_write_file_preview(preview: &WriteFilePreview) -> Vec<Line<'static>> {
    let style = Style::default()
        .fg(Color::Indexed(179))
        .bg(Color::Indexed(235));
    let extension = file_extension(&preview.file_path);
    let highlighted = highlight_syntax(extension, &preview.content);
    let highlighted_text = highlighted
        .into_text()
        .unwrap_or_else(|_| Text::raw(preview.content.clone()));
    let mut lines = Vec::with_capacity(highlighted_text.lines.len() + 1);

    lines.push(Line::from(Span::styled(
        format!("└ {}", preview.file_path),
        Style::default().fg(Color::DarkGray),
    )));

    for line in highlighted_text.lines {
        lines.push(prefixed_code_line("  ", line, style));
    }

    if lines.len() == 1 {
        lines.push(Line::from(Span::styled("  ", style)));
    }

    lines
}

fn render_patch_file_diff(
    preview: &PatchFileDiffPreview,
    viewport_width: usize,
) -> Vec<Line<'static>> {
    let pane_width = diff_pane_width(viewport_width);
    let header_style = Style::default().fg(Color::DarkGray);
    let divider_style = Style::default().fg(Color::Indexed(240));
    let content_width = diff_content_width(pane_width);

    let mut lines = Vec::with_capacity(preview.rows.len() + 3);
    lines.push(Line::from(Span::styled(
        format!(
            "└ {}  (+{} -{} ~{})",
            preview.file_path, preview.stats.added, preview.stats.removed, preview.stats.modified
        ),
        header_style,
    )));
    lines.push(Line::from(vec![
        Span::styled("  ", header_style),
        Span::styled(
            format!(
                "{:>width$} {:<marker$}{:<cell$}",
                "old",
                "",
                fit_to_width("before", content_width),
                width = PATCH_DIFF_LINE_NUMBER_WIDTH,
                marker = PATCH_DIFF_MARKER_WIDTH,
                cell = content_width
            ),
            header_style,
        ),
        Span::styled(" │ ", divider_style),
        Span::styled(
            format!(
                "{:>width$} {:<marker$}{:<cell$}",
                "new",
                "",
                fit_to_width("after", content_width),
                width = PATCH_DIFF_LINE_NUMBER_WIDTH,
                marker = PATCH_DIFF_MARKER_WIDTH,
                cell = content_width
            ),
            header_style,
        ),
    ]));

    if preview.rows.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("  {}", preview.summary),
            header_style,
        )));
        return lines;
    }

    for row in &preview.rows {
        lines.push(render_patch_file_row(row, pane_width));
    }

    lines
}

fn render_patch_file_row(
    row: &PatchFileDiffRow,
    pane_width: usize,
) -> Line<'static> {
    let left_style = match row.change {
        PatchFileDiffChange::Added => Style::default().fg(Color::DarkGray),
        PatchFileDiffChange::Context | PatchFileDiffChange::Omitted => context_diff_style(),
        PatchFileDiffChange::Removed => removed_diff_style(),
        PatchFileDiffChange::Modified => modified_diff_style(),
    };
    let right_style = match row.change {
        PatchFileDiffChange::Added => added_diff_style(),
        PatchFileDiffChange::Context | PatchFileDiffChange::Omitted => context_diff_style(),
        PatchFileDiffChange::Removed => Style::default().fg(Color::DarkGray),
        PatchFileDiffChange::Modified => modified_diff_style(),
    };
    let divider_style = Style::default().fg(Color::Indexed(240));

    Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            format_diff_cell(
                row.left_line_number,
                diff_marker(row.change, DiffPane::Left),
                &row.left_content,
                pane_width,
            ),
            left_style,
        ),
        Span::styled(" │ ", divider_style),
        Span::styled(
            format_diff_cell(
                row.right_line_number,
                diff_marker(row.change, DiffPane::Right),
                &row.right_content,
                pane_width,
            ),
            right_style,
        ),
    ])
}

#[derive(Clone, Copy)]
enum DiffPane {
    Left,
    Right,
}

fn format_diff_cell(
    line_number: Option<usize>,
    marker: char,
    content: &str,
    pane_width: usize,
) -> String {
    let content_width = diff_content_width(pane_width);
    let line_number = line_number
        .map(|value| format!("{value:>PATCH_DIFF_LINE_NUMBER_WIDTH$}"))
        .unwrap_or_else(|| " ".repeat(PATCH_DIFF_LINE_NUMBER_WIDTH));
    let content = fit_to_width(content, content_width);
    format!("{line_number} {marker} {content}")
}

fn diff_pane_width(viewport_width: usize) -> usize {
    let available = viewport_width.saturating_sub(PATCH_DIFF_PREFIX_WIDTH + PATCH_DIFF_DIVIDER_WIDTH);
    (available / 2).max(PATCH_DIFF_MIN_PANE_WIDTH)
}

fn diff_content_width(pane_width: usize) -> usize {
    pane_width.saturating_sub(PATCH_DIFF_CELL_OVERHEAD_WIDTH)
}

fn file_extension(file_path: &str) -> &str {
    file_path
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .unwrap_or_default()
}

fn diff_marker(change: PatchFileDiffChange, pane: DiffPane) -> char {
    match (change, pane) {
        (PatchFileDiffChange::Added, DiffPane::Right) => '+',
        (PatchFileDiffChange::Removed, DiffPane::Left) => '-',
        (PatchFileDiffChange::Modified, _) => '~',
        _ => ' ',
    }
}

fn fit_to_width(content: &str, width: usize) -> String {
    let content_width = content.width();
    if content_width <= width {
        return format!("{content:<width$}");
    }

    if width <= 3 {
        return ".".repeat(width);
    }

    let mut fitted = String::new();
    let mut used_width = 0usize;
    let target_width = width - 3;

    for ch in content.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if used_width + ch_width > target_width {
            break;
        }
        fitted.push(ch);
        used_width += ch_width;
    }

    fitted.push_str("...");
    let fitted_width = fitted.width();
    if fitted_width < width {
        fitted.push_str(&" ".repeat(width - fitted_width));
    }
    fitted
}

fn removed_diff_style() -> Style {
    Style::default()
        .fg(Color::Indexed(224))
        .bg(Color::Indexed(52))
}

fn added_diff_style() -> Style {
    Style::default()
        .fg(Color::Indexed(194))
        .bg(Color::Indexed(22))
}

fn modified_diff_style() -> Style {
    Style::default()
        .fg(Color::Indexed(195))
        .bg(Color::Indexed(24))
}

fn context_diff_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn prefixed_code_line(
    prefix: &str,
    mut line: Line<'static>,
    fallback_style: Style,
) -> Line<'static> {
    let mut spans = vec![Span::styled(prefix.to_string(), fallback_style)];
    if line.spans.is_empty() {
        spans.push(Span::styled(String::new(), fallback_style));
    } else {
        for span in line.spans.drain(..) {
            let content = span.content.to_string();
            let style = fallback_style.patch(span.style);
            spans.push(Span::styled(content, style));
        }
    }
    Line::from(spans)
}

/// Resolves a `tool_name` to the formatter that should render its events.
///
/// New per-tool formatters are added as sibling types with their own match
/// arm; anything unregistered falls through to [`DefaultToolFormatter`].
fn formatter_for(tool_name: &str) -> Box<dyn ToolFormatter> {
    match tool_name {
        "exec" => Box::new(ExecFormatter),
        "patch_file" => Box::new(PatchFileFormatter),
        "write_file" => Box::new(WriteFileFormatter),
        _ => Box::new(DefaultToolFormatter),
    }
}

/// Render a `ToolCallStarted` event using the formatter registered for its tool.
pub fn format_started(
    app: &App,
    tool_outcomes: &ToolOutcomeLookup<'_>,
    started: &ToolCallStarted,
) -> Vec<Line<'static>> {
    formatter_for(&started.tool_name).format_started(app, tool_outcomes, started)
}

/// Render a `ToolCallCompleted` event using the formatter registered for its tool.
pub fn format_completed(
    app: &App,
    completed: &ToolCallCompleted,
    viewport_width: u16,
) -> Vec<Line<'static>> {
    formatter_for(&completed.tool_name).format_completed(app, completed, viewport_width)
}

/// Render a `ToolCallFailed` event using the formatter registered for its tool.
pub fn format_failed(app: &App, failed: &ToolCallFailed) -> Vec<Line<'static>> {
    formatter_for(&failed.tool_name).format_failed(app, failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::Instant;

    use uuid::Uuid;

    use crate::app::{App, LayoutMode};

    fn test_app() -> App {
        App {
            running: true,
            layout: LayoutMode::Main,
            input: String::new(),
            active_chat_id: Some(Uuid::new_v4()),
            pending_submission: None,
            current_provider_name: "test-provider".to_string(),
            current_model_name: "test-model".to_string(),
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_used_tokens: 0,
            context_left_percent: None,
            max_input_tokens: None,
            estimated_cost_usd: None,
            history: Vec::new(),
            in_flight_text: HashMap::new(),
            in_flight_thinking: HashMap::new(),
            conversation_scroll_from_bottom: 0,
            modal: None,
            mention_popup: None,
            workspace_root: PathBuf::from("."),
            ctrl_c_count: 0,
            started_at: Instant::now(),
            turn_in_progress: false,
            last_turn_finish_reason: None,
            mouse_capture_enabled: true,
            storage: None,
            conversation_version: 0,
            cached_conversation: RefCell::new(None),
        }
    }

    #[test]
    fn patch_file_preview_renders_side_by_side_diff() {
        let app = test_app();
        let completed = ToolCallCompleted {
            tool_call_id: xoxo_core::chat::structs::ChatToolCallId("tool-1".to_string()),
            tool_name: "patch_file".to_string(),
            result_preview: serde_json::json!({
                "kind": "patch_file_diff",
                "file_path": "src/main.rs",
                "summary": "File patched: src/main.rs (+1 -1 ~1)",
                "stats": { "added": 1, "removed": 1, "modified": 1 },
                "rows": [
                    {
                        "change": "removed",
                        "left_line_number": 3,
                        "right_line_number": null,
                        "left_content": "old line",
                        "right_content": ""
                    },
                    {
                        "change": "modified",
                        "left_line_number": 7,
                        "right_line_number": 7,
                        "left_content": "before",
                        "right_content": "after"
                    },
                    {
                        "change": "added",
                        "left_line_number": null,
                        "right_line_number": 10,
                        "left_content": "",
                        "right_content": "new line"
                    }
                ]
            })
            .to_string(),
        };

        let lines = format_completed(&app, &completed, 80);

        assert_eq!(lines.len(), 5);
        assert!(lines[0].spans[0].content.contains("src/main.rs"));
        assert_eq!(lines[1].width(), 79);
        assert!(lines[2].spans[1].content.starts_with("   3 - "));
        assert_eq!(lines[2].spans[1].style.bg, Some(Color::Indexed(52)));
        assert_eq!(lines[2].spans[3].style.bg, None);
        assert!(lines[3].spans[1].content.starts_with("   7 ~ "));
        assert!(lines[3].spans[3].content.starts_with("   7 ~ "));
        assert_eq!(lines[3].spans[1].style.bg, Some(Color::Indexed(24)));
        assert_eq!(lines[3].spans[3].style.bg, Some(Color::Indexed(24)));
        assert!(lines[4].spans[3].content.starts_with("  10 + "));
        assert_eq!(lines[4].spans[1].style.bg, None);
        assert_eq!(lines[4].spans[3].style.bg, Some(Color::Indexed(22)));
    }

    #[test]
    fn patch_file_preview_renders_context_and_omitted_rows() {
        let app = test_app();
        let completed = ToolCallCompleted {
            tool_call_id: xoxo_core::chat::structs::ChatToolCallId("tool-1".to_string()),
            tool_name: "patch_file".to_string(),
            result_preview: serde_json::json!({
                "kind": "patch_file_diff",
                "file_path": "src/main.rs",
                "summary": "File patched: src/main.rs (+1 -0 ~1)",
                "stats": { "added": 1, "removed": 0, "modified": 1 },
                "rows": [
                    {
                        "change": "context",
                        "left_line_number": 1,
                        "right_line_number": 1,
                        "left_content": "use std::fmt;",
                        "right_content": "use std::fmt;"
                    },
                    {
                        "change": "modified",
                        "left_line_number": 2,
                        "right_line_number": 2,
                        "left_content": "old()",
                        "right_content": "new()"
                    },
                    {
                        "change": "omitted",
                        "left_line_number": null,
                        "right_line_number": null,
                        "left_content": "...",
                        "right_content": "..."
                    },
                    {
                        "change": "added",
                        "left_line_number": null,
                        "right_line_number": 30,
                        "left_content": "",
                        "right_content": "inserted()"
                    }
                ]
            })
            .to_string(),
        };

        let lines = format_completed(&app, &completed, 80);

        assert!(lines[2].spans[1].content.starts_with("   1   "));
        assert_eq!(lines[2].spans[1].style.bg, None);
        assert_eq!(lines[2].spans[3].style.bg, None);
        assert!(lines[3].spans[1].content.starts_with("   2 ~ "));
        assert!(lines[3].spans[3].content.starts_with("   2 ~ "));
        assert_eq!(lines[3].spans[1].style.bg, Some(Color::Indexed(24)));
        assert_eq!(lines[3].spans[3].style.bg, Some(Color::Indexed(24)));
        assert_eq!(lines[4].spans[1].style.bg, None);
        assert_eq!(lines[4].spans[3].style.bg, None);
        assert_eq!(lines[5].spans[1].style.bg, None);
        assert!(lines[5].spans[3].content.starts_with("  30 + "));
        assert_eq!(lines[5].spans[3].style.bg, Some(Color::Indexed(22)));
    }

    #[test]
    fn legacy_patch_file_preview_falls_back_to_default_lines() {
        let app = test_app();
        let completed = ToolCallCompleted {
            tool_call_id: xoxo_core::chat::structs::ChatToolCallId("tool-1".to_string()),
            tool_name: "patch_file".to_string(),
            result_preview: "File patched: src/main.rs".to_string(),
        };

        let lines = format_completed(&app, &completed, 80);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans[0].content, "└ File patched: src/main.rs");
    }

    #[test]
    fn write_file_preview_renders_file_contents() {
        let app = test_app();
        let completed = ToolCallCompleted {
            tool_call_id: xoxo_core::chat::structs::ChatToolCallId("tool-2".to_string()),
            tool_name: "write_file".to_string(),
            result_preview: serde_json::json!({
                "kind": "write_file_preview",
                "file_path": "src/main.rs",
                "content": "fn main() {\n    println!(\"hi\");\n}\n"
            })
            .to_string(),
        };

        let lines = format_completed(&app, &completed, 80);

        assert_eq!(lines.len(), 4);
        assert!(lines[0].spans[0].content.contains("src/main.rs"));
        assert_eq!(lines[1].spans[0].content, "  ");
        assert!(lines[1].spans.iter().any(|span| span.content.contains("fn")));
        assert!(lines[2]
            .spans
            .iter()
            .any(|span| span.content.contains("println!")));
    }

    #[test]
    fn legacy_write_file_preview_falls_back_to_default_lines() {
        let app = test_app();
        let completed = ToolCallCompleted {
            tool_call_id: xoxo_core::chat::structs::ChatToolCallId("tool-2".to_string()),
            tool_name: "write_file".to_string(),
            result_preview: "File saved: src/main.rs".to_string(),
        };

        let lines = format_completed(&app, &completed, 80);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans[0].content, "└ File saved: src/main.rs");
    }
}
