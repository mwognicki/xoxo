use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use xoxo_core::bus::{BusPayload, TurnEvent};
use xoxo_core::chat::structs::{ChatTextRole, ChatToolCallId, ToolCallEvent, ToolCallStarted};
use crate::syntax_highlighter::highlight_syntax;

use crate::app::{App, CachedConversation, HistoryEntry, HistoryPayload};
use crate::tool_format;
use crate::ui::markdown::render_markdown_message;
use crate::ui::tool_lines::tool_outcome;
use crate::ui::{prefixed_plain_line, prefixed_styled_line};

pub(super) struct ConversationLines {
    pub(super) lines: Vec<Line<'static>>,
    pub(super) turn_finished_positions: Vec<usize>,
}

/// Build — or return a cached copy of — the conversation pane lines.
///
/// Rebuilding runs markdown parsing and syntax highlighting over the entire
/// transcript, so caching pays off any time the frame is redrawn without a
/// meaningful state change (e.g. per-keystroke redraws, idle refresh ticks).
pub(super) fn build_conversation_lines(
    app: &App,
    header_lines: Vec<Line<'static>>,
) -> ConversationLines {
    let key = app.conversation_cache_key();
    if let Some(cached) = app.cached_conversation.borrow().as_ref()
        && cached.key == key
    {
        return ConversationLines {
            lines: cached.lines.clone(),
            turn_finished_positions: cached.turn_finished_positions.clone(),
        };
    }

    let built = build_conversation_lines_uncached(app, header_lines);

    *app.cached_conversation.borrow_mut() = Some(CachedConversation {
        key,
        lines: built.lines.clone(),
        turn_finished_positions: built.turn_finished_positions.clone(),
    });

    built
}

fn build_conversation_lines_uncached(
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
            if let HistoryPayload::Bus(BusPayload::Message(message)) = &entry.payload {
                lines.extend(render_markdown_message(&message.content, highlight_syntax));
            }
        } else {
            lines.extend(render_plain_payload(app, entry));
        }

        if let HistoryPayload::Bus(BusPayload::Turn(TurnEvent::Finished { .. })) = &entry.payload {
            turn_finished_positions.push(lines.len());
        }

        previous_entry = Some(entry);
    }

    let in_flight_thinking = app
        .active_chat_id
        .as_ref()
        .and_then(|chat_id| app.in_flight_thinking.get(chat_id))
        .filter(|buffer| !buffer.is_empty());
    let in_flight_text = app
        .active_chat_id
        .as_ref()
        .and_then(|chat_id| app.in_flight_text.get(chat_id))
        .filter(|buffer| !buffer.is_empty());

    if let Some(buffer) = in_flight_thinking {
        if previous_entry.is_some() {
            lines.push(Line::from(""));
        }
        lines.extend(render_thinking_lines(buffer));
    }

    if let Some(buffer) = in_flight_text {
        if previous_entry.is_some() || in_flight_thinking.is_some() {
            lines.push(Line::from(""));
        }
        lines.extend(render_markdown_message(buffer, highlight_syntax));
    }

    if app.turn_in_progress {
        if previous_entry.is_some()
            || in_flight_thinking.is_some()
            || in_flight_text.is_some()
        {
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
        HistoryPayload::Bus(BusPayload::Message(message)) if message.role == ChatTextRole::Agent
    )
}

fn doing_indicator_style(app: &App) -> Style {
    let phase = (app.started_at.elapsed().as_millis() / 200) % 6;
    let color = match phase {
        0 | 5 => Color::Indexed(166),
        1 | 4 => Color::Indexed(172),
        2 | 3 => Color::Indexed(167),
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
        HistoryPayload::Bus(BusPayload::ToolCall(ToolCallEvent::Completed(_)))
            | HistoryPayload::Bus(BusPayload::ToolCall(ToolCallEvent::Failed(_)))
    )
}

fn should_prepend_spacing(previous_entry: Option<&HistoryEntry>, entry: &HistoryEntry) -> bool {
    previous_entry.is_some() && !is_tool_result_entry(entry)
}

