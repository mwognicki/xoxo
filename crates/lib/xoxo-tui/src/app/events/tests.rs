use super::*;

use std::sync::Arc;

use crossterm::event::{KeyEvent, KeyModifiers};
use uuid::Uuid;
use xoxo_core::bus::BusPayload;
use xoxo_core::chat::structs::{
    ApiCompatibility, ApiProvider, BranchId, Chat, ChatAgent, ChatBranch, ChatEvent,
    ChatEventBody, ChatLogEntry, ChatTextMessage, ChatTextRole, MessageContextState, MessageId,
    ModelConfig,
};
use xoxo_core::llm::LlmFinishReason;
use xoxo_core::storage::Storage;

use crate::app::{HistoryEntry, HistoryPayload, FileWalkerMentionPopup};
use crate::app::mention_file_walker::FileWalkerMentionEntry;
use crate::app::{ConfigFocus, ModalContent};

fn press_enter(app: &mut App) {
    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)))
        .expect("enter event");
}

fn press_char(app: &mut App, character: char) {
    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char(character),
        KeyModifiers::NONE,
    )))
    .expect("char event");
}

fn press_key(app: &mut App, code: KeyCode) {
    app.handle_event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
        .expect("key event");
}

fn sample_chat(chat_id: Uuid, model_name: &str, message: &str) -> Chat {
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
                model_name: model_name.to_string(),
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
            head_message_id: Some(MessageId("message-1".to_string())),
            active_snapshot_id: None,
        }],
        snapshots: Vec::new(),
        events: vec![ChatLogEntry {
            event: ChatEvent {
                id: MessageId("message-1".to_string()),
                parent_id: None,
                branch_id: BranchId("main".to_string()),
                body: ChatEventBody::Message(ChatTextMessage {
                    role: ChatTextRole::User,
                    content: message.to_string(),
                }),
                observability: None,
            },
            context_state: MessageContextState::Active,
        }],
    }
}

#[test]
fn clear_command_resets_current_session() {
    let mut app = App::new(None);
    let chat_id = Uuid::new_v4();
    app.active_chat_id = Some(chat_id);
    app.history.push(HistoryEntry {
        chat_id,
        payload: HistoryPayload::Bus(BusPayload::Message(ChatTextMessage {
            role: ChatTextRole::Agent,
            content: "hello".to_string(),
        })),
    });
    app.turn_in_progress = true;
    app.last_turn_finish_reason = Some(LlmFinishReason::Stop);
    app.input = "/clear".to_string();

    press_enter(&mut app);

    assert_eq!(app.active_chat_id, None);
    assert!(app.history.is_empty());
    assert!(!app.turn_in_progress);
    assert_eq!(app.last_turn_finish_reason, None);
    assert_eq!(app.pending_submission, None);
}

#[test]
fn new_command_resets_current_session() {
    let mut app = App::new(None);
    let chat_id = Uuid::new_v4();
    app.active_chat_id = Some(chat_id);
    app.history.push(HistoryEntry {
        chat_id,
        payload: HistoryPayload::Bus(BusPayload::Message(ChatTextMessage {
            role: ChatTextRole::User,
            content: "hello".to_string(),
        })),
    });
    app.input = "/new".to_string();

    press_enter(&mut app);

    assert_eq!(app.active_chat_id, None);
    assert!(app.history.is_empty());
    assert_eq!(app.pending_submission, None);
    assert_eq!(app.total_input_tokens, 0);
    assert_eq!(app.total_output_tokens, 0);
    assert_eq!(app.total_used_tokens, 0);
    assert_eq!(app.context_left_percent, None);
    assert_eq!(app.max_input_tokens, None);
    assert_eq!(app.estimated_cost_usd, None);
}

#[test]
fn multiline_paste_buffers_input_without_submitting() {
    let mut app = App::new(None);

    app.handle_event(Event::Paste("first\nsecond\nthird".to_string()))
        .expect("paste event");

    assert_eq!(app.input, "first\nsecond\nthird");
    assert_eq!(app.pending_submission, None);
}

#[test]
fn enter_submits_multiline_pasted_input_once() {
    let mut app = App::new(None);
    app.input = "first\nsecond".to_string();

    press_enter(&mut app);

    assert_eq!(app.input, "");
    assert_eq!(app.pending_submission, Some("first\nsecond".to_string()));
}

#[test]
fn q_is_regular_input_text() {
    let mut app = App::new(None);

    for character in "quick question".chars() {
        press_char(&mut app, character);
    }

    assert!(app.running);
    assert_eq!(app.input, "quick question");
}

