use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use tokio::sync::RwLock;

use crate::config::{load_config, McpServerConfig};
use crate::mcp::{
    McpClientSession, McpError, McpServerSummary, McpServerVersion, McpToolDescriptor,
    McpToolSummary,
};

/// Shared lazy MCP catalog for configured servers and discovered tools.
pub struct McpCatalog {
    sessions: RwLock<HashMap<String, Arc<McpClientSession>>>,
    tool_cache: RwLock<HashMap<String, Vec<McpToolDescriptor>>>,
}

impl McpCatalog {
    /// Returns the process-wide lazy MCP catalog instance.
    pub fn shared() -> Arc<Self> {
        static CATALOG: OnceLock<Arc<McpCatalog>> = OnceLock::new();

        CATALOG
            .get_or_init(|| {
                Arc::new(Self {
                    sessions: RwLock::new(HashMap::new()),
                    tool_cache: RwLock::new(HashMap::new()),
                })
            })
            .clone()
    }

    /// Lists configured MCP servers without eagerly connecting to them.
    pub fn list_servers(&self) -> Vec<McpServerSummary> {
        let config = load_config();
        config
            .mcp_servers()
            .iter()
            .map(McpServerSummary::from_config)
            .collect()
    }

    /// Lists tool summaries for a specific configured MCP server.
    pub async fn list_server_tools(&self, server_name: &str) -> Result<Vec<McpToolSummary>, McpError> {
        let tools = self.cached_or_discover_tools(server_name).await?;
        Ok(tools.iter().map(McpToolSummary::from_descriptor).collect())
    }

    /// Describes one specific tool from a configured MCP server.
    pub async fn describe_tool(
        &self,
        server_name: &str,
        tool_name: &str,
    ) -> Result<McpToolDescriptor, McpError> {
        let tools = self.cached_or_discover_tools(server_name).await?;
        tools.into_iter()
            .find(|tool| tool.name == tool_name)
            .ok_or_else(|| McpError::ToolNotFound {
                server_name: server_name.to_string(),
                tool_name: tool_name.to_string(),
            })
    }

    /// Invokes one specific remote MCP tool on a configured server.
    pub async fn invoke_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        let session = self.session(server_name).await?;
        session.call_tool(tool_name, arguments).await
    }

    /// Returns the initialized server version if the server has been connected.
    pub async fn server_version(
        &self,
        server_name: &str,
    ) -> Result<Option<McpServerVersion>, McpError> {
        let session = self.session(server_name).await?;
        session.server_version().await
    }

    async fn cached_or_discover_tools(
        &self,
        server_name: &str,
    ) -> Result<Vec<McpToolDescriptor>, McpError> {
        if let Some(cached) = self.tool_cache.read().await.get(server_name).cloned() {
            return Ok(cached);
        }

        let session = self.session(server_name).await?;
        let discovered = session.list_tools().await?;
        self.tool_cache
            .write()
            .await
            .insert(server_name.to_string(), discovered.clone());
        Ok(discovered)
    }

    async fn session(&self, server_name: &str) -> Result<Arc<McpClientSession>, McpError> {
        if let Some(session) = self.sessions.read().await.get(server_name).cloned() {
            return Ok(session);
        }

        let server = resolve_server_config(server_name)?;
        let session = Arc::new(McpClientSession::from_config(&server)?);

        let mut sessions = self.sessions.write().await;
        Ok(sessions
            .entry(server_name.to_string())
            .or_insert_with(|| session.clone())
            .clone())
    }
}

fn resolve_server_config(server_name: &str) -> Result<McpServerConfig, McpError> {
    let config = load_config();
    config
        .mcp_server(server_name)
        .cloned()
        .ok_or_else(|| McpError::ServerNotFound {
            server_name: server_name.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_servers_matches_current_config_shape() {
        let catalog = McpCatalog::shared();
        let servers = catalog.list_servers();

        assert!(servers.iter().all(|server| !server.name.is_empty()));
    }
}
