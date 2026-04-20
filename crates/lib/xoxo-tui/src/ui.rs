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
use xoxo_core::bus::BusPayload;
use xoxo_core::chat::structs::{
    ChatTextRole, ChatToolCallId, ToolCallEvent, ToolCallStarted,
};

use crate::app::{App, HistoryEntry, LayoutMode};
use crate::tool_format;

const ASSISTANT_PADDING: &str = "  ";

fn is_markdown_assistant_message(entry: &HistoryEntry) -> bool {
    matches!(
        &entry.payload,
        BusPayload::Message(message) if message.role == ChatTextRole::Agent
    )
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

fn format_tool_arguments(arguments: &impl std::fmt::Display) -> String {
    let rendered = arguments.to_string();
    if rendered == "null" {
        String::new()
    } else if rendered.starts_with('{') || rendered.starts_with('[') {
        rendered
    } else {
        format!("({rendered})")
    }
}

fn pulsing_tool_dot_style(app: &App) -> Style {
    let phase = (app.started_at.elapsed().as_millis() / 200) % 6;
    let color = match phase {
        0 | 5 => Color::Indexed(238),
        1 | 4 => Color::Indexed(241),
        _ => Color::White,
    };
    let mut style = Style::default().fg(color);
    if matches!(phase, 2 | 3) {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
}

fn tool_outcome<'a>(app: &'a App, started: &ToolCallStarted) -> Option<&'a ToolCallEvent> {
    app.history.iter().find_map(|entry| match &entry.payload {
        BusPayload::ToolCall(ToolCallEvent::Completed(completed))
            if completed.tool_call_id == started.tool_call_id =>
        {
            Some(match &entry.payload {
                BusPayload::ToolCall(event) => event,
                _ => unreachable!(),
            })
        }
        BusPayload::ToolCall(ToolCallEvent::Failed(failed))
            if failed.tool_call_id == started.tool_call_id =>
        {
            Some(match &entry.payload {
                BusPayload::ToolCall(event) => event,
                _ => unreachable!(),
            })
        }
        _ => None,
    })
}

fn tool_dot_style(app: &App, started: &ToolCallStarted) -> Style {
    match tool_outcome(app, started) {
        Some(ToolCallEvent::Completed(_)) => Style::default().fg(Color::Indexed(70)),
        Some(ToolCallEvent::Failed(_)) => Style::default().fg(Color::Indexed(160)),
        _ => pulsing_tool_dot_style(app),
    }
}

fn doing_indicator_style(app: &App) -> Style {
    let phase = (app.started_at.elapsed().as_millis() / 200) % 6;
    let color = match phase {
        0 | 5 => Color::Indexed(166),
        1 | 4 => Color::Indexed(172),
        2 | 3 => Color::Indexed(202),
        _ => Color::Indexed(208),
    };
    let mut style = Style::default().fg(color);
    if matches!(phase, 2 | 3) {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
}

pub(crate) fn default_tool_byline(app: &App, started: &ToolCallStarted) -> Line<'static> {
    tool_byline(app, started)
}

pub(crate) fn default_tool_result_lines(content: &str, is_error: bool) -> Vec<Line<'static>> {
    tool_result_lines(content, is_error)
}

fn tool_byline(app: &App, started: &ToolCallStarted) -> Line<'static> {
    let mut spans = vec![
        Span::styled("• ", tool_dot_style(app, started)),
        Span::styled(
            started.tool_name.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    let arguments = format_tool_arguments(&started.arguments);
    if !arguments.is_empty() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            arguments,
            Style::default().fg(Color::Gray),
        ));
    }
    Line::from(spans)
}

fn tool_result_lines(content: &str, is_error: bool) -> Vec<Line<'static>> {
    let style = Style::default().fg(Color::DarkGray);
    let mut lines = Vec::new();

    for (index, content_line) in content.lines().enumerate() {
        let prefix = if index == 0 { "└ " } else { "  " };
        let text = if is_error && index == 0 {
            format!("{prefix}error: {content_line}")
        } else {
            format!("{prefix}{content_line}")
        };
        lines.push(Line::from(Span::styled(text, style)));
    }

    if lines.is_empty() {
        let text = if is_error { "└ error:" } else { "└" };
        lines.push(Line::from(Span::styled(text, style)));
    }

    lines
}

