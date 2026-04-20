use thiserror::Error;

use crate::chat::structs::{
    ApiCompatibility, ApiProvider, ChatTextMessage, CostObservability, ModelConfig,
};
use crate::config::{CustomProviderCompatibility, ProviderConfig};
use crate::llm::backends::selector::{select_custom_compatible_backend, select_registered_backend};
use crate::llm::{ProviderRegistry, ProviderRegistryError};

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

/// Public xoxo-owned backend family selected by the facade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmBackendKind {
    /// Execute through the `rig-core` integration path.
    Rig,
    /// Execute through the `ai-lib` integration path.
    AiLib,
    /// Execute through a future custom xoxo-managed integration.
    Custom,
}

/// Resolved LLM target that couples public metadata with a private runtime backend.
#[derive(Debug, Clone)]
pub struct ResolvedLlm {
    model: ModelConfig,
    provider_config: ProviderConfig,
    backend: RuntimeBackend,
}

impl ResolvedLlm {
    /// Returns the resolved model configuration used for execution.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use xoxo_core::config::ProviderConfig;
    /// use xoxo_core::llm::LlmFacade;
    ///
    /// let facade = LlmFacade::new();
    /// let resolved = facade
    ///     .resolve(&ProviderConfig::built_in("openai", None, "secret"), "gpt-4o")
    ///     .unwrap();
    ///
    /// assert_eq!(resolved.model().model_name, "gpt-4o");
    /// ```
    pub fn model(&self) -> &ModelConfig {
        &self.model
    }

    /// Returns the provider config entry that was resolved.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use xoxo_core::config::ProviderConfig;
    /// use xoxo_core::llm::LlmFacade;
    ///
    /// let config = ProviderConfig::built_in("openai", None, "secret");
    /// let facade = LlmFacade::new();
    /// let resolved = facade.resolve(&config, "gpt-4o").unwrap();
    ///
    /// assert_eq!(resolved.provider_config().provider_id(), Some("openai"));
    /// ```
    pub fn provider_config(&self) -> &ProviderConfig {
        &self.provider_config
    }

    /// Returns the selected xoxo backend family.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use xoxo_core::config::ProviderConfig;
    /// use xoxo_core::llm::{LlmBackendKind, LlmFacade};
    ///
    /// let facade = LlmFacade::new();
    /// let resolved = facade
    ///     .resolve(&ProviderConfig::built_in("openai", None, "secret"), "gpt-4o")
    ///     .unwrap();
    ///
    /// assert_eq!(resolved.backend_kind(), LlmBackendKind::Rig);
    /// ```
    pub fn backend_kind(&self) -> LlmBackendKind {
        self.backend.kind()
    }

    /// Returns the resolved base URL override when one was configured.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use xoxo_core::config::ProviderConfig;
    /// use xoxo_core::llm::LlmFacade;
    ///
    /// let facade = LlmFacade::new();
    /// let resolved = facade
    ///     .resolve(
    ///         &ProviderConfig::built_in(
    ///             "openai",
    ///             Some("https://api.openai.com/v1".to_string()),
    ///             "secret",
    ///         ),
    ///         "gpt-4o",
    ///     )
    ///     .unwrap();
    ///
    /// assert_eq!(resolved.base_url(), Some("https://api.openai.com/v1"));
    /// ```
    pub fn base_url(&self) -> Option<&str> {
        self.backend.base_url()
    }

    /// Returns whether the resolved backend supports tool calls.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use xoxo_core::config::ProviderConfig;
    /// use xoxo_core::llm::LlmFacade;
    ///
    /// let facade = LlmFacade::new();
    /// let resolved = facade
    ///     .resolve(&ProviderConfig::built_in("openai", None, "secret"), "gpt-4o")
    ///     .unwrap();
    ///
    /// assert!(resolved.supports_tool_calls());
    /// ```
    pub fn supports_tool_calls(&self) -> bool {
        self.backend.supports_tool_calls()
    }
}

#[derive(Debug, Clone)]
pub(crate) enum RuntimeBackend {
    Rig(crate::llm::backends::rig::RigBackendAdapter),
    AiLib(crate::llm::backends::ai_lib::AiLibBackendAdapter),
    Custom { base_url: Option<String> },
}

