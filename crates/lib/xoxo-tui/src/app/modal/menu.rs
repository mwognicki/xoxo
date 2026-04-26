use crossterm::event::KeyCode;

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

    pub(crate) fn handle_navigation_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up => self.select_previous(),
            KeyCode::Down => self.select_next(),
            KeyCode::Left => self.previous_page(),
            KeyCode::Right => self.next_page(),
            KeyCode::Enter => {}
            _ => {}
        }
    }

    pub(crate) fn selected_value(&self) -> Option<&str> {
        self.items
            .get(self.selected_index)
            .and_then(|item| item.value.as_deref())
    }

    pub(crate) fn display_page_number(&self) -> usize {
        self.page_index + 1
    }

    pub(crate) fn total_pages(&self) -> usize {
        self.items.len().div_ceil(self.page_size).max(1)
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
}
