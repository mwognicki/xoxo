use serde::{Deserialize, Serialize};

use super::structs::{AppEvent, Chat, ChatEventBody, ChatTextRole, MessageContextState, ToolCallCompleted, ToolCallEvent, ToolCallFailed, ToolCallStarted};

/// User-facing projection of a chat without historical internals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserFacingChat {
    pub current_model_name: String,
    pub total_used_tokens: u64,
    pub parent_branch: Option<UserFacingParentBranch>,
    pub compaction: Option<UserFacingCompaction>,
    pub messages: Vec<UserFacingMessage>,
    pub tool_calls: Vec<UserFacingToolCall>,
}

/// Minimal identification for the parent branch of the current view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserFacingParentBranch {
    pub branch_id: String,
    pub branch_name: String,
    pub forked_from_message_id: Option<String>,
}

/// User-visible note that earlier history was compacted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserFacingCompaction {
    pub snapshot_id: String,
}

/// User-facing chat message from one of the two visible parties.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserFacingMessage {
    pub id: String,
    pub role: UserFacingMessageRole,
    pub content: String,
}

/// Visible participant role in the user-facing transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserFacingMessageRole {
    Agent,
    User,
}

/// User-facing tool call information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum UserFacingToolCall {
    Started {
        tool_call_id: String,
        tool_name: String,
        arguments: serde_json::Value,
    },
    Completed {
        tool_call_id: String,
        tool_name: String,
        result_preview: String,
    },
    Failed {
        tool_call_id: String,
        tool_name: String,
        message: String,
    },
}

/// Map a full persisted chat into the user-facing projection used by the app.
///
/// Only the active branch is shown. Compacted-away history and internal app
/// events are not exposed directly, except for a small compaction/parent-branch
/// summary and the derived current model name.
pub fn to_user_facing_chat(chat: &Chat) -> UserFacingChat {
    let active_branch = chat
        .branches
        .iter()
        .find(|branch| branch.id == chat.active_branch_id);

    let current_model_name = derive_current_model_name(chat);
    let total_used_tokens = chat
        .observability
        .as_ref()
        .map(|observability| observability.usage.total_tokens)
        .unwrap_or_else(|| {
            chat.snapshots
                .iter()
                .filter_map(|snapshot| snapshot.observability.as_ref())
                .map(|observability| observability.usage.total_tokens)
                .sum::<u64>()
                + chat
                    .events
                    .iter()
                    .filter_map(|entry| entry.event.observability.as_ref())
                    .map(|observability| observability.usage.total_tokens)
                    .sum::<u64>()
        });

    let parent_branch = active_branch.and_then(|branch| {
        let parent_id = branch.parent_branch_id.as_ref()?;
        let parent_branch = chat.branches.iter().find(|candidate| &candidate.id == parent_id)?;

        Some(UserFacingParentBranch {
            branch_id: parent_branch.id.0.clone(),
            branch_name: parent_branch.name.clone(),
            forked_from_message_id: branch
                .forked_from_message_id
                .as_ref()
                .map(|message_id| message_id.0.clone()),
        })
    });

    let compaction = active_branch
        .and_then(|branch| branch.active_snapshot_id.as_ref())
        .map(|snapshot_id| UserFacingCompaction {
            snapshot_id: snapshot_id.0.clone(),
        });

    let active_branch_id = &chat.active_branch_id;
    let active_entries: Vec<_> = chat
        .events
        .iter()
        .filter(|entry| {
            entry.context_state == MessageContextState::Active
                && entry.event.branch_id == *active_branch_id
        })
        .collect();

    let messages = active_entries
        .iter()
        .filter_map(|entry| match &entry.event.body {
            ChatEventBody::Message(message) => match message.role {
                ChatTextRole::Agent => Some(UserFacingMessage {
                    id: entry.event.id.0.clone(),
                    role: UserFacingMessageRole::Agent,
                    content: message.content.clone(),
                }),
                ChatTextRole::User => Some(UserFacingMessage {
                    id: entry.event.id.0.clone(),
                    role: UserFacingMessageRole::User,
                    content: message.content.clone(),
                }),
                ChatTextRole::System => None,
            },
            ChatEventBody::ToolCall(_) | ChatEventBody::AppEvent(_) => None,
        })
        .collect();

    let tool_calls = active_entries
        .iter()
        .filter_map(|entry| match &entry.event.body {
            ChatEventBody::ToolCall(tool_call) => Some(map_tool_call(tool_call)),
            ChatEventBody::Message(_) | ChatEventBody::AppEvent(_) => None,
        })
        .collect();

    UserFacingChat {
        current_model_name,
        total_used_tokens,
        parent_branch,
        compaction,
        messages,
        tool_calls,
    }
}

fn derive_current_model_name(chat: &Chat) -> String {
    let active_branch_id = &chat.active_branch_id;

    chat.events
        .iter()
        .filter(|entry| {
            entry.context_state == MessageContextState::Active
                && entry.event.branch_id == *active_branch_id
        })
        .filter_map(|entry| match &entry.event.body {
            ChatEventBody::AppEvent(AppEvent::ModelChanged { to, .. }) => {
                Some(to.model_name.clone())
            }
            ChatEventBody::Message(_) | ChatEventBody::ToolCall(_) => None,
        })
        .last()
        .unwrap_or_else(|| chat.agent.model.model_name.clone())
}

fn map_tool_call(tool_call: &ToolCallEvent) -> UserFacingToolCall {
    match tool_call {
        ToolCallEvent::Started(ToolCallStarted {
            tool_call_id,
            tool_name,
            arguments,
            ..
        }) => UserFacingToolCall::Started {
            tool_call_id: tool_call_id.0.clone(),
            tool_name: tool_name.clone(),
            arguments: arguments.clone(),
        },
        ToolCallEvent::Completed(ToolCallCompleted {
            tool_call_id,
            tool_name,
            result_preview,
        }) => UserFacingToolCall::Completed {
            tool_call_id: tool_call_id.0.clone(),
            tool_name: tool_name.clone(),
            result_preview: result_preview.clone(),
        },
        ToolCallEvent::Failed(ToolCallFailed {
            tool_call_id,
            tool_name,
            message,
        }) => UserFacingToolCall::Failed {
            tool_call_id: tool_call_id.0.clone(),
            tool_name: tool_name.clone(),
            message: message.clone(),
        },
    }
}

#[cfg(test)]
#[path = "user_facing_tests.rs"]
mod tests;
