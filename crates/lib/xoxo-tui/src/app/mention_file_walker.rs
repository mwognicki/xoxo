//! State for the `@`-mention file/directory picker popup.
//!
//! The popup is opened when the user types `@` in the input, walks the
//! workspace root once (honoring `.gitignore` via the `ignore` crate), and
//! exposes a live-filtered view capped at [`MAX_VISIBLE`] entries. Tab or
//! Enter commits the highlighted entry; Esc closes the popup.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use ignore::WalkBuilder;

/// Maximum number of entries shown in the popup at a time.
pub const MAX_VISIBLE: usize = 8;

/// A single candidate path that can be picked via the `@`-mention popup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileWalkerMentionEntry {
    /// Path relative to the workspace root, using forward slashes.
    pub rel_path: String,
    /// Whether the entry refers to a directory (rendered with a trailing `/`).
    pub is_dir: bool,
}

/// Live state for the `@`-mention picker popup.
pub struct FileWalkerMentionPopup {
    /// Byte index in `App::input` where the trigger `@` was typed.
    pub trigger_at: usize,
    /// Lowercased filter derived from the characters typed after `@`.
    filter: String,
    /// All candidate entries from the initial workspace walk.
    all_entries: Arc<[FileWalkerMentionEntry]>,
    /// Indices into `all_entries` that match the current filter, capped at
    /// [`MAX_VISIBLE`].
    visible: Vec<usize>,
    /// Index into `visible` of the currently highlighted row.
    selected: usize,
}

impl FileWalkerMentionPopup {
    /// Opens a new popup anchored at `trigger_at` in the input buffer, eagerly
    /// walking the workspace rooted at `workspace_root`.
    ///
    /// # Errors
    /// Returns an error only if the workspace walk reports a fatal I/O error
    /// at the root; per-entry walk errors are skipped silently.
    pub fn open(workspace_root: &Path, trigger_at: usize) -> Result<Self> {
        let all_entries = walk_workspace(workspace_root);
        let mut popup = Self {
            trigger_at,
            filter: String::new(),
            all_entries: Arc::from(all_entries),
            visible: Vec::with_capacity(MAX_VISIBLE),
            selected: 0,
        };
        popup.recompute_visible();
        Ok(popup)
    }

    /// Builds a popup from a pre-supplied list of entries. Intended for tests
    /// that need deterministic candidates without touching the filesystem.
    #[cfg(test)]
    pub fn with_entries(trigger_at: usize, entries: Vec<FileWalkerMentionEntry>) -> Self {
        let mut popup = Self {
            trigger_at,
            filter: String::new(),
            all_entries: Arc::from(entries),
            visible: Vec::with_capacity(MAX_VISIBLE),
            selected: 0,
        };
        popup.recompute_visible();
        popup
    }

    /// Updates the filter and refreshes the visible row set.
    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_lowercase();
        self.recompute_visible();
    }

    /// Moves the highlight up one row, bounded at zero.
    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Moves the highlight down one row, bounded at the last visible row.
    pub fn select_next(&mut self) {
        if self.selected + 1 < self.visible.len() {
            self.selected += 1;
        }
    }

    /// Returns the currently highlighted entry, if any.
    pub fn selected_entry(&self) -> Option<&FileWalkerMentionEntry> {
        let index = *self.visible.get(self.selected)?;
        self.all_entries.get(index)
    }

    /// Returns the visible entries in display order.
    pub fn visible_entries(&self) -> impl Iterator<Item = &FileWalkerMentionEntry> {
        self.visible
            .iter()
            .filter_map(|&index| self.all_entries.get(index))
    }

    /// Returns the index of the highlighted row within the visible list.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Returns the current filter string (already lowercased).
    #[cfg(test)]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    fn recompute_visible(&mut self) {
        self.visible.clear();
        for (index, entry) in self.all_entries.iter().enumerate() {
            if self.visible.len() >= MAX_VISIBLE {
                break;
            }
            if self.filter.is_empty() || entry.rel_path.to_lowercase().contains(&self.filter) {
                self.visible.push(index);
            }
        }
        if self.visible.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.visible.len() {
            self.selected = self.visible.len() - 1;
        }
    }
}