impl RuntimeBackend {
    pub(crate) fn kind(&self) -> LlmBackendKind {
        match self {
            Self::Rig(_) => LlmBackendKind::Rig,
            Self::AiLib(adapter) => {
                let _ = adapter.provider_id();
                LlmBackendKind::AiLib
            }
            Self::Custom { .. } => LlmBackendKind::Custom,
        }
    }

    fn base_url(&self) -> Option<&str> {
        match self {
            Self::Rig(adapter) => adapter.base_url(),
            Self::AiLib(adapter) => adapter.base_url(),
            Self::Custom { base_url } => base_url.as_deref(),
        }
    }

    fn supports_tool_calls(&self) -> bool {
        match self {
            Self::Rig(adapter) => adapter.supports_tool_calls(),
            Self::AiLib(adapter) => adapter.supports_tool_calls(),
            Self::Custom { .. } => false,
        }
    }
}

/// Errors returned while resolving a config entry into a facade runtime target.
#[derive(Debug, Error)]
pub enum LlmResolveError {
    /// A built-in provider id could not be resolved through the provider registry.
    #[error(transparent)]
    UnknownProvider(#[from] ProviderRegistryError),
    /// The provider config is missing fields required for its declared kind.
    #[error("invalid provider config: {0}")]
    InvalidProviderConfig(String),
}

/// Errors returned while executing a completion through the resolved runtime backend.
#[derive(Debug, Error)]
pub enum LlmCompletionError {
    #[error(transparent)]
    Resolve(#[from] LlmResolveError),
    #[error("completion request must contain at least one message")]
    EmptyRequest,
    #[error("unsupported runtime backend for provider {0:?}")]
    UnsupportedProvider(String),
    #[error("completion execution failed: {0}")]
    Execution(String),
}

/// Xoxo-owned facade for resolving config entries into runtime-ready LLM targets.
pub struct LlmFacade {
    registry: ProviderRegistry,
}

impl LlmFacade {
    /// Creates a new facade backed by the current inventory-based provider registry.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use xoxo_core::llm::LlmFacade;
    ///
    /// let _facade = LlmFacade::new();
    /// ```
    pub fn new() -> Self {
        Self {
            registry: ProviderRegistry::new(),
        }
    }

    /// Returns the provider registry used by this facade.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use xoxo_core::llm::LlmFacade;
    ///
    /// let facade = LlmFacade::new();
    /// let _ = facade.registry();
    /// ```
    pub fn registry(&self) -> &ProviderRegistry {
        &self.registry
    }

    /// Resolves a provider config plus model name into a runtime-ready facade target.
    ///
    /// # Errors
    ///
    /// Returns `LlmResolveError` when the config is invalid or references an unknown built-in
    /// provider.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use xoxo_core::config::ProviderConfig;
    /// use xoxo_core::llm::LlmFacade;
    ///
    /// let facade = LlmFacade::new();
    /// let resolved = facade
    ///     .resolve(&ProviderConfig::built_in("openai", None, "secret"), "gpt-4o")
    ///     .unwrap();
    ///
    /// assert_eq!(resolved.model().provider.name, "OpenAI");
    /// ```
    pub fn resolve(
        &self,
        provider_config: &ProviderConfig,
        model_name: impl Into<String>,
    ) -> Result<ResolvedLlm, LlmResolveError> {
        let model_name = model_name.into();

        if provider_config.provider_id().is_some() {
            let resolved = self.registry.resolve(provider_config)?;
            let model = ModelConfig {
                model_name,
                provider: ApiProvider {
                    name: resolved.registration.name.clone(),
                    compatibility: resolved.registration.compatibility.clone(),
                },
            };
            return Ok(ResolvedLlm {
                model: model.clone(),
                provider_config: provider_config.clone(),
                backend: select_registered_backend(
                    &resolved.registration,
                    provider_config,
                ),
            });
        }

        let name = provider_config.custom_name().ok_or_else(|| {
            LlmResolveError::InvalidProviderConfig(
                "custom provider is missing a human-friendly name".to_string(),
            )
        })?;
        let declared_compatibility =
            provider_config.custom_compatibility().ok_or_else(|| {
                LlmResolveError::InvalidProviderConfig(
                    "custom provider is missing declared compatibility".to_string(),
                )
            })?;
        let compatibility = api_compatibility_from_custom(declared_compatibility);
        let model = ModelConfig {
            model_name,
            provider: ApiProvider {
                name: name.to_string(),
                compatibility: compatibility.clone(),
            },
        };

        Ok(ResolvedLlm {
            model: model.clone(),
            provider_config: provider_config.clone(),
            backend: select_custom_compatible_backend(provider_config, declared_compatibility),
        })
    }

