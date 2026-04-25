use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::mcp::McpCatalog;
use crate::tooling::{ErasedTool, Tool, ToolError, ToolMetadata, ToolRegistration, ToolSchema};

#[derive(Debug, Deserialize)]
struct ListMcpServerToolsInput {
    server_name: String,
}

#[derive(Debug, Deserialize)]
struct DescribeMcpToolInput {
    server_name: String,
    tool_name: String,
}

#[derive(Debug, Deserialize)]
struct InvokeMcpToolInput {
    server_name: String,
    tool_name: String,
    arguments: Value,
}

/// Lists configured MCP servers available to xoxo.
pub struct ListMcpServersTool;

impl ListMcpServersTool {
    pub fn new() -> Self {
        Self
    }
}

impl Tool for ListMcpServersTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "list_mcp_servers".to_string(),
            description: "List configured MCP servers available to xoxo, including transport kind and whether each server is enabled. Does not eagerly connect to the servers.".to_string(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            is_read_only: true,
            supports_concurrent_invocation: true,
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let count = output["server_count"].as_u64().unwrap_or(0);
        format!("Found {count} configured MCP server(s)")
    }

    async fn execute(&self, _ctx: &crate::tooling::ToolContext, input: Value) -> Result<Value, ToolError> {
        if input != Value::Null && input != json!({}) {
            return Err(ToolError::InvalidInput(
                "list_mcp_servers does not accept any input".to_string(),
            ));
        }

        let catalog = McpCatalog::shared();
        let servers = catalog.list_servers();

        Ok(json!({
            "server_count": servers.len(),
            "servers": servers,
        }))
    }
}

/// Lists tools exposed by one configured MCP server.
pub struct ListMcpServerToolsTool;

impl ListMcpServerToolsTool {
    pub fn new() -> Self {
        Self
    }
}

impl Tool for ListMcpServerToolsTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "list_mcp_server_tools".to_string(),
            description: "List remote tools exposed by a specific configured MCP server. Connects lazily and caches discovery results per server.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["server_name"],
                "additionalProperties": false,
                "properties": {
                    "server_name": {
                        "type": "string",
                        "description": "Configured MCP server name."
                    }
                }
            }),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            is_read_only: true,
            supports_concurrent_invocation: true,
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let server = output["server_name"].as_str().unwrap_or("unknown");
        let count = output["tool_count"].as_u64().unwrap_or(0);
        format!("Found {count} MCP tool(s) on server {server}")
    }

    async fn execute(&self, _ctx: &crate::tooling::ToolContext, input: Value) -> Result<Value, ToolError> {
        let input: ListMcpServerToolsInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;
        let catalog = McpCatalog::shared();
        let server_version = catalog
            .server_version(&input.server_name)
            .await
            .map_err(map_mcp_error)?;
        let tools = catalog
            .list_server_tools(&input.server_name)
            .await
            .map_err(map_mcp_error)?;

        Ok(json!({
            "server_name": input.server_name,
            "server_version": server_version,
            "tool_count": tools.len(),
            "tools": tools,
        }))
    }
}

/// Returns the detailed descriptor for one remote MCP tool.
pub struct DescribeMcpToolTool;

impl DescribeMcpToolTool {
    pub fn new() -> Self {
        Self
    }
}

/// Invokes one specific remote MCP tool using lazy server resolution.
pub struct InvokeMcpToolTool;

impl InvokeMcpToolTool {
    pub fn new() -> Self {
        Self
    }
}

