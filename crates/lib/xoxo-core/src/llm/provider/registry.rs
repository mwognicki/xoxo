use std::collections::HashMap;

use thiserror::Error;

use crate::config::{Config, ProviderConfig};
use crate::llm::provider::structs::{RegisteredProvider, ResolvedProvider};

/// Static registration entry collected through `inventory`.
pub struct ProviderRegistration {
    /// Stable identifier used by config to refer to this provider.
    pub id: &'static str,
    /// Factory that returns the provider metadata and capabilities.
    pub factory: fn() -> RegisteredProvider,
}

inventory::collect!(ProviderRegistration);

/// Errors produced by the provider registry.
#[derive(Debug, Error)]
pub enum ProviderRegistryError {
    /// No registration matched the configured provider identifier.
    #[error("unknown provider: {0:?}")]
    UnknownProvider(String),
}

/// Inventory-backed registry of provider definitions known to this build.
pub struct ProviderRegistry {
    providers: HashMap<String, RegisteredProvider>,
}

impl ProviderRegistry {
    /// Builds a registry from all `inventory` provider registrations.
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
    /// use xoxo_core::llm::ProviderRegistry;
    ///
    /// let _registry = ProviderRegistry::new();
    /// ```
    pub fn new() -> Self {
        let providers = inventory::iter::<ProviderRegistration>()
            .map(|registration| (registration.id.to_string(), (registration.factory)()))
            .collect();
        Self { providers }
    }

    /// Returns the registered provider metadata for a given identifier.
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
    /// use xoxo_core::llm::ProviderRegistry;
    ///
    /// let registry = ProviderRegistry::new();
    /// let _ = registry.get("openai");
    /// ```
    pub fn get(&self, provider_id: &str) -> Option<&RegisteredProvider> {
        self.providers.get(provider_id)
    }

    /// Returns every registered provider known to the current build.
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
    /// use xoxo_core::llm::ProviderRegistry;
    ///
    /// let registry = ProviderRegistry::new();
    /// let _providers = registry.all();
    /// ```
    pub fn all(&self) -> Vec<&RegisteredProvider> {
        self.providers.values().collect()
    }

    /// Resolves a single persisted provider config into provider metadata.
    ///
    /// # Errors
    ///
    /// Returns `ProviderRegistryError::UnknownProvider` when the config references an
    /// unregistered provider identifier.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use xoxo_core::config::ProviderConfig;
    /// use xoxo_core::llm::ProviderRegistry;
    ///
    /// let registry = ProviderRegistry::new();
    /// let config = ProviderConfig::built_in("openai", None, "secret");
    ///
    /// let _ = registry.resolve(&config);
    /// ```
    pub fn resolve(
        &self,
        config: &ProviderConfig,
    ) -> Result<ResolvedProvider, ProviderRegistryError> {
        let provider_id = config
            .provider_id()
            .ok_or_else(|| {
                ProviderRegistryError::UnknownProvider(
                    config
                        .custom_name()
                        .unwrap_or("custom provider")
                        .to_string(),
                )
            })?;
        let registration = self
            .providers
            .get(provider_id)
            .cloned()
            .ok_or_else(|| ProviderRegistryError::UnknownProvider(provider_id.to_string()))?;

        Ok(ResolvedProvider {
            registration,
            config: config.clone(),
        })
    }

