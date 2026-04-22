//! Modal overlay rendering.
//!
//! Draws whatever [`Modal`](crate::app::Modal) is currently set on the app
//! state as a centered, titled block. The caller decides *what* to show; this
//! module decides *how*.

use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, ModalContent};

/// Render the current modal overlay, if any, centered over the frame.
///
/// The [`title`](crate::app::Modal::title) is drawn on the top border and the
/// [`footer`](crate::app::Modal::footer) on the bottom border, so every modal
/// advertises its own key bindings (typically `Esc` to close).
pub(super) fn render_modal(frame: &mut Frame, app: &App) {
    let Some(modal) = &app.modal else {
        return;
    };

    let modal_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Length(15),
            Constraint::Percentage(30),
        ])
        .split(frame.area())[1];

    let block = Block::default()
        .title(Line::from(modal.title.clone()).alignment(Alignment::Center))
        .title_bottom(Line::from(modal.footer_text()).alignment(Alignment::Center))
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::DarkGray));

    let modal_paragraph = Paragraph::new(modal_lines(&modal.content))
        .style(Style::default().fg(Color::White))
        .block(block);
    frame.render_widget(Clear, modal_area);
    frame.render_widget(modal_paragraph, modal_area);
}

fn modal_lines(content: &ModalContent) -> Vec<Line<'static>> {
    match content {
        ModalContent::Text(body) => body
            .lines()
            .map(|line| Line::from(line.to_string()))
            .collect(),
        ModalContent::Menu(menu) => {
            if menu.items.is_empty() {
                return vec![Line::from(menu.empty_message.clone())];
            }

            let (start, end) = menu.page_bounds();
            menu.items[start..end]
                .iter()
                .enumerate()
                .map(|(offset, item)| {
                    let item_index = start + offset;
                    let prefix = if item_index == menu.selected_index {
                        "> "
                    } else {
                        "  "
                    };
                    let style = if item_index == menu.selected_index {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    Line::from(vec![
                        Span::styled(prefix.to_string(), style),
                        Span::styled(item.label.clone(), style),
                        Span::styled("  ".to_string(), style),
                        Span::styled(item.detail.clone(), style),
                    ])
                })
                .collect()
        }
    }
}
