use super::*;

#[test]
fn chat_is_serde_ready() {
    let chat = Chat {
        title: Some("Example".to_string()),
        id: Uuid::nil(),
        created_at: Some("2026-04-01T00:00:00Z".to_string()),
        updated_at: Some("2026-04-01T00:00:00Z".to_string()),
        parent_chat_id: Some(Uuid::from_u128(1)),
        spawned_by_tool_call_id: Some(ChatToolCallId("tool-parent-1".to_string())),
        path: "chats/example.json".to_string(),
        agent: ChatAgent {
            id: Some(Uuid::nil()),
            name: Some("helper".to_string()),
            model: ModelConfig {
                model_name: "gpt-5.4".to_string(),
                provider: ApiProvider {
                    name: "OpenAI".to_string(),
                    compatibility: ApiCompatibility::OpenAiAndAnthropic,
                },
            },
            base_prompt: "You are helpful.".to_string(),
            allowed_tools: vec!["read_file".to_string(), "grep".to_string()],
            allowed_skills: vec!["rust-best-practices".to_string()],
        },
        observability: Some(CostObservability {
            model_name: Some("gpt-5.4".to_string()),
            provider_name: Some("OpenAI".to_string()),
            usage: TokenUsage {
                input_tokens: 1_200,
                output_tokens: 320,
                cached_input_tokens: 700,
                reasoning_tokens: 80,
                total_tokens: 2_300,
            },
            cost: CostBreakdown {
                input_cost_micros: MoneyAmount {
                    amount: 2_400,
                    currency: "USD".to_string(),
                },
                output_cost_micros: MoneyAmount {
                    amount: 3_200,
                    currency: "USD".to_string(),
                },
                cached_input_cost_micros: MoneyAmount {
                    amount: 350,
                    currency: "USD".to_string(),
                },
                reasoning_cost_micros: MoneyAmount {
                    amount: 0,
                    currency: "USD".to_string(),
                },
                total_cost_micros: MoneyAmount {
                    amount: 5_950,
                    currency: "USD".to_string(),
                },
            },
        }),
        active_branch_id: BranchId("main".to_string()),
        branches: vec![
            ChatBranch {
                id: BranchId("main".to_string()),
                name: "Main".to_string(),
                parent_branch_id: None,
                forked_from_message_id: None,
                head_message_id: Some(MessageId("evt-4".to_string())),
                active_snapshot_id: Some(SnapshotId("snap-1".to_string())),
            },
            ChatBranch {
                id: BranchId("fork-1".to_string()),
                name: "Alternative path".to_string(),
                parent_branch_id: Some(BranchId("main".to_string())),
                forked_from_message_id: Some(MessageId("msg-2".to_string())),
                head_message_id: None,
                active_snapshot_id: None,
            },
        ],
        snapshots: vec![ChatSnapshot {
            id: SnapshotId("snap-1".to_string()),
            branch_id: BranchId("main".to_string()),
            summary: "System prompt and early setup messages.".to_string(),
            replaces_messages_through_id: MessageId("msg-1".to_string()),
            observability: Some(CostObservability {
                model_name: Some("gpt-5.4".to_string()),
                provider_name: Some("OpenAI".to_string()),
                usage: TokenUsage {
                    input_tokens: 600,
                    output_tokens: 90,
                    cached_input_tokens: 0,
                    reasoning_tokens: 0,
                    total_tokens: 690,
                },
                cost: CostBreakdown {
                    input_cost_micros: MoneyAmount {
                        amount: 1_200,
                        currency: "USD".to_string(),
                    },
                    output_cost_micros: MoneyAmount {
                        amount: 900,
                        currency: "USD".to_string(),
                    },
                    cached_input_cost_micros: MoneyAmount {
                        amount: 0,
                        currency: "USD".to_string(),
                    },
                    reasoning_cost_micros: MoneyAmount {
                        amount: 0,
                        currency: "USD".to_string(),
                    },
                    total_cost_micros: MoneyAmount {
                        amount: 2_100,
                        currency: "USD".to_string(),
                    },
                },
            }),
        }],
        events: vec![
            ChatLogEntry {
                event: ChatEvent {
                    id: MessageId("msg-1".to_string()),
                    parent_id: None,
                    branch_id: BranchId("main".to_string()),
                    body: ChatEventBody::Message(ChatTextMessage {
                        role: ChatTextRole::System,
                        content: "You are helpful.".to_string(),
                    }),
                    observability: None,
                },
                context_state: MessageContextState::CompactedAway,
            },
            ChatLogEntry {
                event: ChatEvent {
                    id: MessageId("msg-2".to_string()),
                    parent_id: Some(MessageId("msg-1".to_string())),
                    branch_id: BranchId("main".to_string()),
                    body: ChatEventBody::Message(ChatTextMessage {
                        role: ChatTextRole::User,
                        content: "Summarize this file.".to_string(),
                    }),
                    observability: None,
                },
                context_state: MessageContextState::Active,
            },
            ChatLogEntry {
                event: ChatEvent {
                    id: MessageId("evt-3".to_string()),
                    parent_id: Some(MessageId("msg-2".to_string())),
                    branch_id: BranchId("main".to_string()),
                    body: ChatEventBody::ToolCall(ToolCallEvent::Started(ToolCallStarted {
                        tool_call_id: ChatToolCallId("tool-1".to_string()),
                        tool_name: "read_file".to_string(),
                        arguments: serde_json::json!({
                            "path": "src/main.rs",
                        }),
                        tool_call_kind: ToolCallKind::Generic,
                    })),
                    observability: Some(CostObservability {
                        model_name: None,
                        provider_name: None,
                        usage: TokenUsage::default(),
                        cost: CostBreakdown::default(),
                    }),
                },
                context_state: MessageContextState::Active,
            },
            ChatLogEntry {
                event: ChatEvent {
                    id: MessageId("evt-4".to_string()),
                    parent_id: Some(MessageId("evt-3".to_string())),
                    branch_id: BranchId("main".to_string()),
                    body: ChatEventBody::AppEvent(AppEvent::ModelChanged {
                        from: ModelConfig {
                            model_name: "gpt-5.4".to_string(),
                            provider: ApiProvider {
                                name: "OpenAI".to_string(),
                                compatibility: ApiCompatibility::OpenAi,
                            },
                        },
                        to: ModelConfig {
                            model_name: "claude-sonnet-4".to_string(),
                            provider: ApiProvider {
                                name: "Anthropic".to_string(),
                                compatibility: ApiCompatibility::Anthropic,
                            },
                        },
                    }),
                    observability: None,
                },
                context_state: MessageContextState::Active,
            },
        ],
    };

    let json = serde_json::to_string(&chat).unwrap();
    let back: Chat = serde_json::from_str(&json).unwrap();

    assert_eq!(back, chat);
    assert_eq!(back.path, "chats/example.json");
}

