use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use xoxo_core::bus::{BusPayload, TurnEvent};
use xoxo_core::chat::structs::{ChatTextRole, ChatToolCallId, ToolCallEvent, ToolCallStarted};

use crate::app::{App, HistoryEntry};
use crate::tool_format;
use crate::ui::markdown::render_markdown_message;
use crate::ui::tool_lines::tool_outcome;
use crate::ui::{prefixed_plain_line, prefixed_styled_line};

pub(super) struct ConversationLines {
    pub(super) lines: Vec<Line<'static>>,
    pub(super) turn_finished_positions: Vec<usize>,
}

pub(super) fn build_conversation_lines(
    app: &App,
    header_lines: Vec<Line<'static>>,
) -> ConversationLines {
    let mut lines = header_lines;

    lines.push(Line::from(""));
    lines.push(Line::from(""));

    let mut previous_entry: Option<&HistoryEntry> = None;
    let mut turn_finished_positions = Vec::new();

    for entry in &app.history {
        if should_prepend_spacing(previous_entry, entry) {
            lines.push(Line::from(""));
        }

        if is_markdown_assistant_message(entry) {
            if let BusPayload::Message(message) = &entry.payload {
                lines.extend(render_markdown_message(&message.content));
            }
        } else {
            lines.extend(render_plain_payload(app, entry));
        }

        if let BusPayload::Turn(TurnEvent::Finished { .. }) = &entry.payload {
            turn_finished_positions.push(lines.len());
        }

        previous_entry = Some(entry);
    }

    if app.turn_in_progress {
        if previous_entry.is_some() {
            lines.push(Line::from(""));
        }
        lines.push(prefixed_styled_line("Doing...", doing_indicator_style(app)));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(""));
    lines.push(Line::from(""));

    ConversationLines {
        lines: collapse_blank_lines(lines),
        turn_finished_positions,
    }
}

fn is_markdown_assistant_message(entry: &HistoryEntry) -> bool {
    matches!(
        &entry.payload,
        BusPayload::Message(message) if message.role == ChatTextRole::Agent
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::Instant;

    use uuid::Uuid;
    use xoxo_core::bus::BusPayload;
    use xoxo_core::chat::structs::{
        ChatToolCallId, ToolCallCompleted, ToolCallEvent, ToolCallStarted,
    };

    use crate::app::LayoutMode;

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
