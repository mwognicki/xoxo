/// Execution mode for a rig-core adapter.
///
/// Distinguishes a provider that rig handles through its own native client from one wired
/// via rig's generic OpenAI- or Anthropic-compatible clients. The eventual HTTP dispatch
/// inside rig differs per variant, so the selector pins it up front.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RigAdapterKind {
    /// Provider has a dedicated rig-native client (anthropic, gemini, cohere, ...).
    Native,
    /// Provider speaks the OpenAI wire protocol; use rig's OpenAI-compatible client.
    CompatibleOpenAi,
    /// Provider speaks the Anthropic wire protocol; use rig's Anthropic-compatible client.
    CompatibleAnthropic,
}

/// Private `rig-core` adapter state attached to a resolved facade target.
#[derive(Debug, Clone)]
pub(crate) struct RigBackendAdapter {
    // Runtime dispatch will consume this once the rig HTTP path is wired; for now it is
    // exercised through the test-only `kind()` accessor. See `llm/README.md`.
    #[cfg_attr(not(test), allow(dead_code))]
    kind: RigAdapterKind,
    base_url: Option<String>,
}

impl RigBackendAdapter {
    /// Creates a Rig adapter for a provider speaking the OpenAI wire protocol.
    pub(crate) fn compatible_openai(base_url: Option<String>) -> Self {
        Self {
            kind: RigAdapterKind::CompatibleOpenAi,
            base_url,
        }
    }

    /// Creates a Rig adapter for a provider speaking the Anthropic wire protocol.
    pub(crate) fn compatible_anthropic(base_url: Option<String>) -> Self {
        Self {
            kind: RigAdapterKind::CompatibleAnthropic,
            base_url,
        }
    }

    /// Creates a Rig adapter for a provider natively supported by rig-core.
    pub(crate) fn native(base_url: Option<String>) -> Self {
        Self {
            kind: RigAdapterKind::Native,
            base_url,
        }
    }

