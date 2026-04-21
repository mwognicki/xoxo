use std::collections::HashSet;

use uuid::Uuid;
use xoxo_core::bus::BusPayload;
use xoxo_core::chat::structs::{
    Chat, ChatEventBody, ChatTextRole, MessageContextState,
};

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub chat_id: Uuid,
    pub payload: BusPayload,
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
                    payload: BusPayload::Message(message.clone()),
                })
            }
            ChatEventBody::ToolCall(tool_call) => Some(HistoryEntry {
                chat_id: chat.id,
                payload: BusPayload::ToolCall(tool_call.clone()),
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
        assert!(matches!(history[0].payload, BusPayload::ToolCall(_)));
    }
}
