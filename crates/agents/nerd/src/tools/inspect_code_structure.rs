use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentix::tooling::{
    ErasedTool, Tool, ToolContext, ToolError, ToolMetadata, ToolRegistration, ToolSchema,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::coding::{CodeStructureError, inspect_code_structure};

#[derive(Debug, Deserialize)]
struct InspectCodeStructureInput {
    file_path: String,
}

/// Tool implementation for AST-backed source-code structure inspection.
pub struct InspectCodeStructureTool;

impl InspectCodeStructureTool {
    /// Create a new source-code structure inspection tool.
    pub fn new() -> Self {
        Self
    }
}

impl Tool for InspectCodeStructureTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "inspect_code_structure".to_string(),
            description: "Inspect a supported source file with an AST parser and return bounded structural facts such as imports, types, functions, methods, and line ranges.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["file_path"],
                "additionalProperties": false,
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the source file, absolute or relative to the current working directory."
                    }
                }
            }),
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let language = output["language"].as_str().unwrap_or("unknown");
        let item_count = output["items"].as_array().map_or(0, Vec::len);
        format!("Inspected {language} code structure ({item_count} item(s))")
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            is_read_only: true,
            supports_concurrent_invocation: true,
        }
    }

    async fn execute(&self, _ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let input: InspectCodeStructureInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;
        let content = fs::read_to_string(&input.file_path)
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        let structure = inspect_code_structure(Path::new(&input.file_path), &content)
            .map_err(map_structure_error)?;

        serde_json::to_value(structure).map_err(|err| ToolError::ExecutionFailed(err.to_string()))
    }
}

fn map_structure_error(error: CodeStructureError) -> ToolError {
    match error {
        CodeStructureError::UnsupportedLanguage(_) => ToolError::InvalidInput(error.to_string()),
        CodeStructureError::ParserConfiguration(_) | CodeStructureError::ParseFailed => {
            ToolError::ExecutionFailed(error.to_string())
        }
    }
}

inventory::submit! {
    ToolRegistration {
        name: "inspect_code_structure",
        factory: || Arc::new(InspectCodeStructureTool::new()) as Arc<dyn ErasedTool>,
    }
}
