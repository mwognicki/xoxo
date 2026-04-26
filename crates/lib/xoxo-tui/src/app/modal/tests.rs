use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use xoxo_core::config::{
    CodeQualityConfig, Config, CurrentModelConfig, CurrentProviderConfig, CustomProviderCompatibility,
    ProviderConfig,
};

use super::{ConfigFocus, ConfigModal};

#[test]
fn config_modal_navigation_changes_selected_section() {
    let mut config = ConfigModal::new();
    config.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(config.selected_index, 1);
    assert_eq!(config.detail_scroll, 0);
}

#[test]
fn config_modal_tab_switches_focus() {
    let mut config = ConfigModal::new();
    config.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(config.focus, ConfigFocus::Detail);
}

#[test]
fn providers_pane_reflects_current_provider_and_connected_entries() {
    let config = Config {
        code_quality: CodeQualityConfig {
            max_lines_in_file: 400,
        },
        current_provider: CurrentProviderConfig {
            name: "openai".to_string(),
            compatibility: "open_ai".to_string(),
        },
        current_model: CurrentModelConfig {
            model_name: "gpt-5.4".to_string(),
        },
        providers: Some(vec![
            ProviderConfig::built_in("openai", None, "secret"),
            ProviderConfig::other(
                "Acme Gateway",
                "https://gateway.example.com/v1",
                CustomProviderCompatibility::Anthropic,
                "secret",
            ),
        ]),
        mcp_servers: None,
        ui: None,
    };

    let modal = ConfigModal::from_config(&config);
    let lines = modal.detail_lines();

    assert!(lines.iter().any(|line| line == "Current provider: openai"));
    assert!(lines.iter().any(|line| line == "Connected providers:"));
    assert!(lines.iter().any(|line| line == ">* openai"));
    assert!(lines.iter().any(|line| line == "   built-in"));
    assert!(lines.iter().any(|line| line == " - Acme Gateway"));
    assert!(
        lines.iter()
            .any(|line| line == "   anthropic, https://gateway.example.com/v1")
    );
}
