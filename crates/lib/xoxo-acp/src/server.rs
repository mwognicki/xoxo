//! ACP server scaffolding wired over stdio.
//!
//! [`AcpServer`] is the entry point a binary calls after spawning the daemon.
//! It owns the bus handle, listens on stdio for an ACP-speaking peer, and
//! responds to `initialize` with our agent capabilities. Every other client
//! request is currently rejected with an `internal_error` so the protocol
//! stays well-formed while the rest of the surface fills in.

use std::sync::Arc;

use agent_client_protocol::schema::{AgentCapabilities, InitializeRequest, InitializeResponse};
use agent_client_protocol::{Agent, ByteStreams, Client, ConnectionTo, Dispatch};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use xoxo_core::bus::Bus;
use xoxo_core::storage::Storage;

use crate::error::{AcpError, AcpResult};

/// Fixed name reported to ACP peers; useful when they log their connection.
const AGENT_NAME: &str = "xoxo";

/// ACP front-end embedded alongside the daemon.
///
/// Bus client only — does not reach into daemon internals. Mirrors the role
/// `xoxo-tui::Tui` plays for the terminal UI.
pub struct AcpServer {
    bus: Bus,
    storage: Arc<Storage>,
}

impl AcpServer {
    /// Construct a server bound to a running daemon's bus and shared storage.
    pub fn new(bus: Bus, storage: Arc<Storage>) -> Self {
        Self { bus, storage }
    }

    /// Bus handle this server publishes commands on.
    pub fn bus(&self) -> &Bus {
        &self.bus
    }

    /// Shared storage handle — sessions use it to hydrate chat history when a
    /// peer reconnects to an existing root chat.
    pub fn storage(&self) -> &Arc<Storage> {
        &self.storage
    }

    /// Run the server over stdio until the peer disconnects.
    ///
    /// Today this only answers `initialize`; every other client request is
    /// answered with a generic JSON-RPC `internal_error` carrying a
    /// `not yet implemented` message. The bus and storage handles are kept on
    /// the server so future request handlers can reach them.
    pub async fn serve(self) -> AcpResult<()> {
        let _bus = self.bus.clone();
        let _storage = Arc::clone(&self.storage);

        tracing::info!("acp server starting on stdio");

        Agent
            .builder()
            .name(AGENT_NAME)
            .on_receive_request(
                async move |initialize: InitializeRequest, responder, _connection| {
                    responder.respond(
                        InitializeResponse::new(initialize.protocol_version)
                            .agent_capabilities(AgentCapabilities::new()),
                    )
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_dispatch(
                async move |message: Dispatch, cx: ConnectionTo<Client>| {
                    message.respond_with_error(
                        agent_client_protocol::util::internal_error(
                            "xoxo acp: not yet implemented",
                        ),
                        cx,
                    )
                },
                agent_client_protocol::on_receive_dispatch!(),
            )
            .connect_to(ByteStreams::new(
                tokio::io::stdout().compat_write(),
                tokio::io::stdin().compat(),
            ))
            .await
            .map_err(AcpError::from)?;

        tracing::info!("acp server stopped");
        Ok(())
    }
}
