use anyhow::Result;
use chrono::{DateTime, Local};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use uuid::Uuid;
use xoxo_core::storage::ChatSessionSummary;

use crate::app::{App, LayoutMode, MentionPopup, Modal, ModalMenu, ModalMenuItem};

const HELP_BODY: &str = "
Available Commands:
  /help    - Show this help message
  /quit    - Exit the application
  /clear   - Start a fresh chat
  /new     - Start a fresh chat

Navigation:
  Tab      - Toggle layout mode
  Up/Down  - Scroll conversation
  PgUp/PgDn- Scroll faster
  Home/End - Jump to top/bottom
  Ctrl+C   - Exit (press twice)

Type your message and press Enter to send.";

const HELP_FOOTER: &str = " Esc to close ";
const SESSIONS_PAGE_SIZE: usize = 10;
const SESSIONS_FOOTER: &str = " Up/Down select  Left/Right page  Enter load later  Esc close ";

impl App {
    fn open_help_modal(&mut self) {
        self.modal = Some(Modal::text(" Help ", HELP_BODY, HELP_FOOTER));
    }

    fn open_sessions_modal(&mut self) -> Result<()> {
        let sessions = match &self.storage {
            Some(storage) => storage.list_chat_sessions()?,
            None => Vec::new(),
        };
        let items = sessions
            .iter()
            .map(session_summary_item)
            .collect::<Vec<_>>();
        self.modal = Some(Modal::menu(
            " Sessions ",
            ModalMenu::new(items, SESSIONS_PAGE_SIZE, "No stored sessions found."),
            SESSIONS_FOOTER,
        ));
        Ok(())
    }

    fn selected_modal_chat_id(&self) -> Option<Uuid> {
        self.modal
            .as_ref()
            .and_then(Modal::selected_value)
            .and_then(selected_chat_id_from_value)
    }

    pub fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Paste(content) => {
                if self.modal.is_some() {
                    return Ok(());
                }
                self.mention_popup = None;
                self.input.push_str(&content);
            }
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    return Ok(());
                }

                let is_ctrl_c = matches!(key.code, KeyCode::Char('c'))
                    && key.modifiers.contains(KeyModifiers::CONTROL);
                if is_ctrl_c {
                    self.ctrl_c_count = self.ctrl_c_count.saturating_add(1);
                    if self.ctrl_c_count >= 2 {
                        self.running = false;
                    }
                    return Ok(());
                } else {
                    self.ctrl_c_count = 0;
                }

                if self.modal.is_some() {
                    if matches!(key.code, KeyCode::Esc) {
                        self.modal = None;
                    } else if matches!(key.code, KeyCode::Enter) {
                        if let Some(chat_id) = self.selected_modal_chat_id() {
                            self.load_chat_session(chat_id)?;
                            self.modal = None;
                        }
                    } else if let Some(modal) = &mut self.modal {
                        modal.handle_navigation_key(key.code);
                    }
                    return Ok(());
                }

                if let Some(popup) = &mut self.mention_popup {
                    match key.code {
                        KeyCode::Tab => {
                            self.handle_mention_tab();
                            return Ok(());
                        }
                        KeyCode::Enter => {
                            self.commit_mention_selection();
                            return Ok(());
                        }
                        KeyCode::Esc => {
                            self.mention_popup = None;
                            return Ok(());
                        }
                        KeyCode::Up => {
                            popup.select_prev();
                            return Ok(());
                        }
                        KeyCode::Down => {
                            popup.select_next();
                            return Ok(());
                        }
                        _ => {}
                    }
                }

                let is_ctrl_s = matches!(key.code, KeyCode::Char('s'))
                    && key.modifiers.contains(KeyModifiers::CONTROL);
                if is_ctrl_s {
                    self.toggle_mouse_capture();
                    return Ok(());
                }

                match key.code {
                    KeyCode::Tab => {
                        self.layout = match self.layout {
                            LayoutMode::Main => LayoutMode::Alternate,
                            LayoutMode::Alternate => LayoutMode::Main,
                        };
                    }
                    KeyCode::Up => self.scroll_conversation_up(1),
                    KeyCode::PageUp => self.scroll_conversation_up(Self::PAGE_SCROLL_LINES),
                    KeyCode::Home => {
                        self.conversation_scroll_from_bottom = usize::MAX;
                    }
                    KeyCode::Down => self.scroll_conversation_down(1),
                    KeyCode::PageDown => self.scroll_conversation_down(Self::PAGE_SCROLL_LINES),
                    KeyCode::End => {
                        self.conversation_scroll_from_bottom = 0;
                    }
                    KeyCode::Char(c)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        self.input.push(c);
                    }
                    KeyCode::Backspace => {
                        self.input.pop();
                    }
                    KeyCode::Enter => {
                        let line: String = self.input.drain(..).collect();
                        match line.as_str() {
                            "/quit" => self.running = false,
                            "/clear" | "/new" => self.reset_for_new_chat(),
                            "/help" => self.open_help_modal(),
                            "/sessions" => self.open_sessions_modal()?,
                            _ if !line.is_empty() && !line.starts_with('/') => {
                                self.pending_submission = Some(line);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }

                if let Some(popup) = &mut self.mention_popup {
                    match key.code {
                        KeyCode::Backspace => {
                            if self.input.len() <= popup.trigger_at {
                                self.mention_popup = None;
                            } else {
                                self.refresh_mention_filter();
                            }
                        }
                        KeyCode::Char(c)
                            if !key
                                .modifiers
                                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                        {
                            if c.is_whitespace() {
                                self.mention_popup = None;
                            } else {
                                self.refresh_mention_filter();
                            }
                        }
                        _ => {}
                    }
                } else if let KeyCode::Char('@') = key.code
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                    && self.input_allows_mention_popup()
                {
                    self.open_mention_popup();
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => self.scroll_conversation_up(Self::MOUSE_SCROLL_LINES),
                MouseEventKind::ScrollDown => {
                    self.scroll_conversation_down(Self::MOUSE_SCROLL_LINES);
                }
                _ => {}
            },
            _ => {
                self.ctrl_c_count = 0;
            }
        }
        Ok(())
    }

    fn open_mention_popup(&mut self) {
        let trigger_at = self.input.len().saturating_sub(1);
        debug_assert!(self.input.is_char_boundary(trigger_at));
        match MentionPopup::open(&self.workspace_root, trigger_at) {
            Ok(popup) => self.mention_popup = Some(popup),
            Err(_) => self.mention_popup = None,
        }
    }

    fn input_allows_mention_popup(&self) -> bool {
        let trigger_at = self.input.len().saturating_sub(1);
        self.input[..trigger_at]
            .chars()
            .next_back()
            .is_none_or(char::is_whitespace)
    }

    fn refresh_mention_filter(&mut self) {
        if let Some(popup) = &mut self.mention_popup {
            let start = popup.trigger_at + 1;
            let filter = if start < self.input.len() {
                &self.input[start..]
            } else {
                ""
            };
            popup.set_filter(filter);
        }
    }

    fn handle_mention_tab(&mut self) {
        let selection = self
            .mention_popup
            .as_ref()
            .and_then(|popup| {
                popup
                    .selected_entry()
                    .cloned()
                    .map(|entry| (popup.trigger_at, entry))
            });
        let Some((trigger_at, entry)) = selection else {
            self.mention_popup = None;
            return;
        };

        if entry.is_dir && self.mention_text_after(trigger_at) != entry.rel_path {
            self.replace_mention_text(trigger_at, &entry.rel_path);
            self.refresh_mention_filter();
            return;
        }

        self.commit_mention_selection();
    }

    fn mention_text_after(&self, trigger_at: usize) -> &str {
        let start = trigger_at + 1;
        if start < self.input.len() {
            &self.input[start..]
        } else {
            ""
        }
    }

    fn replace_mention_text(&mut self, trigger_at: usize, replacement: &str) {
        debug_assert!(self.input.is_char_boundary(trigger_at));
        self.input.truncate(trigger_at);
        self.input.push('@');
        self.input.push_str(replacement);
    }

    fn commit_mention_selection(&mut self) {
        let popup = self.mention_popup.take();
        if let Some(popup) = popup
            && let Some(entry) = popup.selected_entry()
        {
            let trigger_at = popup.trigger_at;
            let committed_path = if entry.is_dir {
                format!("{}/", entry.rel_path)
            } else {
                entry.rel_path.clone()
            };
            self.replace_mention_text(trigger_at, &committed_path);
            self.input.push(' ');
        }
    }

    fn scroll_conversation_up(&mut self, lines: usize) {
        self.conversation_scroll_from_bottom =
            self.conversation_scroll_from_bottom.saturating_add(lines);
    }

    fn scroll_conversation_down(&mut self, lines: usize) {
        self.conversation_scroll_from_bottom =
            self.conversation_scroll_from_bottom.saturating_sub(lines);
    }
}

