use std::collections::HashMap;
use std::sync::Arc;

use rust_mcp_sdk::mcp_client::{client_runtime, ClientHandler, ClientRuntime, McpClientOptions};
use rust_mcp_sdk::schema::{
    CallToolRequestParams, ClientCapabilities, Implementation, InitializeRequestParams,
    LATEST_PROTOCOL_VERSION,
};
use rust_mcp_sdk::{
    ClientSseTransport, ClientSseTransportOptions, McpClient, RequestOptions, StdioTransport,
    StreamableTransportOptions, ToMcpClientHandler, TransportOptions,
};

use crate::config::{
    McpAuthConfig, McpServerConfig, McpStdioTransportConfig, McpTransportConfig,
};
use crate::mcp::{McpError, McpServerVersion, McpToolDescriptor};

/// A connected or connectable MCP client session owned by xoxo.
pub struct McpClientSession {
    server_name: String,
    client: Arc<ClientRuntime>,
    started: tokio::sync::Mutex<bool>,
}

impl McpClientSession {
    /// Builds a new MCP client session from persisted xoxo config.
    ///
    /// # Errors
    ///
    /// Returns an error when the server is disabled, transport configuration is
    /// incomplete, or the underlying SDK cannot construct the requested client.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn from_config(server: &McpServerConfig) -> Result<Self, McpError> {
        if !server.is_enabled() {
            return Err(McpError::DisabledServer {
                server_name: server.name.clone(),
            });
        }

        validate_auth(&server.name, server.auth.as_ref())?;

        let client_details = default_client_details();
        let client = build_client(&server.name, &server.transport, client_details)?;

        Ok(Self {
            server_name: server.name.clone(),
            client,
            started: tokio::sync::Mutex::new(false),
        })
    }

    /// Returns the configured server name.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// Starts the MCP session and performs the initialize handshake.
    ///
    /// # Errors
    ///
    /// Returns an error if the transport cannot start or the server rejects
    /// the initialize handshake.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub async fn connect(&self) -> Result<(), McpError> {
        let mut started = self.started.lock().await;
        if *started {
            return Ok(());
        }

        self.client.clone().start().await?;
        *started = true;
        Ok(())
    }

    /// Returns the initialized server identity once connected.
    ///
    /// # Errors
    ///
    /// Returns an error if the session cannot be connected.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub async fn server_version(&self) -> Result<Option<McpServerVersion>, McpError> {
        self.connect().await?;

        Ok(self.client.server_version().map(|server| McpServerVersion {
            name: server.name,
            version: server.version,
            title: server.title,
            description: server.description,
        }))
    }

    /// Lists the remote MCP tools available on the configured server.
    ///
    /// # Errors
    ///
    /// Returns an error if the session cannot be connected or the server does
    /// not successfully answer the tools/list request.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub async fn list_tools(&self) -> Result<Vec<McpToolDescriptor>, McpError> {
        self.connect().await?;

        let result = self.client.request_tool_list(None).await?;
        let mut tools = Vec::with_capacity(result.tools.len());

        for tool in result.tools {
            let raw = serde_json::to_value(&tool)?;
            let name = raw
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
                .ok_or_else(|| McpError::MissingRequiredValue {
                    server_name: self.server_name.clone(),
                    field: "tools[].name",
                })?;
            let description = raw
                .get("description")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned);

            tools.push(McpToolDescriptor {
                name,
                description,
                raw,
            });
        }

        Ok(tools)
    }

    /// Invokes one specific remote MCP tool with JSON object arguments.
    ///
    /// # Errors
    ///
    /// Returns an error if the session cannot be connected, the arguments are
    /// not a JSON object, or the remote server fails the tool call.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        self.connect().await?;

        let arguments = arguments
            .as_object()
            .cloned()
            .ok_or_else(|| McpError::MissingRequiredValue {
                server_name: self.server_name.clone(),
                field: "arguments",
            })?;

        let result = self
            .client
            .request_tool_call(CallToolRequestParams {
                name: tool_name.to_string(),
                arguments: Some(arguments),
                meta: None,
                task: None,
            })
            .await?;

        Ok(serde_json::to_value(&result)?)
    }

    /// Shuts down the active session, including any child stdio process.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying SDK cannot close the session.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub async fn shutdown(&self) -> Result<(), McpError> {
        let mut started = self.started.lock().await;
        if !*started {
            return Ok(());
        }

        self.client.shut_down().await?;
        *started = false;
        Ok(())
    }
}

