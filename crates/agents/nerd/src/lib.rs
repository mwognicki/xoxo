#![deny(warnings)]

//! `nerd` — coding-assistant toolkit for xoxo.

pub mod tools;
pub mod coding;
pub mod prompt;
pub mod types;

pub use prompt::build_base_prompt;
