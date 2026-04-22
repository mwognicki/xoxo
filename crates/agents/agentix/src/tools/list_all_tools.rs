use std::sync::Arc;

use crate::tooling::{ErasedTool, Tool, ToolContext, ToolError, ToolRegistration, ToolSchema};

/// Returns the name and description of every tool registered in the system.
///
/// Intended for agents that compose subagents: call this tool first to
/// discover which tools are available, then include the relevant names in
/// the `spawn_subagent` blueprint. Only agents whose blueprint includes
/// `spawn_subagent` (or the built-in root agent) should be given this tool.
pub struct ListAllToolsTool;

impl Tool for ListAllToolsTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "list_all_tools".into(),
            description: "Return the name and description of every tool registered in the system. \
                Use this before calling spawn_subagent to discover which tools are available \
                to equip a subagent with. Does not filter by the calling agent's own toolset."
                .into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        _input: serde_json::Value,
    ) -> Result<serde_json::Value, ToolError> {
        let tools: Vec<serde_json::Value> = inventory::iter::<ToolRegistration>()
            .map(|r| {
                let schema = (r.factory)().schema();
                serde_json::json!({
                    "name": schema.name,
                    "description": schema.description,
                })
            })
            .collect();

        Ok(serde_json::json!({ "tools": tools }))
    }
}

inventory::submit! {
    ToolRegistration {
        name: "list_all_tools",
        factory: || Arc::new(ListAllToolsTool) as Arc<dyn ErasedTool>,
    }
}