impl Tool for InvokeMcpToolTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "invoke_mcp_tool".to_string(),
            description: "Invoke one specific remote MCP tool on a configured MCP server. Use list_mcp_server_tools or describe_mcp_tool first when you need to inspect available tools or their schemas.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["server_name", "tool_name", "arguments"],
                "additionalProperties": false,
                "properties": {
                    "server_name": {
                        "type": "string",
                        "description": "Configured MCP server name."
                    },
                    "tool_name": {
                        "type": "string",
                        "description": "Remote MCP tool name."
                    },
                    "arguments": {
                        "type": "object",
                        "description": "JSON object arguments for the remote MCP tool."
                    }
                }
            }),
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let server = output["server_name"].as_str().unwrap_or("unknown");
        let tool = output["tool_name"].as_str().unwrap_or("unknown");
        format!("Invoked MCP tool {tool} on server {server}")
    }

    async fn execute(&self, _ctx: &crate::tooling::ToolContext, input: Value) -> Result<Value, ToolError> {
        let input: InvokeMcpToolInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;

        if !input.arguments.is_object() {
            return Err(ToolError::InvalidInput(
                "invoke_mcp_tool requires arguments to be a JSON object".to_string(),
            ));
        }

        let catalog = McpCatalog::shared();
        let result = catalog
            .invoke_tool(&input.server_name, &input.tool_name, input.arguments)
            .await
            .map_err(map_mcp_error)?;

        Ok(json!({
            "server_name": input.server_name,
            "tool_name": input.tool_name,
            "result": result,
        }))
    }
}

impl Tool for DescribeMcpToolTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "describe_mcp_tool".to_string(),
            description: "Describe one specific tool exposed by a configured MCP server, including its raw MCP payload for schema inspection.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["server_name", "tool_name"],
                "additionalProperties": false,
                "properties": {
                    "server_name": {
                        "type": "string",
                        "description": "Configured MCP server name."
                    },
                    "tool_name": {
                        "type": "string",
                        "description": "Remote MCP tool name."
                    }
                }
            }),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            is_read_only: true,
            supports_concurrent_invocation: true,
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let server = output["server_name"].as_str().unwrap_or("unknown");
        let tool = output["tool"]["name"].as_str().unwrap_or("unknown");
        format!("Described MCP tool {tool} on server {server}")
    }

    async fn execute(&self, _ctx: &crate::tooling::ToolContext, input: Value) -> Result<Value, ToolError> {
        let input: DescribeMcpToolInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;
        let catalog = McpCatalog::shared();
        let tool = catalog
            .describe_tool(&input.server_name, &input.tool_name)
            .await
            .map_err(map_mcp_error)?;

        Ok(json!({
            "server_name": input.server_name,
            "tool": tool,
        }))
    }
}

fn map_mcp_error(error: crate::mcp::McpError) -> ToolError {
    match error {
        crate::mcp::McpError::ServerNotFound { .. }
        | crate::mcp::McpError::ToolNotFound { .. }
        | crate::mcp::McpError::UnsupportedTransportKind { .. }
        | crate::mcp::McpError::MissingTransportConfig { .. }
        | crate::mcp::McpError::MissingRequiredValue { .. }
        | crate::mcp::McpError::DisabledServer { .. }
        | crate::mcp::McpError::UnsupportedAuthentication { .. }
        | crate::mcp::McpError::UnsupportedTransportOption { .. } => {
            ToolError::InvalidInput(error.to_string())
        }
        crate::mcp::McpError::Serialization(_) | crate::mcp::McpError::Transport(_) | crate::mcp::McpError::Sdk(_) => {
            ToolError::ExecutionFailed(error.to_string())
        }
    }
}

inventory::submit! {
    ToolRegistration {
        name: "list_mcp_servers",
        factory: || Arc::new(ListMcpServersTool::new()) as Arc<dyn ErasedTool>,
    }
}

inventory::submit! {
    ToolRegistration {
        name: "list_mcp_server_tools",
        factory: || Arc::new(ListMcpServerToolsTool::new()) as Arc<dyn ErasedTool>,
    }
}

inventory::submit! {
    ToolRegistration {
        name: "describe_mcp_tool",
        factory: || Arc::new(DescribeMcpToolTool::new()) as Arc<dyn ErasedTool>,
    }
}

inventory::submit! {
    ToolRegistration {
        name: "invoke_mcp_tool",
        factory: || Arc::new(InvokeMcpToolTool::new()) as Arc<dyn ErasedTool>,
    }
}
