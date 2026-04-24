//! Conversation pane rendering.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;

use super::history::ConversationLines;

/// Render the scrollable conversation/history pane.
pub(super) fn render_conversation(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    conversation: ConversationLines,
) {
    let mut conversation_lines = conversation.lines;
    let horizontal_line = Line::from("─".repeat(area.width as usize));
    for &position in conversation.turn_finished_positions.iter().rev() {
        if position < conversation_lines.len() {
            conversation_lines.insert(position, horizontal_line.clone());
        }
    }

    let conversation_width = usize::from(area.width.max(1));
    let conversation_height = 1 + conversation_lines
        .iter()
        .map(|line| usize::max(1, line.width().div_ceil(conversation_width)))
        .sum::<usize>();
    let conversation_paragraph = Paragraph::new(conversation_lines)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::NONE)
                .style(Style::default().fg(Color::Gray)),
        );
    let visible_height = conversation_height.min(area.height as usize) as u16;
    let conversation_area = if visible_height < area.height {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(visible_height)])
            .split(area)[1]
    } else {
        area
    };
    let max_scroll = conversation_height.saturating_sub(conversation_area.height as usize);
    let scroll_y =
        max_scroll.saturating_sub(app.conversation_scroll_from_bottom.min(max_scroll)) as u16;
    frame.render_widget(
        conversation_paragraph.scroll((scroll_y, 0)),
        conversation_area,
    );
}
