//! Boilerplate per-peer ACP session.
//!
//! A session is the bus client for a single ACP-speaking peer. It subscribes
//! to bus events for the chats the peer cares about and translates the peer's
//! ACP requests into [`Command`](xoxo_core::bus::Command)s on the bus.

use std::sync::Arc;

use xoxo_core::bus::Bus;
use xoxo_core::storage::Storage;

use crate::error::{AcpError, AcpResult};

/// Per-peer ACP session. Owns no daemon state — only a bus handle and a
/// storage handle for chat hydration.
pub struct AcpSession {
    bus: Bus,
    storage: Arc<Storage>,
}

impl AcpSession {
    pub fn new(bus: Bus, storage: Arc<Storage>) -> Self {
        Self { bus, storage }
    }

    /// Bus handle this session publishes commands on.
    pub fn bus(&self) -> &Bus {
        &self.bus
    }

    /// Shared storage handle — used to hydrate chat history on reconnect.
    pub fn storage(&self) -> &Arc<Storage> {
        &self.storage
    }

    /// Drive the session against an already-established transport.
    ///
    /// Boilerplate only: returns [`AcpError::Unimplemented`] until the wire
    /// protocol is filled in.
    pub async fn run(self) -> AcpResult<()> {
        Err(AcpError::Unimplemented("acp session loop"))
    }
}
