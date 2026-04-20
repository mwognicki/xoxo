use tokio::sync::oneshot;

use crate::conversation_store::{ConversationRecord, MessageRole};
use crate::llm::ChatMessage;
use crate::types::{ConversationId, SpawnError, SpawnInput};

use super::{AgentSpawner, SpawnContext};

/// Resolve a resume spawn: load the stored conversation record and reconstruct history.
///
/// Returns a [`SpawnContext`] with history populated from the record and `is_resume = true`.
pub(super) async fn resolve_resume(
    spawner: &AgentSpawner,
    resume_id: String,
    input: SpawnInput,
    response_tx: Option<oneshot::Sender<String>>,
) -> Result<SpawnContext, SpawnError> {
    let store = spawner.conversation_store.as_ref().ok_or_else(|| {
        SpawnError::Internal("conversation store required to resume".into())
    })?;
    let record = store.get(&resume_id).await.map_err(|e| {
        SpawnError::Internal(format!("failed to load conversation {resume_id}: {e}"))
    })?;
    let history = build_history_from_record(&record);
    let id = ConversationId(record.id);

    Ok(SpawnContext {
        blueprint: record.blueprint,
        initial_history: history,
        conversation_id: id,
        is_resume: true,
        initial_prompt: input.initial_prompt,
        parent_path: input.parent_path,
        parent_call_stack: input.parent_call_stack,
        response_tx,
    })
}

/// Reconstruct a `Vec<ChatMessage>` from a stored [`ConversationRecord`].
///
/// Message mapping:
/// - `User` → `ChatMessage::User`
/// - `Agent` → `ChatMessage::Assistant`
/// - `ToolExchange` → `ChatMessage::ToolCall` + `ChatMessage::ToolResult` pair
///
/// Each `ToolExchange` is emitted as its own single-tool assistant turn so
/// that tool call IDs remain valid even when multiple tools were invoked in
/// a single LLM iteration. Messages that cannot be decoded are skipped with
/// a warning.
pub(super) fn build_history_from_record(record: &ConversationRecord) -> Vec<ChatMessage> {
    let mut history = Vec::with_capacity(record.messages.len() * 2);

    for msg in &record.messages {
        match &msg.role {
            MessageRole::User => {
                if let Some(text) = msg.content.as_str() {
                    history.push(ChatMessage::User { content: text.to_string() });
                } else {
                    log::warn!("spawner: skipping user message with non-string content");
                }
            }
            MessageRole::Agent => {
                if let Some(text) = msg.content.as_str() {
                    history.push(ChatMessage::Assistant { content: text.to_string() });
                } else {
                    log::warn!("spawner: skipping agent message with non-string content");
                }
            }
            MessageRole::ToolExchange => {
                let obj = match msg.content.as_object() {
                    Some(o) => o,
                    None => {
                        log::warn!("spawner: skipping tool exchange with non-object content");
                        continue;
                    }
                };
                let id = match obj.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        log::warn!("spawner: skipping tool exchange missing 'id' field");
                        continue;
                    }
                };
                let name = match obj.get("name").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        log::warn!("spawner: skipping tool exchange missing 'name' field");
                        continue;
                    }
                };
                let arguments = obj
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::Value::Object(Default::default()));

                let content = if let Some(err) = obj.get("error").and_then(|v| v.as_str()) {
                    err.to_string()
                } else {
                    obj.get("output")
                        .map(|v| v.to_string())
                        .unwrap_or_default()
                };

                history.push(ChatMessage::ToolCall { id: id.clone(), name, arguments });
                history.push(ChatMessage::ToolResult { id, content });
            }
        }
    }

    history
}
