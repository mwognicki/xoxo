use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use super::types::{PatchFile, PlannedUpdates};

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

pub(crate) fn plan_updates(
    line_count: usize,
    updates: Vec<PatchFile>,
) -> Result<PlannedUpdates, String> {
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

pub(crate) fn patch_file_impl(
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

pub(crate) fn read_file_md5(file_path: &str) -> Result<String, String> {
    let content = fs::read_to_string(file_path).map_err(|err| err.to_string())?;
    Ok(format!("{:x}", md5::compute(content)))
}

pub(crate) fn split_lines(content: &str) -> Vec<String> {
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