#[test]
fn chat_defaults_missing_timestamps_for_legacy_snapshots() {
    let json = serde_json::json!({
        "title": "Legacy",
        "id": "00000000-0000-0000-0000-000000000000",
        "parent_chat_id": null,
        "spawned_by_tool_call_id": null,
        "path": "chats/legacy.json",
        "agent": {
            "id": null,
            "name": "helper",
            "model": {
                "model_name": "gpt-5.4",
                "provider": {
                    "name": "OpenAI",
                    "compatibility": { "kind": "open_ai" }
                }
            },
            "base_prompt": "You are helpful.",
            "allowed_tools": [],
            "allowed_skills": []
        },
        "observability": null,
        "active_branch_id": "main",
        "branches": [],
        "snapshots": [],
        "events": []
    });

    let chat: Chat = serde_json::from_value(json).unwrap();

    assert_eq!(
        chat.created_at,
        Some("2026-04-01T00:00:00Z".to_string())
    );
    assert_eq!(
        chat.updated_at,
        Some("2026-04-01T00:00:00Z".to_string())
    );
}

#[test]
fn capability_catalog_is_serde_ready() {
    let catalog = CapabilityCatalog {
        tools: vec![ToolDescription {
            name: "read_file".to_string(),
            short_desc: "Reads a file from disk.".to_string(),
            is_mcp: false,
            is_global: true,
        }],
        skills: vec![SkillDescription {
            name: "rust-best-practices".to_string(),
            short_desc: "Rust coding conventions for this repo.".to_string(),
            is_global: true,
        }],
    };

    let json = serde_json::to_string(&catalog).unwrap();
    let back: CapabilityCatalog = serde_json::from_str(&json).unwrap();

    assert_eq!(back, catalog);
}

