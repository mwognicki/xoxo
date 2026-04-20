use std::sync::Arc;

use mongodb::bson::DateTime;
use tokio::sync::mpsc;

use crate::agent::execution_context::{AgentExecutionContext, BashOptions};
use crate::agent::handle::{AgentHandle, NatsAgentHandle};
use crate::agent::runner::AgentRunner;
use crate::conversation_store::ConversationRecord;
use crate::helpers::new_id;
use crate::types::{AgentId, AgentMessage, CallStack, ConversationId, ConversationPath, SpawnError};

use super::{AgentSpawner, SpawnContext, CHANNEL_BUFFER};

/// Build and wire the agent after new/resume context is resolved.
///
/// Shared tail for both spawn paths:
/// 1. Resolves the tool set from the blueprint.
/// 2. Creates the execution context (bash session) if needed.
/// 3. Persists the conversation record (new spawns only).
/// 4. Creates the [`NatsAgentHandle`] and starts the [`AgentRunner`] task.
/// 5. Delivers the initial prompt as the first message.
pub(super) async fn do_wire(
    spawner: &AgentSpawner,
    mut ctx: SpawnContext,
) -> Result<ConversationId, SpawnError> {
    let tool_set = spawner
        .tool_registry
        .resolve_set(&ctx.blueprint.tools)
        .map_err(|e| SpawnError::Internal(e.to_string()))?;

    // Apply runtime system-prompt override (if any) and substitute
    // {{tool_names}} with the comma-separated list of this agent's tools.
    // Inline blueprints construct their own prompts in the runner — skip here.
    if !ctx.blueprint.id.0.starts_with("inline:") {
        let base = spawner
            .overrides
            .prompt(&ctx.blueprint.id.0)
            .unwrap_or(&ctx.blueprint.system_prompt)
            .to_string();
        let tool_names = tool_set
            .schemas()
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        ctx.blueprint.system_prompt = base.replace("{{tool_names}}", &tool_names);
    }

    let agent_id = AgentId(new_id());

    // Capture the parent's conversation ID before consuming parent_path.
    let parent_conversation_id = ctx.parent_path.as_ref().map(|p| p.current().0.clone());

    let path = match ctx.parent_path {
        None => ConversationPath::root(ctx.conversation_id.clone()),
        Some(parent) => parent.child(ctx.conversation_id.clone()),
    };

    // Build the child's call stack by appending the new agent ID to
    // the parent's stack. Top-level agents start a fresh stack.
    let call_stack = match ctx.parent_call_stack {
        None => CallStack(vec![agent_id.clone()]),
        Some(CallStack(mut ids)) => {
            ids.push(agent_id.clone());
            CallStack(ids)
        }
    };

    // Create an execution context (bash session + process registry) for
    // agents whose tool set includes session-backed tools. Tools such as
    // `exec`, `eval_script`, `http_request`, and `process` require this;
    // purely stateless tools (e.g. `read_file`) do not.
    const SESSION_BACKED: &[&str] = &["exec", "eval_script", "http_request", "process"];
    let execution_context = if SESSION_BACKED.iter().any(|name| tool_set.get(name).is_some()) {
        match AgentExecutionContext::new(BashOptions::default()).await {
            Ok(ec) => {
                log::debug!("spawner: created execution context for agent {}", agent_id);
                Some(Arc::new(ec))
            }
            Err(e) => {
                log::warn!(
                    "spawner: failed to create execution context for agent {}: {e}",
                    agent_id
                );
                None
            }
        }
    } else {
        None
    };

    // Upgrade the weak self-reference to pass the spawner into the
    // runner's ToolContext so tools (e.g. spawn_subagent) can create
    // further child agents through the same Spawner interface.
    let spawner_arc = spawner
        .weak_self
        .upgrade()
        .ok_or_else(|| SpawnError::Internal("spawner dropped during spawn".into()))?;

    // Inline blueprints (e.g. title generator) are ephemeral framework-internal
    // agents: no conversation record, no message persistence.
    let is_inline = ctx.blueprint.id.0.starts_with("inline:");

    // Persist the conversation record before the first message is sent.
    // Resumed conversations and inline agents skip this step.
    if !ctx.is_resume && !is_inline {
        if let Some(store) = &spawner.conversation_store {
            let provider = spawner
                .llm
                .as_ref()
                .map(|l| l.provider().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let record = ConversationRecord {
                id: ctx.conversation_id.0.clone(),
                parent_conversation_id: parent_conversation_id.clone(),
                blueprint: ctx.blueprint.clone(),
                provider,
                started_at: DateTime::now(),
                ended_at: None,
                initial_prompt: ctx.initial_prompt.clone(),
                title: None,
                messages: Vec::new(),
                metadata: serde_json::Value::Object(Default::default()),
            };
            if let Err(e) = store.create(&record).await {
                log::warn!("spawner: failed to create conversation record: {e}");
            }
        }
    }

    let (tx, rx) = mpsc::channel::<String>(CHANNEL_BUFFER);

    let handle = Arc::new(NatsAgentHandle::new(
        agent_id.clone(),
        path.clone(),
        call_stack.clone(),
        Arc::clone(&spawner.publisher),
        tx,
    ));

    spawner.registry.insert(Arc::clone(&handle) as Arc<dyn AgentHandle>);

    // Inline agents are ephemeral: pass None for conversation_store so the
    // runner skips message persistence and close() entirely.
    let effective_conversation_store = if is_inline {
        None
    } else {
        spawner.conversation_store.clone()
    };

    let runner = AgentRunner::new(
        rx,
        spawner.llm.clone(),
        ctx.blueprint.clone(),
        path,
        ctx.conversation_id.clone(),
        agent_id.clone(),
        call_stack,
        Arc::clone(&spawner.publisher),
        tool_set,
        spawner_arc,
        execution_context,
        effective_conversation_store,
        spawner.memory_store.clone(),
        spawner.skill_store.clone(),
        ctx.initial_history,
        ctx.response_tx,
        Arc::clone(&spawner.overrides),
    );

    tokio::spawn(runner.run());

    // Deliver the initial prompt as the first message to the runner.
    // For resume, initial_prompt may be empty — skip if so.
    if !ctx.initial_prompt.is_empty() {
        handle
            .send(AgentMessage {
                conversation_id: ctx.conversation_id.clone(),
                content: ctx.initial_prompt,
            })
            .await
            .map_err(|e| SpawnError::Internal(e.to_string()))?;
    }

    log::info!(
        "spawner: spawned agent {} (conversation: {}, blueprint: {})",
        agent_id,
        ctx.conversation_id,
        ctx.blueprint.id.0,
    );

    Ok(ctx.conversation_id)
}
