//! Input box rendering, sizing, and text wrapping.

use ratatui::layout::{Alignment, Position, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, TitlePosition};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar as _, UnicodeWidthStr as _};

use crate::app::App;

const INPUT_BOX_BORDER_HEIGHT: u16 = 2;
const MIN_INPUT_BOX_HEIGHT: u16 = INPUT_BOX_BORDER_HEIGHT + 1;

/// Render the bottom input box and position the terminal cursor.
pub(super) fn render_input(frame: &mut Frame, app: &App, area: Rect, input_prompt: &str) {
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

/// Compute the desired height of the input box based on the current buffer.
pub(super) fn input_box_height(app: &App, input_prompt: &str, area: Rect) -> u16 {
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
