//! Header art rendering for the main layout.
//!
//! The hedgehog ASCII banner is templated with runtime stats (current model,
//! provider, token counts, cost, workspace root) and converted into styled
//! [`Line`]s for inclusion at the top of the conversation pane.

use ansi_to_tui::IntoText as _;
use ratatui::text::{Line, Text};

use crate::app::App;

use super::{format_context_left, format_estimated_cost};

const HEADER_ART: &str = include_str!("../../hedgehog");

/// Build the styled header banner lines shown above the conversation.
pub(super) fn render_header_lines(app: &App) -> Vec<Line<'static>> {
    let current_dir = app.workspace_root.display().to_string();
    let header_art = HEADER_ART
        .replace("{VERSION}", env!("CARGO_PKG_VERSION"))
        .replace("{PWD}", &current_dir)
        .replace("{MODEL}", &app.current_model_name)
        .replace("{PROVIDER}", &app.current_provider_name)
        .replace("{INPUT_TOKENS}", &app.total_input_tokens.to_string())
        .replace("{OUTPUT_TOKENS}", &app.total_output_tokens.to_string())
        .replace("{CONTEXT_LEFT}", &format_context_left(app.context_left_percent))
        .replace("{EST_COST}", &format_estimated_cost(app.estimated_cost_usd))
        .replace("\\033[", "\x1b[")
        .replace("/\\", "\x1b[38;5;235m/\\\x1b[0m")
        .replace("\\", "\x1b[38;5;235m\\\x1b[0m")
        .replace("/_", "\x1b[38;5;235m/_\x1b[0m")
        .replace("|", "\x1b[38;5;235m|\x1b[0m")
        .replace("‖", "\x1b[38;5;235m• \x1b[0m")
        .replace("<", "\x1b[38;5;235m<\x1b[0m")
        .replace("_", "\x1b[38;5;235m_\x1b[0m")
        .replace("$", "\x1b[38;5;240m$\x1b[0m");

    let header_text = header_art
        .into_text()
        .unwrap_or_else(|_| Text::raw(header_art.clone()));
    header_text.lines.to_vec()
}
