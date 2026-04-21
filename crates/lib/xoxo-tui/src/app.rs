//! Application state and event loop.

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use std::collections::HashSet;
use std::time::Instant;
use uuid::Uuid;
use xoxo_core::app_state::AppStateRepository;
use xoxo_core::bus::{BusEvent, BusPayload, TurnEvent};
use xoxo_core::chat::to_user_facing_chat;
use xoxo_core::chat::structs::{Chat, ChatEventBody, ChatTextRole, MessageContextState};
use xoxo_core::llm::LlmFinishReason;
use xoxo_core::model_catalog::lookup_model_summary;

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub chat_id: Uuid,
    pub payload: BusPayload,
}

pub struct App {
    /// Controls the main loop.
    pub running: bool,
    /// Current layout mode.
    pub layout: LayoutMode,
    /// Current input buffer for the textarea.
    pub input: String,
    /// Current active chat selected by incoming daemon events.
    pub active_chat_id: Option<Uuid>,
    /// Pending user message waiting to be sent to the daemon.
    pub pending_submission: Option<String>,
    /// Current configured provider shown in the header.
    pub current_provider_name: String,
    /// Current configured model shown in the header.
    pub current_model_name: String,
    /// Total input tokens used by the active chat.
    pub total_input_tokens: u64,
    /// Total output tokens used by the active chat.
    pub total_output_tokens: u64,
    /// Total tokens used by the active chat.
    pub total_used_tokens: u64,
    /// Estimated context remaining for the active chat.
    pub context_left_percent: Option<u8>,
    /// Maximum input context for the active model.
    pub max_input_tokens: Option<u32>,
    /// Estimated total USD cost for the active chat.
    pub estimated_cost_usd: Option<f32>,
    /// Conversation history as structured bus payloads for TUI-owned formatting.
    pub history: Vec<HistoryEntry>,
    /// Manual scroll offset measured upward from the bottom of the conversation pane.
    pub conversation_scroll_from_bottom: usize,
    /// Current modal content (if any).
    pub modal_content: Option<String>,
    /// Counter for consecutive Ctrl+C presses.
    pub ctrl_c_count: u8,
    /// Start time used for lightweight UI animations.
    pub started_at: Instant,
    /// Whether the active chat turn is currently in progress.
    pub turn_in_progress: bool,
    /// Finish reason for the most recently completed turn, if known.
    pub last_turn_finish_reason: Option<LlmFinishReason>,
}

/// Available UI layout variants.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LayoutMode {
    Main,
    Alternate,
}

impl App {
    const PAGE_SCROLL_LINES: usize = 10;
    const MOUSE_SCROLL_LINES: usize = 3;

    pub fn new(restored_chat: Option<Chat>) -> Self {
        let app_state = AppStateRepository::new()
            .load_or_create()
            .ok();
        let restored_summary = restored_chat.as_ref().map(to_user_facing_chat);
        let active_chat_id = restored_chat.as_ref().map(|chat| chat.id);
        let history = restored_chat
            .as_ref()
            .map(history_from_chat)
            .unwrap_or_default();
        let current_model_name = restored_summary
            .as_ref()
            .map(|summary| summary.current_model_name.clone())
            .or_else(|| {
                app_state
                    .as_ref()
                    .map(|state| state.current_model.model_name.clone())
            })
            .unwrap_or_else(|| "<unknown model>".to_string());
        let total_input_tokens = restored_summary
            .as_ref()
            .map(|summary| summary.total_input_tokens)
            .unwrap_or(0);
        let total_output_tokens = restored_summary
            .as_ref()
            .map(|summary| summary.total_output_tokens)
            .unwrap_or(0);
        let total_used_tokens = restored_summary
            .as_ref()
            .map(|summary| summary.total_used_tokens)
            .unwrap_or(0);
        let (context_left_percent, max_input_tokens, estimated_cost_usd) =
            derive_model_stats(
                &current_model_name,
                total_input_tokens,
                total_output_tokens,
                total_used_tokens,
            );
        Self {
            running: true,
            layout: LayoutMode::Main,
            input: String::new(),
            active_chat_id,
            pending_submission: None,
            current_provider_name: app_state
                .as_ref()
                .map(|state| state.current_provider.name.clone())
                .unwrap_or_else(|| "<unknown provider>".to_string()),
            current_model_name,
            total_input_tokens,
            total_output_tokens,
            total_used_tokens,
            context_left_percent,
            max_input_tokens,
            estimated_cost_usd,
            history,
            conversation_scroll_from_bottom: 0,
            modal_content: None,
            ctrl_c_count: 0,
            started_at: Instant::now(),
            turn_in_progress: false,
            last_turn_finish_reason: None,
        }
    }


