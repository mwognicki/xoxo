//! Modal overlay rendering.
//!
//! Draws whatever [`Modal`](crate::app::Modal) is currently set on the app
//! state as a centered, titled block. The caller decides *what* to show; this
//! module decides *how*.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, ConfigFocus, ConfigModal, ModalContent};

/// Render the current modal overlay, if any, centered over the frame.
///
/// The [`title`](crate::app::Modal::title) is drawn on the top border and the
/// [`footer`](crate::app::Modal::footer) on the bottom border, so every modal
/// advertises its own key bindings (typically `Esc` to close).
pub(super) fn render_modal(frame: &mut Frame, app: &App) {
    let Some(modal) = &app.modal else {
        return;
    };

    let modal_area = modal_area(frame.area(), &modal.content);

    let block = Block::default()
        .title(Line::from(modal.title.clone()).alignment(Alignment::Center))
        .title_bottom(Line::from(modal.footer_text()).alignment(Alignment::Center))
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::DarkGray));

    frame.render_widget(Clear, modal_area);
    frame.render_widget(block, modal_area);

    let inner_area = Rect {
        x: modal_area.x + 1,
        y: modal_area.y + 1,
        width: modal_area.width.saturating_sub(2),
        height: modal_area.height.saturating_sub(2),
    };

    match &modal.content {
        ModalContent::Config(config) => render_config_modal(frame, config, inner_area),
        content => {
            let modal_paragraph = Paragraph::new(modal_lines(content))
                .style(Style::default().fg(Color::White));
            frame.render_widget(modal_paragraph, inner_area);
        }
    }
}

fn modal_area(frame_area: Rect, content: &ModalContent) -> Rect {
    match content {
        ModalContent::Config(_) => centered_rect(frame_area, 82, 70),
        ModalContent::Text(_) | ModalContent::Menu(_) => Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Length(15),
                Constraint::Percentage(30),
            ])
            .split(frame_area)[1],
    }
}

fn centered_rect(frame_area: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_percent) / 2),
            Constraint::Percentage(height_percent),
            Constraint::Percentage((100 - height_percent) / 2),
        ])
        .split(frame_area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .split(vertical[1])[1]
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
        ModalContent::Config(_) => Vec::new(),
    }
}

fn render_config_modal(frame: &mut Frame, config: &ConfigModal, area: Rect) {
    if area.width < 12 || area.height < 6 {
        return;
    }

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(34), Constraint::Percentage(66)])
        .split(area);

    let nav_style = match config.focus {
        ConfigFocus::Navigation => Style::default().fg(Color::White),
        ConfigFocus::Detail => Style::default().fg(Color::Gray),
    };
    let detail_style = match config.focus {
        ConfigFocus::Navigation => Style::default().fg(Color::White),
        ConfigFocus::Detail => Style::default().fg(Color::White),
    };

    let nav_lines = config
        .sections
        .iter()
        .enumerate()
        .map(|(index, section)| {
            let is_selected = index == config.selected_index;
            let prefix = if is_selected { ">" } else { " " };
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                nav_style
            };
            Line::from(vec![
                Span::styled(format!("{prefix} "), style),
                Span::styled(ConfigModal::section_label(*section).to_string(), style),
            ])
        })
        .collect::<Vec<_>>();

    let nav_block = Block::default()
        .title(Line::from(" Sections ").alignment(Alignment::Left))
        .borders(Borders::RIGHT);
    frame.render_widget(
        Paragraph::new(nav_lines).block(nav_block).style(nav_style),
        panes[0],
    );

    let detail_lines = config
        .detail_lines()
        .into_iter()
        .map(Line::from)
        .collect::<Vec<_>>();
    let detail_block = Block::default()
        .title(Line::from(format!(" {} ", config.detail_title())).alignment(Alignment::Left));
    frame.render_widget(
        Paragraph::new(detail_lines)
            .block(detail_block)
            .style(detail_style)
            .scroll((config.detail_scroll as u16, 0)),
        panes[1],
    );
}
