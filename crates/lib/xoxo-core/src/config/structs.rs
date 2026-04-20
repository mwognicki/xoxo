use serde::{Deserialize, Serialize};
use toml_example::TomlExample;

/// Declared compatibility for a user-defined provider entry.
#[derive(Default, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CustomProviderCompatibility {
    /// The provider exposes an OpenAI-compatible API surface.
    #[default]
    OpenAi,
    /// The provider exposes an Anthropic-compatible API surface.
    Anthropic,
}

/// Persisted configuration for a single named LLM provider entry.
#[derive(TomlExample, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfig {
    /// Distinguishes built-in providers from user-defined ones.
    #[toml_example(default = "built_in")]
    pub kind: String,
    /// Stable built-in provider id. Required when `kind = "built_in"`.
    #[toml_example]
    pub provider_id: Option<String>,
    /// Human-friendly provider name. Required when `kind = "other"`.
    #[toml_example]
    pub name: Option<String>,
    /// Base URL override for built-ins, or required endpoint URL for `other`.
    #[toml_example]
    pub base_url: Option<String>,
    /// Declared protocol compatibility for `other` providers.
    #[toml_example(default, enum)]
    pub compatibility: Option<CustomProviderCompatibility>,
    /// API key used for this provider entry.
    #[toml_example]
    pub api_key: String,
}

impl ProviderConfig {
    /// Creates a built-in provider config.
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
    ///
    /// let config = ProviderConfig::built_in("openai", None, "secret");
    /// assert_eq!(config.provider_id(), Some("openai"));
    /// ```
    pub fn built_in(provider_id: impl Into<String>, base_url: Option<String>, api_key: impl Into<String>) -> Self {
        Self {
            kind: "built_in".to_string(),
            provider_id: Some(provider_id.into()),
            name: None,
            base_url,
            compatibility: None,
            api_key: api_key.into(),
        }
    }

    /// Creates a user-defined compatible provider config.
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
    /// use xoxo_core::config::{CustomProviderCompatibility, ProviderConfig};
    ///
    /// let config = ProviderConfig::other(
    ///     "Acme Gateway",
    ///     "https://gateway.example.com/v1",
    ///     CustomProviderCompatibility::OpenAi,
    ///     "secret",
    /// );
    ///
    /// assert_eq!(config.custom_name(), Some("Acme Gateway"));
    /// ```
    pub fn other(
        name: impl Into<String>,
        base_url: impl Into<String>,
        compatibility: CustomProviderCompatibility,
        api_key: impl Into<String>,
    ) -> Self {
        Self {
            kind: "other".to_string(),
            provider_id: None,
            name: Some(name.into()),
            base_url: Some(base_url.into()),
            compatibility: Some(compatibility),
            api_key: api_key.into(),
        }
    }

    /// Returns the built-in provider id when this entry targets a built-in provider.
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
    ///
    /// let config = ProviderConfig::built_in("openai", None, "secret");
    /// assert_eq!(config.provider_id(), Some("openai"));
    /// ```
    pub fn provider_id(&self) -> Option<&str> {
        self.provider_id.as_deref()
    }

    /// Returns the custom provider name when this entry is user-defined.
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
    /// use xoxo_core::config::{CustomProviderCompatibility, ProviderConfig};
    ///
    /// let config = ProviderConfig::other(
    ///     "Acme Gateway",
    ///     "https://gateway.example.com/v1",
    ///     CustomProviderCompatibility::OpenAi,
    ///     "secret",
    /// );
    ///
    /// assert_eq!(config.custom_name(), Some("Acme Gateway"));
    /// ```
    pub fn custom_name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns the declared custom-provider compatibility when present.
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
    /// use xoxo_core::config::{CustomProviderCompatibility, ProviderConfig};
    ///
    /// let config = ProviderConfig::other(
    ///     "Acme Gateway",
    ///     "https://gateway.example.com/v1",
    ///     CustomProviderCompatibility::Anthropic,
    ///     "secret",
    /// );
    ///
    /// assert_eq!(
    ///     config.custom_compatibility(),
    ///     Some(&CustomProviderCompatibility::Anthropic)
    /// );
    /// ```
    pub fn custom_compatibility(&self) -> Option<&CustomProviderCompatibility> {
        self.compatibility.as_ref()
    }

    /// Returns the configured base URL, skipping empty strings.
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
    ///
    /// let config = ProviderConfig::built_in(
    ///     "openai",
    ///     Some("https://api.openai.com/v1".to_string()),
    ///     "secret",
    /// );
    ///
    /// assert_eq!(
    ///     config.effective_base_url().as_deref(),
    ///     Some("https://api.openai.com/v1")
    /// );
    /// ```
    pub fn effective_base_url(&self) -> Option<String> {
        self.base_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    }

    /// Returns true when this entry targets the provided built-in id.
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
    ///
    /// let config = ProviderConfig::built_in("openai", None, "secret");
    /// assert!(config.matches_provider_id("openai"));
    /// ```
    pub fn matches_provider_id(&self, provider_id: &str) -> bool {
        self.provider_id() == Some(provider_id)
    }
}

