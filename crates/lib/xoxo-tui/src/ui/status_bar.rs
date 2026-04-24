//! Single-line status bar rendering along the bottom margin.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;

use super::format_estimated_cost;

/// Render the bottom status bar (workspace path, model, tokens, cost).
pub(super) fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let current_dir = app.workspace_root.display().to_string();

    let left_text = format!("{} • {}", current_dir, app.current_model_name);
    let selection_prefix = if app.mouse_capture_enabled {
        String::new()
    } else {
        "selection mode • ".to_string()
    };
    let right_text = format!(
        "{}↓{} ↑{} • {}",
        selection_prefix,
        app.total_input_tokens,
        app.total_output_tokens,
        format_estimated_cost(app.estimated_cost_usd)
    );

    let total_content_length = left_text.len() + right_text.len();
    let available_space = area.width as usize;

    if total_content_length < available_space {
        let spacing = available_space - total_content_length;

        let status_line = Line::from(vec![
            Span::raw(left_text),
            Span::raw(" ".repeat(spacing)),
            Span::styled(right_text, Style::default().fg(Color::DarkGray)),
        ])
        .style(Style::default().fg(Color::DarkGray));

        let status_paragraph = Paragraph::new(status_line)
            .style(Style::default().fg(Color::Gray))
            .block(
                Block::default()
                    .borders(Borders::NONE)
                    .style(Style::default().fg(Color::DarkGray)),
            );
        frame.render_widget(status_paragraph, area);
    } else {
        let status_text = format!("{} | {}", left_text, right_text);
        let status_paragraph = Paragraph::new(status_text)
            .style(Style::default().fg(Color::DarkGray))
            .block(
                Block::default()
                    .borders(Borders::NONE)
                    .style(Style::default().fg(Color::DarkGray)),
            );
        frame.render_widget(status_paragraph, area);
    }
}
