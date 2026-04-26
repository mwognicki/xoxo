use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use xoxo_core::config::{
    save_config, Config, CurrentProviderConfig, CustomProviderCompatibility, ProviderConfig,
};
use xoxo_core::llm::ProviderRegistry;

pub(super) struct ProviderPane {
    config: Config,
    selected_index: usize,
    mode: ProviderPaneMode,
    built_in_options: Vec<BuiltInProviderOption>,
}

enum ProviderPaneMode {
    Browse,
    ConnectKind { selected_index: usize },
    Edit(ProviderEditor),
}

struct ProviderEditor {
    draft: ProviderDraft,
    selected_field: usize,
}

struct ProviderDraft {
    provider_index: Option<usize>,
    kind: DraftKind,
    provider_name: String,
    compatibility: String,
    base_url: String,
    api_key: String,
}

enum DraftKind {
    BuiltIn { selected_builtin_index: usize },
    Custom,
}

struct BuiltInProviderOption {
    id: String,
    name: String,
    compatibility: String,
}

impl ProviderPane {
    pub(super) fn from_config(config: &Config) -> Self {
        let mut pane = Self {
            config: config.clone(),
            selected_index: 0,
            mode: ProviderPaneMode::Browse,
            built_in_options: built_in_provider_options(),
        };
        pane.clamp_selected_index();
        pane
    }

    pub(super) fn current_provider_name(&self) -> &str {
        &self.config.current_provider().name
    }

    pub(super) fn detail_lines(&self) -> Vec<String> {
        match &self.mode {
            ProviderPaneMode::Browse => self.browse_lines(),
            ProviderPaneMode::ConnectKind { selected_index } => connect_kind_lines(*selected_index),
            ProviderPaneMode::Edit(editor) => self.editor_lines(editor),
        }
    }

    pub(super) fn footer_hint(&self) -> String {
        match &self.mode {
            ProviderPaneMode::Browse => {
                " Up/Down select  Enter edit  c current  d disconnect  n connect  Left/Tab sections  Esc close ".to_string()
            }
            ProviderPaneMode::ConnectKind { .. } => {
                " Up/Down choose  Enter continue  Esc cancel ".to_string()
            }
            ProviderPaneMode::Edit(_) => {
                " Up/Down field  Type edit  Backspace delete  Tab next  Shift+Tab prev  Enter save  Esc cancel ".to_string()
            }
        }
    }

    pub(super) fn handle_escape(&mut self) -> bool {
        match self.mode {
            ProviderPaneMode::Browse => false,
            _ => {
                self.mode = ProviderPaneMode::Browse;
                true
            }
        }
    }

    pub(super) fn handle_key(&mut self, key: KeyEvent) -> bool {
        let mode = std::mem::replace(&mut self.mode, ProviderPaneMode::Browse);
        let (handled, next_mode) = match mode {
            ProviderPaneMode::Browse => {
                let handled = self.handle_browse_key(key);
                (handled, std::mem::replace(&mut self.mode, ProviderPaneMode::Browse))
            }
            ProviderPaneMode::ConnectKind { mut selected_index } => {
                let handled = self.handle_connect_kind_key(key, &mut selected_index);
                let mode = std::mem::replace(
                    &mut self.mode,
                    ProviderPaneMode::ConnectKind { selected_index },
                );
                (handled, mode)
            }
            ProviderPaneMode::Edit(mut editor) => {
                let handled = self.handle_edit_key(key, &mut editor);
                let mode = std::mem::replace(&mut self.mode, ProviderPaneMode::Edit(editor));
                (handled, mode)
            }
        };
        self.mode = next_mode;
        handled
    }

