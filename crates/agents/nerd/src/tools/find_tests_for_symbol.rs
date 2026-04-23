use std::path::PathBuf;
use std::sync::Arc;

use agentix::tooling::{
    ErasedTool, Tool, ToolContext, ToolError, ToolRegistration, ToolSchema,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::coding::{
    CodeStructureError, FindTestsForSymbolOptions, find_tests_for_symbol,
};

#[derive(Debug, Deserialize)]
struct FindTestsForSymbolInput {
    file_path: String,
    symbol: String,
}

/// Tool implementation for AST-backed test discovery for a symbol.
pub struct FindTestsForSymbolTool;

impl FindTestsForSymbolTool {
    /// Create a new test discovery tool.
    pub fn new() -> Self {
        Self
    }
}

impl Tool for FindTestsForSymbolTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "find_tests_for_symbol".to_string(),
            description: "Find test-like functions or methods that reference a symbol related to a source file using deterministic AST parsers.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["file_path", "symbol"],
                "additionalProperties": false,
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the source file containing or using the symbol, absolute or relative to the current working directory."
                    },
                    "symbol": {
                        "type": "string",
                        "description": "Exact symbol name to find tests for."
                    }
                }
            }),
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let symbol = output["symbol"].as_str().unwrap_or("symbol");
        let hit_count = output["hit_count"].as_u64().unwrap_or(0);
        format!("Found {hit_count} test(s) for {symbol}")
    }

    async fn execute(&self, _ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let input: FindTestsForSymbolInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;
        let result = find_tests_for_symbol(FindTestsForSymbolOptions {
            file_path: PathBuf::from(input.file_path),
            symbol: input.symbol,
        })
        .map_err(map_tests_error)?;

        serde_json::to_value(result).map_err(|err| ToolError::ExecutionFailed(err.to_string()))
    }
}

fn map_tests_error(error: CodeStructureError) -> ToolError {
    match error {
        CodeStructureError::UnsupportedLanguage(_) => ToolError::InvalidInput(error.to_string()),
        CodeStructureError::ParserConfiguration(_) | CodeStructureError::ParseFailed => {
            ToolError::ExecutionFailed(error.to_string())
        }
    }
}

inventory::submit! {
    ToolRegistration {
        name: "find_tests_for_symbol",
        factory: || Arc::new(FindTestsForSymbolTool::new()) as Arc<dyn ErasedTool>,
    }
}

#[cfg(test)]
mod tests {
    use agentix::tooling::{Tool, ToolContext};
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn finds_tests_for_symbol() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("lib.rs");
        std::fs::write(
            &file_path,
            "struct User;\n\n#[test]\nfn builds_user() {\n    let _user = User;\n}\n",
        )
        .unwrap();

        let output = Tool::execute(
            &FindTestsForSymbolTool::new(),
            &ToolContext {
                execution_context: None,
                spawner: None,
            },
            json!({
                "file_path": file_path.to_string_lossy(),
                "symbol": "User",
            }),
        )
        .await
        .unwrap();

        assert_eq!(output["hit_count"], 1);
        assert_eq!(output["tests"][0]["name"], "builds_user");
    }
}
