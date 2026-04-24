//! `@`-mention picker popup rendering.
//!
//! Anchored above the input box. Height and width adapt to the current set of
//! visible entries; falls back silently to a no-op when there isn't enough
//! vertical space above the input.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr as _;

use crate::app::App;

const EMPTY_MESSAGE: &str = "No matching files";

/// Render the `@`-mention popup, if one is active on the app state.
pub(super) fn render_mention_popup(frame: &mut Frame, app: &App, input_area: Rect) {
    let Some(popup) = &app.mention_popup else {
        return;
    };

    let entries = popup.visible_entries().collect::<Vec<_>>();
    let row_count = entries.len().max(1) as u16;
    let popup_height = row_count.saturating_add(2).min(input_area.y);
    if popup_height < 3 {
        return;
    }

    let content_width = entries
        .iter()
        .map(|entry| entry.rel_path.width() + if entry.is_dir { 1 } else { 0 })
        .max()
        .unwrap_or(EMPTY_MESSAGE.width());
    let max_width = input_area.width.max(1);
    let min_width = 24.min(max_width);
    let popup_width = (content_width as u16)
        .saturating_add(4)
        .clamp(min_width, max_width);
    let popup_area = Rect {
        x: input_area.x,
        y: input_area.y.saturating_sub(popup_height),
        width: popup_width,
        height: popup_height,
    };

    let selected_index = popup.selected_index();
    let lines = if entries.is_empty() {
        vec![Line::from(Span::styled(
            EMPTY_MESSAGE,
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let suffix = if entry.is_dir { "/" } else { "" };
                let style = if index == selected_index {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                Line::from(Span::styled(format!("{}{}", entry.rel_path, suffix), style))
            })
            .collect()
    };

    let popup_paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(Clear, popup_area);
    frame.render_widget(popup_paragraph, popup_area);
}
