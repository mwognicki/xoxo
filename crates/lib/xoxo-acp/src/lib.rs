//! Optional Agent Context Protocol (ACP) front-end for xoxo.
//!
//! This crate is compiled only when the `acp` feature is enabled on the main
//! binary crate. Like [`xoxo-tui`], it is a bus client: it embeds the daemon
//! in-process and bridges it to ACP-speaking peers without reaching into
//! daemon internals.
//!
//! Currently wired:
//!
//! - stdio transport via [`agent_client_protocol::ByteStreams`].
//! - JSON-RPC `initialize` answered with our (empty) agent capabilities.
//! - every other client request answered with a JSON-RPC `internal_error`.
//!
//! The bus/storage bridge for `new_session`, `prompt`, `cancel`, and friends
//! is intentionally left for follow-up work.

mod error;
mod server;
mod session;

pub use error::{AcpError, AcpResult};
pub use server::AcpServer;
pub use session::AcpSession;
