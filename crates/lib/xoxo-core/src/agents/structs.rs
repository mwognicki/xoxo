use crate::agents::AgentHandle;
use crate::bus::{BusEvent, Command};
use crate::chat::structs::{Chat, ChatAgent, ChatPath, CostObservability};
use crate::config::ProviderConfig;
use crate::storage::Storage;
use crate::tooling::{ToolContext, ToolExecutionContext, ToolRegistry, ToolSet};

use std::collections::HashMap;
use std::sync::{Arc, Weak};
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use uuid::Uuid;

use super::HandoffKind;

#[derive(Debug, Clone, PartialEq)]
pub struct SubagentHandoff {
    pub kind: HandoffKind,
    pub chat: Chat,
    pub summary: Option<String>,
    pub observability: Option<CostObservability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineSubagentSpec {
    pub task: String,
    pub system_prompt: String,
    pub tools: Vec<String>,
    pub model: Option<String>,
    pub context: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnInput {
    pub inline_spec: InlineSubagentSpec,
    pub initial_prompt: String,
    pub parent_path: ChatPath,
}

pub struct AgentSpawner {
    pub(crate) self_weak: Weak<Self>,
    pub(crate) registry: Arc<Mutex<HandleRegistry>>,
    pub(crate) persisted_chats: Arc<Mutex<HashMap<Uuid, Chat>>>,
    pub(crate) storage: Arc<Storage>,
    pub(crate) tree_execution_contexts: Arc<Mutex<HashMap<Uuid, Arc<ToolExecutionContext>>>>,
    pub(crate) tool_registry: Arc<ToolRegistry>,
    pub(crate) events: broadcast::Sender<BusEvent>,
}

pub(crate) struct AgentRunner {
    pub(crate) inbound: mpsc::Receiver<Command>,
    pub(crate) history: Chat,
    pub(crate) blueprint: ChatAgent,
    pub(crate) tool_set: ToolSet,
    pub(crate) tool_context: ToolContext,
    pub(crate) events: broadcast::Sender<BusEvent>,
    pub(crate) path: ChatPath,
    pub(crate) handoff_tx: Option<oneshot::Sender<SubagentHandoff>>,
    pub(crate) provider_config: Option<ProviderConfig>,
    pub(crate) storage: Arc<Storage>,
}

#[derive(Default)]
pub struct HandleRegistry {
    pub(crate) handles: HashMap<Uuid, Arc<dyn AgentHandle>>,
}

pub(crate) struct LocalAgentHandle {
    pub(crate) chat_id: Uuid,
    pub(crate) path: ChatPath,
    pub(crate) sender: mpsc::Sender<Command>,
}
