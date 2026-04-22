//! UI layout and rendering.

use std::env;

use ansi_to_tui::IntoText as _;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Position};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, TitlePosition, Wrap};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar as _, UnicodeWidthStr as _};

use crate::app::{App, LayoutMode};

mod history;
mod markdown;
mod tool_lines;

pub(crate) use tool_lines::{default_tool_byline, default_tool_result_lines};

const HEADER_ART: &str = include_str!("../hedgehog");
pub(super) const ASSISTANT_PADDING: &str = "  ";
const INPUT_BOX_BORDER_HEIGHT: u16 = 2;
const MIN_INPUT_BOX_HEIGHT: u16 = INPUT_BOX_BORDER_HEIGHT + 1;

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
    let input_height = input_box_height(app, input_prompt, frame.area());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .vertical_margin(1)
        .constraints([Constraint::Min(0), Constraint::Length(input_height)])
        .split(frame.area());

    let status_area = ratatui::layout::Rect {
        x: 1, // Account for left margin
        y: frame.area().height - 1,
        width: frame.area().width + 6, // Account for both left and right margins
        height: 1,
    };

    render_conversation(frame, app, chunks[0], conversation);
    render_input(frame, app, chunks[1], input_prompt);
    render_status_bar(frame, app, status_area);
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
        .replace("/_", "\x1b[38;5;235m/_\x1b[0m")
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

fn render_input(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, input_prompt: &str) {
    let input_display = input_display_text(&app.input);
    let input_lines = wrap_input_lines(input_prompt, &input_display, area.width);
    let visible_content_height = area.height.saturating_sub(INPUT_BOX_BORDER_HEIGHT) as usize;
    let input_scroll_y = input_lines.len().saturating_sub(visible_content_height) as u16;
    let cursor_line = input_lines.len().saturating_sub(1);
    let cursor_visible_line = cursor_line.saturating_sub(input_scroll_y as usize) as u16;
    let cursor_x = input_lines
        .last()
        .map(|line| line.width() as u16)
        .unwrap_or_default()
        .min(area.width.saturating_sub(1));
    let input_paragraph = Paragraph::new(input_lines)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .title(" Input")
                .title_position(TitlePosition::Top)
                .title_alignment(Alignment::Right)
                .title_style(Style::default().fg(Color::DarkGray))
                .borders(Borders::TOP | Borders::BOTTOM)
                .style(Style::default().fg(Color::DarkGray)),
        )
        .scroll((input_scroll_y, 0));
    frame.render_widget(input_paragraph, area);
    frame.set_cursor_position(Position::new(
        area.x + cursor_x,
        area.y + 1 + cursor_visible_line,
    ));
}

fn input_box_height(app: &App, input_prompt: &str, area: ratatui::layout::Rect) -> u16 {
    let available_height = area.height.saturating_sub(2);
    let max_input_height = available_height.saturating_sub(1).max(1);
    let input_display = input_display_text(&app.input);
    let wrapped_line_count =
        wrap_input_lines(input_prompt, &input_display, area.width).len() as u16;
    let desired_height = wrapped_line_count.saturating_add(INPUT_BOX_BORDER_HEIGHT);

    desired_height
        .max(MIN_INPUT_BOX_HEIGHT)
        .min(max_input_height.max(MIN_INPUT_BOX_HEIGHT))
}

fn wrap_input_lines(input_prompt: &str, input: &str, width: u16) -> Vec<Line<'static>> {
    wrap_input_text(input_prompt, input, usize::from(width.max(1)))
        .into_iter()
        .map(Line::from)
        .collect()
}

fn input_display_text(input: &str) -> String {
    if !input.contains('\n') {
        return input.to_string();
    }

    format!("[pasted content - {} lines]", input_line_count(input))
}

fn input_line_count(input: &str) -> usize {
    if input.is_empty() {
        return 0;
    }

    input.chars().filter(|&character| character == '\n').count() + 1
}

fn wrap_input_text(input_prompt: &str, input: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut current = input_prompt.to_string();

    for character in input.chars() {
        append_wrapped_character(&mut lines, &mut current, character, width);
    }

    lines.push(current);
    lines
}

fn append_wrapped_character(
    lines: &mut Vec<String>,
    current: &mut String,
    character: char,
    width: usize,
) {
    let character_width = character.width().unwrap_or(0);

    if current.width() + character_width <= width {
        current.push(character);
        return;
    }

    if character.is_whitespace() {
        lines.push(std::mem::take(current));
        return;
    }

    wrap_current_line(lines, current);

    while !current.is_empty() && current.width() + character_width > width {
        lines.push(std::mem::take(current));
    }

    current.push(character);
}

fn wrap_current_line(lines: &mut Vec<String>, current: &mut String) {
    let Some((line_end, next_start)) = last_word_boundary(current) else {
        lines.push(std::mem::take(current));
        return;
    };

    let next_line = current[next_start..].to_string();
    current.truncate(line_end);
    lines.push(std::mem::replace(current, next_line));
}

fn last_word_boundary(line: &str) -> Option<(usize, usize)> {
    let mut current_start = None;
    let mut last_boundary = None;

    for (index, character) in line.char_indices() {
        if character.is_whitespace() {
            current_start.get_or_insert(index);
            if let Some(start) = current_start {
                last_boundary = Some((start, index + character.len_utf8()));
            }
        } else {
            current_start = None;
        }
    }

    last_boundary
}

fn render_status_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let current_dir = env::current_dir()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<unknown cwd>".to_string());

    let left_text = format!("{} • {}", current_dir, app.current_model_name);
    let right_text = format!(
        "↓{} ↑{} • {}",
        app.total_input_tokens,
        app.total_output_tokens,
        format_estimated_cost(app.estimated_cost_usd)
    );

    // Calculate available space and create proper spacing
    let total_content_length = left_text.clone().len() + right_text.clone().len();
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
        // Fallback if not enough space
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_wraps_at_word_boundaries() {
        let expected = vec!["> hello".to_string(), "world".to_string()];
        assert_eq!(wrap_input_text("> ", "hello world", 10), expected);
        assert_eq!(wrap_input_text("> ", "hello world", 7), expected);
    }

    #[test]
    fn input_breaks_long_words_when_no_boundary_fits() {
        assert_eq!(
            wrap_input_text("> ", "abcdefgh", 6),
            vec![">".to_string(), "abcdef".to_string(), "gh".to_string()]
        );
    }

    #[test]
    fn multiline_input_displays_paste_placeholder() {
        assert_eq!(
            input_display_text("alpha\nbeta\ngamma"),
            "[pasted content - 3 lines]"
        );
    }

    #[test]
    fn multiline_placeholder_counts_trailing_empty_line() {
        assert_eq!(
            input_display_text("alpha\nbeta\n"),
            "[pasted content - 3 lines]"
        );
    }
}
