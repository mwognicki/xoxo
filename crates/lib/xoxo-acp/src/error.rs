//! Error types for the ACP front-end.

use thiserror::Error;

/// Errors that can be raised by the ACP front-end.
#[derive(Debug, Error)]
pub enum AcpError {
    /// I/O failure on the transport (stdio, socket, etc.).
    #[error("acp transport i/o error: {0}")]
    Io(#[from] std::io::Error),

    /// The underlying ACP protocol implementation reported a JSON-RPC error.
    #[error("acp protocol error: {0:?}")]
    Protocol(agent_client_protocol::Error),

    /// The daemon bus rejected a command or dropped its receiver.
    #[error("acp bus error: {0}")]
    Bus(String),

    /// A feature of the protocol is not yet implemented in this crate.
    #[error("acp feature not implemented: {0}")]
    Unimplemented(&'static str),
}

impl From<agent_client_protocol::Error> for AcpError {
    fn from(value: agent_client_protocol::Error) -> Self {
        Self::Protocol(value)
    }
}

pub type AcpResult<T> = Result<T, AcpError>;
