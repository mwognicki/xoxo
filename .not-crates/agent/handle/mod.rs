pub mod nats;
pub mod spawner;

pub use nats::NatsAgentHandle;
pub use spawner::AgentSpawner;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use std::future::Future;
use std::pin::Pin;

use futures_util::future::join_all;

use crate::types::{AgentId, AgentMessage, ConversationId};

/// Error returned by agent handle operations.
#[derive(Debug)]
pub enum HandleError {
    /// The agent is no longer running and cannot accept messages.
    AgentGone(AgentId),
    /// A runtime or transport failure occurred.
    Internal(String),
}

impl std::fmt::Display for HandleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandleError::AgentGone(id) => write!(f, "agent gone: {id}"),
            HandleError::Internal(msg) => write!(f, "handle error: {msg}"),
        }
    }
}

impl std::error::Error for HandleError {}

/// A live reference to a running agent.
///
/// `AgentHandle` is the **only** way to communicate with an agent from
/// outside its own execution context. It is intentionally narrow: send a
/// message or request shutdown. No other operations are exposed here.
///
/// # Subagents and observability
///
/// Subagents are always spawned via the `spawn_subagent` tool — never
/// through this handle. A parent agent receives a subagent's result as a
/// plain tool return value; it does not hold a handle to the subagent.
///
/// However, the runtime **must** maintain handles to all running agents at
/// every nesting level so the main orchestrator can observe them. Log
/// records emitted by a subagent must carry its [`AgentId`] so they can be
/// traced back through any depth of nesting. External storage logging
/// (e.g. MongoDB operation logs) must also include the originating
/// [`AgentId`].
///
/// # Conversations
///
/// Every agent belongs to exactly one conversation. The conversation is the
/// security boundary — access control decisions (future ACLs) are applied
/// at conversation scope, not per-agent.
/// Boxed future returned by dyn-compatible async trait methods.
type HandleFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait AgentHandle: Send + Sync {
    /// The unique identifier of this agent.
    fn id(&self) -> &AgentId;

    /// The conversation this agent belongs to.
    fn conversation_id(&self) -> &ConversationId;

    /// Send a message to the agent.
    ///
    /// The message is delivered to the agent's execution context
    /// (Tokio task, WASM module, child process, etc.) via whatever
    /// transport the runtime has wired up.
    fn send(&self, message: AgentMessage) -> HandleFuture<'_, Result<(), HandleError>>;

    /// Request the agent to shut down gracefully.
    ///
    /// The agent should complete any in-flight work and then exit.
    /// Returns once the shutdown request has been accepted — not necessarily
    /// once the agent has fully terminated.
    fn shutdown(&self) -> HandleFuture<'_, Result<(), HandleError>>;
}

// ---------------------------------------------------------------------------
// HandleRegistry
// ---------------------------------------------------------------------------

/// Runtime registry of all live top-level agent handles.
///
/// Handles are inserted by [`AgentSpawner`] when an agent is created and
/// removed when an agent shuts down. Any part of the framework can look up
/// or enumerate live agents via this registry.
///
/// Thread-safe: the inner map is guarded by a `Mutex`.
pub struct HandleRegistry {
    handles: Mutex<HashMap<AgentId, Arc<dyn AgentHandle>>>,
}

impl HandleRegistry {
    /// Create a new empty registry.
    pub fn new() -> Arc<Self> {
        Arc::new(Self { handles: Mutex::new(HashMap::new()) })
    }

    /// Insert a handle, replacing any existing handle for the same agent ID.
    pub fn insert(&self, handle: Arc<dyn AgentHandle>) {
        let id = handle.id().clone();
        self.handles.lock().unwrap().insert(id, handle);
    }

    /// Remove and return the handle for `id`, if present.
    pub fn remove(&self, id: &AgentId) -> Option<Arc<dyn AgentHandle>> {
        self.handles.lock().unwrap().remove(id)
    }

