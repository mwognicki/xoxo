use agentix::tooling::{Tool, ToolSchema};
use agentix::skills::SkillDescriptor;

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
pub fn build_base_prompt(
    model_name: &str,
    extra_tool_schemas: &[ToolSchema],
    has_available_mcp_servers: bool,
) -> String {
    let schemas = merged_schemas(extra_tool_schemas);
    let skills = agentix::skills::discover_available_skills();

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
    let skills_block = render_skills_block(&skills);
    let mcp_block = render_mcp_guidance_block(&schemas, has_available_mcp_servers);

    format!(
        "You are helpful assistant, helping experienced software engineers develop software.\n\
        You work within a coding agent software, xoxo, and the xoxo mode you operate in (coding \n\
        support) is called nerd. Your role is not limited to code generation: when asked, you also help \n\
        with practical engineering and operations tooling such as kubectl, ansible, and similar CLI-driven \n\
        workflows. The founder and main engineer behind xoxo is Marek Ognicki, \
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
{tools_block}\n\
{mcp_block}\n\
{skills_block}"
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

fn render_skills_block(skills: &[SkillDescriptor]) -> String {
    if skills.is_empty() {
        return "Available skills: none.".to_string();
    }

    let rendered = skills
        .iter()
        .map(|skill| format!("- {}: {}", skill.name, skill.description))
        .collect::<Vec<_>>()
        .join("\n");
    format!("Available skills:\n{rendered}")
}

fn render_mcp_guidance_block(
    schemas: &[ToolSchema],
    has_available_mcp_servers: bool,
) -> String {
    if !has_available_mcp_servers {
        return String::new();
    }

    let has_list_servers = schemas.iter().any(|schema| schema.name == "list_mcp_servers");
    let has_list_tools = schemas
        .iter()
        .any(|schema| schema.name == "list_mcp_server_tools");
    let has_describe_tool = schemas
        .iter()
        .any(|schema| schema.name == "describe_mcp_tool");
    let has_invoke_tool = schemas.iter().any(|schema| schema.name == "invoke_mcp_tool");

    if !(has_list_servers || has_list_tools || has_describe_tool || has_invoke_tool) {
        return String::new();
    }

    let mut lines = vec![
        "MCP usage guidance:".to_string(),
        "- Treat MCP capabilities as lazy and discover them on demand; do not assume a specific remote server or tool exists without checking.".to_string(),
    ];

    if has_list_servers {
        lines.push(
            "- Start with `list_mcp_servers` when you need external MCP capabilities and do not yet know which configured server is relevant.".to_string(),
        );
    }
    if has_list_tools {
        lines.push(
            "- Use `list_mcp_server_tools` to inspect the remote tools exposed by one chosen MCP server before attempting invocation.".to_string(),
        );
    }
    if has_describe_tool {
        lines.push(
            "- Use `describe_mcp_tool` when you need the detailed schema or raw descriptor for a remote MCP tool before calling it.".to_string(),
        );
    }
    if has_invoke_tool {
        lines.push(
            "- Use `invoke_mcp_tool` only after you have identified the target server and tool; pass arguments as a JSON object matching the described schema.".to_string(),
        );
        lines.push(
            "- Prefer the sequence discover server -> list tools -> describe tool when needed -> invoke tool, especially when the server or schema is unfamiliar.".to_string(),
        );
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use agentix::tooling::ToolSchema;

    use super::{render_mcp_guidance_block, render_skills_block};

    #[test]
    fn renders_empty_skills_block() {
        assert_eq!(render_skills_block(&[]), "Available skills: none.");
    }

    #[test]
    fn omits_mcp_guidance_when_no_mcp_tools_exist() {
        assert!(render_mcp_guidance_block(&[], false).is_empty());
    }

    #[test]
    fn omits_mcp_guidance_when_no_servers_are_available() {
        let guidance = render_mcp_guidance_block(
            &[ToolSchema {
                name: "list_mcp_servers".to_string(),
                description: String::new(),
                parameters: serde_json::json!({}),
            }],
            false,
        );

        assert!(guidance.is_empty());
    }

    #[test]
    fn renders_mcp_guidance_for_lazy_flow() {
        let guidance = render_mcp_guidance_block(&[
            ToolSchema {
                name: "list_mcp_servers".to_string(),
                description: String::new(),
                parameters: serde_json::json!({}),
            },
            ToolSchema {
                name: "list_mcp_server_tools".to_string(),
                description: String::new(),
                parameters: serde_json::json!({}),
            },
            ToolSchema {
                name: "describe_mcp_tool".to_string(),
                description: String::new(),
                parameters: serde_json::json!({}),
            },
            ToolSchema {
                name: "invoke_mcp_tool".to_string(),
                description: String::new(),
                parameters: serde_json::json!({}),
            },
        ], true);

        assert!(guidance.contains("MCP usage guidance:"));
        assert!(guidance.contains("list_mcp_servers"));
        assert!(guidance.contains("list_mcp_server_tools"));
        assert!(guidance.contains("describe_mcp_tool"));
        assert!(guidance.contains("invoke_mcp_tool"));
        assert!(guidance.contains("discover server -> list tools -> describe tool"));
    }
}