fn default_client_details() -> InitializeRequestParams {
    InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "xoxo".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            title: Some("xoxo MCP Client".to_string()),
            description: Some("xoxo MCP client session".to_string()),
            icons: vec![],
            website_url: None,
        },
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
        meta: None,
    }
}

fn build_client(
    server_name: &str,
    transport: &McpTransportConfig,
    client_details: InitializeRequestParams,
) -> Result<Arc<ClientRuntime>, McpError> {
    match transport.kind.as_str() {
        "stdio" => build_stdio_client(server_name, transport, client_details),
        "http" => build_http_client(server_name, transport, client_details),
        "sse" => build_sse_client(server_name, transport, client_details),
        kind => Err(McpError::UnsupportedTransportKind {
            server_name: server_name.to_string(),
            kind: kind.to_string(),
        }),
    }
}

fn build_stdio_client(
    server_name: &str,
    transport: &McpTransportConfig,
    client_details: InitializeRequestParams,
) -> Result<Arc<ClientRuntime>, McpError> {
    let stdio = transport
        .stdio
        .as_ref()
        .ok_or_else(|| McpError::MissingTransportConfig {
            server_name: server_name.to_string(),
            kind: "stdio",
        })?;
    let command = resolve_required_string(server_name, "transport.stdio.command", &stdio.command)?;
    ensure_stdio_cwd_is_unset(server_name, stdio)?;
    let transport = StdioTransport::create_with_server_launch(
        command,
        stdio.resolved_args(),
        resolved_named_map(&stdio.resolved_env()),
        TransportOptions::default(),
    )?;

    Ok(client_runtime::create_client(McpClientOptions {
        client_details,
        transport,
        handler: NoopClientHandler.to_mcp_client_handler(),
        task_store: None,
        server_task_store: None,
        message_observer: None,
    }))
}

fn build_http_client(
    server_name: &str,
    transport: &McpTransportConfig,
    client_details: InitializeRequestParams,
) -> Result<Arc<ClientRuntime>, McpError> {
    let http = transport
        .http
        .as_ref()
        .ok_or_else(|| McpError::MissingTransportConfig {
            server_name: server_name.to_string(),
            kind: "http",
        })?;
    let mcp_url = resolve_required_string(server_name, "transport.http.url", &http.url)?;
    let transport_options = StreamableTransportOptions {
        mcp_url,
        request_options: RequestOptions {
            custom_headers: resolved_named_map(&http.resolved_headers()),
            ..RequestOptions::default()
        },
    };

    Ok(client_runtime::with_transport_options(
        client_details,
        transport_options,
        NoopClientHandler,
        None,
        None,
        None,
    ))
}

fn build_sse_client(
    server_name: &str,
    transport: &McpTransportConfig,
    client_details: InitializeRequestParams,
) -> Result<Arc<ClientRuntime>, McpError> {
    let sse = transport
        .sse
        .as_ref()
        .ok_or_else(|| McpError::MissingTransportConfig {
            server_name: server_name.to_string(),
            kind: "sse",
        })?;
    let sse_url = resolve_required_string(server_name, "transport.sse.url", &sse.url)?;
    let transport = ClientSseTransport::new(
        &sse_url,
        ClientSseTransportOptions {
            custom_headers: resolved_named_map(&sse.resolved_headers()),
            ..ClientSseTransportOptions::default()
        },
    )?;

    Ok(client_runtime::create_client(McpClientOptions {
        client_details,
        transport,
        handler: NoopClientHandler.to_mcp_client_handler(),
        task_store: None,
        server_task_store: None,
        message_observer: None,
    }))
}

fn resolve_required_string(
    server_name: &str,
    field: &'static str,
    value: &crate::config::EnvString,
) -> Result<String, McpError> {
    value.resolve().ok_or_else(|| McpError::MissingRequiredValue {
        server_name: server_name.to_string(),
        field,
    })
}

