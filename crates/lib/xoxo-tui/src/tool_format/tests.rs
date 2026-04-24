use super::*;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use uuid::Uuid;

use crate::app::{App, LayoutMode};

fn test_app() -> App {
    App {
        running: true,
        layout: LayoutMode::Main,
        input: String::new(),
        active_chat_id: Some(Uuid::new_v4()),
        pending_submission: None,
        current_provider_name: "test-provider".to_string(),
        current_model_name: "test-model".to_string(),
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_used_tokens: 0,
        context_left_percent: None,
        max_input_tokens: None,
        estimated_cost_usd: None,
        history: Vec::new(),
        in_flight_text: HashMap::new(),
        in_flight_thinking: HashMap::new(),
        conversation_scroll_from_bottom: 0,
        modal: None,
        mention_popup: None,
        workspace_root: PathBuf::from("."),
        ctrl_c_count: 0,
        started_at: Instant::now(),
        turn_in_progress: false,
        last_turn_finish_reason: None,
        mouse_capture_enabled: true,
        storage: None,
        conversation_version: 0,
        cached_conversation: RefCell::new(None),
    }
}

#[test]
fn patch_file_preview_renders_side_by_side_diff() {
    let app = test_app();
    let completed = ToolCallCompleted {
        tool_call_id: xoxo_core::chat::structs::ChatToolCallId("tool-1".to_string()),
        tool_name: "patch_file".to_string(),
        result_preview: serde_json::json!({
            "kind": "patch_file_diff",
            "file_path": "src/main.rs",
            "summary": "File patched: src/main.rs (+1 -1 ~1)",
            "stats": { "added": 1, "removed": 1, "modified": 1 },
            "rows": [
                {
                    "change": "removed",
                    "left_line_number": 3,
                    "right_line_number": null,
                    "left_content": "old line",
                    "right_content": ""
                },
                {
                    "change": "modified",
                    "left_line_number": 7,
                    "right_line_number": 7,
                    "left_content": "before",
                    "right_content": "after"
                },
                {
                    "change": "added",
                    "left_line_number": null,
                    "right_line_number": 10,
                    "left_content": "",
                    "right_content": "new line"
                }
            ]
        })
        .to_string(),
    };

    let lines = format_completed(&app, &completed, 80);

    assert_eq!(lines.len(), 5);
    assert!(lines[0].spans[0].content.contains("src/main.rs"));
    assert_eq!(lines[1].width(), 79);
    assert!(lines[2].spans[1].content.starts_with("   3 - "));
    assert_eq!(lines[2].spans[1].style.bg, Some(Color::Indexed(52)));
    assert_eq!(lines[2].spans[3].style.bg, None);
    assert!(lines[3].spans[1].content.starts_with("   7 ~ "));
    assert!(lines[3].spans[3].content.starts_with("   7 ~ "));
    assert_eq!(lines[3].spans[1].style.bg, Some(Color::Indexed(24)));
    assert_eq!(lines[3].spans[3].style.bg, Some(Color::Indexed(24)));
    assert!(lines[4].spans[3].content.starts_with("  10 + "));
    assert_eq!(lines[4].spans[1].style.bg, None);
    assert_eq!(lines[4].spans[3].style.bg, Some(Color::Indexed(22)));
}

#[test]
fn patch_file_preview_renders_context_and_omitted_rows() {
    let app = test_app();
    let completed = ToolCallCompleted {
        tool_call_id: xoxo_core::chat::structs::ChatToolCallId("tool-1".to_string()),
        tool_name: "patch_file".to_string(),
        result_preview: serde_json::json!({
            "kind": "patch_file_diff",
            "file_path": "src/main.rs",
            "summary": "File patched: src/main.rs (+1 -0 ~1)",
            "stats": { "added": 1, "removed": 0, "modified": 1 },
            "rows": [
                {
                    "change": "context",
                    "left_line_number": 1,
                    "right_line_number": 1,
                    "left_content": "use std::fmt;",
                    "right_content": "use std::fmt;"
                },
                {
                    "change": "modified",
                    "left_line_number": 2,
                    "right_line_number": 2,
                    "left_content": "old()",
                    "right_content": "new()"
                },
                {
                    "change": "omitted",
                    "left_line_number": null,
                    "right_line_number": null,
                    "left_content": "...",
                    "right_content": "..."
                },
                {
                    "change": "added",
                    "left_line_number": null,
                    "right_line_number": 30,
                    "left_content": "",
                    "right_content": "inserted()"
                }
            ]
        })
        .to_string(),
    };

    let lines = format_completed(&app, &completed, 80);

    assert!(lines[2].spans[1].content.starts_with("   1   "));
    assert_eq!(lines[2].spans[1].style.bg, None);
    assert_eq!(lines[2].spans[3].style.bg, None);
    assert!(lines[3].spans[1].content.starts_with("   2 ~ "));
    assert!(lines[3].spans[3].content.starts_with("   2 ~ "));
    assert_eq!(lines[3].spans[1].style.bg, Some(Color::Indexed(24)));
    assert_eq!(lines[3].spans[3].style.bg, Some(Color::Indexed(24)));
    assert_eq!(lines[4].spans[1].style.bg, None);
    assert_eq!(lines[4].spans[3].style.bg, None);
    assert_eq!(lines[5].spans[1].style.bg, None);
    assert!(lines[5].spans[3].content.starts_with("  30 + "));
    assert_eq!(lines[5].spans[3].style.bg, Some(Color::Indexed(22)));
}