    /// Look up a handle by agent ID.
    pub fn get(&self, id: &AgentId) -> Option<Arc<dyn AgentHandle>> {
        self.handles.lock().unwrap().get(id).cloned()
    }

    /// Look up a handle by conversation ID.
    ///
    /// O(n) scan — intended for low-frequency lookups such as wiring a
    /// routing task immediately after a spawn. Not suitable for hot paths.
    pub fn get_by_conversation_id(&self, id: &ConversationId) -> Option<Arc<dyn AgentHandle>> {
        self.handles
            .lock()
            .unwrap()
            .values()
            .find(|h| h.conversation_id() == id)
            .cloned()
    }

    /// Return the number of live handles.
    pub fn len(&self) -> usize {
        self.handles.lock().unwrap().len()
    }

    /// Return `true` if no agents are currently registered.
    pub fn is_empty(&self) -> bool {
        self.handles.lock().unwrap().is_empty()
    }

    /// Shut down every registered agent concurrently.
    ///
    /// Collects all live handles, calls [`AgentHandle::shutdown`] on each in
    /// parallel, and waits for all shutdown futures to complete. Errors are
    /// logged as warnings — a single failed shutdown does not prevent others.
    ///
    /// Called by the backend on process shutdown after the ingress listener
    /// has stopped accepting new messages.
    pub async fn shutdown_all(&self) {
        let handles: Vec<Arc<dyn AgentHandle>> = {
            self.handles.lock().unwrap().values().cloned().collect()
        };

        if handles.is_empty() {
            return;
        }

        let futs = handles.iter().map(|h| async move {
            if let Err(e) = h.shutdown().await {
                log::warn!("shutdown_all: agent {} shutdown error: {e}", h.id());
            }
        });

        join_all(futs).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgentMessage, ConversationId};

    struct StubHandle {
        id: AgentId,
        conversation_id: ConversationId,
    }

    impl AgentHandle for StubHandle {
        fn id(&self) -> &AgentId { &self.id }
        fn conversation_id(&self) -> &ConversationId { &self.conversation_id }
        fn send(&self, _: AgentMessage) -> HandleFuture<'_, Result<(), HandleError>> {
            Box::pin(async { Ok(()) })
        }
        fn shutdown(&self) -> HandleFuture<'_, Result<(), HandleError>> {
            Box::pin(async { Ok(()) })
        }
    }

    fn stub(agent: &str, conv: &str) -> Arc<StubHandle> {
        Arc::new(StubHandle {
            id: AgentId(agent.into()),
            conversation_id: ConversationId(conv.into()),
        })
    }

    #[test]
    fn insert_and_get() {
        let registry = HandleRegistry::new();
        registry.insert(stub("a1", "c1"));
        assert!(registry.get(&AgentId("a1".into())).is_some());
        assert!(registry.get(&AgentId("a2".into())).is_none());
    }

    #[test]
    fn remove_returns_handle() {
        let registry = HandleRegistry::new();
        registry.insert(stub("a1", "c1"));
        assert!(registry.remove(&AgentId("a1".into())).is_some());
        assert!(registry.get(&AgentId("a1".into())).is_none());
    }

    #[test]
    fn len_tracks_insertions_and_removals() {
        let registry = HandleRegistry::new();
        assert!(registry.is_empty());
        registry.insert(stub("a1", "c1"));
        registry.insert(stub("a2", "c2"));
        assert_eq!(registry.len(), 2);
        registry.remove(&AgentId("a1".into()));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn insert_replaces_existing() {
        let registry = HandleRegistry::new();
        registry.insert(stub("a1", "c1"));
        registry.insert(stub("a1", "c2")); // same agent ID, different conversation
        assert_eq!(registry.len(), 1);
        let handle = registry.get(&AgentId("a1".into())).unwrap();
        assert_eq!(handle.conversation_id().0, "c2");
    }
}
