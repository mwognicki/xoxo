//! Helpers for applying batched line-based updates to file content.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use agentix::tooling::{
    ErasedTool, Tool, ToolContext, ToolError, ToolRegistration, ToolSchema,
};

const PATCH_FILE_CONTEXT_LINES: usize = 4;

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
struct PatchFileInput {
    file_path: String,
    updates: Vec<PatchFile>,
}

#[derive(Debug, Serialize)]
struct PatchFileDiffPreview {
    kind: &'static str,
    file_path: String,
    summary: String,
    stats: PatchFileDiffStats,
    rows: Vec<PatchFileDiffRow>,
}

#[derive(Debug, Serialize)]
struct PatchFileDiffStats {
    added: usize,
    removed: usize,
    modified: usize,
}

#[derive(Debug, Clone, Serialize)]
struct PatchFileDiffRow {
    change: PatchFileDiffChange,
    left_line_number: Option<usize>,
    right_line_number: Option<usize>,
    left_content: String,
    right_content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum PatchFileDiffChange {
    Added,
    Context,
    Omitted,
    Removed,
    Modified,
}

struct PlannedUpdates {
    removed: HashSet<usize>,
    replacements: HashMap<usize, String>,
    inserts: HashMap<usize, Vec<String>>,
}

/// Tool implementation for patching an existing file with line-based updates.
pub struct PatchFileTool;

impl PatchFileTool {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(
        &self,
        ctx: &ToolContext,
        file_path: &str,
        updates: Vec<PatchFile>,
    ) -> Result<Value, ToolError> {
        let original_md5 = if ctx.execution_context.is_some() {
            Some(read_file_md5(file_path).map_err(ToolError::ExecutionFailed)?)
        } else {
            None
        };
        let original_content = fs::read_to_string(file_path).map_err(|err| {
            ToolError::ExecutionFailed(err.to_string())
        })?;
        let diff_preview =
            build_diff_preview(file_path, &original_content, &updates).map_err(ToolError::ExecutionFailed)?;

        if let (Some(exec_ctx), Some(original_md5)) = (&ctx.execution_context, original_md5.as_deref()) {
            exec_ctx
                .file_registry
                .lock()
                .await
                .ensure_read(file_path, original_md5)
                .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        }

        let updated = self
            .patch_file_impl(file_path, updates, None, None)
            .map_err(ToolError::ExecutionFailed)?;
        let updated_md5 = format!("{:x}", md5::compute(&updated));

        if let (Some(exec_ctx), Some(original_md5)) = (&ctx.execution_context, original_md5.as_deref()) {
            exec_ctx
                .file_registry
                .lock()
                .await
                .update(file_path, original_md5, updated_md5.clone())
                .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        }

        Ok(json!({
            "message": format!("File patched: {file_path}"),
            "file_path": file_path,
            "exists": true,
            "md5": updated_md5,
            "line_count": split_lines(&updated).len(),
            "diff_preview": diff_preview,
        }))
    }

