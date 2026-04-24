//! Per-tool output formatting.
//!
//! Each tool kind can customize how its `Started` / `Completed` / `Failed`
//! bus events are rendered in the conversation pane by implementing
//! [`ToolFormatter`]. The trait methods default to the generic rendering used
//! before this module existed, so tools that do not implement a custom
//! formatter keep today's behavior.
//!
//! [`format_started`], [`format_completed`], and [`format_failed`] are the
//! entry points used by `ui.rs`. They pick an implementation based on
//! `tool_name` and fall back to the default trait implementation when no
//! specialization is registered.

mod exec;
mod patch_file;
#[cfg(test)]
mod tests;
mod todo_list;
mod write_file;

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar as _, UnicodeWidthStr as _};
use xoxo_core::chat::structs::{ToolCallCompleted, ToolCallFailed, ToolCallStarted};

use crate::app::App;
use crate::ui::{
    default_tool_byline_with_lookup, default_tool_result_lines, ToolOutcomeLookup,
};

/// Formats the three tool-call bus events for a single tool kind.
///
/// Every method has a default implementation that reproduces the generic
/// rendering (byline for `Started`, dimmed result preview for `Completed`,
/// red error preview for `Failed`). Concrete implementations override only
/// the methods whose output they want to customize.
pub trait ToolFormatter {
    fn format_started(
        &self,
        app: &App,
        tool_outcomes: &ToolOutcomeLookup<'_>,
        started: &ToolCallStarted,
    ) -> Vec<Line<'static>> {
        vec![default_tool_byline_with_lookup(app, tool_outcomes, started)]
    }

    fn format_completed(
        &self,
        _app: &App,
        completed: &ToolCallCompleted,
        _viewport_width: u16,
    ) -> Vec<Line<'static>> {
        default_tool_result_lines(&completed.result_preview, false)
    }

    fn format_failed(&self, _app: &App, failed: &ToolCallFailed) -> Vec<Line<'static>> {
        default_tool_result_lines(&failed.message, true)
    }
}

/// Fallback formatter used when a tool name has no dedicated implementation.
///
/// Relies entirely on the default trait methods.
struct DefaultToolFormatter;

impl ToolFormatter for DefaultToolFormatter {}

pub(super) fn subtle_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub(super) fn divider_style() -> Style {
    Style::default().fg(Color::Indexed(240))
}

pub(super) fn file_extension(file_path: &str) -> &str {
    file_path
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .unwrap_or_default()
}

pub(super) fn fit_to_width(content: &str, width: usize) -> String {
    let content_width = content.width();
    if content_width <= width {
        return format!("{content:<width$}");
    }

    if width <= 3 {
        return ".".repeat(width);
    }

    let mut fitted = String::new();
    let mut used_width = 0usize;
    let target_width = width - 3;

    for ch in content.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if used_width + ch_width > target_width {
            break;
        }
        fitted.push(ch);
        used_width += ch_width;
    }

    fitted.push_str("...");
    let fitted_width = fitted.width();
    if fitted_width < width {
        fitted.push_str(&" ".repeat(width - fitted_width));
    }
    fitted
}

pub(super) fn prefixed_code_line(
    prefix: &str,
    mut line: Line<'static>,
    fallback_style: Style,
) -> Line<'static> {
    let mut spans = vec![Span::styled(prefix.to_string(), fallback_style)];
    if line.spans.is_empty() {
        spans.push(Span::styled(String::new(), fallback_style));
    } else {
        for span in line.spans.drain(..) {
            let content = span.content.to_string();
            let style = fallback_style.patch(span.style);
            spans.push(Span::styled(content, style));
        }
    }
    Line::from(spans)
}

/// Resolves a `tool_name` to the formatter that should render its events.
///
/// New per-tool formatters are added as sibling types with their own match
/// arm; anything unregistered falls through to [`DefaultToolFormatter`].
fn formatter_for(tool_name: &str) -> Box<dyn ToolFormatter> {
    match tool_name {
        "exec" => Box::new(exec::ExecFormatter),
        "patch_file" => Box::new(patch_file::PatchFileFormatter),
        "write_todo_list" => Box::new(todo_list::WriteTodoListFormatter),
        "write_file" => Box::new(write_file::WriteFileFormatter),
        _ => Box::new(DefaultToolFormatter),
    }
}

/// Render a `ToolCallStarted` event using the formatter registered for its tool.
pub fn format_started(
    app: &App,
    tool_outcomes: &ToolOutcomeLookup<'_>,
    started: &ToolCallStarted,
) -> Vec<Line<'static>> {
    formatter_for(&started.tool_name).format_started(app, tool_outcomes, started)
}

/// Render a `ToolCallCompleted` event using the formatter registered for its tool.
pub fn format_completed(
    app: &App,
    completed: &ToolCallCompleted,
    viewport_width: u16,
) -> Vec<Line<'static>> {
    formatter_for(&completed.tool_name).format_completed(app, completed, viewport_width)
}

/// Render a `ToolCallFailed` event using the formatter registered for its tool.
pub fn format_failed(app: &App, failed: &ToolCallFailed) -> Vec<Line<'static>> {
    formatter_for(&failed.tool_name).format_failed(app, failed)
}
