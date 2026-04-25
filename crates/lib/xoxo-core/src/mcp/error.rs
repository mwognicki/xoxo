use thiserror::Error;

/// Errors produced while configuring or using MCP client connections.
#[derive(Debug, Error)]
pub enum McpError {
    /// The selected transport kind is not one of the supported values.
    #[error("unsupported MCP transport kind {kind:?} for server {server_name:?}")]
    UnsupportedTransportKind { server_name: String, kind: String },
    /// The selected transport kind is missing its corresponding config block.
    #[error("missing {kind} transport configuration for server {server_name:?}")]
    MissingTransportConfig { server_name: String, kind: &'static str },
    /// A required config value could not be resolved.
    #[error("missing required MCP config value {field:?} for server {server_name:?}")]
    MissingRequiredValue {
        server_name: String,
        field: &'static str,
    },
    /// The server is disabled and should not be connected.
    #[error("MCP server {server_name:?} is disabled")]
    DisabledServer { server_name: String },
    /// The requested configured server does not exist.
    #[error("MCP server {server_name:?} is not configured")]
    ServerNotFound { server_name: String },
    /// The requested remote MCP tool was not found on the target server.
    #[error("MCP tool {tool_name:?} was not found on server {server_name:?}")]
    ToolNotFound {
        server_name: String,
        tool_name: String,
    },
    /// The current first-slice runtime does not support the configured auth mode yet.
    #[error("MCP authentication kind {kind:?} is not implemented yet for server {server_name:?}")]
    UnsupportedAuthentication { server_name: String, kind: String },
    /// The current first-slice runtime does not support a configured transport option yet.
    #[error("MCP transport option {option:?} is not implemented yet for server {server_name:?}")]
    UnsupportedTransportOption {
        server_name: String,
        option: &'static str,
    },
    /// Failed to serialize an SDK type into a xoxo-owned descriptor.
    #[error("failed to serialize MCP payload: {0}")]
    Serialization(#[from] serde_json::Error),
    /// Error bubbled up from the underlying transport constructor.
    #[error(transparent)]
    Transport(#[from] rust_mcp_sdk::TransportError),
    /// Error bubbled up from the underlying SDK.
    #[error(transparent)]
    Sdk(#[from] rust_mcp_sdk::error::McpSdkError),
}