#[test]
fn quit_command_exits_application() {
    let mut app = App::new(None);
    app.input = "/quit".to_string();

    press_enter(&mut app);

    assert!(!app.running);
}

#[test]
fn help_command_opens_modal_on_enter() {
    let mut app = App::new(None);
    app.input = "/help".to_string();

    press_enter(&mut app);

    assert!(app.modal.is_some(), "help modal should be open");
    let modal = app.modal.as_ref().expect("modal present");
    assert!(
        !modal.footer.is_empty(),
        "help modal footer must advertise its key bindings"
    );
    assert_eq!(app.input, "", "input buffer is drained by Enter");
}

#[test]
fn config_command_opens_config_modal_on_enter() {
    let mut app = App::new(None);
    app.input = "/config".to_string();

    press_enter(&mut app);

    let Some(modal) = &app.modal else {
        panic!("config modal should be open");
    };
    let ModalContent::Config(config) = &modal.content else {
        panic!("expected config modal content");
    };
    assert_eq!(config.selected_index, 0);
    assert_eq!(config.focus, ConfigFocus::Navigation);
    assert_eq!(app.input, "");
}

#[test]
fn typing_help_without_enter_does_not_open_modal() {
    let mut app = App::new(None);

    for character in "/help".chars() {
        press_char(&mut app, character);
    }

    assert!(
        app.modal.is_none(),
        "modal must not auto-open while typing — only on Enter"
    );
}

#[test]
fn tab_accepts_inline_command_suggestion() {
    let mut app = App::new(None);

    for character in "/he".chars() {
        press_char(&mut app, character);
    }
    press_key(&mut app, KeyCode::Tab);

    assert_eq!(app.input, "/help");
    assert_eq!(app.layout, LayoutMode::Main);
}

#[test]
fn enter_accepts_inline_command_suggestion() {
    let mut app = App::new(None);

    for character in "/co".chars() {
        press_char(&mut app, character);
    }
    press_enter(&mut app);

    let Some(modal) = &app.modal else {
        panic!("config modal should open from inline suggestion");
    };
    assert!(
        matches!(modal.content, ModalContent::Config(_)),
        "config modal should open from inline suggestion"
    );
    assert_eq!(app.input, "");
}

#[test]
fn config_modal_navigation_keys_update_focus_and_selection() {
    let mut app = App::new(None);
    app.open_config_modal();

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Tab);

    let Some(modal) = &app.modal else {
        panic!("config modal should still be open");
    };
    let ModalContent::Config(config) = &modal.content else {
        panic!("expected config modal");
    };
    assert_eq!(config.selected_index, 1);
    assert_eq!(config.focus, ConfigFocus::Detail);
}

#[test]
fn tab_without_command_suggestion_toggles_layout() {
    let mut app = App::new(None);

    press_key(&mut app, KeyCode::Tab);

    assert_eq!(app.layout, LayoutMode::Alternate);
}

#[test]
fn enter_while_modal_open_does_not_close_it() {
    let mut app = App::new(None);
    app.open_help_modal();

    press_enter(&mut app);

    assert!(
        app.modal.is_some(),
        "Enter must not close modals — only Esc does"
    );
}

#[test]
fn typing_while_modal_open_does_not_reach_input() {
    let mut app = App::new(None);
    app.open_help_modal();

    press_char(&mut app, 'x');

    assert_eq!(
        app.input, "",
        "keystrokes must be swallowed while a modal is open"
    );
    assert!(app.modal.is_some());
}

#[test]
fn esc_closes_modal() {
    let mut app = App::new(None);
    app.open_help_modal();

    press_key(&mut app, KeyCode::Esc);

    assert!(app.modal.is_none(), "Esc must close the modal");
}

#[test]
fn enter_on_selected_session_loads_persisted_chat() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let storage = Storage::open_at(tempdir.path().join("data")).expect("storage");
    let chat_id = Uuid::new_v4();
    let chat = sample_chat(chat_id, "gpt-5.4", "resume this");
    storage.save_chat(&chat).expect("save chat");
    let storage = Arc::new(storage);
    let mut app = App::new_with_storage(None, Some(storage.clone()));
    app.input = "/sessions".to_string();

    press_enter(&mut app);
    press_enter(&mut app);

    assert!(app.modal.is_none());
    assert_eq!(app.active_chat_id, Some(chat_id));
    assert_eq!(app.current_model_name, "gpt-5.4");
    assert_eq!(app.history.len(), 1);
    assert_eq!(
        storage.last_used_chat_id().expect("last used chat id"),
        Some(chat_id)
    );
}