    /// Check if a command should be activated and show modal
    fn check_command_activation(&mut self) {
        self.modal_content = match self.input.as_str() {
            "/help" => Some("
Available Commands:
  /help    - Show this help message
  /quit    - Exit the application
  /clear   - Start a fresh chat
  /new     - Start a fresh chat

Navigation:
  MouseWheel - Scroll conversation
  Tab      - Toggle layout mode
  Up/Down  - Scroll conversation
  PgUp/PgDn- Scroll faster
  Home/End - Jump to top/bottom
  Ctrl+C   - Exit (press twice)
  q        - Exit immediately

Type your message and press Enter to send.".to_string()),
            _ => None,
        };
    }

    pub fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    return Ok(());
                }

                // Reset ctrl_c_count unless this event is Ctrl+C.
                let is_ctrl_c = matches!(key.code, KeyCode::Char('c'))
                    && key.modifiers.contains(KeyModifiers::CONTROL);
                if is_ctrl_c {
                    self.ctrl_c_count = self.ctrl_c_count.saturating_add(1);
                    if self.ctrl_c_count >= 2 {
                        self.running = false;
                    }
                    // No further processing for Ctrl+C.
                    return Ok(());
                } else {
                    self.ctrl_c_count = 0;
                }

                match key.code {
                    KeyCode::Char('q') => self.running = false,
                    KeyCode::Tab => {
                        // Toggle between layout modes.
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
                        if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        // Add printable character to input buffer.
                        self.input.push(c);
                        // Check for command activation
                        self.check_command_activation();
                    }
                    KeyCode::Backspace => {
                        self.input.pop();
                    }
                    KeyCode::Enter => {
                        let line: String = self.input.drain(..).collect();
                        match line.as_str() {
                            "/clear" | "/new" => self.reset_for_new_chat(),
                            _ if !line.is_empty() && !line.starts_with('/') => {
                                self.pending_submission = Some(line);
                            }
                            _ => {}
                        }
                        // Clear modal when Enter is pressed
                        self.modal_content = None;
                    }
                    _ => {}
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => self.scroll_conversation_up(Self::MOUSE_SCROLL_LINES),
                MouseEventKind::ScrollDown => {
                    self.scroll_conversation_down(Self::MOUSE_SCROLL_LINES)
                }
                _ => {}
            },
            _ => {
                self.ctrl_c_count = 0;
            }
        }
        Ok(())
    }

    fn scroll_conversation_up(&mut self, lines: usize) {
        self.conversation_scroll_from_bottom =
            self.conversation_scroll_from_bottom.saturating_add(lines);
    }

    fn scroll_conversation_down(&mut self, lines: usize) {
        self.conversation_scroll_from_bottom =
            self.conversation_scroll_from_bottom.saturating_sub(lines);
    }

    pub fn active_chat_id(&self) -> Option<Uuid> {
        self.active_chat_id
    }

    pub fn take_submitted_message(&mut self) -> Option<String> {
        self.pending_submission.take()
    }

    pub fn handle_bus_event(&mut self, event: BusEvent) {
        let active_chat_id = *event.path.root_id();
        let chat_id = *event.path.current();
        self.active_chat_id = Some(active_chat_id);
        let payload = event.payload;

        match &payload {
            BusPayload::Turn(TurnEvent::Started) => {
                self.turn_in_progress = true;
                self.last_turn_finish_reason = None;
                return;
            }
            BusPayload::Turn(TurnEvent::Finished { reason }) => {
                self.turn_in_progress = false;
                self.last_turn_finish_reason = Some(*reason);
                return;
            }
            BusPayload::Message(message) => {
                if message.role == ChatTextRole::System {
                    return;
                }
            }
            BusPayload::TextDelta { .. } | BusPayload::ThinkingDelta { .. } => {
                // Ephemeral streaming fragments are display-only; live-buffer wiring lands in a later step.
                return;
            }
            _ => {}
        }

        self.history.push(HistoryEntry {
            chat_id,
            payload,
        });
    }

    pub fn sync_chat_summary(&mut self, chat: &Chat) {
        let summary = to_user_facing_chat(chat);
        self.current_model_name = summary.current_model_name;
        self.total_input_tokens = summary.total_input_tokens;
        self.total_output_tokens = summary.total_output_tokens;
        self.total_used_tokens = summary.total_used_tokens;
        let (context_left_percent, max_input_tokens, estimated_cost_usd) =
            derive_model_stats(
                &self.current_model_name,
                self.total_input_tokens,
                self.total_output_tokens,
                self.total_used_tokens,
            );
        self.context_left_percent = context_left_percent;
        self.max_input_tokens = max_input_tokens;
        self.estimated_cost_usd = estimated_cost_usd;
    }

    fn reset_for_new_chat(&mut self) {
        self.active_chat_id = None;
        self.pending_submission = None;
        self.history.clear();
        self.conversation_scroll_from_bottom = 0;
        self.turn_in_progress = false;
        self.last_turn_finish_reason = None;
        self.total_input_tokens = 0;
        self.total_output_tokens = 0;
        self.total_used_tokens = 0;
        self.context_left_percent = None;
        self.max_input_tokens = None;
        self.estimated_cost_usd = None;
    }
}

