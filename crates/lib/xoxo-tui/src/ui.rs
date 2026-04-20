//! UI layout and rendering.

use std::env;

use ansi_to_tui::IntoText as _;
use comrak::{
    Arena, Options, parse_document,
    nodes::{ListType, NodeCode, NodeHeading, NodeLink, NodeMath, NodeValue},
};
const HEADER_ART: &str = include_str!("../hedgehog");

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style, Modifier},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use ratatui::layout::{Alignment, Position};
use ratatui::widgets::TitlePosition;
use crate::app::{LayoutMode, App};

const ASSISTANT_PADDING: &str = "  ";

/// Parse a message line to extract role and content
/// Expected format: "role[chat_id] content"
fn parse_message_line(line: &str) -> Option<(&str, &str)> {
    // Find the first space to separate role[chat_id] from content
    if let Some(space_pos) = line.find(' ') {
        let role_part = &line[..space_pos];
        let content = &line[space_pos + 1..];
        
        // Extract role from role[chat_id]
        if let Some(bracket_pos) = role_part.find('[') {
            let role = &role_part[..bracket_pos];
            return Some((role, content));
        }
    }
    None
}

fn is_markdown_assistant_message(content: &str) -> bool {
    !content.starts_with("tool[") && !content.starts_with("error:") && content != "shutdown"
}

fn assistant_padding_span() -> Span<'static> {
    Span::raw(ASSISTANT_PADDING.to_string())
}

fn prefixed_plain_line(content: impl Into<String>) -> Line<'static> {
    Line::from(vec![assistant_padding_span(), Span::raw(content.into())])
}

fn prefixed_styled_line(content: impl Into<String>, style: Style) -> Line<'static> {
    Line::from(vec![
        assistant_padding_span(),
        Span::styled(content.into(), style),
    ])
}

fn render_markdown_message(content: &str) -> Vec<Line<'static>> {
    let arena = Arena::new();
    let options = Options::default();
    let root = parse_document(&arena, content, &options);
    let mut renderer = MarkdownRenderer::default();
    renderer.render_document(root);
    renderer.finish()
}

#[derive(Default)]
struct MarkdownRenderer {
    lines: Vec<Line<'static>>,
}

