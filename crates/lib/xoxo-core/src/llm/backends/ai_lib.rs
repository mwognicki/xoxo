use ai_lib::ConnectionOptions;

use crate::config::ProviderConfig;

/// Private `ai-lib` adapter state attached to a resolved facade target.
#[derive(Debug, Clone)]
pub(crate) struct AiLibBackendAdapter {
    provider_id: String,
    connection_options: ConnectionOptions,
}

impl AiLibBackendAdapter {
    /// Creates an `ai-lib` adapter for a supported built-in provider id.
    pub(crate) fn for_provider(
        provider_id: &str,
        provider_config: &ProviderConfig,
    ) -> Result<Self, AiLibBackendError> {
        if !supports_provider(provider_id) {
            return Err(AiLibBackendError::UnsupportedProvider(
                provider_id.to_string(),
            ));
        }

        Ok(Self {
            provider_id: provider_id.to_string(),
            connection_options: ConnectionOptions {
                base_url: provider_config.effective_base_url(),
                proxy: None,
                api_key: Some(provider_config.api_key.clone()),
                timeout: None,
                disable_proxy: false,
            },
        })
    }

    /// Returns the built-in provider id handled by this adapter.
    pub(crate) fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Returns the configured base URL override when one exists.
    pub(crate) fn base_url(&self) -> Option<&str> {
        self.connection_options.base_url.as_deref()
    }

    /// Returns whether this backend supports tool calls.
    pub(crate) fn supports_tool_calls(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum AiLibBackendError {
    #[error("provider {0:?} is not supported by ai-lib adapter")]
    UnsupportedProvider(String),
}

fn supports_provider(provider_id: &str) -> bool {
    matches!(
        provider_id,
        "gemini"
            | "deepseek"
            | "qwen"
            | "baidu_ernie"
            | "tencent_hunyuan"
            | "iflytek_spark"
            | "kimi"
            | "huggingface"
            | "replicate"
            | "perplexity"
            | "ai21"
            | "z.ai"
            | "minimax"
    )
}

#[cfg(test)]
mod tests {
    use ai_lib::{
        ChatCompletionRequest, FunctionCallPolicy, Message, Tool,
    };

    use super::*;
    use crate::chat::structs::{ApiCompatibility, ApiProvider, ChatTextMessage, ChatTextRole, ModelConfig};
    use crate::llm::facade::{
        LlmCompletionRequest, LlmCompletionResponse, LlmToolCall, LlmToolChoice,
        LlmToolDefinition,
    };

    fn to_ai_lib_request(request: &LlmCompletionRequest) -> ChatCompletionRequest {
        let messages = request
            .messages
            .iter()
            .map(|message| match message.role {
                ChatTextRole::System => Message::system(message.content.clone()),
                ChatTextRole::User => Message::user(message.content.clone()),
                ChatTextRole::Agent => Message::assistant(message.content.clone()),
            })
            .collect();

        let mut converted = ChatCompletionRequest::new(request.model.model_name.clone(), messages);
        if !request.tools.is_empty() {
            converted = converted.with_functions(
                request
                    .tools
                    .iter()
                    .map(|tool| Tool {
                        name: tool.name.clone(),
                        description: tool.description.clone(),
                        parameters: Some(tool.parameters.clone()),
                    })
                    .collect(),
            );
        }
        if let Some(choice) = &request.tool_choice {
            converted = converted.with_function_call(match choice {
                LlmToolChoice::Auto => FunctionCallPolicy::Auto("auto".to_string()),
                LlmToolChoice::None => FunctionCallPolicy::None,
                LlmToolChoice::Required => FunctionCallPolicy::Auto("required".to_string()),
                LlmToolChoice::Specific { function_names } => FunctionCallPolicy::Auto(
                    function_names.first().cloned().unwrap_or_else(|| "auto".to_string()),
                ),
            });
        }
        converted
    }

    #[test]
    fn adapter_keeps_provider_id_and_connection_options() {
        let config = ProviderConfig::built_in(
            "gemini",
            Some("https://generativelanguage.googleapis.com/v1beta".to_string()),
            "secret",
        );

        let adapter = AiLibBackendAdapter::for_provider("gemini", &config).expect("adapter builds");

        assert_eq!(adapter.provider_id(), "gemini");
        assert_eq!(
            adapter.base_url(),
            Some("https://generativelanguage.googleapis.com/v1beta")
        );
        assert!(adapter.supports_tool_calls());
    }

    #[test]
    fn adapter_rejects_provider_ids_outside_the_supported_set() {
        let config = ProviderConfig::built_in("openai", None, "secret");

        let error = AiLibBackendAdapter::for_provider("openai", &config).expect_err("unsupported");

        assert_eq!(
            error,
            AiLibBackendError::UnsupportedProvider("openai".to_string())
        );
    }

    #[test]
    fn request_conversion_maps_roles_into_ai_lib_messages() {
        let request = LlmCompletionRequest {
            model: ModelConfig {
                model_name: "gemini-1.5-flash".to_string(),
                provider: ApiProvider {
                    name: "Gemini".to_string(),
                    compatibility: ApiCompatibility::Custom {
                        name: "custom".to_string(),
                    },
                },
            },
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

        let converted = to_ai_lib_request(&request);

        assert_eq!(converted.model, "gemini-1.5-flash");
        assert_eq!(converted.messages.len(), 3);
        assert!(matches!(converted.messages[0].role, ai_lib::Role::System));
        assert!(matches!(converted.messages[1].role, ai_lib::Role::User));
        assert!(matches!(converted.messages[2].role, ai_lib::Role::Assistant));
        assert_eq!(converted.functions.as_ref().map(Vec::len), Some(1));
        assert!(matches!(
            converted.function_call,
            Some(FunctionCallPolicy::Auto(name)) if name == "get_weather"
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
            observability: None,
        };

        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "get_weather");
    }
}
