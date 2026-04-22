//! Generic modal overlay state.
//!
//! A [`Modal`] describes a titled block of text rendered on top of the main
//! layout. It is UI-agnostic with respect to *why* it is being shown — the help
//! screen, confirmation prompts, or any future flow all share the same shape.
//!
//! Modals are dismissed via the `Esc` key. Interactive modals can also handle
//! navigation keys before they are dismissed.

use crossterm::event::KeyCode;

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
}

/// Selectable menu state for modal content.
pub struct ModalMenu {
    /// Menu items in display order.
    pub items: Vec<ModalMenuItem>,
    /// Currently selected item index within the full item list.
    pub selected_index: usize,
    /// Current zero-based page index.
    pub page_index: usize,
    /// Number of items rendered on each page.
    pub page_size: usize,
    /// Text rendered when the menu has no items.
    pub empty_message: String,
}

/// Display data for a single modal menu row.
pub struct ModalMenuItem {
    /// Primary row text.
    pub label: String,
    /// Secondary row text.
    pub detail: String,
    /// Optional opaque value associated with the row.
    pub value: Option<String>,
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

    /// Handles up/down/left/right/enter navigation for interactive modals.
    pub fn handle_navigation_key(&mut self, key: KeyCode) {
        let ModalContent::Menu(menu) = &mut self.content else {
            return;
        };
        match key {
            KeyCode::Up => menu.select_previous(),
            KeyCode::Down => menu.select_next(),
            KeyCode::Left => menu.previous_page(),
            KeyCode::Right => menu.next_page(),
            KeyCode::Enter => {}
            _ => {}
        }
    }

    /// Returns the selected menu item's value when this modal contains a menu.
    pub fn selected_value(&self) -> Option<&str> {
        let ModalContent::Menu(menu) = &self.content else {
            return None;
        };
        menu.selected_value()
    }

    /// Returns the footer text with dynamic menu page information when present.
    pub fn footer_text(&self) -> String {
        match &self.content {
            ModalContent::Text(_) => self.footer.clone(),
            ModalContent::Menu(menu) => format!(
                "{} Page {}/{} ",
                self.footer.trim_end(),
                menu.display_page_number(),
                menu.total_pages()
            ),
        }
    }
}

impl ModalMenu {
    /// Builds modal menu state with a fixed page size.
    pub fn new(
        items: Vec<ModalMenuItem>,
        page_size: usize,
        empty_message: impl Into<String>,
    ) -> Self {
        Self {
            items,
            selected_index: 0,
            page_index: 0,
            page_size: page_size.max(1),
            empty_message: empty_message.into(),
        }
    }

    /// Returns the current page's item range.
    pub fn page_bounds(&self) -> (usize, usize) {
        let start = self.page_index.saturating_mul(self.page_size);
        let end = start.saturating_add(self.page_size).min(self.items.len());
        (start, end)
    }

    fn selected_value(&self) -> Option<&str> {
        self.items
            .get(self.selected_index)
            .and_then(|item| item.value.as_deref())
    }

    fn select_previous(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected_index = self.selected_index.saturating_sub(1);
        self.page_index = self.selected_index / self.page_size;
    }

    fn select_next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1).min(self.items.len() - 1);
        self.page_index = self.selected_index / self.page_size;
    }

    fn previous_page(&mut self) {
        if self.page_index == 0 {
            return;
        }
        self.page_index -= 1;
        self.selected_index = self.page_index * self.page_size;
    }

    fn next_page(&mut self) {
        if self.page_index + 1 >= self.total_pages() {
            return;
        }
        self.page_index += 1;
        self.selected_index = self.page_index * self.page_size;
    }

    fn display_page_number(&self) -> usize {
        self.page_index + 1
    }

    fn total_pages(&self) -> usize {
        self.items.len().div_ceil(self.page_size).max(1)
    }
}
