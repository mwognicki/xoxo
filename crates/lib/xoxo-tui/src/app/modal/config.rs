use crossterm::event::KeyEvent;
use xoxo_core::config::{
    CodeQualityConfig, Config, CurrentModelConfig, CurrentProviderConfig,
};

use super::provider_pane::ProviderPane;

/// Active pane within the two-pane config modal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigFocus {
    Navigation,
    Detail,
}

/// Top-level section displayed in the config modal's left navigation pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigSection {
    Providers,
    Skills,
    Mcp,
}

/// Stateful content for the two-pane config modal.
pub struct ConfigModal {
    /// Ordered top-level sections.
    pub sections: Vec<ConfigSection>,
    /// Currently selected section index.
    pub selected_index: usize,
    /// Currently focused pane.
    pub focus: ConfigFocus,
    /// Scroll offset applied to the right-hand detail pane.
    pub detail_scroll: usize,
    /// Snapshot of provider configuration shown in the Providers pane.
    provider_pane: ProviderPane,
}

impl ConfigModal {
    /// Creates a config modal with default top-level sections.
    pub fn new() -> Self {
        Self::from_config(&fallback_config())
    }

    /// Creates a config modal backed by a concrete loaded config snapshot.
    pub fn from_config(config: &Config) -> Self {
        Self {
            sections: vec![
                ConfigSection::Providers,
                ConfigSection::Skills,
                ConfigSection::Mcp,
            ],
            selected_index: 0,
            focus: ConfigFocus::Navigation,
            detail_scroll: 0,
            provider_pane: ProviderPane::from_config(config),
        }
    }

    /// Returns the selected top-level section.
    pub fn selected_section(&self) -> ConfigSection {
        self.sections
            .get(self.selected_index)
            .copied()
            .unwrap_or(ConfigSection::Providers)
    }

