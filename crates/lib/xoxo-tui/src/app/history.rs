use std::collections::HashSet;

use uuid::Uuid;
use xoxo_core::bus::BusPayload;
use xoxo_core::chat::structs::{
    Chat, ChatEventBody, ChatTextRole, MessageContextState,
};

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub chat_id: Uuid,
    pub payload: HistoryPayload,
}

/// TUI-local conversation entry payload.
///
/// Wraps [`BusPayload`] for canonical, bus-originated entries and adds a TUI-only
/// [`HistoryPayload::Thinking`] variant for completed-but-not-persisted reasoning text.
/// Thinking entries never round-trip through the bus or sled — they exist only for the
/// lifetime of the TUI session so the user keeps seeing reasoning after the turn ends.
#[derive(Debug, Clone)]
pub enum HistoryPayload {
    Bus(BusPayload),
    Thinking(String),
}

impl From<BusPayload> for HistoryPayload {
    fn from(payload: BusPayload) -> Self {
        Self::Bus(payload)
    }
}

impl HistoryPayload {
    pub fn as_bus(&self) -> Option<&BusPayload> {
        match self {
            Self::Bus(payload) => Some(payload),
            Self::Thinking(_) => None,
        }
    }
}

pub(super) fn history_from_chat(chat: &Chat) -> Vec<HistoryEntry> {
    let tool_event_ids: HashSet<_> = chat
        .events
        .iter()
        .filter(|entry| {
            entry.context_state == MessageContextState::Active
                && entry.event.branch_id == chat.active_branch_id
        })
        .filter_map(|entry| match entry.event.body {
            ChatEventBody::ToolCall(_) => Some(entry.event.id.clone()),
            ChatEventBody::Message(_) | ChatEventBody::AppEvent(_) => None,
        })
        .collect();

    chat.events
        .iter()
        .filter(|entry| {
            entry.context_state == MessageContextState::Active
                && entry.event.branch_id == chat.active_branch_id
        })
        .filter_map(|entry| match &entry.event.body {
            ChatEventBody::Message(message)
                if message.role == ChatTextRole::User
                    && entry
                        .event
                        .parent_id
                        .as_ref()
                        .is_some_and(|parent_id| tool_event_ids.contains(parent_id)) =>
            {
                None
            }
            ChatEventBody::Message(message) if message.role != ChatTextRole::System => {
                Some(HistoryEntry {
                    chat_id: chat.id,
                    payload: HistoryPayload::Bus(BusPayload::Message(message.clone())),
                })
            }
            ChatEventBody::ToolCall(tool_call) => Some(HistoryEntry {
                chat_id: chat.id,
                payload: HistoryPayload::Bus(BusPayload::ToolCall(tool_call.clone())),
            }),
            ChatEventBody::Message(_) | ChatEventBody::AppEvent(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use xoxo_core::chat::structs::{
        ApiCompatibility, ApiProvider, BranchId, ChatAgent, ChatBranch, ChatEvent, ChatLogEntry,
        ChatTextMessage, ChatToolCallId, MessageId, ModelConfig, ToolCallCompleted,
        ToolCallEvent,
    };

    #[test]
    fn restored_history_hides_synthetic_tool_result_user_message() {
        let tool_event_id = MessageId("tool-event".to_string());
        let synthetic_message_id = MessageId("synthetic-user-message".to_string());
        let branch_id = BranchId("main".to_string());
        let chat = Chat {
            title: None,
            id: Uuid::new_v4(),
            created_at: Some("2026-04-01T00:00:00Z".to_string()),
            updated_at: Some("2026-04-01T00:00:00Z".to_string()),
            parent_chat_id: None,
            spawned_by_tool_call_id: None,
            path: "root".to_string(),
            agent: ChatAgent {
                id: None,
                name: None,
                model: ModelConfig {
                    model_name: "gpt-4o".to_string(),
                    provider: ApiProvider {
                        name: "openai".to_string(),
                        compatibility: ApiCompatibility::OpenAi,
                    },
                },
                base_prompt: "You are helpful.".to_string(),
                allowed_tools: vec!["read_file".to_string()],
                allowed_skills: Vec::new(),
            },
            observability: None,
            active_branch_id: branch_id.clone(),
            branches: vec![ChatBranch {
                id: branch_id.clone(),
                name: "main".to_string(),
                parent_branch_id: None,
                forked_from_message_id: None,
                head_message_id: Some(synthetic_message_id.clone()),
                active_snapshot_id: None,
            }],
            snapshots: Vec::new(),
            events: vec![
                ChatLogEntry {
                    event: ChatEvent {
                        id: tool_event_id.clone(),
                        parent_id: None,
                        branch_id: branch_id.clone(),
                        body: ChatEventBody::ToolCall(ToolCallEvent::Completed(
                            ToolCallCompleted {
                                tool_call_id: ChatToolCallId("tool-call-1".to_string()),
                                tool_name: "read_file".to_string(),
                                result_preview: "file contents".to_string(),
                            },
                        )),
                        observability: None,
                    },
                    context_state: MessageContextState::Active,
                },
                ChatLogEntry {
                    event: ChatEvent {
                        id: synthetic_message_id,
                        parent_id: Some(tool_event_id),
                        branch_id,
                        body: ChatEventBody::Message(ChatTextMessage {
                            role: ChatTextRole::User,
                            content: "read_file: {\"content\":\"file contents\"}".to_string(),
                        }),
                        observability: None,
                    },
                    context_state: MessageContextState::Active,
                },
            ],
        };

        let history = history_from_chat(&chat);

        assert_eq!(history.len(), 1);
        assert!(matches!(
            history[0].payload,
            HistoryPayload::Bus(BusPayload::ToolCall(_))
        ));
    }
}
