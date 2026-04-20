mod new;
mod resume;
mod wiring;

use std::sync::{Arc, Weak};

use tokio::sync::oneshot;

use crate::agent::blueprint_store::BlueprintStore;
use crate::conversation_store::ConversationStore;
use crate::llm::{ChatMessage, LlmClient};
use crate::memory_store::MemoryStore;
use crate::nats::NatsPublisher;
use crate::overrides::TextOverrides;
use crate::skills::store::SkillStore;
use crate::tool::registry::ToolRegistry;
use crate::types::{
    AgentBlueprint, CallStack, ConversationId, ConversationPath, SpawnError, SpawnInput, Spawner,
};

use super::HandleRegistry;

/// Inbound channel buffer — number of messages that can queue per agent
/// before the sender blocks. Agents process one turn at a time, so a small
/// buffer is sufficient.
pub(super) const CHANNEL_BUFFER: usize = 16;

/// Central spawning authority for all agents — top-level and subagents.
///
/// Both entry points call through here:
/// - **NATS path**: [`NatsListener`] subscribes to [`FRAMEWORK_SPAWN`],
///   receives a `SpawnRequest`, and calls [`Spawner::spawn`].
/// - **Tool path**: the `spawn_subagent` tool receives an `Arc<dyn Spawner>`
///   via [`ToolContext`] and calls [`Spawner::spawn`] directly; the result is
///   returned as a normal tool value with no NATS involvement.
///
/// In both cases, `spawn()` creates a [`NatsAgentHandle`] wired to an
/// internal channel, and starts an [`AgentRunner`] Tokio task that owns the
/// LLM completion loop for that conversation.
///
/// [`NatsListener`]: crate::nats
/// [`FRAMEWORK_SPAWN`]: crate::nats::FRAMEWORK_SPAWN
/// [`ToolContext`]: crate::types::ToolContext
/// [`NatsAgentHandle`]: super::NatsAgentHandle
/// [`AgentRunner`]: crate::agent::runner::AgentRunner
pub struct AgentSpawner {
    publisher: Arc<NatsPublisher>,
    registry: Arc<HandleRegistry>,
    store: Arc<dyn BlueprintStore>,
    llm: Option<Arc<LlmClient>>,
    tool_registry: Arc<ToolRegistry>,
    conversation_store: Option<Arc<dyn ConversationStore>>,
    memory_store: Option<Arc<dyn MemoryStore>>,
    skill_store: Option<Arc<dyn SkillStore>>,
    /// Runtime text overrides for tool descriptions, parameter descriptions,
    /// and system prompts. Applied in `wiring.rs` at spawn time.
    pub(super) overrides: Arc<TextOverrides>,
    /// Weak self-reference used to pass `Arc<dyn Spawner>` to each runner's
    /// [`ToolContext`] without creating an ownership cycle.
    weak_self: Weak<AgentSpawner>,
}

/// Resolved context passed from `resolve_new` / `resolve_resume` into `do_wire`.
///
/// Carries everything needed to build the handle, start the runner, and
/// deliver the first message, after the new/resume divergence is settled.
struct SpawnContext {
    blueprint: AgentBlueprint,
    initial_history: Vec<ChatMessage>,
    conversation_id: ConversationId,
    is_resume: bool,
    initial_prompt: String,
    parent_path: Option<ConversationPath>,
    parent_call_stack: Option<CallStack>,
    response_tx: Option<oneshot::Sender<String>>,
}

impl AgentSpawner {
    pub fn new(
        publisher: Arc<NatsPublisher>,
        registry: Arc<HandleRegistry>,
        store: Arc<dyn BlueprintStore>,
        llm: Option<Arc<LlmClient>>,
        tool_registry: Arc<ToolRegistry>,
        conversation_store: Option<Arc<dyn ConversationStore>>,
        memory_store: Option<Arc<dyn MemoryStore>>,
        skill_store: Option<Arc<dyn SkillStore>>,
        overrides: Arc<TextOverrides>,
    ) -> Arc<Self> {
        Arc::new_cyclic(|weak| Self {
            publisher,
            registry,
            store,
            llm,
            tool_registry,
            conversation_store,
            memory_store,
            skill_store,
            overrides,
            weak_self: weak.clone(),
        })
    }

    /// Core spawn logic shared by both `spawn` and `spawn_and_await`.
    ///
    /// Delegates to `resolve_new` or `resolve_resume` to produce a
    /// [`SpawnContext`], then calls `do_wire` to build and start the agent.
    async fn do_spawn(
        &self,
        input: SpawnInput,
        response_tx: Option<oneshot::Sender<String>>,
    ) -> Result<ConversationId, SpawnError> {
        let ctx = if let Some(resume_id) = input.resume_conversation_id.clone() {
            resume::resolve_resume(self, resume_id, input, response_tx).await?
        } else {
            new::resolve_new(self, input, response_tx).await?
        };
        wiring::do_wire(self, ctx).await
    }
}

impl Spawner for AgentSpawner {
    fn spawn<'a>(
        &'a self,
        input: SpawnInput,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ConversationId, SpawnError>> + Send + 'a>> {
        Box::pin(async move { self.do_spawn(input, None).await })
    }

    fn spawn_and_await<'a>(
        &'a self,
        input: SpawnInput,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(ConversationId, String), SpawnError>> + Send + 'a>> {
        Box::pin(async move {
            let (tx, rx) = oneshot::channel::<String>();
            let conversation_id = self.do_spawn(input, Some(tx)).await?;
            let response = rx.await.map_err(|_| {
                SpawnError::Internal("child agent closed before producing a response".into())
            })?;
            Ok((conversation_id, response))
        })
    }
}