    fn patch_file_impl(
        &self,
        file_path: &str,
        updates: Vec<PatchFile>,
        pre_execution_hook: Option<fn(&str, &str)>,
        callback: Option<fn(&str, usize) -> Result<(), String>>,
    ) -> Result<String, String> {
        patch_file_impl(file_path, updates, pre_execution_hook, callback)
    }
}

impl Tool for PatchFileTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "patch_file".to_string(),
            description: "Apply a batch of line-based edits to an existing file atomically. Requires file state to match the tracked baseline verified via MD5. Multiple sequential calls require re-reading the file after each patch to get fresh line numbers and MD5. All line numbers in a single call are interpreted against the original file before any edits, not incrementally, so operations cannot reference lines added or removed by other updates in the same batch. For multiple unrelated edits, batch them in a single call. Multiple calls require fresh file state from `read_file` to avoid MD5 mismatch failures. Pass a real structured object with `file_path` and `updates` fields, not a JSON-formatted string. Fails with 'MD5 mismatch' if the file changed externally or between calls, and with invalid input errors if updates are provided as JSON strings instead of raw array objects.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["file_path", "updates"],
                "additionalProperties": false,
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to an existing file, absolute or relative to the current working directory. The path must resolve against the same file state tracked by the current MD5 baseline. Provide the raw string path value, not a stringified JSON object."
                    },
                    "updates": {
                        "type": "array",
                        "description": "Batch of line-based update operations, all referenced against the original file before any edits are applied. Provide a real array of operation objects, not JSON-formatted text. Do not split unrelated edits across multiple calls unless you re-read the file first to refresh line numbers and MD5.",
                        "items": {
                            "oneOf": [
                                {
                                    "type": "object",
                                    "required": ["kind", "start", "end"],
                                    "additionalProperties": false,
                                    "properties": {
                                        "kind": { "const": "remove" },
                                        "start": { "type": "integer", "minimum": 1 },
                                        "end": { "type": "integer", "minimum": 1 }
                                    }
                                },
                                {
                                    "type": "object",
                                    "required": ["kind", "line", "content"],
                                    "additionalProperties": false,
                                    "properties": {
                                        "kind": { "const": "replace" },
                                        "line": { "type": "integer", "minimum": 1 },
                                        "content": { "type": "string" }
                                    }
                                },
                                {
                                    "type": "object",
                                    "required": ["kind", "after_line", "lines"],
                                    "additionalProperties": false,
                                    "properties": {
                                        "kind": { "const": "insert" },
                                        "after_line": { "type": "integer", "minimum": 0 },
                                        "lines": {
                                            "type": "array",
                                            "minItems": 1,
                                            "items": { "type": "string" }
                                        }
                                    }
                                }
                            ]
                        }
                    }
                }
            }),
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        if let Some(diff_preview) = output.get("diff_preview") {
            return diff_preview.to_string();
        }

        match (output["file_path"].as_str(), output["md5"].as_str()) {
            (Some(file_path), Some(checksum)) => {
                format!("File patched: {file_path} (MD5: {checksum})")
            }
            (Some(file_path), None) => format!("File patched: {file_path}"),
            _ => "File patched".to_string(),
        }
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        input: Value,
    ) -> Result<Value, ToolError> {
        let input: PatchFileInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;

        PatchFileTool::execute(self, ctx, &input.file_path, input.updates).await
    }
}

/// Apply a batch of updates atomically using original line numbers.
///
/// All line references are interpreted against the original file content, so
/// later operations do not shift the coordinates of earlier ones.
///
/// # Errors
/// Returns an error when an operation references an out-of-range line, when an
/// update is internally inconsistent, or when the same original line is both
/// removed and replaced.
///
/// # Panics
/// Does not panic.
///
/// # Examples
/// ```rust
/// use nerd::tools::patch_file::{PatchFile, apply_updates};
///
/// let content = "one\ntwo\nthree\n".to_string();
/// let updated = apply_updates(
///     content,
///     vec![
///         PatchFile::Replace {
///             line: 2,
///             content: "TWO".to_string(),
///         },
///         PatchFile::Insert {
///             after_line: 3,
///             lines: vec!["four".to_string()],
///         },
///     ],
/// )
/// .unwrap();
///
/// assert_eq!(updated, "one\nTWO\nthree\nfour\n");
/// ```
pub fn apply_updates(content: String, updates: Vec<PatchFile>) -> Result<String, String> {
    let ending = detect_line_ending(&content);
    let has_trailing = content.ends_with('\n') || content.ends_with('\r');
    let original = split_lines(&content);
    let line_count = original.len();
    let PlannedUpdates {
        removed,
        replacements,
        inserts,
    } = plan_updates(line_count, updates)?;

    let mut output = Vec::new();

    if let Some(lines) = inserts.get(&0) {
        output.extend(lines.iter().cloned());
    }

    for line_number in 1..=line_count {
        if !removed.contains(&line_number) {
            let line = replacements
                .get(&line_number)
                .cloned()
                .unwrap_or_else(|| original[line_number - 1].clone());
            output.push(line);
        }

        if let Some(lines) = inserts.get(&line_number) {
            output.extend(lines.iter().cloned());
        }
    }

    let mut result = output.join(ending);
    if has_trailing && !output.is_empty() {
        result.push_str(ending);
    }

    Ok(result)
}

