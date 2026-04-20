use tokio::sync::oneshot;

use crate::agent::blueprint_store::{StoreError, BUILTIN_ROOT_ID};
use crate::helpers::new_id;
use crate::types::{AgentId, ConversationId, SpawnError, SpawnInput};

use super::{AgentSpawner, SpawnContext};

/// Resolve a fresh spawn: look up the blueprint and generate a new conversation ID.
///
/// Returns a [`SpawnContext`] with empty history and `is_resume = false`.
pub(super) async fn resolve_new(
    spawner: &AgentSpawner,
    input: SpawnInput,
    response_tx: Option<oneshot::Sender<String>>,
) -> Result<SpawnContext, SpawnError> {
    let blueprint = if let Some(inline) = input.inline_blueprint {
        inline
    } else {
        let blueprint_id = input.blueprint_id.as_deref().unwrap_or(BUILTIN_ROOT_ID);
        match spawner.store.load(&AgentId(blueprint_id.to_string())).await {
            Ok(b) => b,
            Err(StoreError::NotFound(_)) => {
                return Err(SpawnError::BlueprintNotFound(blueprint_id.to_string()));
            }
            Err(StoreError::Backend(e)) => {
                return Err(SpawnError::Internal(e));
            }
        }
    };

    Ok(SpawnContext {
        blueprint,
        initial_history: Vec::new(),
        conversation_id: ConversationId(new_id()),
        is_resume: false,
        initial_prompt: input.initial_prompt,
        parent_path: input.parent_path,
        parent_call_stack: input.parent_call_stack,
        response_tx,
    })
}
