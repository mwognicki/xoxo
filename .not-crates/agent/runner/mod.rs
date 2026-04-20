use std::sync::Arc;

use mongodb::bson::DateTime;
use tokio::sync::{mpsc, oneshot};

use crate::agent::execution_context::AgentExecutionContext;
use crate::conversation_store::{ConversationStore, MessageRole, StoredMessage};
use crate::memory_store::MemoryStore;
use crate::overrides::TextOverrides;
use crate::skills::store::SkillStore;
use crate::helpers::new_id;
use crate::llm::{ChatMessage, LlmClient};
use crate::nats::{NatsPublisher, conversation_out};
use crate::tool::ToolSet;
use crate::types::{
    AgentBlueprint, AgentId, CallStack, ConversationId, ConversationPath, ModelPolicy, SpawnInput,
    Spawner, ToolContext,
};
use crate::wire::{AgentDone, ConversationTitleSet, Envelope, ErrorPayload, Message, TRANSIENT_PREFIX};

/// System prompt given to the ephemeral title-generator agent.
const TITLE_SYSTEM_PROMPT: &str = "\
You are a conversation title generator. \
Given the user's first message, produce a concise title (3–7 words) that describes the topic. \
Respond with the title only — no punctuation at the end, no surrounding quotes, no explanation.";

/// Models randomly chosen for title generation (lightweight, free-tier models).
const TITLE_MODELS: [&str; 2] = [
    "stepfun/step-3.5-flash:free",
    "nvidia/nemotron-3-super-120b-a12b:free",
];

/// The agent's message-processing loop.
///
/// Owned by a dedicated Tokio task, one per spawned agent. Receives user
/// messages from [`NatsAgentHandle`] via an internal channel, calls the LLM,
/// and publishes the response to the conversation's outbound NATS subject.
///
/// Both spawn paths (NATS `SpawnRequest` and `spawn_subagent` tool) produce
/// an `AgentRunner` — the runner itself is unaware of how it was started.
///
/// [`NatsAgentHandle`]: crate::agent::handle::NatsAgentHandle
pub struct AgentRunner {
    rx: mpsc::Receiver<String>,
    llm: Option<Arc<LlmClient>>,
    blueprint: AgentBlueprint,
    path: ConversationPath,
    conversation_id: ConversationId,
    agent_id: AgentId,
    call_stack: CallStack,
    publisher: Arc<NatsPublisher>,
    tool_set: ToolSet,
    spawner: Arc<dyn Spawner>,
    execution_context: Option<Arc<AgentExecutionContext>>,
    conversation_store: Option<Arc<dyn ConversationStore>>,
    memory_store: Option<Arc<dyn MemoryStore>>,
    skill_store: Option<Arc<dyn SkillStore>>,
    history: Vec<ChatMessage>,
    /// If `Some`, the first response is forwarded here so `spawn_and_await`
    /// callers can receive it without subscribing to NATS.
    response_tx: Option<oneshot::Sender<String>>,
    /// `true` while we still need to generate a title for this conversation.
    ///
    /// Set to `false` on construction for resumed conversations (history
    /// non-empty) and cleared once a non-transient message is processed.
    needs_title: bool,
    /// Runtime text overrides for inline-agent prompts (title generator, etc.).
    overrides: Arc<TextOverrides>,
}

