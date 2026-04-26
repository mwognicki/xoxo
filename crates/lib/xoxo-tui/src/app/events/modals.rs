use anyhow::Result;
use chrono::{DateTime, Local};
use uuid::Uuid;
use xoxo_core::config::load_config;
use xoxo_core::storage::ChatSessionSummary;

use crate::app::{App, ConfigModal, Modal, ModalMenu, ModalMenuItem};

const HELP_BODY: &str = "
Available Commands:
  /config  - Open configuration
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
const CONFIG_FOOTER: &str =
    " Up/Down move  Left/Right focus panes  Tab switch pane  PgUp/PgDn jump  Home/End edge  Esc close ";

impl App {
    pub(crate) fn open_help_modal(&mut self) {
        self.modal = Some(Modal::text(" Help ", HELP_BODY, HELP_FOOTER));
    }

    pub(super) fn open_sessions_modal(&mut self) -> Result<()> {
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

    pub(crate) fn open_config_modal(&mut self) {
        let config = load_config();
        self.modal = Some(Modal::config(
            " Config ",
            ConfigModal::from_config(&config),
            CONFIG_FOOTER,
        ));
    }

    pub(super) fn selected_modal_chat_id(&self) -> Option<Uuid> {
        self.modal
            .as_ref()
            .and_then(Modal::selected_value)
            .and_then(selected_chat_id_from_value)
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