    /// Resolves every configured provider entry from the top-level config.
    ///
    /// # Errors
    ///
    /// Returns the first unknown provider referenced from config.
    ///
    /// # Panics
    ///
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use xoxo_core::config::{CodeQualityConfig, Config};
    /// use xoxo_core::llm::ProviderRegistry;
    ///
    /// let registry = ProviderRegistry::new();
    /// let config = Config {
    ///     code_quality: CodeQualityConfig {
    ///         max_lines_in_file: 400,
    ///     },
    ///     providers: None,
    /// };
    ///
    /// let _ = registry.resolve_all(&config).unwrap();
    /// ```
    pub fn resolve_all(
        &self,
        config: &Config,
    ) -> Result<Vec<ResolvedProvider>, ProviderRegistryError> {
        config
            .providers()
            .iter()
            .map(|provider| self.resolve(provider))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::structs::ApiCompatibility;
    use crate::config::{CodeQualityConfig, Config, CustomProviderCompatibility};

    #[test]
    fn resolve_openai_compatible_provider_without_ai_lib() {
        let registry = ProviderRegistry::new();
        let config = ProviderConfig::built_in(
            "openai",
            Some("https://api.openai.com/v1".to_string()),
            "secret",
        );

        let resolved = registry.resolve(&config).expect("provider resolves");

        assert_eq!(resolved.registration.compatibility, ApiCompatibility::OpenAi);
        assert!(resolved.registration.capabilities.supports_rig);
    }

    #[test]
    fn resolve_ai_lib_only_capability_without_leaking_ai_lib_types() {
        let registry = ProviderRegistry::new();
        let config = ProviderConfig::built_in("qwen", None, "secret");

        let resolved = registry.resolve(&config).expect("provider resolves");

        assert_eq!(
            resolved.registration.compatibility,
            ApiCompatibility::Custom {
                name: "custom".to_string()
            }
        );
        assert!(!resolved.registration.capabilities.supports_rig);
        assert!(resolved.registration.capabilities.supports_ai_lib);
    }

    #[test]
    fn resolve_gemini_exposes_native_compatibility_and_hybrid_capabilities() {
        let registry = ProviderRegistry::new();
        let config = ProviderConfig::built_in("gemini", None, "secret");

        let resolved = registry.resolve(&config).expect("provider resolves");

        assert_eq!(resolved.registration.id, "gemini");
        assert_eq!(resolved.registration.compatibility, ApiCompatibility::Gemini);
        assert!(resolved.registration.capabilities.supports_rig);
        assert!(resolved.registration.capabilities.supports_ai_lib);
    }

    #[test]
    fn resolve_moonshot_exposes_rig_only_capabilities() {
        let registry = ProviderRegistry::new();
        let config = ProviderConfig::built_in("moonshot", None, "secret");

        let resolved = registry.resolve(&config).expect("provider resolves");

        assert_eq!(resolved.registration.id, "moonshot");
        assert_eq!(resolved.registration.name, "Moonshot");
        assert_eq!(resolved.registration.compatibility, ApiCompatibility::Moonshot);
        assert!(resolved.registration.capabilities.supports_rig);
        assert!(!resolved.registration.capabilities.supports_ai_lib);
    }

    #[test]
    fn resolve_z_ai_alias_exposes_only_metadata_and_capabilities() {
        let registry = ProviderRegistry::new();
        let config = ProviderConfig::built_in("z.ai", None, "secret");

        let resolved = registry.resolve(&config).expect("provider resolves");

        assert_eq!(resolved.registration.id, "z.ai");
        assert_eq!(resolved.registration.name, "Z.AI");
        assert!(resolved.registration.capabilities.supports_ai_lib);
    }

    // #[test]
    // fn resolve_all_uses_top_level_config_entries() {
    //     let registry = ProviderRegistry::new();
    //     let config = Config {
    //         code_quality: CodeQualityConfig {
    //             max_lines_in_file: 400,
    //         },
    //         providers: Some(vec![ProviderConfig::built_in("openai", None, "secret")]),
    //     };
    //
    //     let resolved = registry.resolve_all(&config).expect("providers resolve");
    //
    //     assert_eq!(resolved.len(), 1);
    //     assert_eq!(resolved[0].registration.id, "openai");
    // }

    #[test]
    fn resolve_rejects_custom_provider_until_runtime_support_exists() {
        let registry = ProviderRegistry::new();
        let config = ProviderConfig::other(
            "Acme Gateway",
            "https://gateway.example.com/v1",
            CustomProviderCompatibility::OpenAi,
            "secret",
        );

        let error = registry.resolve(&config).expect_err("custom provider should not resolve yet");

        assert!(matches!(
            error,
            ProviderRegistryError::UnknownProvider(name) if name == "Acme Gateway"
        ));
    }
}