/// Simple code-quality configuration loaded from the main config file.
#[derive(TomlExample, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct CodeQualityConfig {
    /// Maximum allowed lines in a source file.
    #[toml_example(default = 400)]
    pub max_lines_in_file: i32,
}

/// Current provider selection persisted in the main config file.
#[derive(TomlExample, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct CurrentProviderConfig {
    /// Stable provider name used by the daemon for new root chats.
    #[toml_example(default = "openrouter")]
    pub name: String,
    /// Declared compatibility for the selected provider.
    #[toml_example(default = "open_router")]
    pub compatibility: String,
}

/// Current model selection persisted in the main config file.
#[derive(TomlExample, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct CurrentModelConfig {
    /// Stable model name used by the daemon for new root chats.
    #[toml_example(default = "minimax-m2.5:free")]
    pub model_name: String,
}

/// Top-level xoxo configuration file.
#[derive(TomlExample, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Code-quality rules used by local tooling.
    #[toml_example(nesting)]
    pub code_quality: CodeQualityConfig,
    /// Current provider selection used for new root chats.
    #[toml_example(nesting)]
    pub current_provider: CurrentProviderConfig,
    /// Current model selection used for new root chats.
    #[toml_example(nesting)]
    pub current_model: CurrentModelConfig,
    /// Optional configured LLM providers.
    #[toml_example(nesting)]
    pub providers: Option<Vec<ProviderConfig>>,
}

impl Config {
    /// Returns the current provider selection.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn current_provider(&self) -> &CurrentProviderConfig {
        &self.current_provider
    }

    /// Returns the current model selection.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn current_model(&self) -> &CurrentModelConfig {
        &self.current_model
    }

    /// Returns all configured providers as a slice.
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
    /// use xoxo_core::config::{CodeQualityConfig, Config};
    ///
    /// let config = Config {
    ///     code_quality: CodeQualityConfig {
    ///         max_lines_in_file: 400,
    ///     },
    ///     current_provider: CurrentProviderConfig {
    ///         name: "openrouter".to_string(),
    ///         compatibility: "open_router".to_string(),
    ///     },
    ///     current_model: CurrentModelConfig {
    ///         model_name: "minimax-m2.5:free".to_string(),
    ///     },
    ///     providers: None,
    /// };
    ///
    /// assert!(config.providers().is_empty());
    /// ```
    pub fn providers(&self) -> &[ProviderConfig] {
        self.providers.as_deref().unwrap_or(&[])
    }

    /// Finds a built-in provider entry by its stable identifier.
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
    /// use xoxo_core::config::{
    ///     CodeQualityConfig, Config, CurrentModelConfig, CurrentProviderConfig, ProviderConfig
    /// };
    ///
    /// let config = Config {
    ///     code_quality: CodeQualityConfig {
    ///         max_lines_in_file: 400,
    ///     },
    ///     current_provider: CurrentProviderConfig {
    ///         name: "openrouter".to_string(),
    ///         compatibility: "open_router".to_string(),
    ///     },
    ///     current_model: CurrentModelConfig {
    ///         model_name: "minimax-m2.5:free".to_string(),
    ///     },
    ///     providers: Some(vec![ProviderConfig::built_in("openai", None, "secret")]),
    /// };
    ///
    /// assert_eq!(
    ///     config.provider("openai").and_then(|provider| provider.provider_id()),
    ///     Some("openai")
    /// );
    /// ```
    pub fn provider(&self, provider_id: &str) -> Option<&ProviderConfig> {
        self.providers()
            .iter()
            .find(|provider| provider.matches_provider_id(provider_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_provider_keeps_stable_id() {
        let provider = ProviderConfig::built_in("openai", None, "secret");

        assert_eq!(provider.provider_id(), Some("openai"));
        assert_eq!(provider.custom_name(), None);
        assert_eq!(provider.custom_compatibility(), None);
    }

    #[test]
    fn other_provider_keeps_name_base_url_and_compatibility() {
        let provider = ProviderConfig::other(
            "Acme Gateway",
            "https://gateway.example.com/v1",
            CustomProviderCompatibility::OpenAi,
            "secret",
        );

        assert_eq!(provider.provider_id(), None);
        assert_eq!(provider.custom_name(), Some("Acme Gateway"));
        assert_eq!(
            provider.custom_compatibility(),
            Some(&CustomProviderCompatibility::OpenAi)
        );
        assert_eq!(
            provider.effective_base_url().as_deref(),
            Some("https://gateway.example.com/v1")
        );
    }

    #[test]
    fn config_keeps_current_provider_and_model() {
        let config = Config {
            code_quality: CodeQualityConfig {
                max_lines_in_file: 400,
            },
            current_provider: CurrentProviderConfig {
                name: "openrouter".to_string(),
                compatibility: "open_router".to_string(),
            },
            current_model: CurrentModelConfig {
                model_name: "minimax-m2.5:free".to_string(),
            },
            providers: None,
        };

        assert_eq!(config.current_provider().name, "openrouter");
        assert_eq!(config.current_model().model_name, "minimax-m2.5:free");
    }
}
