use std::sync::Arc;

use tokio::sync::mpsc;

use crate::nats::{NatsPublisher, FRAMEWORK_EVENTS};
use crate::types::{AgentId, AgentMessage, CallStack, ConversationId, ConversationPath};
use crate::wire::{Envelope, ErrorPayload, Message};

use super::{AgentHandle, HandleError, HandleFuture};

/// Handle to a running agent.
///
/// Delivers messages via an internal [`tokio::sync::mpsc`] channel directly
/// to the agent's [`AgentRunner`] task — no NATS round-trip on the hot path.
///
/// Used for both top-level agents (spawned via NATS `SpawnRequest`) and
/// subagents (spawned via the `spawn_subagent` tool). The handle is
/// transport-agnostic; the runner receives the same channel regardless of
/// which spawn path was used.
///
/// Carries the full [`CallStack`] ancestry so any component holding this
/// handle can construct a complete provenance chain for observability.
///
/// [`AgentRunner`]: crate::agent::runner::AgentRunner
pub struct NatsAgentHandle {
    agent_id: AgentId,
    conversation_id: ConversationId,
    path: ConversationPath,
    call_stack: CallStack,
    publisher: Arc<NatsPublisher>,
    /// Inbound channel to the agent's runner task.
    tx: mpsc::Sender<String>,
}

impl NatsAgentHandle {
    /// Construct a handle for an agent at the given conversation path.
    ///
    /// `call_stack` must already include the new agent's own [`AgentId`]
    /// as its last element — the spawner is responsible for building it.
    /// `tx` is the sender end of the channel connected to the agent's runner.
    pub fn new(
        agent_id: AgentId,
        path: ConversationPath,
        call_stack: CallStack,
        publisher: Arc<NatsPublisher>,
        tx: mpsc::Sender<String>,
    ) -> Self {
        let conversation_id = path.current().clone();
        Self { agent_id, conversation_id, path, call_stack, publisher, tx }
    }

    /// The full agent call stack for this handle, root-first.
    pub fn call_stack(&self) -> &CallStack {
        &self.call_stack
    }
}

impl AgentHandle for NatsAgentHandle {
    fn id(&self) -> &AgentId {
        &self.agent_id
    }

    fn conversation_id(&self) -> &ConversationId {
        &self.conversation_id
    }

    fn send(&self, message: AgentMessage) -> HandleFuture<'_, Result<(), HandleError>> {
        Box::pin(async move {
            self.tx
                .send(message.content)
                .await
                .map_err(|_| HandleError::AgentGone(self.agent_id.clone()))
        })
    }

    /// Request the agent to shut down.
    ///
    /// Publishes a lifecycle event to [`FRAMEWORK_EVENTS`]. The agent runtime
    /// (future work) subscribes to this subject and terminates the agent on
    /// receipt. An `Error` payload with code `"shutdown"` is used as the
    /// signal until a dedicated `ShutdownRequest` wire message is added.
    fn shutdown(&self) -> HandleFuture<'_, Result<(), HandleError>> {
        Box::pin(async move {
            let envelope = Envelope::new(Message::Error(ErrorPayload {
                code: "shutdown".into(),
                message: format!("shutdown requested for agent {}", self.agent_id),
            }));
            publish(&self.publisher, FRAMEWORK_EVENTS.into(), envelope).await
        })
    }
}

async fn publish(
    publisher: &NatsPublisher,
    subject: String,
    envelope: Envelope,
) -> Result<(), HandleError> {
    let bytes = envelope.encode().map_err(|e| HandleError::Internal(e.to_string()))?;
    publisher.publish(subject, bytes).await.map_err(|e| HandleError::Internal(e.to_string()))
}
