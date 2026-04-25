use serde::{Deserialize, Serialize};
use toml_example::TomlExample;

use crate::config::env_extensions::EnvString;

/// A named key/value pair whose value may come from an environment extension.
#[derive(TomlExample, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct McpNamedValueConfig {
    /// Stable key name.
    #[toml_example(default = "Authorization")]
    pub name: String,
    /// Literal or environment-backed value.
    #[toml_example(nesting)]
    pub value: EnvString,
}

impl McpNamedValueConfig {
    /// Resolves the pair when the value can be materialized.
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
    /// use xoxo_core::config::{EnvString, McpNamedValueConfig};
    ///
    /// let pair = McpNamedValueConfig {
    ///     name: "Accept".to_string(),
    ///     value: EnvString::literal("application/json"),
    /// };
    ///
    /// assert_eq!(
    ///     pair.resolve(),
    ///     Some(("Accept".to_string(), "application/json".to_string()))
    /// );
    /// ```
    pub fn resolve(&self) -> Option<(String, String)> {
        self.value
            .resolve()
            .map(|value| (self.name.clone(), value))
    }
}

/// OAuth metadata for an MCP server that requires delegated authorization.
#[derive(TomlExample, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct McpOAuthConfig {
    /// Optional human-friendly client name to present during auth flows.
    #[toml_example(default = "xoxo")]
    pub client_name: Option<String>,
    /// Optional audience or resource indicator expected by the MCP server.
    #[toml_example(default = "https://mcp.example.com")]
    pub audience: Option<String>,
    /// Optional scopes the client should request automatically.
    #[toml_example]
    pub scopes: Option<Vec<String>>,
}

/// Authentication strategy for an MCP server.
#[derive(TomlExample, Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct McpAuthConfig {
    /// Authentication kind. Use `oauth` for OAuth-guarded MCP servers.
    #[toml_example(default = "oauth")]
    pub kind: String,
    /// OAuth-specific settings remembered for automatic authentication.
    #[toml_example(nesting)]
    pub oauth: Option<McpOAuthConfig>,
}

impl McpAuthConfig {
    /// Returns true when this config represents OAuth authentication.
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
    /// use xoxo_core::config::McpAuthConfig;
    ///
    /// let auth = McpAuthConfig {
    ///     kind: "oauth".to_string(),
    ///     oauth: None,
    /// };
    ///
    /// assert!(auth.is_oauth());
    /// ```
    pub fn is_oauth(&self) -> bool {
        self.kind == "oauth"
    }
}

/// Configuration for an MCP server reached over standard IO.
#[derive(TomlExample, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct McpStdioTransportConfig {
    /// Executable path or command name.
    #[toml_example(nesting)]
    pub command: EnvString,
    /// Optional command-line arguments.
    #[toml_example]
    pub args: Option<Vec<String>>,
    /// Optional environment variables for the server process.
    #[toml_example(nesting)]
    pub env: Option<Vec<McpNamedValueConfig>>,
    /// Optional working directory for the spawned server process.
    #[toml_example(nesting)]
    pub cwd: Option<EnvString>,
}

impl McpStdioTransportConfig {
    /// Returns all configured command-line arguments.
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
    /// use xoxo_core::config::{EnvString, McpStdioTransportConfig};
    ///
    /// let transport = McpStdioTransportConfig {
    ///     command: EnvString::literal("npx"),
    ///     args: Some(vec!["-y".to_string()]),
    ///     env: None,
    ///     cwd: None,
    /// };
    ///
    /// assert_eq!(transport.resolved_args(), vec!["-y".to_string()]);
    /// ```
    pub fn resolved_args(&self) -> Vec<String> {
        self.args.clone().unwrap_or_default()
    }

    /// Resolves all configured environment variables.
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
    /// use xoxo_core::config::{EnvString, McpNamedValueConfig, McpStdioTransportConfig};
    ///
    /// let transport = McpStdioTransportConfig {
    ///     command: EnvString::literal("server"),
    ///     args: None,
    ///     env: Some(vec![McpNamedValueConfig {
    ///         name: "MODE".to_string(),
    ///         value: EnvString::literal("dev"),
    ///     }]),
    ///     cwd: None,
    /// };
    ///
    /// assert_eq!(
    ///     transport.resolved_env(),
    ///     vec![("MODE".to_string(), "dev".to_string())]
    /// );
    /// ```
    pub fn resolved_env(&self) -> Vec<(String, String)> {
        self.env
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter_map(McpNamedValueConfig::resolve)
            .collect()
    }
}

/// Configuration for an MCP server reached over HTTP.
#[derive(TomlExample, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct McpHttpTransportConfig {
    /// Base URL for the remote MCP endpoint.
    #[toml_example(nesting)]
    pub url: EnvString,
    /// Optional request headers.
    #[toml_example(nesting)]
    pub headers: Option<Vec<McpNamedValueConfig>>,
}

impl McpHttpTransportConfig {
    /// Resolves all configured HTTP headers.
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
    /// use xoxo_core::config::{EnvString, McpHttpTransportConfig, McpNamedValueConfig};
    ///
    /// let transport = McpHttpTransportConfig {
    ///     url: EnvString::literal("https://mcp.example.com"),
    ///     headers: Some(vec![McpNamedValueConfig {
    ///         name: "Accept".to_string(),
    ///         value: EnvString::literal("application/json"),
    ///     }]),
    /// };
    ///
    /// assert_eq!(
    ///     transport.resolved_headers(),
    ///     vec![("Accept".to_string(), "application/json".to_string())]
    /// );
    /// ```
    pub fn resolved_headers(&self) -> Vec<(String, String)> {
        self.headers
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter_map(McpNamedValueConfig::resolve)
            .collect()
    }
}

