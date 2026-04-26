//! Generic modal overlay state.
//!
//! A [`Modal`] describes a titled block of text rendered on top of the main
//! layout. It is UI-agnostic with respect to *why* it is being shown — the help
//! screen, confirmation prompts, or any future flow all share the same shape.
//!
//! Modals are dismissed via the `Esc` key. Interactive modals can also handle
//! navigation keys before they are dismissed.

mod config;
mod menu;
mod provider_pane;

#[cfg(test)]
mod tests;

use crossterm::event::KeyCode;

pub use config::{ConfigFocus, ConfigModal};
pub use menu::{ModalMenu, ModalMenuItem};

/// Content of a modal overlay currently shown to the user.
pub struct Modal {
    /// Text rendered as the modal's border title (already framed with spaces,
    /// e.g. `" Help "`).
    pub title: String,
    /// Modal body content.
    pub content: ModalContent,
    /// Key-binding hint rendered on the modal's bottom border.
    pub footer: String,
}

/// Body content rendered inside a modal overlay.
pub enum ModalContent {
    /// Plain text modal body.
    Text(String),
    /// Selectable, paginated menu body.
    Menu(ModalMenu),
    /// Two-pane configuration shell used for future settings workflows.
    Config(ConfigModal),
}

impl Modal {
    /// Builds a plain text modal.
    pub fn text(
        title: impl Into<String>,
        body: impl Into<String>,
        footer: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            content: ModalContent::Text(body.into()),
            footer: footer.into(),
        }
    }

    /// Builds a modal containing a selectable menu.
    pub fn menu(
        title: impl Into<String>,
        menu: ModalMenu,
        footer: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            content: ModalContent::Menu(menu),
            footer: footer.into(),
        }
    }

    /// Builds the dedicated config modal shell.
    pub fn config(
        title: impl Into<String>,
        config: ConfigModal,
        footer: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            content: ModalContent::Config(config),
            footer: footer.into(),
        }
    }

    /// Handles up/down/left/right/enter navigation for interactive modals.
    pub fn handle_navigation_key(&mut self, key: KeyCode) {
        match &mut self.content {
            ModalContent::Menu(menu) => menu.handle_navigation_key(key),
            ModalContent::Config(_) => {}
            ModalContent::Text(_) => {}
        }
    }

    /// Returns the selected menu item's value when this modal contains a menu.
    pub fn selected_value(&self) -> Option<&str> {
        let ModalContent::Menu(menu) = &self.content else {
            return None;
        };
        menu.selected_value()
    }

    /// Returns the footer text with dynamic navigation information when present.
    pub fn footer_text(&self) -> String {
        match &self.content {
            ModalContent::Text(_) => self.footer.clone(),
            ModalContent::Menu(menu) => format!(
                "{} Page {}/{} ",
                self.footer.trim_end(),
                menu.display_page_number(),
                menu.total_pages()
            ),
            ModalContent::Config(config) => format!(
                "{} {} Section {}/{} ",
                self.footer.trim_end(),
                config.footer_hint().trim(),
                config.selected_index + 1,
                config.sections.len().max(1)
            ),
        }
    }
}