    fn handle_browse_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Up => {
                self.selected_index = self.selected_index.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                if self.selected_index + 1 < self.config.providers().len() {
                    self.selected_index += 1;
                }
                true
            }
            KeyCode::Char('n') => {
                self.mode = ProviderPaneMode::ConnectKind { selected_index: 0 };
                true
            }
            KeyCode::Char('c') => {
                self.make_selected_current();
                true
            }
            KeyCode::Char('d') => {
                self.disconnect_selected();
                true
            }
            KeyCode::Enter | KeyCode::Char('e') => {
                if self.config.providers().is_empty() {
                    self.mode = ProviderPaneMode::ConnectKind { selected_index: 0 };
                } else {
                    self.open_editor_for_selected();
                }
                true
            }
            _ => false,
        }
    }

    fn handle_connect_kind_key(&mut self, key: KeyEvent, selected_index: &mut usize) -> bool {
        match key.code {
            KeyCode::Up => {
                *selected_index = selected_index.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                *selected_index = (*selected_index + 1).min(2);
                true
            }
            KeyCode::Enter => {
                let next_mode = match *selected_index {
                    0 => ProviderPaneMode::Edit(ProviderEditor::new_built_in(&self.built_in_options)),
                    1 => ProviderPaneMode::Edit(ProviderEditor::new_custom("open_ai")),
                    _ => ProviderPaneMode::Edit(ProviderEditor::new_custom("anthropic")),
                };
                self.mode = next_mode;
                true
            }
            _ => false,
        }
    }

    fn handle_edit_key(&mut self, key: KeyEvent, editor: &mut ProviderEditor) -> bool {
        match key.code {
            KeyCode::Up => {
                editor.selected_field = editor.selected_field.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                let last_field = editor.field_count().saturating_sub(1);
                editor.selected_field = (editor.selected_field + 1).min(last_field);
                true
            }
            KeyCode::Tab => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    editor.selected_field = editor.selected_field.saturating_sub(1);
                } else {
                    let last_field = editor.field_count().saturating_sub(1);
                    editor.selected_field = (editor.selected_field + 1).min(last_field);
                }
                true
            }
            KeyCode::Backspace => {
                editor.selected_value_mut().pop();
                true
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                editor.selected_value_mut().push(c);
                true
            }
            KeyCode::Enter => {
                self.save_editor(editor);
                true
            }
            _ => false,
        }
    }

    fn browse_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("Current provider: {}", self.config.current_provider().name),
            format!(
                "Compatibility: {}",
                self.config.current_provider().compatibility
            ),
            String::new(),
            "Connected providers:".to_string(),
        ];

        if self.config.providers().is_empty() {
            lines.push("No providers configured in config.toml.".to_string());
        } else {
            for (index, provider) in self.config.providers().iter().enumerate() {
                let is_selected = index == self.selected_index;
                let prefix = if is_selected { ">" } else { " " };
                let current_marker = if provider_matches_current(provider, &self.config) {
                    "*"
                } else {
                    "-"
                };
                lines.push(format!(
                    "{prefix}{current_marker} {}",
                    provider_label(provider)
                ));
                lines.push(format!("   {}", provider_detail(provider)));
            }
        }

        lines.push(String::new());
        lines.push("Actions:".to_string());
        lines.push("Enter edit highlighted provider".to_string());
        lines.push("c set highlighted provider as current".to_string());
        lines.push("d disconnect highlighted provider when it is not current".to_string());
        lines.push("n connect a new provider".to_string());
        lines
    }

    fn editor_lines(&self, editor: &ProviderEditor) -> Vec<String> {
        let mut lines = vec![
            editor.title().to_string(),
            String::new(),
        ];
        for (index, (label, value, masked)) in editor.display_fields().into_iter().enumerate() {
            let prefix = if index == editor.selected_field { ">" } else { " " };
            let display_value = if masked {
                "*".repeat(value.chars().count().max(1))
            } else if value.is_empty() {
                "<empty>".to_string()
            } else {
                value
            };
            lines.push(format!("{prefix} {label}: {display_value}"));
        }
        lines.push(String::new());
        lines.push("Press Enter to save this provider.".to_string());
        lines
    }

    fn open_editor_for_selected(&mut self) {
        let Some(provider) = self.config.providers().get(self.selected_index) else {
            return;
        };
        self.mode = ProviderPaneMode::Edit(ProviderEditor::from_provider(
            provider,
            self.selected_index,
            &self.built_in_options,
        ));
    }

    fn make_selected_current(&mut self) {
        let Some(provider) = self.config.providers().get(self.selected_index) else {
            return;
        };
        let current_provider = match provider.provider_id() {
            Some(provider_id) => built_in_current_provider(provider_id, &self.built_in_options),
            None => CurrentProviderConfig {
                name: provider.custom_name().unwrap_or_default().to_string(),
                compatibility: provider
                    .custom_compatibility()
                    .map(custom_compatibility_label)
                    .unwrap_or("open_ai")
                    .to_string(),
            },
        };
        self.config.current_provider = current_provider;
        let _ = save_config(&self.config);
    }

    fn disconnect_selected(&mut self) {
        let Some(provider) = self.config.providers().get(self.selected_index) else {
            return;
        };
        if provider_matches_current(provider, &self.config) {
            return;
        }
        let mut providers = self.config.providers.clone().unwrap_or_default();
        if self.selected_index < providers.len() {
            providers.remove(self.selected_index);
            self.config.providers = (!providers.is_empty()).then_some(providers);
            self.clamp_selected_index();
            let _ = save_config(&self.config);
        }
    }

    fn save_editor(&mut self, editor: &ProviderEditor) {
        let draft = &editor.draft;
        let provider = match &draft.kind {
            DraftKind::BuiltIn {
                selected_builtin_index,
            } => {
                let Some(option) =
                    resolve_built_in_option(&draft.provider_name, &self.built_in_options)
                        .or_else(|| self.built_in_options.get(*selected_builtin_index))
                else {
                    return;
                };
                ProviderConfig::built_in(
                    option.id.clone(),
                    optional_string(&draft.base_url),
                    draft.api_key.clone(),
                )
            }
            DraftKind::Custom => ProviderConfig::other(
                draft.provider_name.clone(),
                draft.base_url.clone(),
                parse_custom_compatibility(&draft.compatibility),
                draft.api_key.clone(),
            ),
        };

        let provider = match &draft.kind {
            DraftKind::BuiltIn { .. } => provider,
            DraftKind::Custom => provider,
        };

        let mut providers = self.config.providers.clone().unwrap_or_default();
        if let Some(provider_index) = draft.provider_index {
            let was_current = providers
                .get(provider_index)
                .is_some_and(|existing| provider_matches_current(existing, &self.config));
            if provider_index < providers.len() {
                providers[provider_index] = provider.clone();
            }
            if was_current {
                self.config.current_provider = current_provider_from_provider(
                    &provider,
                    &self.built_in_options,
                );
            }
        } else {
            providers.push(provider);
            self.selected_index = providers.len().saturating_sub(1);
        }
        self.config.providers = Some(providers);
        let _ = save_config(&self.config);
        self.mode = ProviderPaneMode::Browse;
    }

    fn clamp_selected_index(&mut self) {
        if self.config.providers().is_empty() {
            self.selected_index = 0;
        } else {
            self.selected_index = self.selected_index.min(self.config.providers().len() - 1);
        }
    }
}