fn walk_workspace(root: &Path) -> Vec<FileWalkerMentionEntry> {
    let mut entries = Vec::new();
    for result in WalkBuilder::new(root).standard_filters(true).build() {
        let Ok(entry) = result else { continue };
        // Skip the root itself.
        if entry.depth() == 0 {
            continue;
        }
        let path = entry.path();
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let rel_path = relative.to_string_lossy().replace('\\', "/");
        if rel_path.is_empty() {
            continue;
        }
        let is_dir = entry
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false);
        entries.push(FileWalkerMentionEntry { rel_path, is_dir });
    }
    entries
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    fn sample_entries() -> Vec<FileWalkerMentionEntry> {
        vec![
            FileWalkerMentionEntry {
                rel_path: "SRC/lib.rs".to_string(),
                is_dir: false,
            },
            FileWalkerMentionEntry {
                rel_path: "src/app.rs".to_string(),
                is_dir: false,
            },
            FileWalkerMentionEntry {
                rel_path: "docs/readme.md".to_string(),
                is_dir: false,
            },
            FileWalkerMentionEntry {
                rel_path: "crates".to_string(),
                is_dir: true,
            },
        ]
    }

    #[test]
    fn filter_substring_is_case_insensitive() {
        let mut popup = FileWalkerMentionPopup::with_entries(0, sample_entries());
        popup.set_filter("SRC");
        let paths: Vec<&str> = popup
            .visible_entries()
            .map(|entry| entry.rel_path.as_str())
            .collect();
        assert_eq!(paths, vec!["SRC/lib.rs", "src/app.rs"]);
    }

    #[test]
    fn filter_truncates_to_max_visible() {
        let entries = (0..20)
            .map(|index| FileWalkerMentionEntry {
                rel_path: format!("match-{index}.rs"),
                is_dir: false,
            })
            .collect();
        let mut popup = FileWalkerMentionPopup::with_entries(0, entries);
        popup.set_filter("match");
        assert_eq!(popup.visible_entries().count(), MAX_VISIBLE);
    }

    #[test]
    fn empty_filter_shows_first_entries() {
        let popup = FileWalkerMentionPopup::with_entries(0, sample_entries());
        assert_eq!(popup.visible_entries().count(), 4);
    }

    #[test]
    fn select_next_bounded() {
        let mut popup = FileWalkerMentionPopup::with_entries(0, sample_entries());
        for _ in 0..10 {
            popup.select_next();
        }
        assert_eq!(popup.selected_index(), 3);
    }

    #[test]
    fn select_prev_bounded() {
        let mut popup = FileWalkerMentionPopup::with_entries(0, sample_entries());
        popup.select_next();
        popup.select_prev();
        popup.select_prev();
        assert_eq!(popup.selected_index(), 0);
    }

    #[test]
    fn selected_clamps_when_filter_narrows() {
        let mut popup = FileWalkerMentionPopup::with_entries(0, sample_entries());
        popup.select_next();
        popup.select_next();
        popup.select_next();
        assert_eq!(popup.selected_index(), 3);
        popup.set_filter("src");
        assert_eq!(popup.visible_entries().count(), 2);
        assert_eq!(popup.selected_index(), 1);
    }

    #[test]
    fn select_bounds_respect_visible_cap() {
        // There must be more than MAX_VISIBLE matches so the cap is exercised.
        let entries = (0..(MAX_VISIBLE + 5))
            .map(|index| FileWalkerMentionEntry {
                rel_path: format!("entry-{index}.rs"),
                is_dir: false,
            })
            .collect();
        let mut popup = FileWalkerMentionPopup::with_entries(0, entries);
        for _ in 0..50 {
            popup.select_next();
        }
        assert_eq!(popup.selected_index(), MAX_VISIBLE - 1);
    }

    #[test]
    fn walk_respects_gitignore() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        // `ignore` only honors `.gitignore` when the walk root is inside a git
        // repo, so initialize one. An empty `.git/` directory with a HEAD file
        // is enough to mark the root as a repo for the `ignore` crate.
        fs::create_dir(root.join(".git")).expect("git dir");
        fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").expect("HEAD file");
        fs::write(root.join(".gitignore"), "secret.txt\ntarget\n").expect("gitignore");
        fs::write(root.join("visible.txt"), "").expect("visible file");
        fs::write(root.join("secret.txt"), "").expect("secret file");
        fs::create_dir(root.join("target")).expect("target dir");
        fs::write(root.join("target/build.log"), "").expect("target file");

        let popup = FileWalkerMentionPopup::open(root, 0).expect("open popup");
        let paths: Vec<&str> = popup
            .visible_entries()
            .map(|entry| entry.rel_path.as_str())
            .collect();

        assert!(paths.contains(&"visible.txt"));
        assert!(!paths.iter().any(|path| path.contains("secret.txt")));
        assert!(!paths.iter().any(|path| path.starts_with("target")));
    }
}
