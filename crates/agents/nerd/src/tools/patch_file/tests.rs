#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{LazyLock, Mutex};

    use agentix::tooling::{Tool, ToolContext};
    use serde_json::json;
    use tempfile::NamedTempFile;

    use super::super::apply::{patch_file_impl, read_file_md5};
    use super::super::apply_updates;
    use super::super::diff_preview::build_diff_preview;
    use super::super::tool::PatchFileTool;
    use super::super::types::PatchFileDiffChange;
    use super::super::PatchFile;

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
        assert!(preview.rows.iter().any(|row| {
            row.left_line_number == Some(1) && matches!(row.change, PatchFileDiffChange::Context)
        }));
        assert!(preview.rows.iter().any(|row| {
            row.left_line_number == Some(18) && matches!(row.change, PatchFileDiffChange::Context)
        }));
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

        let updated = patch_file_impl(
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

        let updated = patch_file_impl(
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

        let error = patch_file_impl(
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
        let expected_md5 = read_file_md5(file.path().to_str().unwrap()).unwrap();
        *PRE_HOOK_DATA.lock().unwrap() = None;

        let updated = patch_file_impl(
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

        assert_eq!(
            output["message"],
            "File patched: ".to_string() + file.path().to_str().unwrap()
        );
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
