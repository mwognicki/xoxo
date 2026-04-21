use comrak::nodes::{ListType, NodeCode, NodeHeading, NodeLink, NodeMath, NodeValue};
use comrak::{parse_document, Arena, Options};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::ui::{
    prefixed_plain_line, prefixed_styled_line, ASSISTANT_PADDING,
};

pub(super) fn render_markdown_message(content: &str) -> Vec<Line<'static>> {
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
        for (index, child) in children.iter().enumerate() {
            self.render_block(*child, ASSISTANT_PADDING, ASSISTANT_PADDING);
            if index + 1 < children.len() {
                self.lines.push(Line::from(""));
            }
        }
    }

    fn render_block<'a>(
        &mut self,
        node: comrak::Node<'a>,
        first_prefix: &str,
        continuation_prefix: &str,
    ) {
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
                for (index, child) in children.iter().enumerate() {
                    self.render_block(*child, &quoted_first, &quoted_continuation);
                    if index + 1 < children.len() {
                        self.lines.push(Line::from(""));
                    }
                }
            }
            NodeValue::List(list) => {
                let items: Vec<_> = node.children().collect();
                for (index, item) in items.iter().enumerate() {
                    let marker = match list.list_type {
                        ListType::Bullet => "• ".to_string(),
                        ListType::Ordered => format!("{}. ", list.start + index),
                    };
                    let item_first = format!("{first_prefix}{marker}");
                    let item_continuation =
                        format!("{continuation_prefix}{}", " ".repeat(marker.chars().count()));
                    self.render_list_item(*item, &item_first, &item_continuation);
                }
            }
            NodeValue::ThematicBreak => {
                self.lines.push(prefixed_styled_line(
                    format!("{first_prefix}{}", "─".repeat(24))
                        .trim_start_matches(ASSISTANT_PADDING),
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

    fn render_list_item<'a>(
        &mut self,
        item: comrak::Node<'a>,
        first_prefix: &str,
        continuation_prefix: &str,
    ) {
        let children: Vec<_> = item.children().collect();
        for (index, child) in children.iter().enumerate() {
            let block_first_prefix = if index == 0 {
                first_prefix
            } else {
                continuation_prefix
            };
            self.render_block(*child, block_first_prefix, continuation_prefix);
            if index + 1 < children.len() {
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
            render_inline_children(
                node,
                builder,
                style.add_modifier(Modifier::ITALIC),
                continuation_prefix,
            );
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
            builder.push_text(html, style.fg(Color::DarkGray));
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
    let link_style = style.fg(Color::Cyan).add_modifier(Modifier::UNDERLINED);
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