fn has_matching_tool_start(app: &App, tool_call_id: &ChatToolCallId) -> bool {
    app.history.iter().any(|entry| {
        matches!(
            &entry.payload,
            HistoryPayload::Bus(BusPayload::ToolCall(ToolCallEvent::Started(started)))
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

fn render_thinking_lines(content: &str) -> Vec<Line<'static>> {
    let border_style = Style::default().fg(Color::DarkGray);
    let content_style = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::ITALIC);
    content
        .lines()
        .map(|content_line| {
            let border_span = Span::styled("┋", border_style);
            let content_span = Span::styled(content_line.to_string(), content_style);
            Line::from(vec![border_span, Span::raw(" "), content_span])
        })
        .collect()
}

fn render_plain_payload(app: &App, entry: &HistoryEntry) -> Vec<Line<'static>> {
    match &entry.payload {
        HistoryPayload::Thinking(content) => render_thinking_lines(content),
        HistoryPayload::Bus(BusPayload::Message(message)) => message
            .content
            .lines()
            .map(|content_line| {
                let border_span = Span::styled("┋", Style::default().fg(Color::Indexed(167)));
                let content_span = Span::styled(
                    content_line.to_string(),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                );
                Line::from(vec![border_span, Span::raw(" "), content_span])
            })
            .collect(),
        HistoryPayload::Bus(BusPayload::ToolCall(ToolCallEvent::Started(started))) => {
            let mut lines = tool_format::format_started(app, started);
            lines.extend(render_tool_outcome_lines(app, started));
            lines
        }
        HistoryPayload::Bus(BusPayload::ToolCall(ToolCallEvent::Completed(completed))) => {
            if has_matching_tool_start(app, &completed.tool_call_id) {
                Vec::new()
            } else {
                tool_format::format_completed(app, completed)
            }
        }
        HistoryPayload::Bus(BusPayload::ToolCall(ToolCallEvent::Failed(failed))) => {
            if has_matching_tool_start(app, &failed.tool_call_id) {
                Vec::new()
            } else {
                tool_format::format_failed(app, failed)
            }
        }
        HistoryPayload::Bus(BusPayload::TextDelta { .. })
        | HistoryPayload::Bus(BusPayload::ThinkingDelta { .. }) => Vec::new(),
        HistoryPayload::Bus(BusPayload::Turn(_)) => Vec::new(),
        HistoryPayload::Bus(BusPayload::AgentShutdown) => vec![prefixed_plain_line("shutdown")],
        HistoryPayload::Bus(BusPayload::Error(error)) => {
            vec![prefixed_plain_line(format!("error: {}", error.message))]
        }
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

    use std::cell::RefCell;
    use std::time::Instant;

    use uuid::Uuid;
    use xoxo_core::bus::BusPayload;
    use xoxo_core::chat::structs::{
        ChatTextMessage, ChatTextRole, ChatToolCallId, ToolCallCompleted, ToolCallEvent,
        ToolCallStarted,
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
            in_flight_text: std::collections::HashMap::new(),
            in_flight_thinking: std::collections::HashMap::new(),
            conversation_scroll_from_bottom: 0,
            modal: None,
            mention_popup: None,
            workspace_root: std::path::PathBuf::from("."),
            ctrl_c_count: 0,
            started_at: Instant::now(),
            turn_in_progress: false,
            last_turn_finish_reason: None,
            mouse_capture_enabled: true,
            storage: None,
            conversation_version: 0,
            cached_conversation: RefCell::new(None),
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
                payload: HistoryPayload::Bus(BusPayload::ToolCall(ToolCallEvent::Started(
                    started.clone(),
                ))),
            },
            HistoryEntry {
                chat_id: Uuid::new_v4(),
                payload: HistoryPayload::Bus(BusPayload::ToolCall(ToolCallEvent::Completed(
                    completed,
                ))),
            },
        ]);

        let started_lines = render_plain_payload(
            &app,
            &HistoryEntry {
                chat_id: Uuid::new_v4(),
                payload: HistoryPayload::Bus(BusPayload::ToolCall(ToolCallEvent::Started(started))),
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
                payload: HistoryPayload::Bus(BusPayload::ToolCall(ToolCallEvent::Started(
                    ToolCallStarted {
                        tool_call_id: tool_call_id.clone(),
                        tool_name: "read_file".to_string(),
                        arguments: serde_json::json!({}),
                        tool_call_kind: xoxo_core::chat::structs::ToolCallKind::Generic,
                    },
                ))),
            },
            HistoryEntry {
                chat_id: Uuid::new_v4(),
                payload: HistoryPayload::Bus(BusPayload::ToolCall(ToolCallEvent::Completed(
                    ToolCallCompleted {
                        tool_call_id: tool_call_id.clone(),
                        tool_name: "read_file".to_string(),
                        result_preview: "done".to_string(),
                    },
                ))),
            },
        ]);
        let completed_entry = HistoryEntry {
            chat_id: Uuid::new_v4(),
            payload: HistoryPayload::Bus(BusPayload::ToolCall(ToolCallEvent::Completed(
                ToolCallCompleted {
                    tool_call_id,
                    tool_name: "read_file".to_string(),
                    result_preview: "done".to_string(),
                },
            ))),
        };

        assert!(render_plain_payload(&app, &completed_entry).is_empty());
    }

    #[test]
    fn assistant_code_blocks_use_core_syntax_highlighting() {
        let app = test_app_with_history(vec![HistoryEntry {
            chat_id: Uuid::new_v4(),
            payload: HistoryPayload::Bus(BusPayload::Message(ChatTextMessage {
                role: ChatTextRole::Agent,
                content: "```rs\nfn main() {}\n```".to_string(),
            })),
        }]);

        let conversation = build_conversation_lines(&app, Vec::new());
        let function_line = conversation
            .lines
            .iter()
            .find(|line| line.spans.iter().any(|span| span.content.contains("fn")))
            .expect("highlighted function line");

        assert!(
            function_line
                .spans
                .iter()
                .any(|span| span.content.contains("fn") && span.style.fg.is_some())
        );
    }
}
