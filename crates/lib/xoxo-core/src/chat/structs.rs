//! Chat persistence structs.
//!
//! These types intentionally exist ahead of their first runtime call sites so
//! the storage shape can stabilize before the chat subsystem is fully wired up.

#![allow(dead_code)] // Staged persistence model: serde-verified here, wired into runtime next.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Short reusable metadata for a tool known to the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDescription {
    pub name: String,
    pub short_desc: String,
    pub is_mcp: bool,
    pub is_global: bool,
}

/// Short reusable metadata for a skill known to the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDescription {
    pub name: String,
    pub short_desc: String,
    pub is_global: bool,
}

/// The request/response shape an API provider is compatible with.
///
/// Variants come in two flavors:
///
/// * **Wire-format tags** (`OpenAiLike`, `AnthropicLike`, `OpenAiAndAnthropic`) describe a
///   provider that speaks the corresponding wire protocol but has no dedicated native client
///   in a backend crate. These are routed through a generic compatible adapter.
/// * **Per-provider tags** (`Anthropic`, `Gemini`, `Cohere`, ...) mark a provider that has
///   a dedicated native client (e.g. a rig-core native provider). These are routed through
///   the native adapter for that provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApiCompatibility {
    /// Provider speaks the OpenAI wire protocol without a dedicated native client.
    OpenAiLike,
    /// Provider speaks the Anthropic wire protocol without a dedicated native client.
    AnthropicLike,
    /// Provider speaks both the OpenAI and Anthropic wire protocols (e.g. a gateway).
    OpenAiAndAnthropic,
    /// OpenAI — rig-native.
    OpenAi,
    /// Anthropic — rig-native.
    Anthropic,
    /// Google Gemini — rig-native.
    Gemini,
    /// Cohere — rig-native.
    Cohere,
    /// xAI — rig-native.
    XAi,
    /// Azure OpenAI — rig-native.
    AzureOpenAi,
    /// DeepSeek — rig-native.
    DeepSeek,
    /// Ollama — rig-native.
    Ollama,
    /// OpenRouter — rig-native.
    OpenRouter,
    /// Groq — rig-native.
    Groq,
    /// Hyperbolic — rig-native.
    Hyperbolic,
    /// Together AI — rig-native.
    Together,
    /// Galadriel — rig-native.
    Galadriel,
    /// Mira — rig-native.
    Mira,
    /// Perplexity — OpenAI-compatible (no dedicated rig-native client).
    Perplexity,
    /// Moonshot — OpenAI-compatible (no dedicated rig-native client).
    Moonshot,
    /// Config-defined custom provider with a user-supplied name.
    Custom { name: String },
}

/// A configured model provider together with its compatibility contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiProvider {
    pub name: String,
    pub compatibility: ApiCompatibility,
}

/// Model configuration selected for a chat agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelConfig {
    pub model_name: String,
    pub provider: ApiProvider,
}

/// Reusable workspace-level catalog of known tools and skills.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CapabilityCatalog {
    pub tools: Vec<ToolDescription>,
    pub skills: Vec<SkillDescription>,
}

/// Agent configuration stored as part of a chat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatAgent {
    pub id: Option<Uuid>,
    pub name: Option<String>,
    pub model: ModelConfig,
    pub base_prompt: String,
    pub allowed_tools: Vec<String>,
    pub allowed_skills: Vec<String>,
}

/// Token-usage observability for a single model operation or aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MoneyAmount {
    pub amount: u64,
    pub currency: String,
}

/// Cost observability in USD micros to avoid floating-point drift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CostBreakdown {
    pub input_cost_micros: MoneyAmount,
    pub output_cost_micros: MoneyAmount,
    pub cached_input_cost_micros: MoneyAmount,
    pub reasoning_cost_micros: MoneyAmount,
    pub total_cost_micros: MoneyAmount,
}

/// Combined usage and cost observability for a model-backed operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CostObservability {
    pub model_name: Option<String>,
    pub provider_name: Option<String>,
    pub usage: TokenUsage,
    pub cost: CostBreakdown,
}

/// Stable identifier for a persisted chat message.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub String);

/// Stable identifier for a chat branch.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchId(pub String);