fn is_tool_result_entry(entry: &HistoryEntry) -> bool {
    matches!(
        entry.payload,
        BusPayload::ToolCall(ToolCallEvent::Completed(_))
            | BusPayload::ToolCall(ToolCallEvent::Failed(_))
    )
}

fn should_prepend_spacing(previous_entry: Option<&HistoryEntry>, entry: &HistoryEntry) -> bool {
    previous_entry.is_some() && !is_tool_result_entry(entry)
}

fn has_matching_tool_start(app: &App, tool_call_id: &ChatToolCallId) -> bool {
    app.history.iter().any(|entry| {
        matches!(
            &entry.payload,
            BusPayload::ToolCall(ToolCallEvent::Started(started))
                if &started.tool_call_id == tool_call_id
        )
    })
}

fn render_tool_outcome_lines(app: &App, started: &ToolCallStarted) -> Vec<Line<'static>> {
    match tool_outcome(app, started) {
        Some(ToolCallEvent::Completed(completed)) => tool_format::format_completed(app, completed),
        Some(ToolCallEvent::Failed(failed)) => tool_format::format_failed(app, failed),
        Some(ToolCallEvent::Started(_)) | None => Vec::new(),
    }
}

fn render_plain_payload(app: &App, entry: &HistoryEntry) -> Vec<Line<'static>> {
    match &entry.payload {
        BusPayload::Message(message) => message
            .content
            .lines()
            .map(|content_line| {
                let border_span = Span::styled("│", Style::default().fg(Color::Indexed(202)));
                let content_span = Span::styled(
                    content_line.to_string(),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                );
                Line::from(vec![border_span, Span::raw(" "), content_span])
            })
            .collect(),
        BusPayload::ToolCall(ToolCallEvent::Started(started)) => {
            let mut lines = tool_format::format_started(app, started);
            lines.extend(render_tool_outcome_lines(app, started));
            lines
        }
        BusPayload::ToolCall(ToolCallEvent::Completed(completed)) => {
            if has_matching_tool_start(app, &completed.tool_call_id) {
                Vec::new()
            } else {
                tool_format::format_completed(app, completed)
            }
        }
        BusPayload::ToolCall(ToolCallEvent::Failed(failed)) => {
            if has_matching_tool_start(app, &failed.tool_call_id) {
                Vec::new()
            } else {
                tool_format::format_failed(app, failed)
            }
        }
        BusPayload::Turn(_) => Vec::new(),
        BusPayload::AgentShutdown => vec![prefixed_plain_line("shutdown")],
        BusPayload::Error(error) => vec![prefixed_plain_line(format!("error: {}", error.message))],
    }
}

