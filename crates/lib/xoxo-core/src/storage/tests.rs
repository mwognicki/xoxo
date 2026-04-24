#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::chat::structs::{
        ApiCompatibility, ApiProvider, BranchId, Chat, ChatAgent, ChatBranch, ChatEvent,
        ChatEventBody, ChatLogEntry, ChatToolCallId, MessageContextState, MessageId, ModelConfig,
        ToolCallEvent, ToolCallKind, ToolCallStarted,
    };

    use super::super::api::Storage;
    use super::super::helpers::CHATS_TREE;
    use super::super::types::ChatSessionSummary;

    fn sample_chat(chat_id: Uuid) -> Chat {
        Chat {
            title: Some("Example".to_string()),
            id: chat_id,
            created_at: Some("2026-04-01T00:00:00Z".to_string()),
            updated_at: Some("2026-04-01T00:00:00Z".to_string()),
            parent_chat_id: None,
            spawned_by_tool_call_id: None,
            path: format!("chats/{chat_id}.json"),
            agent: ChatAgent {
                id: None,
                name: Some("nerd".to_string()),
                model: ModelConfig {
                    model_name: "gpt-5.4".to_string(),
                    provider: ApiProvider {
                        name: "OpenAI".to_string(),
                        compatibility: ApiCompatibility::OpenAi,
                    },
                },
                base_prompt: "You are helpful.".to_string(),
                allowed_tools: Vec::new(),
                allowed_skills: Vec::new(),
            },
            observability: None,
            active_branch_id: BranchId("main".to_string()),
            branches: vec![ChatBranch {
                id: BranchId("main".to_string()),
                name: "Main".to_string(),
                parent_branch_id: None,
                forked_from_message_id: None,
                head_message_id: None,
                active_snapshot_id: None,
            }],
            snapshots: Vec::new(),
            events: Vec::new(),
        }
    }

    fn sample_chat_with_timestamp(chat_id: Uuid, updated_at: &str, model_name: &str) -> Chat {
        let mut chat = sample_chat(chat_id);
        chat.updated_at = Some(updated_at.to_string());
        chat.agent.model.model_name = model_name.to_string();
        chat
    }

    fn sample_started_tool_chat(chat_id: Uuid) -> Chat {
        let mut chat = sample_chat(chat_id);
        chat.events.push(ChatLogEntry {
            event: ChatEvent {
                id: MessageId("evt-tool-started-1".to_string()),
                parent_id: None,
                branch_id: BranchId("main".to_string()),
                body: ChatEventBody::ToolCall(ToolCallEvent::Started(ToolCallStarted {
                    tool_call_id: ChatToolCallId("tool-1".to_string()),
                    tool_name: "find_patterns".to_string(),
                    arguments: serde_json::json!({ "pattern": "sled" }),
                    tool_call_kind: ToolCallKind::Generic,
                })),
                observability: None,
            },
            context_state: MessageContextState::Active,
        });
        chat.branches[0].head_message_id = Some(MessageId("evt-tool-started-1".to_string()));
        chat
    }

    #[test]
    fn open_at_creates_storage_directory() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("data");

        let storage = Storage::open_at(&path).expect("storage");

        assert_eq!(storage.path(), path.as_path());
        assert!(path.exists());
    }

    #[test]
    fn save_chat_round_trips_chat_snapshot() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(tempdir.path().join("data")).expect("storage");
        let chat = sample_chat(Uuid::new_v4());

        storage.save_chat(&chat).expect("save chat");

        let loaded = storage
            .load_chat(chat.id)
            .expect("load chat")
            .expect("stored chat");
        let mut expected = chat;
        expected.updated_at = loaded.updated_at.clone();
        assert_eq!(loaded, expected);
    }

    #[test]
    fn last_used_chat_id_round_trips() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(tempdir.path().join("data")).expect("storage");
        let chat_id = Uuid::new_v4();

        storage
            .set_last_used_chat_id(chat_id)
            .expect("set last used chat id");

        let loaded = storage.last_used_chat_id().expect("load last used chat id");
        assert_eq!(loaded, Some(chat_id));
    }

    #[test]
    fn purge_removes_storage_directory() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("data");
        let storage = Storage::open_at(&path).expect("storage");

        storage.purge().expect("purge storage");

        assert!(!path.exists());
    }

    #[test]
    fn save_chat_uses_tool_call_kind_field_name() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(tempdir.path().join("data")).expect("storage");
        let chat = sample_started_tool_chat(Uuid::new_v4());

        storage.save_chat(&chat).expect("save chat");

        let raw = storage.load_raw_chat(chat.id).expect("load raw chat");
        let raw = raw.expect("raw chat");
        assert!(raw.contains("\"tool_call_kind\":{\"kind\":\"generic\"}"));
        assert!(!raw.contains(",\"kind\":{\"kind\":\"generic\"}"));
    }

    #[test]
    fn list_chat_sessions_returns_recent_sessions_first() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(tempdir.path().join("data")).expect("storage");
        let older = sample_chat_with_timestamp(Uuid::new_v4(), "2026-04-01T00:00:00Z", "gpt-5.2");
        let newer = sample_chat_with_timestamp(Uuid::new_v4(), "2026-04-02T00:00:00Z", "gpt-5.4");
        let chats = storage.db().open_tree(CHATS_TREE).expect("chats tree");

        for chat in [&older, &newer] {
            let raw = serde_json::to_vec(chat).expect("serialize chat");
            chats
                .insert(chat.id.to_string().as_bytes(), raw)
                .expect("insert chat");
        }

        let sessions = storage.list_chat_sessions().expect("list chat sessions");

        assert_eq!(
            sessions,
            vec![
                ChatSessionSummary {
                    id: newer.id,
                    updated_at: newer.updated_at.clone(),
                    model_name: "gpt-5.4".to_string(),
                },
                ChatSessionSummary {
                    id: older.id,
                    updated_at: older.updated_at.clone(),
                    model_name: "gpt-5.2".to_string(),
                },
            ]
        );
    }

    #[test]
    fn load_chat_repairs_legacy_duplicate_kind_payload() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open_at(tempdir.path().join("data")).expect("storage");
        let chat = sample_started_tool_chat(Uuid::new_v4());
        let chats = storage.db().open_tree(CHATS_TREE).expect("chats tree");

        let broken_raw = serde_json::to_string(&chat)
            .expect("serialize chat")
            .replace(
                "\"tool_call_kind\":{\"kind\":\"generic\"}",
                "\"kind\":{\"kind\":\"generic\"}",
            );
        chats
            .insert(chat.id.to_string().as_bytes(), broken_raw.as_bytes())
            .expect("insert broken raw chat");

        let loaded = storage.load_chat(chat.id).expect("load repaired chat");
        assert_eq!(loaded, Some(chat));
    }
}