/// Configuration for an MCP server reached over SSE.
#[derive(TomlExample, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct McpSseTransportConfig {
    /// SSE endpoint URL for the remote MCP server.
    #[toml_example(nesting)]
    pub url: EnvString,
    /// Optional request headers.
    #[toml_example(nesting)]
    pub headers: Option<Vec<McpNamedValueConfig>>,
}

impl McpSseTransportConfig {
    /// Resolves all configured SSE request headers.
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
    /// use xoxo_core::config::{EnvString, McpNamedValueConfig, McpSseTransportConfig};
    ///
    /// let transport = McpSseTransportConfig {
    ///     url: EnvString::literal("https://mcp.example.com/sse"),
    ///     headers: Some(vec![McpNamedValueConfig {
    ///         name: "Accept".to_string(),
    ///         value: EnvString::literal("text/event-stream"),
    ///     }]),
    /// };
    ///
    /// assert_eq!(
    ///     transport.resolved_headers(),
    ///     vec![("Accept".to_string(), "text/event-stream".to_string())]
    /// );
    /// ```
    pub fn resolved_headers(&self) -> Vec<(String, String)> {
        self.headers
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter_map(McpNamedValueConfig::resolve)
            .collect()
    }
}

/// Transport configuration for a single MCP server.
#[derive(TomlExample, Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct McpTransportConfig {
    /// Transport kind. Supported values are `stdio`, `http`, and `sse`.
    #[toml_example(default = "stdio")]
    pub kind: String,
    /// Process-spawn settings for `stdio` servers.
    #[toml_example(nesting)]
    pub stdio: Option<McpStdioTransportConfig>,
    /// Remote endpoint settings for `http` servers.
    #[toml_example(nesting)]
    pub http: Option<McpHttpTransportConfig>,
    /// Remote endpoint settings for `sse` servers.
    #[toml_example(nesting)]
    pub sse: Option<McpSseTransportConfig>,
}

impl McpTransportConfig {
    /// Returns true when this config represents a stdio server.
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
    /// use xoxo_core::config::McpTransportConfig;
    ///
    /// let transport = McpTransportConfig {
    ///     kind: "stdio".to_string(),
    ///     stdio: None,
    ///     http: None,
    ///     sse: None,
    /// };
    ///
    /// assert!(transport.is_stdio());
    /// ```
    pub fn is_stdio(&self) -> bool {
        self.kind == "stdio"
    }
}

/// Persisted configuration for one named MCP server.
#[derive(TomlExample, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct McpServerConfig {
    /// Stable server name used for lookup and UI display.
    #[toml_example(default = "filesystem")]
    pub name: String,
    /// Whether the server should be considered active by default.
    #[toml_example(default = true)]
    pub enabled: Option<bool>,
    /// Optional authentication strategy.
    #[toml_example(nesting)]
    pub auth: Option<McpAuthConfig>,
    /// Transport details for reaching the MCP server.
    #[toml_example(nesting)]
    pub transport: McpTransportConfig,
}

impl McpServerConfig {
    /// Returns whether the server is enabled, defaulting to true.
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
    /// use xoxo_core::config::{McpServerConfig, McpTransportConfig};
    ///
    /// let server = McpServerConfig {
    ///     name: "filesystem".to_string(),
    ///     enabled: None,
    ///     auth: None,
    ///     transport: McpTransportConfig {
    ///         kind: "stdio".to_string(),
    ///         stdio: None,
    ///         http: None,
    ///         sse: None,
    ///     },
    /// };
    ///
    /// assert!(server.is_enabled());
    /// ```
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_transport_resolves_args_and_env() {
        let transport = McpStdioTransportConfig {
            command: EnvString::literal("npx"),
            args: Some(vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
            ]),
            env: Some(vec![McpNamedValueConfig {
                name: "NODE_ENV".to_string(),
                value: EnvString::literal("production"),
            }]),
            cwd: None,
        };

        assert_eq!(
            transport.resolved_args(),
            vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
            ]
        );
        assert_eq!(
            transport.resolved_env(),
            vec![("NODE_ENV".to_string(), "production".to_string())]
        );
    }

    #[test]
    fn http_transport_resolves_headers() {
        let transport = McpHttpTransportConfig {
            url: EnvString::literal("https://mcp.example.com"),
            headers: Some(vec![McpNamedValueConfig {
                name: "Accept".to_string(),
                value: EnvString::literal("application/json"),
            }]),
        };

        assert_eq!(
            transport.resolved_headers(),
            vec![("Accept".to_string(), "application/json".to_string())]
        );
    }

    #[test]
    fn sse_transport_resolves_headers() {
        let transport = McpSseTransportConfig {
            url: EnvString::literal("https://mcp.example.com/sse"),
            headers: Some(vec![McpNamedValueConfig {
                name: "Accept".to_string(),
                value: EnvString::literal("text/event-stream"),
            }]),
        };

        assert_eq!(
            transport.resolved_headers(),
            vec![("Accept".to_string(), "text/event-stream".to_string())]
        );
    }
}
