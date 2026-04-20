use xoxo_core::tooling::ToolSchema;

pub fn build_base_prompt(model_name: &str, tool_schemas: &[ToolSchema]) -> String {
    let tools_block = if tool_schemas.is_empty() {
        "Available tools: none.".to_string()
    } else {
        let rendered = tool_schemas
            .iter()
            .map(|schema| format!("- {}: {}", schema.name, schema.description))
            .collect::<Vec<_>>()
            .join("\n");
        format!("Available tools:\n{rendered}")
    };

    format!(
        "You are helpful assistant, who answers questions very briefly.\n\
        You are operating within a CLI tool called `xoxo`.\n\
Current model: {model_name}\n\
{tools_block}"
    )
}
