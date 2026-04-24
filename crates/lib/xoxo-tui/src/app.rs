//! Application state.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use ratatui::text::Line;
use uuid::Uuid;
use xoxo_core::app_state::AppStateRepository;
use xoxo_core::chat::structs::Chat;
use xoxo_core::chat::to_user_facing_chat;
use xoxo_core::llm::LlmFinishReason;
use xoxo_core::storage::Storage;

mod events;
mod history;
pub(crate) mod mention;
mod modal;
mod stats;
mod sync;

pub use history::{HistoryEntry, HistoryPayload};
pub use mention::{MentionPopup};
pub use modal::{Modal, ModalContent, ModalMenu, ModalMenuItem};

use history::history_from_chat;
use stats::derive_model_stats;

/// Cache key that identifies whether a previously built set of conversation
/// lines is still valid. Bumps of `conversation_version` cover every piece of
/// `App` state that feeds the conversation pane (history, in-flight buffers,
/// turn flag, header stats). The spinner phase is folded in separately because
/// it advances on time, not on state mutation, and only matters while a turn
/// is in progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ConversationCacheKey {
    pub(crate) version: u64,
    pub(crate) spinner_phase: Option<u128>,
}

/// Cached conversation pane produced by the UI layer. Owned `Line<'static>`s
/// make caching safe across frames without borrow-checker gymnastics.
pub(crate) struct CachedConversation {
    pub(crate) key: ConversationCacheKey,
    pub(crate) lines: Vec<Line<'static>>,
    pub(crate) turn_finished_positions: Vec<usize>,
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
    /// Transient, per-chat buffer of assistant text deltas streamed during the in-flight turn.
    /// Cleared when the canonical [`BusPayload::Message`] for the chat arrives.
    pub in_flight_text: HashMap<Uuid, String>,
    /// Transient, per-chat buffer of assistant thinking/reasoning deltas streamed during the
    /// in-flight turn. Cleared when the canonical [`BusPayload::Message`] arrives. Not persisted.
    pub in_flight_thinking: HashMap<Uuid, String>,
    /// Manual scroll offset measured upward from the bottom of the conversation pane.
    pub conversation_scroll_from_bottom: usize,
    /// Current modal overlay (if any).
    pub modal: Option<Modal>,
    /// Current `@`-mention popup state (if any).
    pub mention_popup: Option<MentionPopup>,
    /// Workspace root captured at startup; used for the `@`-mention picker and
    /// path display in the header/status bar.
    pub workspace_root: PathBuf,
    /// Counter for consecutive Ctrl+C presses.
    pub ctrl_c_count: u8,
    /// Start time used for lightweight UI animations.
    pub started_at: Instant,
    /// Whether the active chat turn is currently in progress.
    pub turn_in_progress: bool,
    /// Finish reason for the most recently completed turn, if known.
    pub last_turn_finish_reason: Option<LlmFinishReason>,
    /// Whether terminal mouse capture is currently active.
    ///
    /// When `false` the TUI gives up scroll-wheel events so that the terminal
    /// can perform native drag-to-select. Toggled at runtime via `Ctrl+S`.
    pub mouse_capture_enabled: bool,
    pub(crate) storage: Option<Arc<Storage>>,
    /// Monotonic counter bumped by every mutation that would change the
    /// rendered conversation pane. Used as the primary cache key component;
    /// see [`ConversationCacheKey`].
    pub(crate) conversation_version: u64,
    /// Memoised output of the last conversation-pane build. Invalidated by the
    /// cache key computed from `conversation_version` plus the current spinner
    /// phase while a turn is in progress.
    pub(crate) cached_conversation: RefCell<Option<CachedConversation>>,
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
        Self::new_with_storage(restored_chat, None)
    }

    pub fn new_with_storage(restored_chat: Option<Chat>, storage: Option<Arc<Storage>>) -> Self {
        let app_state = AppStateRepository::new().load_or_create().ok();
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
        let (context_left_percent, max_input_tokens, estimated_cost_usd) = derive_model_stats(
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
            in_flight_text: HashMap::new(),
            in_flight_thinking: HashMap::new(),
            conversation_scroll_from_bottom: 0,
            modal: None,
            mention_popup: None,
            workspace_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            ctrl_c_count: 0,
            started_at: Instant::now(),
            turn_in_progress: false,
            last_turn_finish_reason: None,
            mouse_capture_enabled: true,
            storage,
            conversation_version: 0,
            cached_conversation: RefCell::new(None),
        }
    }

    /// Bump the conversation cache version. Call from every mutation that
    /// would change the conversation pane output (history, in-flight buffers,
    /// `turn_in_progress`, `active_chat_id`, or header stats consumed by
    /// [`ui::render_header_lines`]).
    pub(crate) fn invalidate_conversation_cache(&mut self) {
        self.conversation_version = self.conversation_version.wrapping_add(1);
    }

    /// Current spinner phase, if one would be drawn. Matches the cadence used
    /// by `doing_indicator_style` / `pulsing_tool_dot_style` (200ms per phase).
    pub(crate) fn spinner_phase(&self) -> Option<u128> {
        if self.turn_in_progress {
            Some(self.started_at.elapsed().as_millis() / 200)
        } else {
            None
        }
    }

    /// Cache key identifying the current conversation build inputs.
    pub(crate) fn conversation_cache_key(&self) -> ConversationCacheKey {
        ConversationCacheKey {
            version: self.conversation_version,
            spinner_phase: self.spinner_phase(),
        }
    }

    pub fn toggle_mouse_capture(&mut self) {
        self.mouse_capture_enabled = !self.mouse_capture_enabled;
    }

    pub fn active_chat_id(&self) -> Option<Uuid> {
        self.active_chat_id
    }

    pub fn take_submitted_message(&mut self) -> Option<String> {
        self.pending_submission.take()
    }
}
