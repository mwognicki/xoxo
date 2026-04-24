use thiserror::Error;

use crate::chat::structs::{ApiCompatibility, ApiProvider, ModelConfig};
use crate::config::{CustomProviderCompatibility, ProviderConfig};
use crate::llm::backends::selector::{select_custom_compatible_backend, select_registered_backend};
use crate::llm::{ProviderRegistry, ProviderRegistryError};

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
    pub(crate) model: ModelConfig,
    pub(crate) provider_config: ProviderConfig,
    pub(crate) backend: RuntimeBackend,
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

pub(crate) fn resolve_with_registry(
    registry: &ProviderRegistry,
    provider_config: &ProviderConfig,
    model_name: impl Into<String>,
) -> Result<ResolvedLlm, LlmResolveError> {
    let model_name = model_name.into();

    if provider_config.provider_id().is_some() {
        let resolved = registry.resolve(provider_config)?;
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
            backend: select_registered_backend(&resolved.registration, provider_config),
        });
    }

    let name = provider_config.custom_name().ok_or_else(|| {
        LlmResolveError::InvalidProviderConfig(
            "custom provider is missing a human-friendly name".to_string(),
        )
    })?;
    let declared_compatibility = provider_config.custom_compatibility().ok_or_else(|| {
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

pub(crate) fn api_compatibility_from_custom(
    compatibility: &CustomProviderCompatibility,
) -> ApiCompatibility {
    match compatibility {
        CustomProviderCompatibility::OpenAi => ApiCompatibility::OpenAiLike,
        CustomProviderCompatibility::Anthropic => ApiCompatibility::AnthropicLike,
    }
}