#[test]
fn legacy_patch_file_preview_falls_back_to_default_lines() {
    let app = test_app();
    let completed = ToolCallCompleted {
        tool_call_id: xoxo_core::chat::structs::ChatToolCallId("tool-1".to_string()),
        tool_name: "patch_file".to_string(),
        result_preview: "File patched: src/main.rs".to_string(),
    };

    let lines = format_completed(&app, &completed, 80);

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].spans[0].content, "└ File patched: src/main.rs");
}

#[test]
fn write_file_preview_renders_file_contents() {
    let app = test_app();
    let completed = ToolCallCompleted {
        tool_call_id: xoxo_core::chat::structs::ChatToolCallId("tool-2".to_string()),
        tool_name: "write_file".to_string(),
        result_preview: serde_json::json!({
            "kind": "write_file_preview",
            "file_path": "src/main.rs",
            "content": "fn main() {\n    println!(\"hi\");\n}\n"
        })
        .to_string(),
    };

    let lines = format_completed(&app, &completed, 80);

    assert_eq!(lines.len(), 4);
    assert!(lines[0].spans[0].content.contains("src/main.rs"));
    assert_eq!(lines[1].spans[0].content, "  ");
    assert!(lines[1].spans.iter().any(|span| span.content.contains("fn")));
    assert!(lines[2]
        .spans
        .iter()
        .any(|span| span.content.contains("println!")));
}

#[test]
fn legacy_write_file_preview_falls_back_to_default_lines() {
    let app = test_app();
    let completed = ToolCallCompleted {
        tool_call_id: xoxo_core::chat::structs::ChatToolCallId("tool-2".to_string()),
        tool_name: "write_file".to_string(),
        result_preview: "File saved: src/main.rs".to_string(),
    };

    let lines = format_completed(&app, &completed, 80);

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].spans[0].content, "└ File saved: src/main.rs");
}

#[test]
fn write_todo_list_preview_renders_task_snapshot() {
    let app = test_app();
    let completed = ToolCallCompleted {
        tool_call_id: xoxo_core::chat::structs::ChatToolCallId("tool-3".to_string()),
        tool_name: "write_todo_list".to_string(),
        result_preview: serde_json::json!({
            "kind": "write_todo_list_preview",
            "action": "updated",
            "task_count": 4,
            "tasks": [
                {
                    "id": "task_1",
                    "content": "Inspect the current registry",
                    "priority": "high",
                    "state": "completed"
                },
                {
                    "id": "task_2",
                    "content": "Add the tool formatter",
                    "priority": "high",
                    "state": "in_progress"
                },
                {
                    "id": "task_3",
                    "content": "Write focused tests",
                    "priority": "medium",
                    "state": "pending"
                },
                {
                    "id": "task_4",
                    "content": "Drop the stale idea",
                    "priority": "low",
                    "state": "cancelled"
                }
            ]
        })
        .to_string(),
    };

    let lines = format_completed(&app, &completed, 80);

    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0].spans[0].content, "└ updated todo list");
    assert_eq!(lines[1].spans[1].content, "▣ ");
    assert_eq!(lines[2].spans[1].content, "◧ ");
    assert_eq!(lines[3].spans[1].content, "□ ");
    assert_eq!(lines[4].spans[1].content, "⊠ ");
    assert_eq!(lines[2].spans[2].content, "[high] ");
    assert_eq!(lines[3].spans[2].content, "[medium] ");
    assert!(lines[2].spans[3].content.contains("Add the tool formatter"));
}

#[test]
fn legacy_write_todo_list_preview_falls_back_to_default_lines() {
    let app = test_app();
    let completed = ToolCallCompleted {
        tool_call_id: xoxo_core::chat::structs::ChatToolCallId("tool-3".to_string()),
        tool_name: "write_todo_list".to_string(),
        result_preview: "updated todo list with 2 task(s)".to_string(),
    };

    let lines = format_completed(&app, &completed, 80);

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].spans[0].content, "└ updated todo list with 2 task(s)");
}
