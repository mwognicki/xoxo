use futures::stream::{self, StreamExt};
use thiserror::Error;

use crate::chat::structs::{ChatTextMessage, CostObservability};
use crate::config::ProviderConfig;
use crate::llm::ProviderRegistry;

use super::runtime::{LlmResolveError, ResolvedLlm, RuntimeBackend, resolve_with_registry};
use super::types::{
    LlmCompletionRequest, LlmCompletionResponse, LlmFinishReason, LlmStreamEvent, LlmToolCall,
    LlmToolChoice,
};

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
    #[error("completion stream ended without emitting a final response")]
    MissingFinal,
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
        resolve_with_registry(&self.registry, provider_config, model_name)
    }

    pub async fn complete(
        &self,
        provider_config: &ProviderConfig,
        request: LlmCompletionRequest,
    ) -> Result<LlmCompletionResponse, LlmCompletionError> {
        let mut stream = Box::pin(self.complete_streaming(provider_config, request));
        while let Some(event) = stream.next().await {
            match event? {
                LlmStreamEvent::Final(response) => return Ok(*response),
                LlmStreamEvent::TextDelta(_) | LlmStreamEvent::ThinkingDelta(_) => {}
            }
        }
        Err(LlmCompletionError::MissingFinal)
    }

    /// Executes a completion and yields [`LlmStreamEvent`]s as the backend produces output.
    ///
    /// The rig backend path produces real [`LlmStreamEvent::TextDelta`] and
    /// [`LlmStreamEvent::ThinkingDelta`] events as chunks arrive, followed by a single
    /// [`LlmStreamEvent::Final`] carrying the assembled response. The ai-lib backend path
    /// still runs to completion blocking and emits exactly one [`LlmStreamEvent::Final`];
    /// wiring real streaming there is deferred.
    ///
    /// # Errors
    ///
    /// The yielded `Result` surfaces [`LlmCompletionError`] variants raised by the resolver
    /// or the selected backend adapter.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn complete_streaming(
        &self,
        provider_config: &ProviderConfig,
        request: LlmCompletionRequest,
    ) -> futures::stream::BoxStream<'_, Result<LlmStreamEvent, LlmCompletionError>> {
        let resolved = match self.resolve(provider_config, request.model.model_name.clone()) {
            Ok(resolved) => resolved,
            Err(error) => {
                return stream::once(async move { Err(LlmCompletionError::from(error)) }).boxed();
            }
        };

        match &resolved.backend {
            RuntimeBackend::Rig(_) => {
                crate::llm::rig_stream::openrouter_stream(provider_config.clone(), request)
            }
            RuntimeBackend::AiLib(_) => stream::once(async move {
                let response = self.complete_with_ai_lib(&resolved, request).await?;
                Ok(LlmStreamEvent::Final(Box::new(response)))
            })
            .boxed(),
            RuntimeBackend::Custom { .. } => {
                let provider_name = resolved.model.provider.name.clone();
                stream::once(async move {
                    Err(LlmCompletionError::UnsupportedProvider(provider_name))
                })
                .boxed()
            }
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

        let mut builder =
            AiClientBuilder::new(provider).with_default_chat_model(&request.model.model_name);
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
                    function_names
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "auto".to_string()),
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
            .map(|call| LlmToolCall {
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
                content: response
                    .first_text()
                    .map_err(|error| LlmCompletionError::Execution(error.to_string()))?
                    .to_string(),
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