#[test]
fn tool_call_message_is_serde_ready() {
    let message = ChatEvent {
        id: MessageId("evt-tool-1".to_string()),
        parent_id: Some(MessageId("msg-2".to_string())),
        branch_id: BranchId("main".to_string()),
        body: ChatEventBody::ToolCall(ToolCallEvent::Completed(ToolCallCompleted {
            tool_call_id: ChatToolCallId("tool-1".to_string()),
            tool_name: "read_file".to_string(),
            result_preview: "Read 128 lines".to_string(),
        })),
        observability: Some(CostObservability {
            model_name: None,
            provider_name: None,
            usage: TokenUsage::default(),
            cost: CostBreakdown::default(),
        }),
    };

    let json = serde_json::to_string(&message).unwrap();
    let back: ChatEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(back, message);
}

#[test]
fn started_tool_call_with_kind_is_serde_ready() {
    let event = ChatEvent {
        id: MessageId("evt-tool-started-1".to_string()),
        parent_id: Some(MessageId("msg-2".to_string())),
        branch_id: BranchId("main".to_string()),
        body: ChatEventBody::ToolCall(ToolCallEvent::Started(ToolCallStarted {
            tool_call_id: ChatToolCallId("tool-spawn-1".to_string()),
            tool_name: "spawn_subagent".to_string(),
            arguments: serde_json::json!({
                "task": "Inspect src/main.rs",
                "tools": ["read_file"],
            }),
            tool_call_kind: ToolCallKind::SpawnSubagent {
                child_chat_id: Uuid::from_u128(2),
                child_path: ChatPath(vec![Uuid::from_u128(1), Uuid::from_u128(2)]),
                spec_summary: "Inspect src/main.rs with read-only tools".to_string(),
            },
        })),
        observability: Some(CostObservability {
            model_name: None,
            provider_name: None,
            usage: TokenUsage::default(),
            cost: CostBreakdown::default(),
        }),
    };

    let json = serde_json::to_string(&event).unwrap();
    let back: ChatEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(back, event);
}

#[test]
fn model_change_app_event_is_serde_ready() {
    let event = ChatEvent {
        id: MessageId("evt-model-1".to_string()),
        parent_id: Some(MessageId("msg-2".to_string())),
        branch_id: BranchId("main".to_string()),
        body: ChatEventBody::AppEvent(AppEvent::ModelChanged {
            from: ModelConfig {
                model_name: "gpt-5.4".to_string(),
                provider: ApiProvider {
                    name: "OpenAI".to_string(),
                    compatibility: ApiCompatibility::OpenAi,
                },
            },
            to: ModelConfig {
                model_name: "claude-sonnet-4".to_string(),
                provider: ApiProvider {
                    name: "Anthropic".to_string(),
                    compatibility: ApiCompatibility::Anthropic,
                },
            },
        }),
        observability: None,
    };

    let json = serde_json::to_string(&event).unwrap();
    let back: ChatEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(back, event);
}
