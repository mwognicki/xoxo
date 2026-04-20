//! AI tool calls module.
//!
//! This module contains submodules, each representing a specific AI tool call.
//! Each tool is implemented as a plain Rust function in its own file.

// Declare tool modules here - one per tool
pub mod read_file;
pub mod write_file;
pub mod http;
pub mod process;
pub mod exec;
pub mod eval_script;
pub mod patch_file;
pub mod list_all_tools;
pub mod find_patterns;
pub mod find_files;