impl AgentRunner {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rx: mpsc::Receiver<String>,
        llm: Option<Arc<LlmClient>>,
        blueprint: AgentBlueprint,
        path: ConversationPath,
        conversation_id: ConversationId,
        agent_id: AgentId,
        call_stack: CallStack,
        publisher: Arc<NatsPublisher>,
        tool_set: ToolSet,
        spawner: Arc<dyn Spawner>,
        execution_context: Option<Arc<AgentExecutionContext>>,
        conversation_store: Option<Arc<dyn ConversationStore>>,
        memory_store: Option<Arc<dyn MemoryStore>>,
        skill_store: Option<Arc<dyn SkillStore>>,
        initial_history: Vec<ChatMessage>,
        response_tx: Option<oneshot::Sender<String>>,
        overrides: Arc<TextOverrides>,
    ) -> Self {
        // Title generation is only relevant for top-level, user-facing conversations:
        // - inline blueprints (e.g. title generator) must never generate a title — recurse guard.
        // - subagents (path depth > 1) are internal and don't need UI titles.
        let is_inline = blueprint.id.0.starts_with("inline:");
        let is_top_level = path.ids().len() == 1;
        let needs_title = initial_history.is_empty() && is_top_level && !is_inline;
        Self {
            rx,
            llm,
            blueprint,
            path,
            conversation_id,
            agent_id,
            call_stack,
            publisher,
            tool_set,
            spawner,
            execution_context,
            conversation_store,
            memory_store,
            skill_store,
            history: initial_history,
            response_tx,
            needs_title,
            overrides,
        }
    }

    /// Run the agent loop until the inbound channel is closed.
    ///
    /// For each received user message:
    /// 1. Appends it to the conversation history.
    /// 2. Resolves the model from the blueprint's [`ModelPolicy`].
    /// 3. Calls [`LlmClient::complete`] with the full history.
    /// 4. Publishes [`AgentDone`] (success) or [`ErrorPayload`] (failure) to
    ///    `conversation_out`.
    ///
    /// Exits cleanly when the channel sender is dropped (agent shutdown or
    /// parent handle removed from the registry). Shuts down the execution
    /// context (bash session + processes) on exit.
    pub async fn run(mut self) {
        let out_subject = conversation_out(&self.path);
        let conv_id = self.conversation_id.0.clone();
        log::info!("runner: started for agent {} (conversation {})", self.agent_id, self.path.current());

        while let Some(content) = self.rx.recv().await {
            log::debug!("runner {}: received message ({} chars)", self.agent_id, content.len());

            // Strip the transient prefix before the message reaches the LLM.
            // Transient messages are framework-injected and not typed by the user.
            let is_transient = content.starts_with(TRANSIENT_PREFIX);
            let llm_content = if is_transient {
                content[TRANSIENT_PREFIX.len()..].to_string()
            } else {
                content.clone()
            };

            // Capture the content for title generation before it is moved.
            // Only the first non-transient message triggers title generation.
            let title_content = if self.needs_title && !is_transient {
                self.needs_title = false;
                Some(llm_content.clone())
            } else {
                None
            };

            self.store_message(MessageRole::User, llm_content.clone()).await;
            self.history.push(ChatMessage::User { content: llm_content });

            let response = self.turn(&out_subject).await;

            match response {
                Ok(text) => {
                    self.store_message(MessageRole::Agent, text.clone()).await;
                    self.history.push(ChatMessage::Assistant { content: text.clone() });
                    if let Some(user_msg) = title_content {
                        spawn_title_task(
                            Arc::clone(&self.spawner),
                            Arc::clone(&self.publisher),
                            self.conversation_store.clone(),
                            out_subject.clone(),
                            conv_id.clone(),
                            user_msg,
                            Arc::clone(&self.overrides),
                        );
                    }
                    self.publish_done(&out_subject, text).await;
                }
                Err(e) => {
                    log::warn!("runner {}: turn error: {e}", self.agent_id);
                    self.publish_error(&out_subject, &e).await;
                    // Drop any waiting oneshot so the receiver gets an error
                    // rather than blocking indefinitely.
                    self.response_tx.take();
                }
            }
        }

        log::info!("runner: channel closed for agent {}, exiting", self.agent_id);

        if let Some(store) = &self.conversation_store {
            if let Err(e) = store.close(&conv_id, DateTime::now()).await {
                log::warn!("runner {}: failed to close conversation record: {e}", self.agent_id);
            }
        }

        if let Some(ctx) = &self.execution_context {
            ctx.shutdown().await;
            log::debug!("runner {}: execution context shut down", self.agent_id);
        }
    }

    async fn store_message(&self, role: MessageRole, content: String) {
        if let Some(store) = &self.conversation_store {
            let msg = StoredMessage {
                role,
                content: serde_json::Value::String(content),
                timestamp: DateTime::now(),
            };
            if let Err(e) = store.append_message(&self.conversation_id.0, msg).await {
                log::warn!("runner {}: failed to append message to store: {e}", self.agent_id);
            }
        }
    }

    /// Execute one LLM turn and return the text response.
    async fn turn(&self, _out_subject: &str) -> Result<String, String> {
        let llm = self.llm.as_ref().ok_or_else(|| {
            "LLM unavailable: OPENROUTER_API_KEY is not set".to_string()
        })?;

        let model = self.resolve_model()?;

        let ctx = ToolContext {
            call_id: new_id(),
            agent_id: self.agent_id.clone(),
            conversation_path: self.path.clone(),
            call_stack: self.call_stack.clone(),
            nats: Arc::clone(&self.publisher),
            execution_context: self.execution_context.clone(),
            spawner: Arc::clone(&self.spawner),
            memory_store: self.memory_store.clone(),
            skill_store: self.skill_store.clone(),
        };

        llm.complete(
            &model,
            &self.blueprint.model_policy,
            &self.blueprint.system_prompt,
            self.history.clone(),
            &self.tool_set,
            &ctx,
            self.conversation_store.clone(),
            &self.conversation_id.0,
        )
        .await
        .map_err(|e| e.to_string())
    }

    /// Resolve the concrete model string from the blueprint's policy.
    ///
    /// `AllowList` — use the first listed model.
    /// `BlockList` / `ToolDetermined` — not yet supported; returns an error.
    fn resolve_model(&self) -> Result<String, String> {
        match &self.blueprint.model_policy {
            ModelPolicy::AllowList(models) => models
                .first()
                .cloned()
                .ok_or_else(|| "blueprint allow list is empty — no model to use".to_string()),
            ModelPolicy::BlockList(_) => Err(
                "ModelPolicy::BlockList is not yet supported for model resolution".to_string(),
            ),
            ModelPolicy::ToolDetermined { tool_name } => Err(format!(
                "ModelPolicy::ToolDetermined (tool: {tool_name}) is not yet supported"
            )),
        }
    }

    async fn publish_done(&mut self, subject: &str, content: String) {
        // Notify a waiting spawn_and_await caller on the first response.
        if let Some(tx) = self.response_tx.take() {
            let _ = tx.send(content.clone());
        }
        let env = Envelope::new(Message::AgentDone(AgentDone {
            conversation_id: self.path.current().0.clone(),
            content,
        }));
        if let Ok(bytes) = env.encode() {
            self.publisher.publish(subject.to_string(), bytes).await.ok();
        }
    }

    async fn publish_error(&self, subject: &str, message: &str) {
        let env = Envelope::new(Message::Error(ErrorPayload {
            code: "agent_error".to_string(),
            message: message.to_string(),
        }));
        if let Ok(bytes) = env.encode() {
            self.publisher.publish(subject.to_string(), bytes).await.ok();
        }
    }
}