    pub async fn complete(
        &self,
        provider_config: &ProviderConfig,
        request: LlmCompletionRequest,
    ) -> Result<LlmCompletionResponse, LlmCompletionError> {
        let resolved = self.resolve(provider_config, request.model.model_name.clone())?;

        match resolved.backend {
            RuntimeBackend::Rig(_) => self.complete_with_rig(&resolved, request).await,
            RuntimeBackend::AiLib(_) => self.complete_with_ai_lib(&resolved, request).await,
            RuntimeBackend::Custom { .. } => Err(LlmCompletionError::UnsupportedProvider(
                resolved.model.provider.name,
            )),
        }
    }

    async fn complete_with_rig(
        &self,
        resolved: &ResolvedLlm,
        request: LlmCompletionRequest,
    ) -> Result<LlmCompletionResponse, LlmCompletionError> {
        use rig::client::completion::CompletionClient;
        use rig::completion::{AssistantContent, CompletionRequestBuilder, ToolDefinition as RigToolDefinition};

        let prompt = request
            .messages
            .last()
            .cloned()
            .ok_or(LlmCompletionError::EmptyRequest)?;
        let history = request
            .messages
            .iter()
            .take(request.messages.len().saturating_sub(1))
            .cloned()
            .map(to_rig_message);
        let tools = request
            .tools
            .iter()
            .map(|tool| RigToolDefinition {
                name: tool.name.clone(),
                description: tool.description.clone().unwrap_or_default(),
                parameters: tool.parameters.clone(),
            })
            .collect::<Vec<_>>();
        let tool_choice = to_rig_tool_choice(&request.tool_choice);
        let model_name = request.model.model_name.clone();

        match resolved.provider_config.provider_id() {
            Some("openrouter") => {
                let mut client = rig::providers::openrouter::Client::builder()
                    .api_key(resolved.provider_config.api_key.clone());
                if let Some(base_url) = resolved.provider_config.effective_base_url() {
                    client = client.base_url(base_url);
                }
                let client = client
                    .build()
                    .map_err(|error| LlmCompletionError::Execution(error.to_string()))?;
                let model = client.completion_model(model_name);
                let mut builder = CompletionRequestBuilder::new(model, to_rig_message(prompt))
                    .messages(history)
                    .max_tokens(2048);
                if !tools.is_empty() {
                    builder = builder.tools(tools);
                }
                if let Some(choice) = tool_choice {
                    builder = builder.tool_choice(choice);
                }
                let response = builder
                    .send()
                    .await
                    .map_err(|error| LlmCompletionError::Execution(error.to_string()))?;

                let message = ChatTextMessage {
                    role: crate::chat::structs::ChatTextRole::Agent,
                    content: response
                        .choice
                        .iter()
                        .filter_map(|content| match content {
                            AssistantContent::Text(text) => Some(text.text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                };
                let tool_calls: Vec<crate::llm::LlmToolCall> = response
                    .choice
                    .iter()
                    .filter_map(|content| match content {
                        AssistantContent::ToolCall(tool_call) => Some(crate::llm::LlmToolCall {
                            name: tool_call.function.name.clone(),
                            arguments: Some(tool_call.function.arguments.clone()),
                        }),
                        _ => None,
                    })
                    .collect();
                let finish_reason = if tool_calls.is_empty() {
                    LlmFinishReason::Stop
                } else {
                    LlmFinishReason::ToolCalls
                };

                Ok(LlmCompletionResponse {
                    message,
                    tool_calls,
                    finish_reason,
                    observability: Some(CostObservability {
                        model_name: Some(request.model.model_name),
                        provider_name: Some(request.model.provider.name),
                        usage: crate::chat::structs::TokenUsage {
                            input_tokens: response.usage.input_tokens,
                            output_tokens: response.usage.output_tokens,
                            cached_input_tokens: response.usage.cached_input_tokens,
                            reasoning_tokens: 0,
                            total_tokens: response.usage.total_tokens,
                        },
                        cost: Default::default(),
                    }),
                })
            }
            other => Err(LlmCompletionError::UnsupportedProvider(
                other.unwrap_or("custom").to_string(),
            )),
        }
    }

    async fn complete_with_ai_lib(
        &self,
        resolved: &ResolvedLlm,
        request: LlmCompletionRequest,
    ) -> Result<LlmCompletionResponse, LlmCompletionError> {
        use ai_lib::client::{AiClientBuilder, Provider as AiLibProvider};
        use ai_lib::{ChatCompletionRequest, FunctionCallPolicy, Message, Tool};

        let provider = match resolved.provider_config.provider_id() {
            Some("openrouter") => AiLibProvider::OpenRouter,
            Some("minimax") => AiLibProvider::MiniMax,
            Some("deepseek") => AiLibProvider::DeepSeek,
            Some("qwen") => AiLibProvider::Qwen,
            Some("gemini") => AiLibProvider::Gemini,
            Some(other) => {
                return Err(LlmCompletionError::UnsupportedProvider(other.to_string()));
            }
            None => return Err(LlmCompletionError::UnsupportedProvider("custom".to_string())),
        };

        let mut builder = AiClientBuilder::new(provider)
            .with_default_chat_model(&request.model.model_name);
        if let Some(base_url) = resolved.provider_config.effective_base_url() {
            builder = builder.with_base_url(&base_url);
        }
        let client = builder
            .build()
            .map_err(|error| LlmCompletionError::Execution(error.to_string()))?;

        let mut converted = ChatCompletionRequest::new(
            request.model.model_name.clone(),
            request
                .messages
                .iter()
                .map(|message| match message.role {
                    crate::chat::structs::ChatTextRole::System => {
                        Message::system(message.content.clone())
                    }
                    crate::chat::structs::ChatTextRole::User => {
                        Message::user(message.content.clone())
                    }
                    crate::chat::structs::ChatTextRole::Agent => {
                        Message::assistant(message.content.clone())
                    }
                })
                .collect(),
        );
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

        let response = client
            .chat_completion(converted)
            .await
            .map_err(|error| LlmCompletionError::Execution(error.to_string()))?;

        let tool_calls = response
            .choices
            .iter()
            .filter_map(|choice| choice.message.function_call.as_ref())
            .map(|call| crate::llm::LlmToolCall {
                name: call.name.clone(),
                arguments: call.arguments.clone(),
            })
            .collect::<Vec<_>>();
        let finish_reason = if tool_calls.is_empty() {
            LlmFinishReason::Stop
        } else {
            LlmFinishReason::ToolCalls
        };

        Ok(LlmCompletionResponse {
            message: ChatTextMessage {
                role: crate::chat::structs::ChatTextRole::Agent,
                content: response.first_text().map_err(|error| {
                    LlmCompletionError::Execution(error.to_string())
                })?.to_string(),
            },
            tool_calls,
            finish_reason,
            observability: Some(CostObservability {
                model_name: Some(request.model.model_name),
                provider_name: Some(request.model.provider.name),
                usage: crate::chat::structs::TokenUsage {
                    input_tokens: response.usage.prompt_tokens as u64,
                    output_tokens: response.usage.completion_tokens as u64,
                    cached_input_tokens: 0,
                    reasoning_tokens: 0,
                    total_tokens: response.usage.total_tokens as u64,
                },
                cost: Default::default(),
            }),
        })
    }
}

impl Default for LlmFacade {
    fn default() -> Self {
        Self::new()
    }
}

fn api_compatibility_from_custom(
    compatibility: &CustomProviderCompatibility,
) -> ApiCompatibility {
    match compatibility {
        CustomProviderCompatibility::OpenAi => ApiCompatibility::OpenAiLike,
        CustomProviderCompatibility::Anthropic => ApiCompatibility::AnthropicLike,
    }
}

fn to_rig_message(message: ChatTextMessage) -> rig::completion::Message {
    match message.role {
        crate::chat::structs::ChatTextRole::System => rig::completion::Message::system(message.content),
        crate::chat::structs::ChatTextRole::User => rig::completion::Message::user(message.content),
        crate::chat::structs::ChatTextRole::Agent => rig::completion::Message::assistant(message.content),
    }
}

fn to_rig_tool_choice(choice: &Option<LlmToolChoice>) -> Option<rig::message::ToolChoice> {
    choice.as_ref().map(|choice| match choice {
        LlmToolChoice::Auto => rig::message::ToolChoice::Auto,
        LlmToolChoice::None => rig::message::ToolChoice::None,
        LlmToolChoice::Required => rig::message::ToolChoice::Required,
        LlmToolChoice::Specific { function_names } => rig::message::ToolChoice::Specific {
            function_names: function_names.clone(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_built_in_provider_reuses_registered_metadata_and_selects_rig() {
        let facade = LlmFacade::new();
        let resolved = facade
            .resolve(&ProviderConfig::built_in("openai", None, "secret"), "gpt-4o")
            .expect("provider resolves");

        assert_eq!(resolved.model().provider.name, "OpenAI");
        assert_eq!(resolved.model().provider.compatibility, ApiCompatibility::OpenAi);
        assert_eq!(resolved.backend_kind(), LlmBackendKind::Rig);
        assert!(resolved.supports_tool_calls());
    }

    #[test]
    fn resolve_ai_lib_only_provider_selects_ai_lib_backend_family() {
        let facade = LlmFacade::new();
        let resolved = facade
            .resolve(&ProviderConfig::built_in("qwen", None, "secret"), "qwen-max")
            .expect("provider resolves");

        assert_eq!(resolved.model().provider.name, "Qwen");
        assert_eq!(resolved.backend_kind(), LlmBackendKind::AiLib);
        assert!(resolved.supports_tool_calls());
    }

    #[test]
    fn resolve_gemini_hybrid_provider_prefers_rig() {
        let facade = LlmFacade::new();
        let resolved = facade
            .resolve(
                &ProviderConfig::built_in("gemini", None, "secret"),
                "gemini-1.5-flash",
            )
            .expect("provider resolves");

        assert_eq!(resolved.model().provider.name, "Gemini");
        assert_eq!(resolved.model().provider.compatibility, ApiCompatibility::Gemini);
        assert_eq!(resolved.backend_kind(), LlmBackendKind::Rig);
        assert!(resolved.supports_tool_calls());
    }

    #[test]
    fn resolve_custom_openai_compatible_provider_selects_rig_backend_family() {
        let facade = LlmFacade::new();
        let resolved = facade
            .resolve(
                &ProviderConfig::other(
                    "Acme Gateway",
                    "https://gateway.example.com/v1",
                    CustomProviderCompatibility::OpenAi,
                    "secret",
                ),
                "acme-chat-1",
            )
            .expect("custom provider resolves");

        assert_eq!(resolved.model().provider.name, "Acme Gateway");
        assert_eq!(resolved.model().provider.compatibility, ApiCompatibility::OpenAiLike);
        assert_eq!(resolved.backend_kind(), LlmBackendKind::Rig);
        assert_eq!(resolved.base_url(), Some("https://gateway.example.com/v1"));
        assert!(resolved.supports_tool_calls());
    }

    #[test]
    fn resolve_custom_anthropic_compatible_provider_selects_rig_backend_family() {
        let facade = LlmFacade::new();
        let resolved = facade
            .resolve(
                &ProviderConfig::other(
                    "Acme Anthropic Gateway",
                    "https://anthropic-gw.example.com",
                    CustomProviderCompatibility::Anthropic,
                    "secret",
                ),
                "acme-anthropic-1",
            )
            .expect("custom provider resolves");

        assert_eq!(resolved.model().provider.name, "Acme Anthropic Gateway");
        assert_eq!(
            resolved.model().provider.compatibility,
            ApiCompatibility::AnthropicLike
        );
        assert_eq!(resolved.backend_kind(), LlmBackendKind::Rig);
        assert_eq!(resolved.base_url(), Some("https://anthropic-gw.example.com"));
        assert!(resolved.supports_tool_calls());
    }
}