fn derive_model_stats(
    current_model_name: &str,
    total_input_tokens: u64,
    total_output_tokens: u64,
    total_used_tokens: u64,
) -> (Option<u8>, Option<u32>, Option<f32>) {
    let model_summary = lookup_model_summary(current_model_name);
    let context_left_percent = model_summary
        .as_ref()
        .and_then(|summary| summary.context_left_percent(total_used_tokens));
    let max_input_tokens = model_summary.as_ref().map(|summary| summary.max_input_tokens);
    let estimated_cost_usd = model_summary
        .as_ref()
        .and_then(|summary| summary.estimate_total_cost_usd(total_input_tokens, total_output_tokens));

    (context_left_percent, max_input_tokens, estimated_cost_usd)
}

fn history_from_chat(chat: &Chat) -> Vec<HistoryEntry> {
    let tool_event_ids: HashSet<_> = chat
        .events
        .iter()
        .filter(|entry| {
            entry.context_state == MessageContextState::Active
                && entry.event.branch_id == chat.active_branch_id
        })
        .filter_map(|entry| match entry.event.body {
            ChatEventBody::ToolCall(_) => Some(entry.event.id.clone()),
            ChatEventBody::Message(_) | ChatEventBody::AppEvent(_) => None,
        })
        .collect();

    chat.events
        .iter()
        .filter(|entry| {
            entry.context_state == MessageContextState::Active
                && entry.event.branch_id == chat.active_branch_id
        })
        .filter_map(|entry| match &entry.event.body {
            ChatEventBody::Message(message)
                if message.role == ChatTextRole::User
                    && entry
                        .event
                        .parent_id
                        .as_ref()
                        .is_some_and(|parent_id| tool_event_ids.contains(parent_id)) =>
            {
                None
            }
            ChatEventBody::Message(message) if message.role != ChatTextRole::System => {
                Some(HistoryEntry {
                    chat_id: chat.id,
                    payload: BusPayload::Message(message.clone()),
                })
            }
            ChatEventBody::ToolCall(tool_call) => Some(HistoryEntry {
                chat_id: chat.id,
                payload: BusPayload::ToolCall(tool_call.clone()),
            }),
            ChatEventBody::Message(_) | ChatEventBody::AppEvent(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossterm::event::{KeyEvent, KeyModifiers};
    use xoxo_core::chat::structs::{
        ApiCompatibility, ApiProvider, BranchId, ChatAgent, ChatBranch, ChatEvent,
        ChatLogEntry, ChatTextMessage, ChatToolCallId, MessageId, ModelConfig,
        ToolCallCompleted, ToolCallEvent,
    };

    fn press_enter(app: &mut App) {
        app.handle_event(Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)))
            .expect("enter event");
    }

    #[test]
    fn clear_command_resets_current_session() {
        let mut app = App::new(None);
        let chat_id = Uuid::new_v4();
        app.active_chat_id = Some(chat_id);
        app.history.push(HistoryEntry {
            chat_id,
            payload: BusPayload::Message(ChatTextMessage {
                role: ChatTextRole::Agent,
                content: "hello".to_string(),
            }),
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
            payload: BusPayload::Message(ChatTextMessage {
                role: ChatTextRole::User,
                content: "hello".to_string(),
            }),
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
    fn restored_history_hides_synthetic_tool_result_user_message() {
        let tool_event_id = MessageId("tool-event".to_string());
        let synthetic_message_id = MessageId("synthetic-user-message".to_string());
        let branch_id = BranchId("main".to_string());
        let chat = Chat {
            title: None,
            id: Uuid::new_v4(),
            parent_chat_id: None,
            spawned_by_tool_call_id: None,
            path: "root".to_string(),
            agent: ChatAgent {
                id: None,
                name: None,
                model: ModelConfig {
                    model_name: "gpt-4o".to_string(),
                    provider: ApiProvider {
                        name: "openai".to_string(),
                        compatibility: ApiCompatibility::OpenAi,
                    },
                },
                base_prompt: "You are helpful.".to_string(),
                allowed_tools: vec!["read_file".to_string()],
                allowed_skills: Vec::new(),
            },
            observability: None,
            active_branch_id: branch_id.clone(),
            branches: vec![ChatBranch {
                id: branch_id.clone(),
                name: "main".to_string(),
                parent_branch_id: None,
                forked_from_message_id: None,
                head_message_id: Some(synthetic_message_id.clone()),
                active_snapshot_id: None,
            }],
            snapshots: Vec::new(),
            events: vec![
                ChatLogEntry {
                    event: ChatEvent {
                        id: tool_event_id.clone(),
                        parent_id: None,
                        branch_id: branch_id.clone(),
                        body: ChatEventBody::ToolCall(ToolCallEvent::Completed(ToolCallCompleted {
                            tool_call_id: ChatToolCallId("tool-call-1".to_string()),
                            tool_name: "read_file".to_string(),
                            result_preview: "file contents".to_string(),
                        })),
                        observability: None,
                    },
                    context_state: MessageContextState::Active,
                },
                ChatLogEntry {
                    event: ChatEvent {
                        id: synthetic_message_id,
                        parent_id: Some(tool_event_id),
                        branch_id,
                        body: ChatEventBody::Message(ChatTextMessage {
                            role: ChatTextRole::User,
                            content: "read_file: {\"content\":\"file contents\"}".to_string(),
                        }),
                        observability: None,
                    },
                    context_state: MessageContextState::Active,
                },
            ],
        };

        let history = history_from_chat(&chat);

        assert_eq!(history.len(), 1);
        assert!(matches!(history[0].payload, BusPayload::ToolCall(_)));
    }
}
