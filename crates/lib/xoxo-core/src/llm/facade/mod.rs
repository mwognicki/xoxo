mod conversions;
mod runtime;
mod service;
mod tests;
mod types;

pub use runtime::{LlmBackendKind, LlmResolveError, ResolvedLlm};
pub use service::{LlmCompletionError, LlmFacade};
pub use types::{
    LlmCompletionRequest, LlmCompletionResponse, LlmFinishReason, LlmStreamEvent, LlmToolCall,
    LlmToolChoice, LlmToolDefinition,
};

pub(crate) use conversions::{to_rig_message, to_rig_tool_choice};
pub(crate) use runtime::RuntimeBackend;
