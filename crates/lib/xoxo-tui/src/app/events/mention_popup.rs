use crate::app::{App, FileWalkerMentionPopup};

impl App {
    pub(crate) fn open_mention_popup(&mut self) {
        let trigger_at = self.input.len().saturating_sub(1);
        debug_assert!(self.input.is_char_boundary(trigger_at));
        match FileWalkerMentionPopup::open(&self.workspace_root, trigger_at) {
            Ok(popup) => self.mention_popup = Some(popup),
            Err(_) => self.mention_popup = None,
        }
    }

    pub(crate) fn input_allows_mention_popup(&self) -> bool {
        let trigger_at = self.input.len().saturating_sub(1);
        self.input[..trigger_at]
            .chars()
            .next_back()
            .is_none_or(char::is_whitespace)
    }

    pub(crate) fn refresh_mention_filter(&mut self) {
        if let Some(popup) = &mut self.mention_popup {
            let start = popup.trigger_at + 1;
            let filter = if start < self.input.len() {
                &self.input[start..]
            } else {
                ""
            };
            popup.set_filter(filter);
        }
    }

    pub(crate) fn handle_mention_tab(&mut self) {
        let selection = self
            .mention_popup
            .as_ref()
            .and_then(|popup| {
                popup
                    .selected_entry()
                    .cloned()
                    .map(|entry| (popup.trigger_at, entry))
            });
        let Some((trigger_at, entry)) = selection else {
            self.mention_popup = None;
            return;
        };

        if entry.is_dir && self.mention_text_after(trigger_at) != entry.rel_path {
            self.replace_mention_text(trigger_at, &entry.rel_path);
            self.refresh_mention_filter();
            return;
        }

        self.commit_mention_selection();
    }

    pub(crate) fn mention_text_after(&self, trigger_at: usize) -> &str {
        let start = trigger_at + 1;
        if start < self.input.len() {
            &self.input[start..]
        } else {
            ""
        }
    }

    pub(crate) fn replace_mention_text(&mut self, trigger_at: usize, replacement: &str) {
        debug_assert!(self.input.is_char_boundary(trigger_at));
        self.input.truncate(trigger_at);
        self.input.push('@');
        self.input.push_str(replacement);
    }

    pub(crate) fn commit_mention_selection(&mut self) {
        let popup = self.mention_popup.take();
        if let Some(popup) = popup
            && let Some(entry) = popup.selected_entry()
        {
            let trigger_at = popup.trigger_at;
            let committed_path = if entry.is_dir {
                format!("{}/", entry.rel_path)
            } else {
                entry.rel_path.clone()
            };
            self.replace_mention_text(trigger_at, &committed_path);
            self.input.push(' ');
        }
    }
}
