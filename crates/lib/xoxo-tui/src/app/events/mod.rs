mod modals;

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};

use crate::app::{App, LayoutMode, ModalContent};
use crate::app::commands::{SlashCommand, inline_suggestion_suffix, resolve_slash_command};

impl App {
    pub fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Paste(content) => {
                if self.modal.is_some() {
                    return Ok(());
                }
                self.mention_popup = None;
                self.input.push_str(&content);
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

                if self.modal.is_some() {
                    if let Some(modal) = &mut self.modal {
                        match &mut modal.content {
                            ModalContent::Config(config) => {
                                if matches!(key.code, KeyCode::Esc) {
                                    if !config.handle_escape() {
                                        self.modal = None;
                                    }
                                } else {
                                    config.handle_key(key);
                                    self.current_provider_name =
                                        config.current_provider_name().to_string();
                                }
                            }
                            _ if matches!(key.code, KeyCode::Esc) => {
                                self.modal = None;
                            }
                            _ if matches!(key.code, KeyCode::Enter) => {
                                if let Some(chat_id) = self.selected_modal_chat_id() {
                                    self.load_chat_session(chat_id)?;
                                    self.modal = None;
                                }
                            }
                            _ => {
                                modal.handle_navigation_key(key.code);
                            }
                        }
                    }
                    return Ok(());
                }

                if let Some(popup) = &mut self.mention_popup {
                    match key.code {
                        KeyCode::Tab => {
                            self.handle_mention_tab();
                            return Ok(());
                        }
                        KeyCode::Enter => {
                            self.commit_mention_selection();
                            return Ok(());
                        }
                        KeyCode::Esc => {
                            self.mention_popup = None;
                            return Ok(());
                        }
                        KeyCode::Up => {
                            popup.select_prev();
                            return Ok(());
                        }
                        KeyCode::Down => {
                            popup.select_next();
                            return Ok(());
                        }
                        _ => {}
                    }
                }

                let is_ctrl_s = matches!(key.code, KeyCode::Char('s'))
                    && key.modifiers.contains(KeyModifiers::CONTROL);
                if is_ctrl_s {
                    self.toggle_mouse_capture();
                    return Ok(());
                }

                if matches!(key.code, KeyCode::Tab)
                    && let Some(suffix) = inline_suggestion_suffix(&self.input)
                {
                    self.input.push_str(suffix);
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
                    }
                    KeyCode::Backspace => {
                        self.input.pop();
                    }
                    KeyCode::Enter => {
                        let line: String = self.input.drain(..).collect();
                        match resolve_slash_command(&line) {
                            Some(SlashCommand::Quit) => self.running = false,
                            Some(SlashCommand::Clear | SlashCommand::New) => {
                                self.reset_for_new_chat();
                            }
                            Some(SlashCommand::Config) => self.open_config_modal(),
                            Some(SlashCommand::Help) => self.open_help_modal(),
                            Some(SlashCommand::Sessions) => self.open_sessions_modal()?,
                            _ if !line.is_empty() && !line.starts_with('/') => {
                                self.pending_submission = Some(line);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }

                if let Some(popup) = &mut self.mention_popup {
                    match key.code {
                        KeyCode::Backspace => {
                            if self.input.len() <= popup.trigger_at {
                                self.mention_popup = None;
                            } else {
                                self.refresh_mention_filter();
                            }
                        }
                        KeyCode::Char(c)
                            if !key
                                .modifiers
                                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                        {
                            if c.is_whitespace() {
                                self.mention_popup = None;
                            } else {
                                self.refresh_mention_filter();
                            }
                        }
                        _ => {}
                    }
                } else if let KeyCode::Char('@') = key.code
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                    && self.input_allows_mention_popup()
                {
                    self.open_mention_popup();
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
mod tests;
mod mention_popup;