    /// Returns the adapter's execution mode.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn kind(&self) -> RigAdapterKind {
        self.kind
    }

    /// Returns the configured base URL override when one exists.
    pub(crate) fn base_url(&self) -> Option<&str> {
        self.base_url.as_deref()
    }

    /// Returns whether this backend supports tool calls.
    pub(crate) fn supports_tool_calls(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rig::completion::{Message as RigMessage, ToolDefinition};
    use rig::message::ToolChoice;

    use crate::chat::structs::{ApiCompatibility, ApiProvider, ChatTextMessage, ChatTextRole, ModelConfig};
    use crate::llm::facade::{
        LlmCompletionRequest, LlmCompletionResponse, LlmFinishReason, LlmToolCall, LlmToolChoice,
        LlmToolDefinition,
    };

    fn to_rig_messages(request: &LlmCompletionRequest) -> Vec<RigMessage> {
        request
            .messages
            .iter()
            .map(|message| match message.role {
                ChatTextRole::System => RigMessage::system(message.content.clone()),
                ChatTextRole::User => RigMessage::user(message.content.clone()),
                ChatTextRole::Agent => RigMessage::assistant(message.content.clone()),
            })
            .collect()
    }

    fn to_rig_tools(request: &LlmCompletionRequest) -> Vec<ToolDefinition> {
        request
            .tools
            .iter()
            .map(|tool| ToolDefinition {
                name: tool.name.clone(),
                description: tool.description.clone().unwrap_or_default(),
                parameters: tool.parameters.clone(),
            })
            .collect()
    }

    fn to_rig_tool_choice(choice: &Option<LlmToolChoice>) -> Option<ToolChoice> {
        choice.as_ref().map(|choice| match choice {
            LlmToolChoice::Auto => ToolChoice::Auto,
            LlmToolChoice::None => ToolChoice::None,
            LlmToolChoice::Required => ToolChoice::Required,
            LlmToolChoice::Specific { function_names } => ToolChoice::Specific {
                function_names: function_names.clone(),
            },
        })
    }

    #[test]
    fn compatible_openai_adapter_keeps_base_url_and_kind() {
        let adapter =
            RigBackendAdapter::compatible_openai(Some("https://api.openai.com/v1".to_string()));

        assert_eq!(adapter.base_url(), Some("https://api.openai.com/v1"));
        assert_eq!(adapter.kind(), RigAdapterKind::CompatibleOpenAi);
        assert!(adapter.supports_tool_calls());
    }

    #[test]
    fn compatible_anthropic_adapter_keeps_base_url_and_kind() {
        let adapter = RigBackendAdapter::compatible_anthropic(Some(
            "https://api.anthropic.com".to_string(),
        ));

        assert_eq!(adapter.base_url(), Some("https://api.anthropic.com"));
        assert_eq!(adapter.kind(), RigAdapterKind::CompatibleAnthropic);
        assert!(adapter.supports_tool_calls());
    }

    #[test]
    fn native_adapter_keeps_base_url_and_kind() {
        let adapter = RigBackendAdapter::native(None);

        assert_eq!(adapter.base_url(), None);
        assert_eq!(adapter.kind(), RigAdapterKind::Native);
        assert!(adapter.supports_tool_calls());
    }

    #[test]
    fn adapter_converts_chat_messages_to_rig_messages() {
        let model = ModelConfig {
            model_name: "gpt-4o".to_string(),
            provider: ApiProvider {
                name: "OpenAI".to_string(),
                compatibility: ApiCompatibility::OpenAi,
            },
        };
        let request = LlmCompletionRequest {
            model,
            messages: vec![
                ChatTextMessage {
                    role: ChatTextRole::System,
                    content: "Be concise.".to_string(),
                },
                ChatTextMessage {
                    role: ChatTextRole::User,
                    content: "Hello".to_string(),
                },
                ChatTextMessage {
                    role: ChatTextRole::Agent,
                    content: "Hi".to_string(),
                },
            ],
            tools: vec![],
            tool_choice: None,
        };

        let messages = to_rig_messages(&request);

        assert_eq!(messages[0], RigMessage::system("Be concise."));
        assert_eq!(messages[1], RigMessage::user("Hello"));
        assert_eq!(messages[2], RigMessage::assistant("Hi"));
    }

    #[test]
    fn request_conversion_maps_tools_and_tool_choice() {
        let request = LlmCompletionRequest {
            model: ModelConfig {
                model_name: "gpt-4o".to_string(),
                provider: ApiProvider {
                    name: "OpenAI".to_string(),
                    compatibility: ApiCompatibility::OpenAi,
                },
            },
            messages: vec![ChatTextMessage {
                role: ChatTextRole::User,
                content: "Weather?".to_string(),
            }],
            tools: vec![LlmToolDefinition {
                name: "get_weather".to_string(),
                description: Some("Get the current weather".to_string()),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "city": { "type": "string" }
                    }
                }),
            }],
            tool_choice: Some(LlmToolChoice::Specific {
                function_names: vec!["get_weather".to_string()],
            }),
        };

        let tools = to_rig_tools(&request);
        let choice = to_rig_tool_choice(&request.tool_choice);

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "get_weather");
        assert_eq!(tools[0].description, "Get the current weather");
        assert!(matches!(
            choice,
            Some(ToolChoice::Specific { function_names }) if function_names == vec!["get_weather"]
        ));
    }

    #[test]
    fn tool_call_response_shape_is_representable() {
        let response = LlmCompletionResponse {
            message: ChatTextMessage {
                role: ChatTextRole::Agent,
                content: String::new(),
            },
            tool_calls: vec![LlmToolCall {
                name: "get_weather".to_string(),
                arguments: Some(serde_json::json!({ "city": "Warsaw" })),
            }],
            finish_reason: LlmFinishReason::ToolCalls,
            observability: None,
        };

        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "get_weather");
    }
}
