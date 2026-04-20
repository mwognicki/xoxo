#![deny(warnings)]

//! Core library for `xoxo`. Subsystems (bus, config, LLM adapters,
//! persistence, tool runner) live here. Contents to be filled in per design.

pub mod bus;
pub mod agents;
pub mod app_state;

pub mod syntax_highlighter;
pub mod tooling;
pub mod helpers;

#[cfg(feature = "log-broadcast")]
pub mod log_layer;
pub mod chat;
pub mod config;
pub mod llm;