impl ProviderEditor {
    fn new_built_in(options: &[BuiltInProviderOption]) -> Self {
        let selected_builtin_index = 0;
        let provider_name = options
            .get(selected_builtin_index)
            .map(|option| option.id.clone())
            .unwrap_or_default();
        let compatibility = options
            .get(selected_builtin_index)
            .map(|option| option.compatibility.clone())
            .unwrap_or_else(|| "open_ai".to_string());
        Self {
            draft: ProviderDraft {
                provider_index: None,
                kind: DraftKind::BuiltIn {
                    selected_builtin_index,
                },
                provider_name,
                compatibility,
                base_url: String::new(),
                api_key: String::new(),
            },
            selected_field: 0,
        }
    }

    fn new_custom(compatibility: &str) -> Self {
        Self {
            draft: ProviderDraft {
                provider_index: None,
                kind: DraftKind::Custom,
                provider_name: String::new(),
                compatibility: compatibility.to_string(),
                base_url: String::new(),
                api_key: String::new(),
            },
            selected_field: 0,
        }
    }

    fn from_provider(
        provider: &ProviderConfig,
        provider_index: usize,
        options: &[BuiltInProviderOption],
    ) -> Self {
        if let Some(provider_id) = provider.provider_id() {
            let selected_builtin_index = options
                .iter()
                .position(|option| option.id == provider_id)
                .unwrap_or(0);
            return Self {
                draft: ProviderDraft {
                    provider_index: Some(provider_index),
                    kind: DraftKind::BuiltIn {
                        selected_builtin_index,
                    },
                    provider_name: provider_id.to_string(),
                    compatibility: options
                        .get(selected_builtin_index)
                        .map(|option| option.compatibility.clone())
                        .unwrap_or_else(|| "open_ai".to_string()),
                    base_url: provider.effective_base_url().unwrap_or_default(),
                    api_key: provider.api_key.clone(),
                },
                selected_field: 1,
            };
        }

        Self {
            draft: ProviderDraft {
                provider_index: Some(provider_index),
                kind: DraftKind::Custom,
                provider_name: provider.custom_name().unwrap_or_default().to_string(),
                compatibility: provider
                    .custom_compatibility()
                    .map(custom_compatibility_label)
                    .unwrap_or("open_ai")
                    .to_string(),
                base_url: provider.effective_base_url().unwrap_or_default(),
                api_key: provider.api_key.clone(),
            },
            selected_field: 0,
        }
    }

