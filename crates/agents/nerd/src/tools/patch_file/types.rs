use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// A single file update operation addressed against the original file.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PatchFile {
    /// Remove lines `start` through `end` inclusive (1-based).
    Remove { start: usize, end: usize },
    /// Replace a single line with new content (1-based).
    Replace { line: usize, content: String },
    /// Insert lines after an anchor (`0` = before the first line).
    Insert { after_line: usize, lines: Vec<String> },
}

#[derive(Debug, Deserialize)]
pub(crate) struct PatchFileInput {
    pub(crate) file_path: String,
    pub(crate) updates: Vec<PatchFile>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PatchFileDiffPreview {
    pub(crate) kind: &'static str,
    pub(crate) file_path: String,
    pub(crate) summary: String,
    pub(crate) stats: PatchFileDiffStats,
    pub(crate) rows: Vec<PatchFileDiffRow>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PatchFileDiffStats {
    pub(crate) added: usize,
    pub(crate) removed: usize,
    pub(crate) modified: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PatchFileDiffRow {
    pub(crate) change: PatchFileDiffChange,
    pub(crate) left_line_number: Option<usize>,
    pub(crate) right_line_number: Option<usize>,
    pub(crate) left_content: String,
    pub(crate) right_content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PatchFileDiffChange {
    Added,
    Context,
    Omitted,
    Removed,
    Modified,
}

pub(crate) struct PlannedUpdates {
    pub(crate) removed: HashSet<usize>,
    pub(crate) replacements: HashMap<usize, String>,
    pub(crate) inserts: HashMap<usize, Vec<String>>,
}