/// Spawn a background task that generates a short title for the conversation.
///
/// Uses one of [`TITLE_MODELS`] (chosen based on subsecond time parity) via an
/// inline blueprint with no tools. When the title agent responds, the title is:
/// 1. Persisted to the conversation record via [`ConversationStore::set_title`].
/// 2. Published as [`ConversationTitleSet`] to the parent conversation's
///    outbound NATS subject so connected clients can update their UI.
fn spawn_title_task(
    spawner: Arc<dyn Spawner>,
    publisher: Arc<NatsPublisher>,
    conversation_store: Option<Arc<dyn ConversationStore>>,
    out_subject: String,
    conv_id: String,
    user_message: String,
    overrides: Arc<TextOverrides>,
) {
    tokio::spawn(async move {
        let model = {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0);
            TITLE_MODELS[(nanos % TITLE_MODELS.len() as u32) as usize]
        };

        let system_prompt = overrides
            .prompt("title_generator")
            .unwrap_or(TITLE_SYSTEM_PROMPT)
            .to_string();

        let blueprint = AgentBlueprint {
            id: AgentId("inline:title-generator".to_string()),
            name: "title-generator".to_string(),
            description: String::new(),
            system_prompt,
            tools: vec![],
            model_policy: ModelPolicy::AllowList(vec![model.to_string()]),
            tags: vec![],
            handoff_description: None,
            output_schema: None,
            metadata: serde_json::Value::Object(Default::default()),
        };

        let input = SpawnInput {
            blueprint_id: None,
            inline_blueprint: Some(blueprint),
            initial_prompt: user_message,
            parent_path: None,
            parent_call_stack: None,
            resume_conversation_id: None,
        };

        match spawner.spawn_and_await(input).await {
            Ok((_, raw_title)) => {
                let title = raw_title.trim().to_string();
                log::debug!("runner: generated title for {conv_id}: {title:?}");

                if let Some(store) = &conversation_store {
                    if let Err(e) = store.set_title(&conv_id, &title).await {
                        log::warn!("runner: failed to persist title for {conv_id}: {e}");
                    }
                }

                let env = Envelope::new(Message::ConversationTitleSet(ConversationTitleSet {
                    conversation_id: conv_id.clone(),
                    title,
                }));
                if let Ok(bytes) = env.encode() {
                    publisher.publish(out_subject, bytes).await.ok();
                }
            }
            Err(e) => {
                log::warn!("runner: title generation failed for {conv_id}: {e}");
            }
        }
    });
}
