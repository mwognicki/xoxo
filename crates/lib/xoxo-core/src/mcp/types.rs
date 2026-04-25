use serde::{Deserialize, Serialize};

use crate::config::McpServerConfig;

/// Lightweight xoxo-owned view of an MCP server's identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerVersion {
    /// Stable server name reported during initialization.
    pub name: String,
    /// Server version reported during initialization.
    pub version: String,
    /// Optional human-facing title.
    pub title: Option<String>,
    /// Optional server description.
    pub description: Option<String>,
}

/// Lightweight xoxo-owned description of a remote MCP tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolDescriptor {
    /// Stable tool name used when calling the remote server.
    pub name: String,
    /// Optional tool description reported by the server.
    pub description: Option<String>,
    /// Raw MCP tool payload for future richer adaptation into xoxo tools.
    pub raw: serde_json::Value,
}

/// Lightweight xoxo-owned summary of one configured MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerSummary {
    /// Stable configured server name.
    pub name: String,
    /// Whether the server is enabled in config.
    pub enabled: bool,
    /// Configured transport kind.
    pub transport_kind: String,
    /// Optional configured auth kind.
    pub auth_kind: Option<String>,
}

impl McpServerSummary {
    /// Builds a summary from persisted config.
    pub fn from_config(server: &McpServerConfig) -> Self {
        Self {
            name: server.name.clone(),
            enabled: server.is_enabled(),
            transport_kind: server.transport.kind.clone(),
            auth_kind: server.auth.as_ref().map(|auth| auth.kind.clone()),
        }
    }
}

/// Lightweight summary of one remote MCP tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpToolSummary {
    /// Stable remote tool name.
    pub name: String,
    /// Optional remote tool description.
    pub description: Option<String>,
}

impl McpToolSummary {
    /// Builds a summary from a richer remote tool descriptor.
    pub fn from_descriptor(tool: &McpToolDescriptor) -> Self {
        Self {
            name: tool.name.clone(),
            description: tool.description.clone(),
        }
    }
}
