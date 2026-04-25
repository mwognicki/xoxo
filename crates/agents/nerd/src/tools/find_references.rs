use std::path::PathBuf;
use std::sync::Arc;

use agentix::tooling::{
    ErasedTool, Tool, ToolContext, ToolError, ToolRegistration, ToolSchema,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::coding::{CodeStructureError, FindReferencesOptions, find_references};

#[derive(Debug, Deserialize)]
struct FindReferencesInput {
    symbol: String,
    #[serde(default)]
    scope: Option<String>,
}

/// Tool implementation for AST-backed exact reference search.
pub struct FindReferencesTool;

impl FindReferencesTool {
    /// Create a new exact reference search tool.
    pub fn new() -> Self {
        Self
    }
}

impl Tool for FindReferencesTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "find_references".to_string(),
            description: "Find exact identifier-like references to a symbol under an optional file or directory scope using deterministic AST parsers.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["symbol"],
                "additionalProperties": false,
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "Exact symbol name to find references for."
                    },
                    "scope": {
                        "type": "string",
                        "description": "Optional file or directory to search. Defaults to the current working directory."
                    }
                }
            }),
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let symbol = output["symbol"].as_str().unwrap_or("symbol");
        let hit_count = output["hit_count"].as_u64().unwrap_or(0);
        format!("Found {hit_count} reference(s) for {symbol}")
    }

    async fn execute(&self, _ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let input: FindReferencesInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;
        let result = find_references(FindReferencesOptions {
            symbol: input.symbol,
            scope: input.scope.map(PathBuf::from),
        })
        .map_err(map_references_error)?;

        serde_json::to_value(result).map_err(|err| ToolError::ExecutionFailed(err.to_string()))
    }
}

fn map_references_error(error: CodeStructureError) -> ToolError {
    match error {
        CodeStructureError::UnsupportedLanguage(_) => ToolError::InvalidInput(error.to_string()),
        CodeStructureError::ParserConfiguration(_) | CodeStructureError::ParseFailed => {
            ToolError::ExecutionFailed(error.to_string())
        }
    }
}

inventory::submit! {
    ToolRegistration {
        name: "find_references",
        factory: || Arc::new(FindReferencesTool::new()) as Arc<dyn ErasedTool>,
    }
}

#[cfg(test)]
mod tests {
    use agentix::tooling::{Tool, ToolContext};
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn finds_references_in_scope() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("lib.rs");
        std::fs::write(
            &file_path,
            "struct User;\nfn build() -> User { User }\n// User\n",
        )
        .unwrap();

        let output = Tool::execute(
            &FindReferencesTool::new(),
            &ToolContext {
                execution_context: None,
                available_tools: None,
                spawner: None,
            },
            json!({
                "symbol": "User",
                "scope": temp.path().to_string_lossy(),
            }),
        )
        .await
        .unwrap();

        assert_eq!(output["hit_count"], 3);
        assert_eq!(output["hits"][0]["file_path"], "lib.rs");
    }
}
