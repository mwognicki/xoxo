use serde::{Deserialize, Serialize};
use toml_example::TomlExample;

/// A string config value that may be provided directly or through an environment variable.
#[derive(TomlExample, Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct EnvString {
    /// Literal fallback value stored in config.
    #[toml_example(default = "value")]
    pub value: Option<String>,
    /// Environment variable to read at runtime.
    #[toml_example(default = "XOXO_MCP_VALUE")]
    pub env: Option<String>,
    /// Fallback when the environment variable is not set.
    #[toml_example(default = "fallback")]
    pub default: Option<String>,
}

impl EnvString {
    /// Creates a literal config value.
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
    /// use xoxo_core::config::EnvString;
    ///
    /// let value = EnvString::literal("hello");
    /// assert_eq!(value.resolve(), Some("hello".to_string()));
    /// ```
    pub fn literal(value: impl Into<String>) -> Self {
        Self {
            value: Some(value.into()),
            env: None,
            default: None,
        }
    }

    /// Resolves the effective string value.
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
    /// use xoxo_core::config::EnvString;
    ///
    /// let value = EnvString::literal("hello");
    /// assert_eq!(value.resolve(), Some("hello".to_string()));
    /// ```
    pub fn resolve(&self) -> Option<String> {
        self.env
            .as_deref()
            .and_then(|name| std::env::var(name).ok())
            .or_else(|| self.default.clone())
            .or_else(|| self.value.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_string_uses_literal_value() {
        let literal = EnvString::literal("value");

        assert_eq!(literal.resolve(), Some("value".to_string()));
    }

    #[test]
    fn env_extension_uses_default_when_env_is_missing() {
        let extension = EnvString {
            value: None,
            env: Some("XOXO_TEST_ENV_EXTENSION_DOES_NOT_EXIST".to_string()),
            default: Some("fallback".to_string()),
        };

        assert_eq!(extension.resolve(), Some("fallback".to_string()));
    }
}
