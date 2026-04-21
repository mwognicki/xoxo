use xoxo_core::bus::{BusEvent, BusPayload, TurnEvent};
use xoxo_core::chat::structs::{Chat, ChatTextRole};
use xoxo_core::chat::to_user_facing_chat;

use crate::app::history::HistoryEntry;
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
            _ => {}
        }

        self.history.push(HistoryEntry { chat_id, payload });
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

    pub(super) fn reset_for_new_chat(&mut self) {
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