fn session_summary_item(session: &ChatSessionSummary) -> ModalMenuItem {
    ModalMenuItem {
        label: format!(
            "{:<17}",
            session
                .updated_at
                .as_deref()
                .map(format_session_updated_at)
                .unwrap_or_else(|| "unknown time".to_string())
        ),
        detail: session.model_name.clone(),
        value: Some(session.id.to_string()),
    }
}

fn selected_chat_id_from_value(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value).ok()
}

fn format_session_updated_at(updated_at: &str) -> String {
    DateTime::parse_from_rfc3339(updated_at)
        .map(|timestamp| {
            timestamp
                .with_timezone(&Local)
                .format("%b %-d, %Y %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| updated_at.to_string())
}

#[cfg(test)]
mod tests {
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

    use crate::app::{HistoryEntry, HistoryPayload, MentionPopup};
    use crate::app::mention::MentionEntry;

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
        let entries = vec![MentionEntry {
            rel_path: "src/main.rs".to_string(),
            is_dir: false,
        }];
        app.mention_popup = Some(MentionPopup::with_entries(0, entries));
        press_key(&mut app, KeyCode::Tab);
        assert_eq!(app.input, "@src/main.rs ");
        assert!(app.mention_popup.is_none());
    }

    #[test]
    fn first_tab_on_directory_updates_mention_text() {
        let mut app = App::new(None);
        app.input = "inspect @s".to_string();
        let entries = vec![
            MentionEntry {
                rel_path: "src".to_string(),
                is_dir: true,
            },
            MentionEntry {
                rel_path: "src/main.rs".to_string(),
                is_dir: false,
            },
        ];
        app.mention_popup = Some(MentionPopup::with_entries("inspect ".len(), entries));

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
            MentionEntry {
                rel_path: "src".to_string(),
                is_dir: true,
            },
            MentionEntry {
                rel_path: "src/main.rs".to_string(),
                is_dir: false,
            },
        ];
        let mut popup = MentionPopup::with_entries(0, entries);
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
        let entries = vec![MentionEntry {
            rel_path: "Cargo.toml".to_string(),
            is_dir: false,
        }];
        app.mention_popup = Some(MentionPopup::with_entries(0, entries));
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
}