/// Stable identifier for a compacted chat snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotId(pub String);

/// Stable identifier for a persisted tool call entry in the chat history.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChatToolCallId(pub String);

/// Root-first lineage for a chat tree, with the current chat id last.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChatPath(pub Vec<Uuid>);

impl ChatPath {
    pub fn root_id(&self) -> &Uuid {
        self.0
            .first()
            .expect("ChatPath must contain at least one chat id")
    }

    pub fn parent_id(&self) -> Option<&Uuid> {
        self.0
            .len()
            .checked_sub(2)
            .and_then(|index| self.0.get(index))
    }

    pub fn current(&self) -> &Uuid {
        self.0
            .last()
            .expect("ChatPath must contain at least one chat id")
    }

    pub fn depth(&self) -> usize {
        self.0.len().saturating_sub(1)
    }

    pub fn child(&self, id: Uuid) -> ChatPath {
        let mut path = self.0.clone();
        path.push(id);
        ChatPath(path)
    }
}

/// Role of a plain text chat message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatTextRole {
    System,
    Agent,
    User,
}

/// A plain text message in the chat transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatTextMessage {
    pub role: ChatTextRole,
    pub content: String,
}

/// A persisted event describing a tool call within the chat transcript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ToolCallEvent {
    Started(ToolCallStarted),
    Completed(ToolCallCompleted),
    Failed(ToolCallFailed),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolCallKind {
    Generic,
    SpawnSubagent {
        child_chat_id: Uuid,
        child_path: ChatPath,
        spec_summary: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallStarted {
    pub tool_call_id: ChatToolCallId,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub kind: ToolCallKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallCompleted {
    pub tool_call_id: ChatToolCallId,
    pub tool_name: String,
    pub result_preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallFailed {
    pub tool_call_id: ChatToolCallId,
    pub tool_name: String,
    pub message: String,
}

/// An internal app event stored in the chat history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "app_event_kind", rename_all = "snake_case")]
pub enum AppEvent {
    ModelChanged {
        from: ModelConfig,
        to: ModelConfig,
    },
}

/// One persisted event in a chat transcript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatEventBody {
    Message(ChatTextMessage),
    ToolCall(ToolCallEvent),
    AppEvent(AppEvent),
}

/// A persisted chat event with a stable string id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatEvent {
    pub id: MessageId,
    pub parent_id: Option<MessageId>,
    pub branch_id: BranchId,
    pub body: ChatEventBody,
    pub observability: Option<CostObservability>,
}

/// How a message should be treated when building the live agent context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageContextState {
    Active,
    CompactedAway,
}

/// A message record stored in the chat log, together with its context status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatLogEntry {
    pub event: ChatEvent,
    pub context_state: MessageContextState,
}

/// A compacted summary of earlier branch history.
///
/// The app can still read the original messages, but the active agent context
/// should use this snapshot plus only later `active` messages on the same
/// branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSnapshot {
    pub id: SnapshotId,
    pub branch_id: BranchId,
    pub summary: String,
    pub replaces_messages_through_id: MessageId,
    pub observability: Option<CostObservability>,
}

/// A named branch of the conversation history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatBranch {
    pub id: BranchId,
    pub name: String,
    pub parent_branch_id: Option<BranchId>,
    pub forked_from_message_id: Option<MessageId>,
    pub head_message_id: Option<MessageId>,
    pub active_snapshot_id: Option<SnapshotId>,
}

/// Serialized chat metadata and the agent configuration used for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chat {
    pub title: Option<String>,
    pub id: Uuid,
    pub parent_chat_id: Option<Uuid>,
    pub spawned_by_tool_call_id: Option<ChatToolCallId>,
    pub path: String,
    pub agent: ChatAgent,
    pub observability: Option<CostObservability>,
    pub active_branch_id: BranchId,
    pub branches: Vec<ChatBranch>,
    pub snapshots: Vec<ChatSnapshot>,
    pub events: Vec<ChatLogEntry>,
}

pub struct ConversationPath {
    pub root_id: Uuid,
    pub parent_id: Uuid,
    pub depth: i32,
}

#[cfg(test)]
#[path = "structs_tests.rs"]
mod tests;
