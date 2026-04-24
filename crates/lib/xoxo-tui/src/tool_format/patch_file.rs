use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use xoxo_core::chat::structs::ToolCallCompleted;

use crate::app::App;

use super::{DefaultToolFormatter, ToolFormatter, divider_style, fit_to_width, subtle_style};

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

/// Formatter for `patch_file`.
///
/// The tool emits a structured JSON preview that contains the file path and a
/// compact list of changed rows. This formatter renders those rows as a
/// side-by-side diff with stable-width panes, line numbers, and colored
/// backgrounds for additions/removals.
pub(super) struct PatchFileFormatter;

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

fn render_patch_file_diff(
    preview: &PatchFileDiffPreview,
    viewport_width: usize,
) -> Vec<Line<'static>> {
    let pane_width = diff_pane_width(viewport_width);
    let header_style = subtle_style();
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
        Span::styled(" │ ", divider_style()),
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

fn render_patch_file_row(row: &PatchFileDiffRow, pane_width: usize) -> Line<'static> {
    let left_style = match row.change {
        PatchFileDiffChange::Added => subtle_style(),
        PatchFileDiffChange::Context | PatchFileDiffChange::Omitted => context_diff_style(),
        PatchFileDiffChange::Removed => removed_diff_style(),
        PatchFileDiffChange::Modified => modified_diff_style(),
    };
    let right_style = match row.change {
        PatchFileDiffChange::Added => added_diff_style(),
        PatchFileDiffChange::Context | PatchFileDiffChange::Omitted => context_diff_style(),
        PatchFileDiffChange::Removed => subtle_style(),
        PatchFileDiffChange::Modified => modified_diff_style(),
    };

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
        Span::styled(" │ ", divider_style()),
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
    let available =
        viewport_width.saturating_sub(PATCH_DIFF_PREFIX_WIDTH + PATCH_DIFF_DIVIDER_WIDTH);
    (available / 2).max(PATCH_DIFF_MIN_PANE_WIDTH)
}

fn diff_content_width(pane_width: usize) -> usize {
    pane_width.saturating_sub(PATCH_DIFF_CELL_OVERHEAD_WIDTH)
}

fn diff_marker(change: PatchFileDiffChange, pane: DiffPane) -> char {
    match (change, pane) {
        (PatchFileDiffChange::Added, DiffPane::Right) => '+',
        (PatchFileDiffChange::Removed, DiffPane::Left) => '-',
        (PatchFileDiffChange::Modified, _) => '~',
        _ => ' ',
    }
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
    subtle_style()
}
