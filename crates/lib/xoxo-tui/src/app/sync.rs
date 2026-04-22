use anyhow::{anyhow, Result};
use uuid::Uuid;
use xoxo_core::bus::{BusEvent, BusPayload, TurnEvent};
use xoxo_core::chat::structs::{Chat, ChatTextRole};
use xoxo_core::chat::to_user_facing_chat;

use crate::app::history::{history_from_chat, HistoryEntry, HistoryPayload};
use crate::app::stats::derive_model_stats;
use crate::app::App;

impl App {
    pub fn handle_bus_event(&mut self, event: BusEvent) {
        let active_chat_id = *event.path.root_id();
        let chat_id = *event.path.current();
        self.active_chat_id = Some(active_chat_id);
        let payload = event.payload;

        match &payload {
            BusPayload::Turn(TurnEvent::Started) => {
                self.turn_in_progress = true;
                self.last_turn_finish_reason = None;
                self.in_flight_text.remove(&chat_id);
                self.in_flight_thinking.remove(&chat_id);
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
                // Canonical assistant message supersedes any in-flight text buffer for this chat.
                // Streamed thinking is preserved in TUI history (but never persisted to sled).
                if message.role == ChatTextRole::Agent {
                    self.in_flight_text.remove(&chat_id);
                    if let Some(thinking) = self.in_flight_thinking.remove(&chat_id) {
                        if !thinking.is_empty() {
                            self.history.push(HistoryEntry {
                                chat_id,
                                payload: HistoryPayload::Thinking(thinking),
                            });
                        }
                    }
                }
            }
            BusPayload::TextDelta { delta } => {
                self.in_flight_text
                    .entry(chat_id)
                    .or_default()
                    .push_str(delta);
                return;
            }
            BusPayload::ThinkingDelta { delta } => {
                self.in_flight_thinking
                    .entry(chat_id)
                    .or_default()
                    .push_str(delta);
                return;
            }
            _ => {}
        }

        self.history.push(HistoryEntry {
            chat_id,
            payload: HistoryPayload::Bus(payload),
        });
    }

    pub fn sync_chat_summary(&mut self, chat: &Chat) {
        let summary = to_user_facing_chat(chat);
        self.current_model_name = summary.current_model_name;
        self.total_input_tokens = summary.total_input_tokens;
        self.total_output_tokens = summary.total_output_tokens;
        self.total_used_tokens = summary.total_used_tokens;
        let (context_left_percent, max_input_tokens, estimated_cost_usd) = derive_model_stats(
            &self.current_model_name,
            self.total_input_tokens,
            self.total_output_tokens,
            self.total_used_tokens,
        );
        self.context_left_percent = context_left_percent;
        self.max_input_tokens = max_input_tokens;
        self.estimated_cost_usd = estimated_cost_usd;
    }

    pub fn load_chat_session(&mut self, chat_id: Uuid) -> Result<()> {
        let storage = self
            .storage
            .as_ref()
            .ok_or_else(|| anyhow!("session loading requires storage"))?
            .clone();
        let chat = storage
            .load_chat(chat_id)?
            .ok_or_else(|| anyhow!("stored chat {chat_id} was not found"))?;

        self.active_chat_id = Some(chat.id);
        self.pending_submission = None;
        self.history = history_from_chat(&chat);
        self.in_flight_text.clear();
        self.in_flight_thinking.clear();
        self.conversation_scroll_from_bottom = 0;
        self.turn_in_progress = false;
        self.last_turn_finish_reason = None;
        self.sync_chat_summary(&chat);
        storage.set_last_used_chat_id(chat.id)?;
        Ok(())
    }

    pub(super) fn reset_for_new_chat(&mut self) {
        self.active_chat_id = None;
        self.pending_submission = None;
        self.history.clear();
        self.in_flight_text.clear();
        self.in_flight_thinking.clear();
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

#[cfg(test)]
mod tests {
    use super::*;

    use uuid::Uuid;
    use xoxo_core::bus::BusEnvelope;
    use xoxo_core::chat::structs::{ChatPath, ChatTextMessage};

    #[test]
    fn text_deltas_accumulate_and_are_cleared_by_canonical_message() {
        let mut app = App::new(None);
        let chat_id = Uuid::new_v4();
        let path = ChatPath(vec![chat_id]);

        app.handle_bus_event(BusEnvelope {
            path: path.clone(),
            payload: BusPayload::TextDelta {
                delta: "Hel".to_string(),
            },
        });
        app.handle_bus_event(BusEnvelope {
            path: path.clone(),
            payload: BusPayload::TextDelta {
                delta: "lo".to_string(),
            },
        });

        assert_eq!(
            app.in_flight_text.get(&chat_id).map(String::as_str),
            Some("Hello")
        );
        // Deltas must not land in the persisted history.
        assert!(app.history.is_empty());

        app.handle_bus_event(BusEnvelope {
            path,
            payload: BusPayload::Message(ChatTextMessage {
                role: ChatTextRole::Agent,
                content: "Hello, world!".to_string(),
            }),
        });

        assert!(app.in_flight_text.get(&chat_id).is_none());
        assert_eq!(app.history.len(), 1);
        assert!(matches!(
            app.history[0].payload,
            HistoryPayload::Bus(BusPayload::Message(_))
        ));
    }

    #[test]
    fn thinking_deltas_accumulate_and_are_cleared_by_canonical_message() {
        let mut app = App::new(None);
        let chat_id = Uuid::new_v4();
        let path = ChatPath(vec![chat_id]);

        app.handle_bus_event(BusEnvelope {
            path: path.clone(),
            payload: BusPayload::ThinkingDelta {
                delta: "Let me ".to_string(),
            },
        });
        app.handle_bus_event(BusEnvelope {
            path: path.clone(),
            payload: BusPayload::ThinkingDelta {
                delta: "think.".to_string(),
            },
        });

        assert_eq!(
            app.in_flight_thinking.get(&chat_id).map(String::as_str),
            Some("Let me think.")
        );
        // Thinking deltas must not land in persisted history as bus entries.
        assert!(app.history.is_empty());

        app.handle_bus_event(BusEnvelope {
            path,
            payload: BusPayload::Message(ChatTextMessage {
                role: ChatTextRole::Agent,
                content: "Here's the answer.".to_string(),
            }),
        });

        assert!(app.in_flight_thinking.get(&chat_id).is_none());
    }

    #[test]
    fn turn_started_clears_prior_in_flight_thinking() {
        let mut app = App::new(None);
        let chat_id = Uuid::new_v4();
        let path = ChatPath(vec![chat_id]);

        app.in_flight_thinking.insert(chat_id, "stale".to_string());

        app.handle_bus_event(BusEnvelope {
            path,
            payload: BusPayload::Turn(TurnEvent::Started),
        });

        assert!(app.in_flight_thinking.get(&chat_id).is_none());
    }

    #[test]
    fn turn_started_clears_prior_in_flight_text() {
        let mut app = App::new(None);
        let chat_id = Uuid::new_v4();
        let path = ChatPath(vec![chat_id]);

        app.in_flight_text.insert(chat_id, "stale".to_string());

        app.handle_bus_event(BusEnvelope {
            path,
            payload: BusPayload::Turn(TurnEvent::Started),
        });

        assert!(app.in_flight_text.get(&chat_id).is_none());
        assert!(app.turn_in_progress);
    }
}
