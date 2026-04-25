use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentix::tooling::{
    ErasedTool, Tool, ToolContext, ToolError, ToolRegistration, ToolSchema,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::coding::{RenameSymbolError, rename_symbol_in_content};

#[derive(Debug, Deserialize)]
struct RenameSymbolInput {
    file_path: String,
    symbol: String,
    replacement: String,
}

/// Tool implementation for deterministic AST-backed symbol rename.
pub struct RenameSymbolTool;

impl RenameSymbolTool {
    /// Create a new symbol rename tool.
    pub fn new() -> Self {
        Self
    }

    /// Execute the rename-symbol tool with contract-shaped arguments.
    pub async fn execute(
        &self,
        ctx: &ToolContext,
        file_path: &str,
        symbol: &str,
        replacement: &str,
    ) -> Result<Value, ToolError> {
        let original = fs::read_to_string(file_path)
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        let original_md5 = format!("{:x}", md5::compute(&original));

        if let Some(exec_ctx) = &ctx.execution_context {
            exec_ctx
                .file_registry
                .lock()
                .await
                .ensure_read(file_path, &original_md5)
                .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        }

        let edit = rename_symbol_in_content(
            Path::new(file_path),
            &original,
            symbol,
            replacement,
        )
        .map_err(map_rename_error)?;

        fs::write(file_path, &edit.updated_content)
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        let updated_md5 = format!("{:x}", md5::compute(&edit.updated_content));

        if let Some(exec_ctx) = &ctx.execution_context {
            exec_ctx
                .file_registry
                .lock()
                .await
                .update(file_path, &original_md5, updated_md5.clone())
                .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        }

        Ok(json!({
            "message": format!("Renamed symbol {symbol} to {replacement} in {file_path}"),
            "file_path": file_path,
            "symbol": edit.symbol,
            "replacement": edit.replacement,
            "language": edit.language,
            "definition_count": edit.definition_count,
            "occurrence_count": edit.occurrence_count,
            "md5": updated_md5,
            "line_count": edit.updated_content.lines().count(),
        }))
    }
}

impl Tool for RenameSymbolTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "rename_symbol".to_string(),
            description: "Rename a symbol inside one supported source file using AST-backed deterministic identifier ranges. The file must contain an AST-visible definition for the requested symbol.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["file_path", "symbol", "replacement"],
                "additionalProperties": false,
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the source file, absolute or relative to the current working directory."
                    },
                    "symbol": {
                        "type": "string",
                        "description": "Exact symbol name to rename."
                    },
                    "replacement": {
                        "type": "string",
                        "description": "Replacement symbol name. Must be a conservative identifier: ASCII letters, digits, and underscores, not starting with a digit."
                    }
                }
            }),
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let file_path = output["file_path"].as_str().unwrap_or("file");
        let symbol = output["symbol"].as_str().unwrap_or("symbol");
        let replacement = output["replacement"].as_str().unwrap_or("replacement");
        let occurrence_count = output["occurrence_count"].as_u64().unwrap_or(0);
        format!(
            "Renamed {symbol} to {replacement} in {file_path} ({occurrence_count} occurrence(s))"
        )
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let input: RenameSymbolInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;

        RenameSymbolTool::execute(
            self,
            ctx,
            &input.file_path,
            &input.symbol,
            &input.replacement,
        )
        .await
    }
}

fn map_rename_error(error: RenameSymbolError) -> ToolError {
    match error {
        RenameSymbolError::InvalidReplacement(_)
        | RenameSymbolError::SymbolNotFound(_)
        | RenameSymbolError::UnsupportedLanguage(_) => ToolError::InvalidInput(error.to_string()),
        RenameSymbolError::SourceHasErrors
        | RenameSymbolError::ParserConfiguration(_)
        | RenameSymbolError::ParseFailed => ToolError::ExecutionFailed(error.to_string()),
    }
}

inventory::submit! {
    ToolRegistration {
        name: "rename_symbol",
        factory: || Arc::new(RenameSymbolTool::new()) as Arc<dyn ErasedTool>,
    }
}

#[cfg(test)]
mod tests {
    use agentix::tooling::ToolContext;

    use super::*;

    #[tokio::test]
    async fn renames_symbol_in_file() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("lib.rs");
        fs::write(
            &file_path,
            "struct User;\nfn build() -> User { User }\n// User\n",
        )
        .unwrap();

        let output = RenameSymbolTool::new()
            .execute(
                &ToolContext {
                    execution_context: None,
                    available_tools: None,
                    spawner: None,
                },
                file_path.to_str().unwrap(),
                "User",
                "Account",
            )
            .await
            .unwrap();

        assert_eq!(output["occurrence_count"], 3);
        assert_eq!(
            fs::read_to_string(file_path).unwrap(),
            "struct Account;\nfn build() -> Account { Account }\n// User\n"
        );
    }
}
