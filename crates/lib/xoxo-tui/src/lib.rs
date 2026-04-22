//! Optional ratatui UI for xoxo.
//!
//! This crate is compiled only when the `tui` feature is enabled on the main
//! binary crate. It wraps the daemon's in-process bus as a TUI client.

mod app;
mod syntax_highlighter;
mod tool_format;
mod tui;
mod ui;

pub use app::{App, LayoutMode};
pub use tui::Tui;
pub use ui::draw;
