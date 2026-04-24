use ansi_to_tui::IntoText as _;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use xoxo_core::chat::structs::ToolCallCompleted;

use crate::app::App;
use crate::syntax_highlighter::highlight_syntax;

use super::{
    DefaultToolFormatter, ToolFormatter, file_extension, prefixed_code_line, subtle_style,
};

#[derive(Debug)]
struct WriteFilePreview {
    file_path: String,
    content: String,
}

pub(super) struct WriteFileFormatter;

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
        subtle_style(),
    )));

    for line in highlighted_text.lines {
        lines.push(prefixed_code_line("  ", line, style));
    }

    if lines.len() == 1 {
        lines.push(Line::from(Span::styled("  ", style)));
    }

    lines
}
