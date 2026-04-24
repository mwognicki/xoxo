//! Helpers for applying batched line-based updates to file content.

mod apply;
mod diff_preview;
mod tests;
mod tool;
mod types;

pub use apply::apply_updates;
pub use tool::PatchFileTool;
pub use types::PatchFile;