fn ensure_stdio_cwd_is_unset(
    server_name: &str,
    transport: &McpStdioTransportConfig,
) -> Result<(), McpError> {
    if transport.cwd.as_ref().and_then(crate::config::EnvString::resolve).is_some() {
        return Err(McpError::UnsupportedTransportOption {
            server_name: server_name.to_string(),
            option: "transport.stdio.cwd",
        });
    }

    Ok(())
}

fn validate_auth(server_name: &str, auth: Option<&McpAuthConfig>) -> Result<(), McpError> {
    if let Some(auth) = auth {
        return Err(McpError::UnsupportedAuthentication {
            server_name: server_name.to_string(),
            kind: auth.kind.clone(),
        });
    }

    Ok(())
}

fn resolved_named_map(entries: &[(String, String)]) -> Option<HashMap<String, String>> {
    if entries.is_empty() {
        return None;
    }

    Some(entries.iter().cloned().collect())
}

struct NoopClientHandler;

impl ClientHandler for NoopClientHandler {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::{EnvString, McpHttpTransportConfig, McpOAuthConfig, McpSseTransportConfig};

    #[test]
    fn builds_stdio_session_from_config() {
        let server = McpServerConfig {
            name: "filesystem".to_string(),
            enabled: Some(true),
            auth: None,
            transport: McpTransportConfig {
                kind: "stdio".to_string(),
                stdio: Some(McpStdioTransportConfig {
                    command: EnvString::literal("npx"),
                    args: Some(vec!["-y".to_string()]),
                    env: None,
                    cwd: None,
                }),
                http: None,
                sse: None,
            },
        };

        let session = McpClientSession::from_config(&server).expect("stdio config should build");
        assert_eq!(session.server_name(), "filesystem");
    }

    #[test]
    fn builds_http_session_from_config() {
        let server = McpServerConfig {
            name: "remote".to_string(),
            enabled: Some(true),
            auth: None,
            transport: McpTransportConfig {
                kind: "http".to_string(),
                stdio: None,
                http: Some(McpHttpTransportConfig {
                    url: EnvString::literal("http://127.0.0.1:3001/mcp"),
                    headers: None,
                }),
                sse: None,
            },
        };

        let session = McpClientSession::from_config(&server).expect("http config should build");
        assert_eq!(session.server_name(), "remote");
    }

    #[test]
    fn builds_sse_session_from_config() {
        let server = McpServerConfig {
            name: "remote-sse".to_string(),
            enabled: Some(true),
            auth: None,
            transport: McpTransportConfig {
                kind: "sse".to_string(),
                stdio: None,
                http: None,
                sse: Some(McpSseTransportConfig {
                    url: EnvString::literal("http://127.0.0.1:3001/sse"),
                    headers: None,
                }),
            },
        };

        let session = McpClientSession::from_config(&server).expect("sse config should build");
        assert_eq!(session.server_name(), "remote-sse");
    }

    #[test]
    fn rejects_auth_until_runtime_support_exists() {
        let server = McpServerConfig {
            name: "oauth".to_string(),
            enabled: Some(true),
            auth: Some(McpAuthConfig {
                kind: "oauth".to_string(),
                oauth: Some(McpOAuthConfig {
                    client_name: Some("xoxo".to_string()),
                    audience: None,
                    scopes: None,
                }),
            }),
            transport: McpTransportConfig {
                kind: "http".to_string(),
                stdio: None,
                http: Some(McpHttpTransportConfig {
                    url: EnvString::literal("http://127.0.0.1:3001/mcp"),
                    headers: None,
                }),
                sse: None,
            },
        };

        let error = match McpClientSession::from_config(&server) {
            Ok(_) => panic!("auth is not implemented"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            McpError::UnsupportedAuthentication { .. }
        ));
    }

    #[test]
    fn rejects_stdio_cwd_until_transport_support_exists() {
        let server = McpServerConfig {
            name: "filesystem".to_string(),
            enabled: Some(true),
            auth: None,
            transport: McpTransportConfig {
                kind: "stdio".to_string(),
                stdio: Some(McpStdioTransportConfig {
                    command: EnvString::literal("npx"),
                    args: None,
                    env: None,
                    cwd: Some(EnvString::literal("/tmp")),
                }),
                http: None,
                sse: None,
            },
        };

        let error = match McpClientSession::from_config(&server) {
            Ok(_) => panic!("cwd is not supported"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            McpError::UnsupportedTransportOption { .. }
        ));
    }
}
