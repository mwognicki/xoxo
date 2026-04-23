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
pub mod ensure_import;
pub mod find_patterns;
pub mod find_files;
pub mod find_references;
pub mod find_tests_for_symbol;
pub mod inspect_code_structure;
pub mod find_symbol;
pub mod patch_symbol;
pub mod rename_symbol;

pub use agentix::tools::list_all_tools;
