use super::*;
use crate::chat::structs::*;

use uuid::Uuid;

#[test]
fn user_facing_chat_shows_active_branch_without_history_or_internal_events() {
    let chat = sample_chat(Some(CostObservability {
        model_name: Some("gpt-5.4".to_string()),
        provider_name: Some("OpenAI".to_string()),
        usage: TokenUsage {
            input_tokens: 10,
            output_tokens: 20,
            cached_input_tokens: 5,
            reasoning_tokens: 1,
            total_tokens: 36,
        },
        cost: CostBreakdown::default(),
    }));

    let view = to_user_facing_chat(&chat);

    assert_eq!(view.current_model_name, "claude-sonnet-4");
    assert_eq!(view.total_used_tokens, 36);
    assert_eq!(
        view.parent_branch,
        Some(UserFacingParentBranch {
            branch_id: "main".to_string(),
            branch_name: "Main".to_string(),
            forked_from_message_id: Some("msg-2".to_string()),
        })
    );
    assert_eq!(
        view.compaction,
        Some(UserFacingCompaction {
            snapshot_id: "snap-fork".to_string(),
        })
    );
    assert_eq!(
        view.messages,
        vec![
            UserFacingMessage {
                id: "msg-2".to_string(),
                role: UserFacingMessageRole::User,
                content: "Summarize this file.".to_string(),
            },
            UserFacingMessage {
                id: "msg-3".to_string(),
                role: UserFacingMessageRole::Agent,
                content: "Here is the summary.".to_string(),
            },
        ]
    );
    assert_eq!(
        view.tool_calls,
        vec![UserFacingToolCall::Completed {
            tool_call_id: "tool-1".to_string(),
            tool_name: "read_file".to_string(),
            result_preview: "Read 128 lines".to_string(),
        }]
    );
}

#[test]
fn user_facing_chat_falls_back_to_summed_tokens_when_chat_total_is_missing() {
    let chat = sample_chat(None);

    let view = to_user_facing_chat(&chat);

    assert_eq!(view.total_used_tokens, 24);
}

