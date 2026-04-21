//! UI layout and rendering.

use std::env;

use ansi_to_tui::IntoText as _;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Position};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, TitlePosition, Wrap};
use ratatui::Frame;

use crate::app::{App, LayoutMode};

mod history;
mod markdown;
mod tool_lines;

pub(crate) use tool_lines::{default_tool_byline, default_tool_result_lines};

const HEADER_ART: &str = include_str!("../hedgehog");
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
    let conversation = history::build_conversation_lines(app, header_lines);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(frame.area());

    render_conversation(frame, app, chunks[0], conversation);
    render_input(frame, app, chunks[1], input_prompt);
    render_modal(frame, app);
}

fn render_header_lines(app: &App) -> Vec<Line<'static>> {
    let current_dir = env::current_dir()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<unknown cwd>".to_string());
    let header_art = HEADER_ART
        .replace("{VERSION}", env!("CARGO_PKG_VERSION"))
        .replace("{PWD}", &current_dir)
        .replace("{MODEL}", &app.current_model_name)
        .replace("{PROVIDER}", &app.current_provider_name)
        .replace("{INPUT_TOKENS}", &app.total_input_tokens.to_string())
        .replace("{OUTPUT_TOKENS}", &app.total_output_tokens.to_string())
        .replace("{CONTEXT_LEFT}", &format_context_left(app.context_left_percent))
        .replace("{EST_COST}", &format_estimated_cost(app.estimated_cost_usd))
        .replace("\\033[", "\x1b[")
        .replace("/\\", "\x1b[38;5;235m/\\\x1b[0m")
        .replace("\\", "\x1b[38;5;235m\\\x1b[0m")
        .replace("/ ", "\x1b[38;5;235m/ \x1b[0m")
        .replace("|", "\x1b[38;5;235m|\x1b[0m")
        .replace("‖", "\x1b[38;5;235m• \x1b[0m")
        .replace("<", "\x1b[38;5;235m<\x1b[0m")
        .replace("_", "\x1b[38;5;235m_\x1b[0m")
        .replace("$", "\x1b[38;5;240m$\x1b[0m");

    let header_text = header_art
        .into_text()
        .unwrap_or_else(|_| Text::raw(header_art.clone()));
    header_text.lines.iter().cloned().collect()
}

fn render_conversation(
    frame: &mut Frame,
    app: &App,
    area: ratatui::layout::Rect,
    conversation: history::ConversationLines,
) {
    let mut conversation_lines = conversation.lines;
    let horizontal_line = Line::from("─".repeat(area.width as usize));
    for &position in conversation.turn_finished_positions.iter().rev() {
        if position < conversation_lines.len() {
            conversation_lines.insert(position, horizontal_line.clone());
        }
    }

    let conversation_width = usize::from(area.width.max(1));
    let conversation_height = conversation_lines
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

fn render_input(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, input_prompt: &str) {
    let input_paragraph = Paragraph::new(format!("{input_prompt}{}", app.input))
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .title(" Input")
                .title_position(TitlePosition::Top)
                .title_alignment(Alignment::Right)
                .title_style(Style::default().fg(Color::DarkGray))
                .borders(Borders::TOP)
                .style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(input_paragraph, area);
    frame.set_cursor_position(Position::new(
        area.x + input_prompt.chars().count() as u16 + app.input.chars().count() as u16,
        area.y + 1,
    ));
}

fn render_modal(frame: &mut Frame, app: &App) {
    let Some(modal_content) = &app.modal_content else {
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

    let modal_paragraph = Paragraph::new(modal_content.clone())
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .title(" Help ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::DarkGray)),
        );
    frame.render_widget(modal_paragraph, modal_area);
}

fn draw_alternate(frame: &mut Frame) {
    let block = Block::default()
        .title("Alternate")
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Magenta));

    let paragraph = Paragraph::new("You are in the alternate layout")
        .style(Style::default().fg(Color::White))
        .block(block);

    let area = Layout::default()
        .direction(Direction::Vertical)
        .margin(3)
        .constraints([Constraint::Percentage(100)])
        .split(frame.area())[0];

    frame.render_widget(paragraph, area);
}

fn format_context_left(context_left_percent: Option<u8>) -> String {
    context_left_percent
        .map(|percent| format!("{percent}% left"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn format_estimated_cost(estimated_cost_usd: Option<f32>) -> String {
    estimated_cost_usd
        .map(|cost| format!("~${cost:.4}"))
        .unwrap_or_else(|| "n/a".to_string())
}