    /// Human-readable rows shown in the left navigation pane.
    pub fn section_label(section: ConfigSection) -> &'static str {
        match section {
            ConfigSection::Providers => "Providers",
            ConfigSection::Skills => "Skills",
            ConfigSection::Mcp => "MCP",
        }
    }

    /// Title rendered in the right-hand pane.
    pub fn detail_title(&self) -> &'static str {
        match self.selected_section() {
            ConfigSection::Providers => "Providers",
            ConfigSection::Skills => "Skills",
            ConfigSection::Mcp => "MCP Servers",
        }
    }

    /// Placeholder detail copy for the selected section.
    pub fn detail_lines(&self) -> Vec<String> {
        match self.selected_section() {
            ConfigSection::Providers => self.provider_pane.detail_lines(),
            ConfigSection::Skills => vec![
                "Inspect and manage available skills.".to_string(),
                String::new(),
                "Planned surface:".to_string(),
                "- installed skills list".to_string(),
                "- enabled / disabled state".to_string(),
                "- short descriptions and provenance".to_string(),
                "- install or refresh actions".to_string(),
                String::new(),
                "UX recommendation:".to_string(),
                "Keep the left pane for categories and use the right pane for searchable skill rows."
                    .to_string(),
            ],
            ConfigSection::Mcp => vec![
                "Review MCP servers, tools, and connection health.".to_string(),
                String::new(),
                "Planned surface:".to_string(),
                "- configured server list".to_string(),
                "- transport and command details".to_string(),
                "- connection status".to_string(),
                "- enabled tools / resources summary".to_string(),
                String::new(),
                "UX recommendation:".to_string(),
                "Once wired, Right or Tab should move focus to a per-server list before Enter opens details."
                    .to_string(),
            ],
        }
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) {
        if self.selected_section() == ConfigSection::Providers
            && self.focus == ConfigFocus::Detail
            && self.provider_pane.handle_key(key)
        {
            return;
        }

        match key.code {
            crossterm::event::KeyCode::Tab => self.toggle_focus(),
            crossterm::event::KeyCode::Left => self.focus = ConfigFocus::Navigation,
            crossterm::event::KeyCode::Right => self.focus = ConfigFocus::Detail,
            crossterm::event::KeyCode::Up => self.move_up(),
            crossterm::event::KeyCode::Down => self.move_down(),
            crossterm::event::KeyCode::Home => self.jump_to_start(),
            crossterm::event::KeyCode::End => self.jump_to_end(),
            crossterm::event::KeyCode::PageUp => self.page_up(),
            crossterm::event::KeyCode::PageDown => self.page_down(),
            _ => {}
        }
    }

    pub(crate) fn handle_escape(&mut self) -> bool {
        self.selected_section() == ConfigSection::Providers
            && self.focus == ConfigFocus::Detail
            && self.provider_pane.handle_escape()
    }

    pub(crate) fn current_provider_name(&self) -> &str {
        self.provider_pane.current_provider_name()
    }

    pub(crate) fn footer_hint(&self) -> String {
        match self.selected_section() {
            ConfigSection::Providers => self.provider_pane.footer_hint(),
            ConfigSection::Skills => {
                " Up/Down move  Left/Right focus panes  Tab switch pane  Esc close ".to_string()
            }
            ConfigSection::Mcp => {
                " Up/Down move  Left/Right focus panes  Tab switch pane  Esc close ".to_string()
            }
        }
    }

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            ConfigFocus::Navigation => ConfigFocus::Detail,
            ConfigFocus::Detail => ConfigFocus::Navigation,
        };
    }

    fn move_up(&mut self) {
        match self.focus {
            ConfigFocus::Navigation => {
                self.selected_index = self.selected_index.saturating_sub(1);
                self.detail_scroll = 0;
            }
            ConfigFocus::Detail => {
                if self.selected_section() != ConfigSection::Providers {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                }
            }
        }
    }

    fn move_down(&mut self) {
        match self.focus {
            ConfigFocus::Navigation => {
                if !self.sections.is_empty() {
                    self.selected_index = (self.selected_index + 1).min(self.sections.len() - 1);
                    self.detail_scroll = 0;
                }
            }
            ConfigFocus::Detail => {
                if self.selected_section() != ConfigSection::Providers {
                    self.detail_scroll = self.detail_scroll.saturating_add(1);
                }
            }
        }
    }

    fn jump_to_start(&mut self) {
        match self.focus {
            ConfigFocus::Navigation => {
                self.selected_index = 0;
                self.detail_scroll = 0;
            }
            ConfigFocus::Detail => {
                if self.selected_section() != ConfigSection::Providers {
                    self.detail_scroll = 0;
                }
            }
        }
    }

    fn jump_to_end(&mut self) {
        match self.focus {
            ConfigFocus::Navigation => {
                if !self.sections.is_empty() {
                    self.selected_index = self.sections.len() - 1;
                    self.detail_scroll = 0;
                }
            }
            ConfigFocus::Detail => {
                if self.selected_section() != ConfigSection::Providers {
                    self.detail_scroll = self.detail_lines().len().saturating_sub(1);
                }
            }
        }
    }

    fn page_up(&mut self) {
        match self.focus {
            ConfigFocus::Navigation => self.selected_index = self.selected_index.saturating_sub(3),
            ConfigFocus::Detail => {
                if self.selected_section() != ConfigSection::Providers {
                    self.detail_scroll = self.detail_scroll.saturating_sub(5);
                }
            }
        }
    }

    fn page_down(&mut self) {
        match self.focus {
            ConfigFocus::Navigation => {
                if !self.sections.is_empty() {
                    self.selected_index = (self.selected_index + 3).min(self.sections.len() - 1);
                    self.detail_scroll = 0;
                }
            }
            ConfigFocus::Detail => {
                if self.selected_section() != ConfigSection::Providers {
                    self.detail_scroll = self.detail_scroll.saturating_add(5);
                }
            }
        }
    }
}

fn fallback_config() -> Config {
    Config {
        code_quality: CodeQualityConfig {
            max_lines_in_file: 400,
        },
        current_provider: CurrentProviderConfig {
            name: "openrouter".to_string(),
            compatibility: "open_router".to_string(),
        },
        current_model: CurrentModelConfig {
            model_name: "minimax-m2.5:free".to_string(),
        },
        providers: None,
        mcp_servers: None,
        ui: None,
    }
}
