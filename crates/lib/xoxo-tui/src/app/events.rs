use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};

use crate::app::{App, LayoutMode};

impl App {
    /// Check if a command should be activated and show modal
    fn check_command_activation(&mut self) {
        self.modal_content = match self.input.as_str() {
            "/help" => Some(
                "
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

Type your message and press Enter to send."
                    .to_string(),
            ),
            _ => None,
        };
    }

    pub fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Paste(content) => {
                self.input.push_str(&content);
                self.check_command_activation();
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
                        self.check_command_activation();
                    }
                    KeyCode::Backspace => {
                        self.input.pop();
                    }
                    KeyCode::Enter => {
                        let line: String = self.input.drain(..).collect();
                        match line.as_str() {
                            "/quit" => self.running = false,
                            "/clear" | "/new" => self.reset_for_new_chat(),
                            _ if !line.is_empty() && !line.starts_with('/') => {
                                self.pending_submission = Some(line);
                            }
                            _ => {}
                        }
                        self.modal_content = None;
                    }
                    _ => {}
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

    fn scroll_conversation_up(&mut self, lines: usize) {
        self.conversation_scroll_from_bottom =
            self.conversation_scroll_from_bottom.saturating_add(lines);
    }

    fn scroll_conversation_down(&mut self, lines: usize) {
        self.conversation_scroll_from_bottom =
            self.conversation_scroll_from_bottom.saturating_sub(lines);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossterm::event::{KeyEvent, KeyModifiers};
    use uuid::Uuid;
    use xoxo_core::bus::BusPayload;
    use xoxo_core::chat::structs::{ChatTextMessage, ChatTextRole};
    use xoxo_core::llm::LlmFinishReason;

    use crate::app::{HistoryEntry, HistoryPayload};

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
}
