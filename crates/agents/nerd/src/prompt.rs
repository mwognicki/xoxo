use agentix::tooling::{Tool, ToolSchema};

use crate::tools::{
    ensure_import::EnsureImportTool, eval_script::EvalScriptTool, exec::ExecTool,
    http::HttpTool, process::ProcessTool, read_file::ReadFileTool,
};
use crate::tools::find_files::FindFilesTool;
use crate::tools::find_patterns::FindPatternsTool;
use crate::tools::find_references::FindReferencesTool;
use crate::tools::find_symbol::FindSymbolTool;
use crate::tools::find_tests_for_symbol::FindTestsForSymbolTool;
use crate::tools::inspect_code_structure::InspectCodeStructureTool;
use crate::tools::patch_symbol::PatchSymbolTool;
use crate::tools::rename_symbol::RenameSymbolTool;

/// Build the root nerd agent's system prompt.
///
/// `extra_tool_schemas` are agent-supplied tools layered on top of the
/// nerd base set. The nerd-specific tools (`http_request`, `exec`,
/// `read_file`, `eval_script`, `process`) are always included and declared
/// here — not injected by the caller — because they define what makes an
/// agent a "nerd". Extras with the same name as a base tool override the
/// base schema.
pub fn build_base_prompt(model_name: &str, extra_tool_schemas: &[ToolSchema]) -> String {
    let schemas = merged_schemas(extra_tool_schemas);

    let tools_block = if schemas.is_empty() {
        "Available tools: none.".to_string()
    } else {
        let rendered = schemas
            .iter()
            .map(|schema| format!("- {}: {}", schema.name, schema.description))
            .collect::<Vec<_>>()
            .join("\n");
        format!("Available tools:\n{rendered}")
    };

    format!(
        "You are helpful assistant, helping experienced software engineers develop software.\n\
        You work within a coding agent software, xoxo, and the xoxo mode you operate in (coding \n\
        support) is called nerd. The founder and main engineer behind xoxo is Marek Ognicki, \
        and the company behind xoxo is a Polish company Toturi. \n\
        Quality of your help is measured by how lean and accurate your responses are. \n\
        Anticipating next problems or recommending further steps without explicit instructions \n\
        are detrimental. \n\
        You operate in the CLI environment, so inputs you will receive might be formatted in a way
        specific to terminals. However, xoxo fully supports Markdown, so you are free to format your
        responses in Markdown.
        Your current model name is: {model_name}. \n\
        You have access to following tools you can use for your tasks. Remember that tools can be \n\
        used not only explicitly, but also in clever ways. For example, when you are asked about \n\
        current time, you can always use the shell execution or script evaluation tools. \n\
{tools_block}"
    )
}

/// Nerd base tools, in the order they should appear in the prompt.
fn base_tool_schemas() -> Vec<ToolSchema> {
    vec![
        HttpTool::new().schema(),
        ExecTool::new().schema(),
        ReadFileTool::new().schema(),
        EvalScriptTool::new().schema(),
        ProcessTool::new().schema(),
        FindPatternsTool::new().schema(),
        FindFilesTool::new().schema(),
        FindReferencesTool::new().schema(),
        FindTestsForSymbolTool::new().schema(),
        InspectCodeStructureTool::new().schema(),
        FindSymbolTool::new().schema(),
        RenameSymbolTool::new().schema(),
        PatchSymbolTool::new().schema(),
        EnsureImportTool::new().schema(),
    ]
}

/// Merge base schemas with caller-provided extras.
///
/// Base tools come first; extras are appended in order. When an extra
/// shares a name with a base tool, the extra wins — replacing the base
/// entry in place so the ordering remains stable.
fn merged_schemas(extras: &[ToolSchema]) -> Vec<ToolSchema> {
    let mut merged = base_tool_schemas();
    for extra in extras {
        if let Some(existing) = merged.iter_mut().find(|s| s.name == extra.name) {
            *existing = extra.clone();
        } else {
            merged.push(extra.clone());
        }
    }
    merged
}
