//! UI layout entry point and shared line-building helpers.
//!
//! Rendering is split across submodules: [`header`], [`conversation`],
//! [`input`], [`status_bar`], [`mention`], and [`modal`]. This file wires the
//! main layout and hosts small helpers (assistant padding, formatting of
//! optional stats) shared across rendering modules.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, LayoutMode};

mod conversation;
mod header;
mod history;
mod input;
mod markdown;
mod mention;
mod modal;
mod status_bar;
mod tool_lines;

use conversation::render_conversation;
use header::render_header_lines;
use input::{input_box_height, render_input};
use mention::render_mention_popup;
use modal::render_modal;
use status_bar::render_status_bar;

pub(crate) use tool_lines::{
    default_tool_byline_with_lookup, default_tool_result_lines, ToolOutcomeLookup,
};

pub(super) const ASSISTANT_PADDING: &str = "  ";

pub(super) fn assistant_padding_span() -> Span<'static> {
    Span::raw(ASSISTANT_PADDING.to_string())
}

pub(super) fn prefixed_plain_line(content: impl Into<String>) -> Line<'static> {
    Line::from(vec![assistant_padding_span(), Span::raw(content.into())])
}

pub(super) fn prefixed_styled_line(
    content: impl Into<String>,
    style: Style,
) -> Line<'static> {
    Line::from(vec![
        assistant_padding_span(),
        Span::styled(content.into(), style),
    ])
}

/// Draw the UI using the selected layout mode.
pub fn draw(frame: &mut Frame, mode: LayoutMode, app: &App) {
    match mode {
        LayoutMode::Main => draw_main(frame, app),
        LayoutMode::Alternate => draw_alternate(frame),
    }
}

fn draw_main(frame: &mut Frame, app: &App) {
    let input_prompt = "> ";
    let header_lines = render_header_lines(app);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .vertical_margin(1)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(input_box_height(app, input_prompt, frame.area())),
        ])
        .split(frame.area());
    let conversation = history::build_conversation_lines(app, header_lines, chunks[0].width);

    let status_area = Rect {
        x: 1, // Account for left margin
        y: frame.area().height - 1,
        width: frame.area().width + 6, // Account for both left and right margins
        height: 1,
    };

    render_conversation(frame, app, chunks[0], conversation);
    render_input(frame, app, chunks[1], input_prompt);
    render_mention_popup(frame, app, chunks[1]);
    render_status_bar(frame, app, status_area);
    render_modal(frame, app);
}

fn draw_alternate(frame: &mut Frame) {
    let block = Block::default()
        .title("Alternate")
        .borders(Borders::ALL)
        .style(Style::default().fg(ratatui::style::Color::Magenta));

    let paragraph = Paragraph::new("You are in the alternate layout")
        .style(Style::default().fg(ratatui::style::Color::White))
        .block(block);

    let area = Layout::default()
        .direction(Direction::Vertical)
        .margin(3)
        .constraints([Constraint::Percentage(100)])
        .split(frame.area())[0];

    frame.render_widget(paragraph, area);
}

pub(super) fn format_context_left(context_left_percent: Option<u8>) -> String {
    context_left_percent
        .map(|percent| format!("{percent}% left"))
        .unwrap_or_else(|| "n/a".to_string())
}

pub(super) fn format_estimated_cost(estimated_cost_usd: Option<f32>) -> String {
    estimated_cost_usd
        .map(|cost| format!("~${cost:.4}"))
        .unwrap_or_else(|| "n/a".to_string())
}
