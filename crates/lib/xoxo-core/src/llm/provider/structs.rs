use crate::chat::structs::ApiCompatibility;
use crate::config::ProviderConfig;

/// Backend capabilities advertised by a registered provider definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProviderCapabilities {
    /// `rig-core` can handle this provider natively.
    pub supports_rig: bool,
    /// `ai-lib` can handle this provider when selected as fallback.
    pub supports_ai_lib: bool,
}

impl ProviderCapabilities {
    /// Creates a capability set for a provider supported by both backend crates.
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
    /// use xoxo_core::llm::ProviderCapabilities;
    ///
    /// let capabilities = ProviderCapabilities::hybrid();
    /// assert!(capabilities.supports_rig);
    /// assert!(capabilities.supports_ai_lib);
    /// ```
    pub const fn hybrid() -> Self {
        Self {
            supports_rig: true,
            supports_ai_lib: true,
        }
    }

    /// Creates a capability set for a provider supported only by `rig-core`.
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
    /// use xoxo_core::llm::ProviderCapabilities;
    ///
    /// let capabilities = ProviderCapabilities::rig_only();
    /// assert!(capabilities.supports_rig);
    /// assert!(!capabilities.supports_ai_lib);
    /// ```
    pub const fn rig_only() -> Self {
        Self {
            supports_rig: true,
            supports_ai_lib: false,
        }
    }

    /// Creates a capability set for a provider supported only by `ai-lib`.
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
    /// use xoxo_core::llm::ProviderCapabilities;
    ///
    /// let capabilities = ProviderCapabilities::ai_lib_only();
    /// assert!(!capabilities.supports_rig);
    /// assert!(capabilities.supports_ai_lib);
    /// ```
    pub const fn ai_lib_only() -> Self {
        Self {
            supports_rig: false,
            supports_ai_lib: true,
        }
    }
}

/// Static metadata for a provider known to the build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredProvider {
    /// Stable persisted identifier used from config.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Request/response compatibility advertised by this provider.
    pub compatibility: ApiCompatibility,
    /// Backend capabilities available for this provider.
    pub capabilities: ProviderCapabilities,
}

/// A provider resolved against persisted configuration and ready for backend selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProvider {
    /// Static metadata for the matched provider definition.
    pub registration: RegisteredProvider,
    /// Persisted configuration used to instantiate this provider later.
    pub config: ProviderConfig,
}
