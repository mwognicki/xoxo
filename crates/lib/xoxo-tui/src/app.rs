//! Application state and event loop.

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use uuid::Uuid;
use xoxo_core::app_state::AppStateRepository;
use xoxo_core::bus::{BusEvent, BusPayload};
use xoxo_core::chat::structs::{ChatTextRole, ToolCallCompleted, ToolCallEvent, ToolCallFailed, ToolCallStarted};

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
    /// Conversation history (each entry is a line).
    pub history: Vec<String>,
    /// Manual scroll offset measured upward from the bottom of the conversation pane.
    pub conversation_scroll_from_bottom: usize,
    /// Current modal content (if any).
    pub modal_content: Option<String>,
    /// Counter for consecutive Ctrl+C presses.
    pub ctrl_c_count: u8,
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

    pub fn new() -> Self {
        let app_state = AppStateRepository::new()
            .load_or_create()
            .ok();
        Self {
            running: true,
            layout: LayoutMode::Main,
            input: String::new(),
            active_chat_id: None,
            pending_submission: None,
            current_provider_name: app_state
                .as_ref()
                .map(|state| state.current_provider.name.clone())
                .unwrap_or_else(|| "<unknown provider>".to_string()),
            current_model_name: app_state
                .as_ref()
                .map(|state| state.current_model.model_name.clone())
                .unwrap_or_else(|| "<unknown model>".to_string()),
            history: Vec::new(),
            conversation_scroll_from_bottom: 0,
            modal_content: None,
            ctrl_c_count: 0,
        }
    }


    /// Check if a command should be activated and show modal
    fn check_command_activation(&mut self) {
        if self.input == "/help" {
            let help_text = "
Available Commands:
  /help    - Show this help message
  /quit    - Exit the application
  /clear   - Clear the conversation history

Navigation:
  MouseWheel - Scroll conversation
  Tab      - Toggle layout mode
  Up/Down  - Scroll conversation
  PgUp/PgDn- Scroll faster
  Home/End - Jump to top/bottom
  Ctrl+C   - Exit (press twice)
  q        - Exit immediately

Type your message and press Enter to send.".to_string();
            self.modal_content = Some(help_text);
        }
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
                        if !line.is_empty() && !line.starts_with('/') {
                            self.pending_submission = Some(line);
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
        let chat_id = *event.path.current();
        self.active_chat_id = Some(chat_id);

        match event.payload {
            BusPayload::Message(message) => {
                let role = match message.role {
                    ChatTextRole::System => return,
                    ChatTextRole::Agent => "assistant",
                    ChatTextRole::User => "user",
                };
                self.history.push(format!("{role}[{chat_id}] {}", message.content));
            }
            BusPayload::ToolCall(ToolCallEvent::Started(ToolCallStarted {
                tool_call_id,
                tool_name,
                arguments,
                ..
            })) => {
                self.history.push(format!(
                    "assistant[{chat_id}] tool[{0}] {tool_name} call: {arguments}",
                    tool_call_id.0,
                ));
            }
            BusPayload::ToolCall(ToolCallEvent::Completed(ToolCallCompleted {
                tool_call_id,
                result_preview,
                ..
            })) => {
                self.history.push(format!(
                    "assistant[{chat_id}] tool[{0}] done: {result_preview}",
                    tool_call_id.0,
                ));
            }
            BusPayload::ToolCall(ToolCallEvent::Failed(ToolCallFailed {
                tool_call_id,
                message,
                ..
            })) => {
                self.history.push(format!(
                    "assistant[{chat_id}] tool[{0}] error: {message}",
                    tool_call_id.0,
                ));
            }
            BusPayload::AgentShutdown => {
                self.history.push(format!("assistant[{chat_id}] shutdown"));
            }
            BusPayload::Error(error) => {
                self.history.push(format!("assistant[{chat_id}] error: {}", error.message));
            }
        }
    }
}