fn collapse_blank_lines(lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    let mut collapsed = Vec::with_capacity(lines.len());
    let mut previous_was_blank = false;

    for line in lines {
        let is_blank = line
            .spans
            .iter()
            .all(|span| span.content.trim().is_empty());
        if is_blank && previous_was_blank {
            continue;
        }
        previous_was_blank = is_blank;
        collapsed.push(line);
    }

    collapsed
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
                .replace("{INPUT_TOKENS}", &app.total_input_tokens.to_string())
                .replace("{OUTPUT_TOKENS}", &app.total_output_tokens.to_string())
                .replace("{CONTEXT_LEFT}", &format_context_left(app.context_left_percent))
                .replace("{EST_COST}", &format_estimated_cost(app.estimated_cost_usd))
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

            // Add conversation history with spacing controlled by the incoming entry.
            let mut previous_entry: Option<&HistoryEntry> = None;
            for entry in &app.history {
                if should_prepend_spacing(previous_entry, entry) {
                    conversation_lines.push(Line::from(""));
                }

                if is_markdown_assistant_message(entry) {
                    if let BusPayload::Message(message) = &entry.payload {
                        conversation_lines.extend(render_markdown_message(&message.content));
                    }
                } else {
                    conversation_lines.extend(render_plain_payload(app, entry));
                }
                previous_entry = Some(entry);
            }

            if app.turn_in_progress {
                if previous_entry.is_some() {
                    conversation_lines.push(Line::from(""));
                }
                conversation_lines.push(prefixed_styled_line(
                    "Doing...",
                    doing_indicator_style(app),
                ));
            }
            
            // Add three empty lines as top margin above the input textarea
            conversation_lines.push(Line::from(""));
            conversation_lines.push(Line::from(""));
            conversation_lines.push(Line::from(""));
            conversation_lines = collapse_blank_lines(conversation_lines);

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

    use std::time::Instant;
    use uuid::Uuid;
    use xoxo_core::bus::BusPayload;
    use xoxo_core::chat::structs::{ChatToolCallId, ToolCallCompleted, ToolCallEvent, ToolCallStarted};

    fn test_app_with_history(history: Vec<HistoryEntry>) -> App {
        App {
            running: true,
            layout: LayoutMode::Main,
            input: String::new(),
            active_chat_id: Some(Uuid::new_v4()),
            pending_submission: None,
            current_provider_name: "test-provider".to_string(),
            current_model_name: "test-model".to_string(),
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_used_tokens: 0,
            context_left_percent: None,
            max_input_tokens: None,
            estimated_cost_usd: None,
            history,
            conversation_scroll_from_bottom: 0,
            modal_content: None,
            ctrl_c_count: 0,
            started_at: Instant::now(),
            turn_in_progress: false,
            last_turn_finish_reason: None,
        }
    }

    #[test]
    fn started_tool_call_renders_matching_result_once() {
        let tool_call_id = ChatToolCallId("tool-1".to_string());
        let started = ToolCallStarted {
            tool_call_id: tool_call_id.clone(),
            tool_name: "read_file".to_string(),
            arguments: serde_json::json!({ "path": "Cargo.toml" }),
            tool_call_kind: xoxo_core::chat::structs::ToolCallKind::Generic,
        };
        let completed = ToolCallCompleted {
            tool_call_id: tool_call_id.clone(),
            tool_name: "read_file".to_string(),
            result_preview: "done".to_string(),
        };
        let app = test_app_with_history(vec![
            HistoryEntry {
                chat_id: Uuid::new_v4(),
                payload: BusPayload::ToolCall(ToolCallEvent::Started(started.clone())),
            },
            HistoryEntry {
                chat_id: Uuid::new_v4(),
                payload: BusPayload::ToolCall(ToolCallEvent::Completed(completed)),
            },
        ]);

        let started_lines = render_plain_payload(
            &app,
            &HistoryEntry {
                chat_id: Uuid::new_v4(),
                payload: BusPayload::ToolCall(ToolCallEvent::Started(started)),
            },
        );

        assert_eq!(started_lines.len(), 2);
        assert!(started_lines[1].spans[0].content.contains("done"));
    }

    #[test]
    fn completed_tool_call_is_hidden_when_start_exists() {
        let tool_call_id = ChatToolCallId("tool-1".to_string());
        let app = test_app_with_history(vec![
            HistoryEntry {
                chat_id: Uuid::new_v4(),
                payload: BusPayload::ToolCall(ToolCallEvent::Started(ToolCallStarted {
                    tool_call_id: tool_call_id.clone(),
                    tool_name: "read_file".to_string(),
                    arguments: serde_json::json!({}),
                    tool_call_kind: xoxo_core::chat::structs::ToolCallKind::Generic,
                })),
            },
            HistoryEntry {
                chat_id: Uuid::new_v4(),
                payload: BusPayload::ToolCall(ToolCallEvent::Completed(ToolCallCompleted {
                    tool_call_id: tool_call_id.clone(),
                    tool_name: "read_file".to_string(),
                    result_preview: "done".to_string(),
                })),
            },
        ]);
        let completed_entry = HistoryEntry {
            chat_id: Uuid::new_v4(),
            payload: BusPayload::ToolCall(ToolCallEvent::Completed(ToolCallCompleted {
                tool_call_id,
                tool_name: "read_file".to_string(),
                result_preview: "done".to_string(),
            })),
        };

        assert!(render_plain_payload(&app, &completed_entry).is_empty());
    }
}