#[test]
fn at_sign_opens_mention_popup() {
    let mut app = App::new(None);
    press_char(&mut app, '@');
    let popup = app.mention_popup.as_ref().expect("popup should be open");
    assert_eq!(popup.trigger_at, 0);
}

#[test]
fn at_sign_after_whitespace_opens_mention_popup() {
    let mut app = App::new(None);
    app.input = "check ".to_string();

    press_char(&mut app, '@');

    let popup = app.mention_popup.as_ref().expect("popup should be open");
    assert_eq!(popup.trigger_at, "check ".len());
}

#[test]
fn embedded_at_sign_does_not_open_mention_popup() {
    let mut app = App::new(None);
    app.input = "email".to_string();

    press_char(&mut app, '@');

    assert_eq!(app.input, "email@");
    assert!(app.mention_popup.is_none());
}

#[test]
fn space_after_at_closes_popup() {
    let mut app = App::new(None);
    press_char(&mut app, '@');
    assert!(app.mention_popup.is_some());
    press_char(&mut app, ' ');
    assert!(app.mention_popup.is_none());
    assert!(app.input.ends_with("@ "));
}

#[test]
fn backspace_past_at_closes_popup() {
    let mut app = App::new(None);
    press_char(&mut app, '@');
    press_char(&mut app, 'x');
    assert!(app.mention_popup.is_some());
    press_key(&mut app, KeyCode::Backspace);
    press_key(&mut app, KeyCode::Backspace);
    assert!(app.mention_popup.is_none());
}

#[test]
fn tab_commits_selection() {
    let mut app = App::new(None);
    app.input = "@".to_string();
    let entries = vec![FileWalkerMentionEntry {
        rel_path: "src/main.rs".to_string(),
        is_dir: false,
    }];
    app.mention_popup = Some(FileWalkerMentionPopup::with_entries(0, entries));
    press_key(&mut app, KeyCode::Tab);
    assert_eq!(app.input, "@src/main.rs ");
    assert!(app.mention_popup.is_none());
}

#[test]
fn first_tab_on_directory_updates_mention_text() {
    let mut app = App::new(None);
    app.input = "inspect @s".to_string();
    let entries = vec![
        FileWalkerMentionEntry {
            rel_path: "src".to_string(),
            is_dir: true,
        },
        FileWalkerMentionEntry {
            rel_path: "src/main.rs".to_string(),
            is_dir: false,
        },
    ];
    app.mention_popup = Some(FileWalkerMentionPopup::with_entries("inspect ".len(), entries));

    press_key(&mut app, KeyCode::Tab);

    assert_eq!(app.input, "inspect @src");
    let popup = app.mention_popup.as_ref().expect("popup should stay open");
    assert_eq!(popup.filter(), "src");
}

#[test]
fn second_tab_on_matching_directory_commits_selection() {
    let mut app = App::new(None);
    app.input = "@src".to_string();
    let entries = vec![
        FileWalkerMentionEntry {
            rel_path: "src".to_string(),
            is_dir: true,
        },
        FileWalkerMentionEntry {
            rel_path: "src/main.rs".to_string(),
            is_dir: false,
        },
    ];
    let mut popup = FileWalkerMentionPopup::with_entries(0, entries);
    popup.set_filter("src");
    app.mention_popup = Some(popup);

    press_key(&mut app, KeyCode::Tab);

    assert_eq!(app.input, "@src/ ");
    assert!(app.mention_popup.is_none());
}

#[test]
fn esc_closes_popup_leaves_input() {
    let mut app = App::new(None);
    press_char(&mut app, '@');
    press_char(&mut app, 'x');
    press_key(&mut app, KeyCode::Esc);
    assert_eq!(app.input, "@x");
    assert!(app.mention_popup.is_none());
}

#[test]
fn enter_while_popup_open_commits_not_submits() {
    let mut app = App::new(None);
    app.input = "@".to_string();
    let entries = vec![FileWalkerMentionEntry {
        rel_path: "Cargo.toml".to_string(),
        is_dir: false,
    }];
    app.mention_popup = Some(FileWalkerMentionPopup::with_entries(0, entries));
    press_enter(&mut app);
    assert!(app.pending_submission.is_none());
    assert_eq!(app.input, "@Cargo.toml ");
    assert!(app.mention_popup.is_none());
}

#[test]
fn typing_filter_updates_popup() {
    let mut app = App::new(None);
    press_char(&mut app, '@');
    press_char(&mut app, 'a');
    let popup = app.mention_popup.as_ref().expect("popup should be open");
    assert_eq!(popup.filter(), "a");
}
