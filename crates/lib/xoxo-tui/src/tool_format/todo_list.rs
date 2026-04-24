use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use xoxo_core::chat::structs::ToolCallCompleted;

use crate::app::App;

use super::{DefaultToolFormatter, ToolFormatter, subtle_style};

#[derive(Debug)]
struct WriteTodoListPreview {
    action: String,
    tasks: Vec<WriteTodoListTaskPreview>,
}

#[derive(Debug)]
struct WriteTodoListTaskPreview {
    content: String,
    priority: String,
    state: String,
}

pub(super) struct WriteTodoListFormatter;

impl ToolFormatter for WriteTodoListFormatter {
    fn format_completed(
        &self,
        app: &App,
        completed: &ToolCallCompleted,
        viewport_width: u16,
    ) -> Vec<Line<'static>> {
        let Some(preview) = parse_write_todo_list_preview(&completed.result_preview) else {
            return DefaultToolFormatter.format_completed(app, completed, viewport_width);
        };

        render_write_todo_list_preview(&preview)
    }
}

fn parse_write_todo_list_preview(preview: &str) -> Option<WriteTodoListPreview> {
    let value: serde_json::Value = serde_json::from_str(preview).ok()?;
    let kind = value.get("kind")?.as_str()?;
    if kind != "write_todo_list_preview" {
        return None;
    }

    let tasks = value.get("tasks")?.as_array()?;
    let mut parsed_tasks = Vec::with_capacity(tasks.len());

    for task in tasks {
        parsed_tasks.push(WriteTodoListTaskPreview {
            content: task.get("content")?.as_str()?.to_string(),
            priority: task.get("priority")?.as_str()?.to_string(),
            state: task.get("state")?.as_str()?.to_string(),
        });
    }

    Some(WriteTodoListPreview {
        action: value.get("action")?.as_str()?.to_string(),
        tasks: parsed_tasks,
    })
}

fn render_write_todo_list_preview(preview: &WriteTodoListPreview) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(preview.tasks.len().max(1) + 1);
    let header_style = subtle_style();

    lines.push(Line::from(Span::styled(
        format!("└ {} todo list", preview.action),
        header_style,
    )));

    if preview.tasks.is_empty() {
        lines.push(Line::from(Span::styled("  (empty)", header_style)));
        return lines;
    }

    for task in &preview.tasks {
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("{} ", todo_state_symbol(&task.state)),
                todo_state_style(&task.state),
            ),
            Span::styled(
                format!("[{}] ", task.priority),
                todo_priority_style(&task.priority),
            ),
            Span::styled(task.content.clone(), todo_state_style(&task.state)),
        ]));
    }

    lines
}

fn todo_state_symbol(state: &str) -> &'static str {
    match state {
        "completed" => "▣",
        "in_progress" => "◧",
        "cancelled" => "⊠",
        _ => "□",
    }
}

fn todo_state_style(state: &str) -> Style {
    match state {
        "completed" => Style::default().fg(Color::Indexed(113)),
        "in_progress" => Style::default().fg(Color::Indexed(220)),
        "cancelled" => Style::default().fg(Color::Indexed(245)),
        _ => Style::default().fg(Color::Indexed(250)),
    }
}

fn todo_priority_style(priority: &str) -> Style {
    match priority {
        "high" => Style::default().fg(Color::Indexed(203)),
        "medium" => Style::default().fg(Color::Indexed(180)),
        "low" => Style::default().fg(Color::Indexed(110)),
        _ => subtle_style(),
    }
}