    fn title(&self) -> &'static str {
        match self.draft.kind {
            DraftKind::BuiltIn { .. } => "Edit provider",
            DraftKind::Custom => "Edit custom provider",
        }
    }

    fn field_count(&self) -> usize {
        match self.draft.kind {
            DraftKind::BuiltIn { .. } => 3,
            DraftKind::Custom => 4,
        }
    }

    fn display_fields(&self) -> Vec<(String, String, bool)> {
        match &self.draft.kind {
            DraftKind::BuiltIn {
                selected_builtin_index,
            } => vec![
                (
                    "Provider".to_string(),
                    self.draft.provider_name_for_builtin(*selected_builtin_index),
                    false,
                ),
                ("Base URL".to_string(), self.draft.base_url.clone(), false),
                ("API key".to_string(), self.draft.api_key.clone(), true),
            ],
            DraftKind::Custom => vec![
                ("Name".to_string(), self.draft.provider_name.clone(), false),
                (
                    "Compatibility".to_string(),
                    self.draft.compatibility.clone(),
                    false,
                ),
                ("Base URL".to_string(), self.draft.base_url.clone(), false),
                ("API key".to_string(), self.draft.api_key.clone(), true),
            ],
        }
    }

    fn selected_value_mut(&mut self) -> &mut String {
        match &mut self.draft.kind {
            DraftKind::BuiltIn { .. } => match self.selected_field {
                0 => &mut self.draft.provider_name,
                1 => &mut self.draft.base_url,
                _ => &mut self.draft.api_key,
            },
            DraftKind::Custom => match self.selected_field {
                0 => &mut self.draft.provider_name,
                1 => &mut self.draft.compatibility,
                2 => &mut self.draft.base_url,
                _ => &mut self.draft.api_key,
            },
        }
    }
}

impl ProviderDraft {
    fn provider_name_for_builtin(&self, selected_builtin_index: usize) -> String {
        if self.provider_name.is_empty() {
            return String::new();
        }
        let _ = selected_builtin_index;
        self.provider_name.clone()
    }
}

