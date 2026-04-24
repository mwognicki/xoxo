use super::apply::{plan_updates, split_lines};
use super::types::{
    PatchFile, PatchFileDiffChange, PatchFileDiffPreview, PatchFileDiffRow, PatchFileDiffStats,
    PlannedUpdates,
};

const PATCH_FILE_CONTEXT_LINES: usize = 4;

pub(crate) fn build_diff_preview(
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
        summary: format!("File patched: {file_path} (+{added} -{removed_count} ~{modified})"),
        stats: PatchFileDiffStats {
            added,
            removed: removed_count,
            modified,
        },
        rows,
    })
}

fn select_context_rows(
    rows: &[PatchFileDiffRow],
    context_lines: usize,
) -> Vec<PatchFileDiffRow> {
    let changed_indexes: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            if matches!(
                row.change,
                PatchFileDiffChange::Added
                    | PatchFileDiffChange::Removed
                    | PatchFileDiffChange::Modified
            ) {
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