fn sample_chat(observability: Option<CostObservability>) -> Chat {
    Chat {
        title: Some("Example".to_string()),
        id: Uuid::nil(),
        parent_chat_id: None,
        spawned_by_tool_call_id: None,
        path: "chats/example.json".to_string(),
        agent: ChatAgent {
            id: Some(Uuid::nil()),
            name: Some("helper".to_string()),
            model: ModelConfig {
                model_name: "gpt-5.4".to_string(),
                provider: ApiProvider {
                    name: "OpenAI".to_string(),
                    compatibility: ApiCompatibility::OpenAi,
                },
            },
            base_prompt: "You are helpful.".to_string(),
            allowed_tools: vec!["read_file".to_string()],
            allowed_skills: vec!["rust-best-practices".to_string()],
        },
        observability,
        active_branch_id: BranchId("fork-1".to_string()),
        branches: vec![
            ChatBranch {
                id: BranchId("main".to_string()),
                name: "Main".to_string(),
                parent_branch_id: None,
                forked_from_message_id: None,
                head_message_id: Some(MessageId("msg-3".to_string())),
                active_snapshot_id: Some(SnapshotId("snap-main".to_string())),
            },
            ChatBranch {
                id: BranchId("fork-1".to_string()),
                name: "Alternative path".to_string(),
                parent_branch_id: Some(BranchId("main".to_string())),
                forked_from_message_id: Some(MessageId("msg-2".to_string())),
                head_message_id: Some(MessageId("evt-6".to_string())),
                active_snapshot_id: Some(SnapshotId("snap-fork".to_string())),
            },
        ],
        snapshots: vec![
            ChatSnapshot {
                id: SnapshotId("snap-main".to_string()),
                branch_id: BranchId("main".to_string()),
                summary: "Earlier main-branch history.".to_string(),
                replaces_messages_through_id: MessageId("msg-1".to_string()),
                observability: Some(CostObservability {
                    model_name: Some("gpt-5.4".to_string()),
                    provider_name: Some("OpenAI".to_string()),
                    usage: TokenUsage {
                        input_tokens: 2,
                        output_tokens: 1,
                        cached_input_tokens: 0,
                        reasoning_tokens: 0,
                        total_tokens: 3,
                    },
                    cost: CostBreakdown::default(),
                }),
            },
            ChatSnapshot {
                id: SnapshotId("snap-fork".to_string()),
                branch_id: BranchId("fork-1".to_string()),
                summary: "Earlier fork history.".to_string(),
                replaces_messages_through_id: MessageId("msg-1".to_string()),
                observability: Some(CostObservability {
                    model_name: Some("gpt-5.4".to_string()),
                    provider_name: Some("OpenAI".to_string()),
                    usage: TokenUsage {
                        input_tokens: 4,
                        output_tokens: 2,
                        cached_input_tokens: 0,
                        reasoning_tokens: 0,
                        total_tokens: 6,
                    },
                    cost: CostBreakdown::default(),
                }),
            },
        ],
        events: vec![
            ChatLogEntry {
                event: ChatEvent {
                    id: MessageId("msg-1".to_string()),
                    parent_id: None,
                    branch_id: BranchId("fork-1".to_string()),
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
                    branch_id: BranchId("fork-1".to_string()),
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
                    id: MessageId("msg-3".to_string()),
                    parent_id: Some(MessageId("msg-2".to_string())),
                    branch_id: BranchId("fork-1".to_string()),
                    body: ChatEventBody::Message(ChatTextMessage {
                        role: ChatTextRole::Agent,
                        content: "Here is the summary.".to_string(),
                    }),
                    observability: Some(CostObservability {
                        model_name: Some("gpt-5.4".to_string()),
                        provider_name: Some("OpenAI".to_string()),
                        usage: TokenUsage {
                            input_tokens: 1,
                            output_tokens: 2,
                            cached_input_tokens: 0,
                            reasoning_tokens: 0,
                            total_tokens: 3,
                        },
                        cost: CostBreakdown::default(),
                    }),
                },
                context_state: MessageContextState::Active,
            },
            ChatLogEntry {
                event: ChatEvent {
                    id: MessageId("evt-4".to_string()),
                    parent_id: Some(MessageId("msg-3".to_string())),
                    branch_id: BranchId("fork-1".to_string()),
                    body: ChatEventBody::ToolCall(ToolCallEvent::Completed(ToolCallCompleted {
                        tool_call_id: ChatToolCallId("tool-1".to_string()),
                        tool_name: "read_file".to_string(),
                        result_preview: "Read 128 lines".to_string(),
                    })),
                    observability: Some(CostObservability {
                        model_name: None,
                        provider_name: None,
                        usage: TokenUsage {
                            input_tokens: 0,
                            output_tokens: 0,
                            cached_input_tokens: 0,
                            reasoning_tokens: 0,
                            total_tokens: 0,
                        },
                        cost: CostBreakdown::default(),
                    }),
                },
                context_state: MessageContextState::Active,
            },
            ChatLogEntry {
                event: ChatEvent {
                    id: MessageId("evt-5".to_string()),
                    parent_id: Some(MessageId("evt-4".to_string())),
                    branch_id: BranchId("fork-1".to_string()),
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
            ChatLogEntry {
                event: ChatEvent {
                    id: MessageId("msg-main".to_string()),
                    parent_id: None,
                    branch_id: BranchId("main".to_string()),
                    body: ChatEventBody::Message(ChatTextMessage {
                        role: ChatTextRole::User,
                        content: "This should not be shown.".to_string(),
                    }),
                    observability: Some(CostObservability {
                        model_name: Some("gpt-5.4".to_string()),
                        provider_name: Some("OpenAI".to_string()),
                        usage: TokenUsage {
                            input_tokens: 7,
                            output_tokens: 5,
                            cached_input_tokens: 0,
                            reasoning_tokens: 0,
                            total_tokens: 12,
                        },
                        cost: CostBreakdown::default(),
                    }),
                },
                context_state: MessageContextState::Active,
            },
        ],
    }
}