fn plan_updates(line_count: usize, updates: Vec<PatchFile>) -> Result<PlannedUpdates, String> {
    let mut removed = HashSet::new();
    let mut replacements = HashMap::new();
    let mut inserts: HashMap<usize, Vec<String>> = HashMap::new();

    for update in updates {
        match update {
            PatchFile::Remove { start, end } => {
                if start < 1 {
                    return Err(format!("remove: start must be >= 1, got {start}"));
                }
                if end < start {
                    return Err(format!("remove: invalid range {start}..={end}"));
                }
                if end > line_count {
                    return Err(format!(
                        "remove: end {end} out of range (file has {line_count} lines)"
                    ));
                }
                for line in start..=end {
                    removed.insert(line);
                }
            }
            PatchFile::Replace {
                line,
                content: new_content,
            } => {
                if line < 1 || line > line_count {
                    return Err(format!(
                        "replace: line {line} out of range (file has {line_count} lines)"
                    ));
                }
                if replacements.contains_key(&line) {
                    return Err(format!("replace: duplicate operation for line {line}"));
                }
                replacements.insert(line, new_content);
            }
            PatchFile::Insert {
                after_line,
                lines: new_lines,
            } => {
                if after_line > line_count {
                    return Err(format!(
                        "insert: after_line {after_line} out of range (file has {line_count} lines)"
                    ));
                }
                if new_lines.is_empty() {
                    return Err("insert: requires at least one line".to_string());
                }
                inserts.entry(after_line).or_default().extend(new_lines);
            }
        }
    }

    for line in replacements.keys() {
        if removed.contains(line) {
            return Err(format!("line {line} is both removed and replaced"));
        }
    }

    Ok(PlannedUpdates {
        removed,
        replacements,
        inserts,
    })
}

fn build_diff_preview(
    file_path: &str,
    original_content: &str,
    updates: &[PatchFile],
) -> Result<PatchFileDiffPreview, String> {
    let original_lines = split_lines(original_content);
    let PlannedUpdates {
        removed,
        replacements,
        inserts,
    } = plan_updates(original_lines.len(), updates.to_vec())?;

    let mut all_rows = Vec::new();
    let mut right_line_number = 1usize;
    let mut added = 0usize;
    let mut removed_count = 0usize;
    let mut modified = 0usize;

    if let Some(lines) = inserts.get(&0) {
        for line in lines {
            all_rows.push(PatchFileDiffRow {
                change: PatchFileDiffChange::Added,
                left_line_number: None,
                right_line_number: Some(right_line_number),
                left_content: String::new(),
                right_content: line.clone(),
            });
            right_line_number += 1;
            added += 1;
        }
    }

    for original_line_number in 1..=original_lines.len() {
        let original_line = &original_lines[original_line_number - 1];
        if removed.contains(&original_line_number) {
            all_rows.push(PatchFileDiffRow {
                change: PatchFileDiffChange::Removed,
                left_line_number: Some(original_line_number),
                right_line_number: None,
                left_content: original_line.clone(),
                right_content: String::new(),
            });
            removed_count += 1;
        } else if let Some(replacement) = replacements.get(&original_line_number) {
            all_rows.push(PatchFileDiffRow {
                change: PatchFileDiffChange::Modified,
                left_line_number: Some(original_line_number),
                right_line_number: Some(right_line_number),
                left_content: original_line.clone(),
                right_content: replacement.clone(),
            });
            right_line_number += 1;
            modified += 1;
        } else {
            all_rows.push(PatchFileDiffRow {
                change: PatchFileDiffChange::Context,
                left_line_number: Some(original_line_number),
                right_line_number: Some(right_line_number),
                left_content: original_line.clone(),
                right_content: original_line.clone(),
            });
            right_line_number += 1;
        }

        if let Some(lines) = inserts.get(&original_line_number) {
            for line in lines {
                all_rows.push(PatchFileDiffRow {
                    change: PatchFileDiffChange::Added,
                    left_line_number: None,
                    right_line_number: Some(right_line_number),
                    left_content: String::new(),
                    right_content: line.clone(),
                });
                right_line_number += 1;
                added += 1;
            }
        }
    }

    let rows = select_context_rows(&all_rows, PATCH_FILE_CONTEXT_LINES);

    Ok(PatchFileDiffPreview {
        kind: "patch_file_diff",
        file_path: file_path.to_string(),
        summary: format!(
            "File patched: {file_path} (+{added} -{removed_count} ~{modified})"
        ),
        stats: PatchFileDiffStats {
            added,
            removed: removed_count,
            modified,
        },
        rows,
    })
}

