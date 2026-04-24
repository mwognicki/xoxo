use crate::chat::structs::{ChatTextMessage, CostObservability, ModelConfig};

/// Unified xoxo request shape for a single LLM completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmCompletionRequest {
    /// Model/provider configuration selected for the request.
    pub model: ModelConfig,
    /// Ordered prompt messages sent to the provider.
    pub messages: Vec<ChatTextMessage>,
    /// Tool definitions made available to the model.
    pub tools: Vec<LlmToolDefinition>,
    /// Optional tool-calling policy override.
    pub tool_choice: Option<LlmToolChoice>,
}

/// Unified xoxo response shape for a single LLM completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmCompletionResponse {
    /// Final assistant message returned by the model.
    pub message: ChatTextMessage,
    /// Tool calls emitted by the assistant, if any.
    pub tool_calls: Vec<LlmToolCall>,
    /// Provider-reported or adapter-inferred finish reason for this completion.
    pub finish_reason: LlmFinishReason,
    /// Optional token/cost observability collected during execution.
    pub observability: Option<CostObservability>,
}

/// Normalized completion finish reasons surfaced across backend adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmFinishReason {
    /// The model finished the turn normally.
    Stop,
    /// The model yielded tool calls and expects tool results before continuing.
    ToolCalls,
}

/// Unified xoxo tool definition used across backend adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmToolDefinition {
    /// Stable tool name exposed to the model.
    pub name: String,
    /// Human-facing tool description.
    pub description: Option<String>,
    /// JSON Schema describing tool parameters.
    pub parameters: serde_json::Value,
}

/// Unified xoxo tool choice policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmToolChoice {
    /// Allow the model to decide whether to call a tool.
    Auto,
    /// Disallow tool use for this request.
    None,
    /// Require at least one tool call before answering.
    Required,
    /// Restrict tool use to a specific set of function names.
    Specific { function_names: Vec<String> },
}

/// Unified xoxo representation of a model-emitted tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmToolCall {
    /// Tool/function name chosen by the model.
    pub name: String,
    /// Structured tool arguments if supplied by the model.
    pub arguments: Option<serde_json::Value>,
}

/// Event emitted by [`crate::llm::LlmFacade::complete_streaming`] while a completion is in flight.
///
/// Step 1 of streaming only emits a single [`LlmStreamEvent::Final`]; delta variants are
/// reserved for later steps that wire incremental decoding into the backend adapters.
///
/// The `Final` variant is boxed to keep enum size balanced across the small delta variants
/// and the fully-assembled response payload.
#[derive(Debug, Clone)]
pub enum LlmStreamEvent {
    /// Incremental assistant text produced by the model.
    TextDelta(String),
    /// Incremental reasoning/thinking text produced by the model.
    ThinkingDelta(String),
    /// Fully-assembled completion response marking the end of the stream.
    Final(Box<LlmCompletionResponse>),
}
