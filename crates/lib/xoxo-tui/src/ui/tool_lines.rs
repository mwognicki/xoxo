use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use xoxo_core::bus::BusPayload;
use xoxo_core::chat::structs::{ToolCallEvent, ToolCallStarted};

use crate::app::App;

pub(crate) fn default_tool_byline(app: &App, started: &ToolCallStarted) -> Line<'static> {
    tool_byline(app, started)
}

pub(crate) fn default_tool_result_lines(content: &str, is_error: bool) -> Vec<Line<'static>> {
    tool_result_lines(content, is_error)
}

pub(super) fn tool_outcome<'a>(
    app: &'a App,
    started: &ToolCallStarted,
) -> Option<&'a ToolCallEvent> {
    app.history.iter().find_map(|entry| match entry.payload.as_bus()? {
        BusPayload::ToolCall(event @ ToolCallEvent::Completed(completed))
            if completed.tool_call_id == started.tool_call_id =>
        {
            Some(event)
        }
        BusPayload::ToolCall(event @ ToolCallEvent::Failed(failed))
            if failed.tool_call_id == started.tool_call_id =>
        {
            Some(event)
        }
        _ => None,
    })
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

fn tool_dot_style(app: &App, started: &ToolCallStarted) -> Style {
    match tool_outcome(app, started) {
        Some(ToolCallEvent::Completed(_)) => Style::default().fg(Color::Indexed(70)),
        Some(ToolCallEvent::Failed(_)) => Style::default().fg(Color::Indexed(160)),
        _ => pulsing_tool_dot_style(app),
    }
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
        spans.push(Span::styled(arguments, Style::default().fg(Color::Gray)));
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