fn select_context_rows(rows: &[PatchFileDiffRow], context_lines: usize) -> Vec<PatchFileDiffRow> {
    let changed_indexes: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            if matches!(row.change, PatchFileDiffChange::Added | PatchFileDiffChange::Removed | PatchFileDiffChange::Modified) {
                Some(index)
            } else {
                None
            }
        })
        .collect();

    if changed_indexes.is_empty() {
        return Vec::new();
    }

    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for index in changed_indexes {
        let start = index.saturating_sub(context_lines);
        let end = (index + context_lines + 1).min(rows.len());
        if let Some(last) = ranges.last_mut()
            && start <= last.1
        {
            last.1 = last.1.max(end);
        } else {
            ranges.push((start, end));
        }
    }

    let mut selected = Vec::new();
    for (range_index, (start, end)) in ranges.iter().copied().enumerate() {
        if range_index > 0 {
            selected.push(PatchFileDiffRow {
                change: PatchFileDiffChange::Omitted,
                left_line_number: None,
                right_line_number: None,
                left_content: "...".to_string(),
                right_content: "...".to_string(),
            });
        }
        selected.extend(rows[start..end].iter().cloned());
    }

    selected
}

fn patch_file_impl(
    file_path: &str,
    updates: Vec<PatchFile>,
    pre_execution_hook: Option<fn(&str, &str)>,
    callback: Option<fn(&str, usize) -> Result<(), String>>,
) -> Result<String, String> {
    if !Path::new(file_path).exists() {
        return Err(format!("File not found: {file_path}"));
    }

    let content = fs::read_to_string(file_path).map_err(|err| err.to_string())?;
    let checksum = format!("{:x}", md5::compute(&content));

    if let Some(pre_execution_hook) = pre_execution_hook {
        pre_execution_hook(file_path, &checksum);
    }

    let updated = apply_updates(content, updates)?;
    fs::write(file_path, &updated).map_err(|err| err.to_string())?;
    let line_count = split_lines(&updated).len();

    if let Some(callback) = callback {
        callback(file_path, line_count)?;
    }

    Ok(updated)
}

fn read_file_md5(file_path: &str) -> Result<String, String> {
    let content = fs::read_to_string(file_path).map_err(|err| err.to_string())?;
    Ok(format!("{:x}", md5::compute(content)))
}

fn split_lines(content: &str) -> Vec<String> {
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    let stripped = normalized.strip_suffix('\n').unwrap_or(&normalized);
    if stripped.is_empty() {
        Vec::new()
    } else {
        stripped.split('\n').map(String::from).collect()
    }
}

fn detect_line_ending(content: &str) -> &str {
    if content.contains("\r\n") {
        "\r\n"
    } else if content.contains('\n') {
        "\n"
    } else if content.contains('\r') {
        "\r"
    } else {
        "\n"
    }
}

#[cfg(test)]
mod tests {
    use super::{PatchFile, PatchFileDiffChange, PatchFileTool, apply_updates, build_diff_preview};
    use serde_json::json;
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{LazyLock, Mutex};
    use tempfile::NamedTempFile;
    use agentix::tooling::{Tool, ToolContext};

    static CALLBACK_CALLED: AtomicBool = AtomicBool::new(false);
    static PRE_HOOK_DATA: LazyLock<Mutex<Option<(String, String)>>> =
        LazyLock::new(|| Mutex::new(None));

    #[test]
    fn applies_mixed_updates_against_original_lines() {
        let result = apply_updates(
            "one\ntwo\nthree\nfour\n".to_string(),
            vec![
                PatchFile::Insert {
                    after_line: 0,
                    lines: vec!["zero".to_string()],
                },
                PatchFile::Replace {
                    line: 2,
                    content: "TWO".to_string(),
                },
                PatchFile::Remove { start: 3, end: 3 },
                PatchFile::Insert {
                    after_line: 4,
                    lines: vec!["five".to_string()],
                },
            ],
        )
        .unwrap();

        assert_eq!(result, "zero\none\nTWO\nfour\nfive\n");
    }

    #[test]
    fn preserves_crlf_endings() {
        let result = apply_updates(
            "one\r\ntwo\r\n".to_string(),
            vec![PatchFile::Insert {
                after_line: 2,
                lines: vec!["three".to_string()],
            }],
        )
        .unwrap();

        assert_eq!(result, "one\r\ntwo\r\nthree\r\n");
    }