fn connect_kind_lines(selected_index: usize) -> Vec<String> {
    let options = [
        "Well-known provider",
        "Custom OpenAI-compatible provider",
        "Custom Anthropic-compatible provider",
    ];
    let mut lines = vec![
        "Connect a provider".to_string(),
        String::new(),
    ];
    for (index, option) in options.iter().enumerate() {
        let prefix = if index == selected_index { ">" } else { " " };
        lines.push(format!("{prefix} {option}"));
    }
    lines
}

fn built_in_provider_options() -> Vec<BuiltInProviderOption> {
    let mut options = ProviderRegistry::new()
        .all()
        .into_iter()
        .map(|provider| BuiltInProviderOption {
            id: provider.id.clone(),
            name: provider.name.clone(),
            compatibility: format!("{:?}", provider.compatibility).to_lowercase(),
        })
        .collect::<Vec<_>>();
    options.sort_by(|left, right| left.name.cmp(&right.name));
    options
}

fn resolve_built_in_option<'a>(
    value: &str,
    options: &'a [BuiltInProviderOption],
) -> Option<&'a BuiltInProviderOption> {
    options.iter().find(|option| {
        option.id.eq_ignore_ascii_case(value) || option.name.eq_ignore_ascii_case(value)
    })
}

fn provider_matches_current(provider: &ProviderConfig, config: &Config) -> bool {
    provider.provider_id() == Some(config.current_provider().name.as_str())
        || provider.custom_name() == Some(config.current_provider().name.as_str())
}

fn current_provider_from_provider(
    provider: &ProviderConfig,
    built_in_options: &[BuiltInProviderOption],
) -> CurrentProviderConfig {
    match provider.provider_id() {
        Some(provider_id) => built_in_current_provider(provider_id, built_in_options),
        None => CurrentProviderConfig {
            name: provider.custom_name().unwrap_or_default().to_string(),
            compatibility: provider
                .custom_compatibility()
                .map(custom_compatibility_label)
                .unwrap_or("open_ai")
                .to_string(),
        },
    }
}

fn built_in_current_provider(
    provider_id: &str,
    built_in_options: &[BuiltInProviderOption],
) -> CurrentProviderConfig {
    let compatibility = built_in_options
        .iter()
        .find(|option| option.id == provider_id)
        .map(|option| option.compatibility.clone())
        .unwrap_or_else(|| "open_ai".to_string());
    CurrentProviderConfig {
        name: provider_id.to_string(),
        compatibility,
    }
}

fn provider_label(provider: &ProviderConfig) -> String {
    provider
        .provider_id()
        .or_else(|| provider.custom_name())
        .unwrap_or("unnamed provider")
        .to_string()
}

fn provider_detail(provider: &ProviderConfig) -> String {
    match provider.provider_id() {
        Some(_) => provider
            .effective_base_url()
            .map(|base_url| format!("built-in, base URL: {base_url}"))
            .unwrap_or_else(|| "built-in".to_string()),
        None => {
            let compatibility = provider
                .custom_compatibility()
                .map(custom_compatibility_label)
                .unwrap_or("unknown");
            provider
                .effective_base_url()
                .map(|base_url| format!("{compatibility}, {base_url}"))
                .unwrap_or_else(|| compatibility.to_string())
        }
    }
}

fn custom_compatibility_label(compatibility: &CustomProviderCompatibility) -> &'static str {
    match compatibility {
        CustomProviderCompatibility::OpenAi => "open_ai",
        CustomProviderCompatibility::Anthropic => "anthropic",
    }
}

fn parse_custom_compatibility(value: &str) -> CustomProviderCompatibility {
    if value.eq_ignore_ascii_case("anthropic") {
        CustomProviderCompatibility::Anthropic
    } else {
        CustomProviderCompatibility::OpenAi
    }
}

fn optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