impl MarkdownRenderer {
    fn render_document<'a>(&mut self, root: comrak::Node<'a>) {
        let children: Vec<_> = root.children().collect();
        for (index, child) in children.into_iter().enumerate() {
            self.render_block(child, ASSISTANT_PADDING, ASSISTANT_PADDING);
            if index + 1 < root.children().count() {
                self.lines.push(Line::from(""));
            }
        }
    }

    fn render_block<'a>(&mut self, node: comrak::Node<'a>, first_prefix: &str, continuation_prefix: &str) {
        match node.data().value.clone() {
            NodeValue::Paragraph => {
                self.lines.extend(render_inline_block(
                    node,
                    first_prefix,
                    continuation_prefix,
                    Style::default(),
                ));
            }
            NodeValue::Heading(heading) => {
                self.lines.extend(render_inline_block(
                    node,
                    first_prefix,
                    continuation_prefix,
                    heading_style(heading),
                ));
            }
            NodeValue::CodeBlock(code_block) => {
                let style = Style::default()
                    .fg(Color::Indexed(179))
                    .bg(Color::Indexed(235));
                let mut emitted = false;
                for line in code_block.literal.lines() {
                    self.lines.push(prefixed_styled_line(
                        format!("{first_prefix}{line}").trim_start_matches(ASSISTANT_PADDING),
                        style,
                    ));
                    emitted = true;
                }
                if !emitted {
                    self.lines.push(prefixed_styled_line("", style));
                }
            }
            NodeValue::BlockQuote => {
                let quoted_first = format!("{first_prefix}> ");
                let quoted_continuation = format!("{continuation_prefix}> ");
                let children: Vec<_> = node.children().collect();
                for (index, child) in children.into_iter().enumerate() {
                    self.render_block(child, &quoted_first, &quoted_continuation);
                    if index + 1 < node.children().count() {
                        self.lines.push(Line::from(""));
                    }
                }
            }
            NodeValue::List(list) => {
                let items: Vec<_> = node.children().collect();
                for (index, item) in items.into_iter().enumerate() {
                    let marker = match list.list_type {
                        ListType::Bullet => "• ".to_string(),
                        ListType::Ordered => format!("{}. ", list.start + index),
                    };
                    let item_first = format!("{first_prefix}{marker}");
                    let item_continuation =
                        format!("{continuation_prefix}{}", " ".repeat(marker.chars().count()));
                    self.render_list_item(item, &item_first, &item_continuation);
                }
            }
            NodeValue::ThematicBreak => {
                self.lines.push(prefixed_styled_line(
                    format!("{first_prefix}{}", "─".repeat(24)).trim_start_matches(ASSISTANT_PADDING),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            NodeValue::HtmlBlock(html) => {
                let style = Style::default().fg(Color::DarkGray);
                for line in html.literal.lines() {
                    self.lines.push(prefixed_styled_line(
                        format!("{first_prefix}{line}").trim_start_matches(ASSISTANT_PADDING),
                        style,
                    ));
                }
            }
            _ => {
                let fallback = collect_plain_text(node);
                if fallback.is_empty() {
                    self.lines.push(prefixed_plain_line(""));
                } else {
                    self.lines.extend(render_inline_block(
                        node,
                        first_prefix,
                        continuation_prefix,
                        Style::default(),
                    ));
                }
            }
        }
    }

    fn render_list_item<'a>(&mut self, item: comrak::Node<'a>, first_prefix: &str, continuation_prefix: &str) {
        let children: Vec<_> = item.children().collect();
        for (index, child) in children.into_iter().enumerate() {
            let block_first_prefix = if index == 0 {
                first_prefix
            } else {
                continuation_prefix
            };
            self.render_block(child, block_first_prefix, continuation_prefix);
            if index + 1 < item.children().count() {
                self.lines.push(Line::from(""));
            }
        }
    }

    fn finish(self) -> Vec<Line<'static>> {
        if self.lines.is_empty() {
            vec![prefixed_plain_line("")]
        } else {
            self.lines
        }
    }
}

fn render_inline_block<'a>(
    node: comrak::Node<'a>,
    first_prefix: &str,
    continuation_prefix: &str,
    base_style: Style,
) -> Vec<Line<'static>> {
    let mut builder = InlineBlockBuilder::new(first_prefix);
    for child in node.children() {
        render_inline_node(child, &mut builder, base_style, continuation_prefix);
    }
    builder.finish()
}

struct InlineBlockBuilder {
    lines: Vec<Vec<Span<'static>>>,
}

impl InlineBlockBuilder {
    fn new(prefix: &str) -> Self {
        Self {
            lines: vec![vec![Span::raw(prefix.to_string())]],
        }
    }

    fn push_span(&mut self, span: Span<'static>) {
        if let Some(line) = self.lines.last_mut() {
            line.push(span);
        }
    }

    fn push_text(&mut self, text: impl Into<String>, style: Style) {
        let text = text.into();
        if text.is_empty() {
            return;
        }
        self.push_span(Span::styled(text, style));
    }

    fn break_line(&mut self, prefix: &str) {
        self.lines.push(vec![Span::raw(prefix.to_string())]);
    }

    fn finish(self) -> Vec<Line<'static>> {
        self.lines.into_iter().map(Line::from).collect()
    }
}