    #[test]
    fn builds_structured_diff_preview_for_mixed_changes() {
        let preview = build_diff_preview(
            "src/main.rs",
            "one\ntwo\nthree\nfour\n",
            &[
                PatchFile::Insert {
                    after_line: 0,
                    lines: vec!["zero".to_string()],
                },
                PatchFile::Replace {
                    line: 2,
                    content: "TWO".to_string(),
                },
                PatchFile::Remove { start: 3, end: 3 },
                PatchFile::Insert {
                    after_line: 4,
                    lines: vec!["five".to_string()],
                },
            ],
        )
        .unwrap();

        assert_eq!(preview.kind, "patch_file_diff");
        assert_eq!(preview.file_path, "src/main.rs");
        assert_eq!(preview.stats.added, 2);
        assert_eq!(preview.stats.removed, 1);
        assert_eq!(preview.stats.modified, 1);
        assert_eq!(preview.rows.len(), 6);

        assert!(matches!(preview.rows[0].change, PatchFileDiffChange::Added));
        assert_eq!(preview.rows[0].right_line_number, Some(1));
        assert_eq!(preview.rows[0].right_content, "zero");

        assert!(matches!(preview.rows[1].change, PatchFileDiffChange::Context));
        assert_eq!(preview.rows[1].left_line_number, Some(1));
        assert_eq!(preview.rows[1].right_line_number, Some(2));
        assert_eq!(preview.rows[1].left_content, "one");

        assert!(matches!(preview.rows[2].change, PatchFileDiffChange::Modified));
        assert_eq!(preview.rows[2].left_line_number, Some(2));
        assert_eq!(preview.rows[2].right_line_number, Some(3));
        assert_eq!(preview.rows[2].left_content, "two");
        assert_eq!(preview.rows[2].right_content, "TWO");

        assert!(matches!(preview.rows[3].change, PatchFileDiffChange::Removed));
        assert_eq!(preview.rows[3].left_line_number, Some(3));
        assert_eq!(preview.rows[3].right_line_number, None);
        assert_eq!(preview.rows[3].left_content, "three");

        assert!(matches!(preview.rows[4].change, PatchFileDiffChange::Context));
        assert_eq!(preview.rows[4].left_line_number, Some(4));
        assert_eq!(preview.rows[4].right_line_number, Some(4));
        assert_eq!(preview.rows[4].left_content, "four");

        assert!(matches!(preview.rows[5].change, PatchFileDiffChange::Added));
        assert_eq!(preview.rows[5].right_line_number, Some(5));
        assert_eq!(preview.rows[5].right_content, "five");
    }

    #[test]
    fn diff_preview_collapses_distant_hunks_with_context() {
        let preview = build_diff_preview(
            "src/main.rs",
            "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n",
            &[
                PatchFile::Replace {
                    line: 2,
                    content: "two".to_string(),
                },
                PatchFile::Replace {
                    line: 15,
                    content: "eleven".to_string(),
                },
            ],
        )
        .unwrap();

        assert!(preview
            .rows
            .iter()
            .any(|row| matches!(row.change, PatchFileDiffChange::Omitted)));
        assert!(preview
            .rows
            .iter()
            .any(|row| row.left_line_number == Some(1) && matches!(row.change, PatchFileDiffChange::Context)));
        assert!(preview
            .rows
            .iter()
            .any(|row| row.left_line_number == Some(18) && matches!(row.change, PatchFileDiffChange::Context)));
    }

    #[test]
    fn rejects_remove_and_replace_on_same_line() {
        let error = apply_updates(
            "one\ntwo\n".to_string(),
            vec![
                PatchFile::Remove { start: 2, end: 2 },
                PatchFile::Replace {
                    line: 2,
                    content: "TWO".to_string(),
                },
            ],
        )
        .unwrap_err();

        assert_eq!(error, "line 2 is both removed and replaced");
    }

    #[test]
    fn rejects_out_of_range_insert() {
        let error = apply_updates(
            "one\ntwo\n".to_string(),
            vec![PatchFile::Insert {
                after_line: 3,
                lines: vec!["three".to_string()],
            }],
        )
        .unwrap_err();

        assert_eq!(error, "insert: after_line 3 out of range (file has 2 lines)");
    }

    #[test]
    fn patches_existing_file_on_disk() {
        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), "one\ntwo\nthree\n").unwrap();

        let updated = PatchFileTool
            .patch_file_impl(
                file.path().to_str().unwrap(),
                vec![
                    PatchFile::Replace {
                        line: 2,
                        content: "TWO".to_string(),
                    },
                    PatchFile::Insert {
                        after_line: 3,
                        lines: vec!["four".to_string()],
                    },
                ],
                None,
                None,
            )
            .unwrap();

        assert_eq!(updated, "one\nTWO\nthree\nfour\n");
        assert_eq!(fs::read_to_string(file.path()).unwrap(), updated);
    }

    #[test]
    fn invokes_callback_before_return() {
        fn callback(path: &str, line_count: usize) -> Result<(), String> {
            CALLBACK_CALLED.store(true, Ordering::SeqCst);
            assert!(path.ends_with(".tmp") || !path.is_empty());
            assert_eq!(line_count, 3);
            Ok(())
        }

        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), "one\ntwo\n").unwrap();
        CALLBACK_CALLED.store(false, Ordering::SeqCst);

        let updated = PatchFileTool
            .patch_file_impl(
                file.path().to_str().unwrap(),
                vec![
                    PatchFile::Replace {
                        line: 2,
                        content: "TWO".to_string(),
                    },
                    PatchFile::Insert {
                        after_line: 2,
                        lines: vec!["three".to_string()],
                    },
                ],
                None,
                Some(callback),
            )
            .unwrap();

        assert!(CALLBACK_CALLED.load(Ordering::SeqCst));
        assert_eq!(updated, "one\nTWO\nthree\n");
    }

    #[test]
    fn propagates_callback_error() {
        fn callback(_path: &str, _line_count: usize) -> Result<(), String> {
            Err("Callback failed".to_string())
        }

        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), "one\ntwo\n").unwrap();

        let error = PatchFileTool
            .patch_file_impl(
                file.path().to_str().unwrap(),
                vec![PatchFile::Replace {
                    line: 2,
                    content: "TWO".to_string(),
                }],
                None,
                Some(callback),
            )
            .unwrap_err();

        assert_eq!(error, "Callback failed");
    }

    #[test]
    fn invokes_pre_execution_hook_with_md5_of_original_file() {
        fn pre_hook(path: &str, md5: &str) {
            *PRE_HOOK_DATA.lock().unwrap() = Some((path.to_string(), md5.to_string()));
        }

        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), "one\ntwo\n").unwrap();
        let original = fs::read_to_string(file.path()).unwrap();
        let expected_md5 = format!("{:x}", md5::compute(&original));
        *PRE_HOOK_DATA.lock().unwrap() = None;

        let updated = PatchFileTool
            .patch_file_impl(
                file.path().to_str().unwrap(),
                vec![PatchFile::Replace {
                    line: 2,
                    content: "TWO".to_string(),
                }],
                Some(pre_hook),
                None,
            )
            .unwrap();

        let stored = PRE_HOOK_DATA.lock().unwrap().clone().unwrap();
        assert_eq!(stored.0, file.path().to_str().unwrap());
        assert_eq!(stored.1, expected_md5);
        assert_eq!(updated, "one\nTWO\n");
    }

    #[test]
    fn tool_execute_from_contract() {
        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), "one\ntwo\n").unwrap();

        let output = futures::executor::block_on(Tool::execute(
            &PatchFileTool,
            &ToolContext {
                execution_context: None,
                spawner: None,
            },
            json!({
                "file_path": file.path().to_str().unwrap(),
                "updates": [
                    {
                        "kind": "replace",
                        "line": 2,
                        "content": "TWO"
                    },
                    {
                        "kind": "insert",
                        "after_line": 2,
                        "lines": ["three"]
                    }
                ]
            }),
        ))
        .unwrap();

        assert_eq!(output["message"], "File patched: ".to_string() + file.path().to_str().unwrap());
        assert_eq!(output["file_path"], file.path().to_str().unwrap());
        assert_eq!(output["exists"], true);
        assert_eq!(
            output["md5"],
            format!("{:x}", md5::compute("one\nTWO\nthree\n"))
        );
        assert_eq!(output["line_count"], 3);
        assert_eq!(fs::read_to_string(file.path()).unwrap(), "one\nTWO\nthree\n");
    }
}

inventory::submit! {
    ToolRegistration {
        name: "patch_file",
        factory: || Arc::new(PatchFileTool::new()) as Arc<dyn ErasedTool>,
    }
}