fn render_inline_node<'a>(
    node: comrak::Node<'a>,
    builder: &mut InlineBlockBuilder,
    style: Style,
    continuation_prefix: &str,
) {
    match node.data().value.clone() {
        NodeValue::Text(text) => builder.push_text(text.to_string(), style),
        NodeValue::SoftBreak | NodeValue::LineBreak => builder.break_line(continuation_prefix),
        NodeValue::Code(NodeCode { literal, .. }) => builder.push_text(
            literal,
            style
                .fg(Color::Indexed(179))
                .bg(Color::Indexed(235))
                .add_modifier(Modifier::BOLD),
        ),
        NodeValue::Emph => render_inline_children(
            node,
            builder,
            style.add_modifier(Modifier::ITALIC),
            continuation_prefix,
        ),
        NodeValue::Strong => render_inline_children(
            node,
            builder,
            style.add_modifier(Modifier::BOLD),
            continuation_prefix,
        ),
        NodeValue::Strikethrough => render_inline_children(
            node,
            builder,
            style.add_modifier(Modifier::CROSSED_OUT),
            continuation_prefix,
        ),
        NodeValue::Link(link) => render_link(node, link, builder, style, continuation_prefix),
        NodeValue::Image(link) => {
            builder.push_text("[image: ", style.fg(Color::DarkGray));
            render_inline_children(node, builder, style.add_modifier(Modifier::ITALIC), continuation_prefix);
            if node.first_child().is_none() {
                builder.push_text(link.url, style.add_modifier(Modifier::ITALIC));
            }
            builder.push_text("]", style.fg(Color::DarkGray));
        }
        NodeValue::Math(NodeMath { literal, .. }) => builder.push_text(
            literal,
            style
                .fg(Color::Indexed(179))
                .add_modifier(Modifier::ITALIC),
        ),
        NodeValue::HtmlInline(html) | NodeValue::Raw(html) => {
            builder.push_text(html, style.fg(Color::DarkGray))
        }
        _ => render_inline_children(node, builder, style, continuation_prefix),
    }
}

fn render_inline_children<'a>(
    node: comrak::Node<'a>,
    builder: &mut InlineBlockBuilder,
    style: Style,
    continuation_prefix: &str,
) {
    for child in node.children() {
        render_inline_node(child, builder, style, continuation_prefix);
    }
}

fn render_link<'a>(
    node: comrak::Node<'a>,
    link: Box<NodeLink>,
    builder: &mut InlineBlockBuilder,
    style: Style,
    continuation_prefix: &str,
) {
    let link_style = style
        .fg(Color::Cyan)
        .add_modifier(Modifier::UNDERLINED);
    if node.first_child().is_none() {
        builder.push_text(link.url, link_style);
        return;
    }
    render_inline_children(node, builder, link_style, continuation_prefix);
}

fn heading_style(heading: NodeHeading) -> Style {
    let color = match heading.level {
        1 => Color::LightCyan,
        2 => Color::Cyan,
        3 => Color::LightBlue,
        _ => Color::White,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn collect_plain_text<'a>(node: comrak::Node<'a>) -> String {
    let mut text = String::new();
    for descendant in node.descendants() {
        match descendant.data().value.clone() {
            NodeValue::Text(value) => text.push_str(&value),
            NodeValue::Code(NodeCode { literal, .. }) => text.push_str(&literal),
            NodeValue::SoftBreak | NodeValue::LineBreak => text.push('\n'),
            _ => {}
        }
    }
    text
}


#[allow(dead_code)]
/// Draw the UI using a generic layout.
///
/// The layout is built from a slice of `Section`s. For the proof‑of‑concept we create two sections:
/// * a main area that shows “Hello, world!”
/// * a status bar that displays a quit hint.
///
/// The function computes equal vertical constraints for all sections, builds a block for each,
/// and renders them in order.
pub fn draw(frame: &mut Frame, mode: LayoutMode, app: &App) {
    // Choose layout based on the current mode.
    match mode {
        LayoutMode::Main => {
            // Main layout – header as part of conversation history, input textarea snapped to bottom.
            let current_dir = env::current_dir()
                .ok()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<unknown cwd>".to_string());
            let header_art = HEADER_ART
                .replace("{VERSION}", env!("CARGO_PKG_VERSION"))
                .replace("{PWD}", &current_dir)
                .replace("{MODEL}", &app.current_model_name)
                .replace("{PROVIDER}", &app.current_provider_name)
                .replace("\\033[", "\x1b[")
                .replace("/\\","\x1b[38;5;235m/\\\x1b[0m")
                .replace("\\","\x1b[38;5;235m\\\x1b[0m")
                .replace("/ ","\x1b[38;5;235m/ \x1b[0m")
                .replace("|","\x1b[38;5;235m|\x1b[0m")
                .replace("‖","\x1b[38;5;235m• \x1b[0m")
                .replace("<","\x1b[38;5;235m<\x1b[0m")
                .replace("_","\x1b[38;5;235m_\x1b[0m")
                .replace("$","\x1b[38;5;240m$\x1b[0m");
            let input_prompt = "> ";
            
            // Build conversation content in visual order so the whole pane can anchor from the bottom.
            let mut conversation_lines = Vec::new();

            let header_text = header_art
                .into_text()
                .unwrap_or_else(|_| Text::raw(header_art.clone()));
            for line in header_text.lines.iter().cloned() {
                conversation_lines.push(line);
            }

            // Margin between header and history
            conversation_lines.push(Line::from(""));
            conversation_lines.push(Line::from(""));

            // Add conversation history with empty lines between entries
            for (index, line) in app.history.iter().enumerate() {
                // Parse the line to extract role and content
                if let Some((role, content)) = parse_message_line(line) {
                    if role == "user" {
                        // Format user messages with bold white text and left border
                        // Handle multi-line content by splitting on newlines
                        for content_line in content.lines() {
                            let border_span = Span::styled("│", Style::default().fg(Color::Indexed(202)));
                            let content_span = Span::styled(content_line, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
                            let styled_line = Line::from(vec![border_span, Span::raw(" "), content_span]);
                            conversation_lines.push(styled_line);
                        }
                    } else {
                        if is_markdown_assistant_message(content) {
                            conversation_lines.extend(render_markdown_message(content));
                        } else {
                            // Keep tool/error/status messages on the existing plain-text path.
                            for content_line in content.lines() {
                                conversation_lines.push(prefixed_plain_line(content_line));
                            }
                        }
                    }
                } else {
                    // Fallback for unparseable lines
                    conversation_lines.push(Line::from(line.clone()));
                }
                
                if index < app.history.len() - 1 {
                    conversation_lines.push(Line::from(""));
                }
            }
            
            // Add three empty lines as top margin above the input textarea
            conversation_lines.push(Line::from(""));
            conversation_lines.push(Line::from(""));
            conversation_lines.push(Line::from(""));

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Min(0),      // Conversation area fills available space
                    Constraint::Length(3),   // Input area (fixed at bottom)
                ])
                .split(frame.area());

            // Render the conversation in a bottom-aligned viewport that ends exactly above the
            // input box. When content is taller than the viewport, scroll inside that viewport.
            let conversation_width = usize::from(chunks[0].width.max(1));
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
            let visible_height = conversation_height.min(chunks[0].height as usize) as u16;
            let conversation_area = if visible_height < chunks[0].height {
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(visible_height)])
                    .split(chunks[0])[1]
            } else {
                chunks[0]
            };
            let max_scroll = conversation_height.saturating_sub(conversation_area.height as usize);
            let scroll_y = max_scroll
                .saturating_sub(app.conversation_scroll_from_bottom.min(max_scroll)) as u16;
            frame.render_widget(
                conversation_paragraph.scroll((scroll_y, 0)),
                conversation_area,
            );

            // Render input field
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
            frame.render_widget(input_paragraph, chunks[1]);
            frame.set_cursor_position(Position::new(
                chunks[1].x + input_prompt.chars().count() as u16 + app.input.chars().count() as u16,
                chunks[1].y + 1,
            ));

            // Modal popup (appears centered when modal_content is set)
            if let Some(modal_content) = &app.modal_content {
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
        }
        LayoutMode::Alternate => {
            // Alternate layout – a single centered message.
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
    }
}
