use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentix::tooling::{
    ErasedTool, Tool, ToolContext, ToolError, ToolRegistration, ToolSchema,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::coding::{PatchSymbolError, patch_symbol_in_content};

#[derive(Debug, Deserialize)]
struct PatchSymbolInput {
    file_path: String,
    symbol: String,
    replacement: String,
}

/// Tool implementation for deterministic AST-backed symbol definition patching.
pub struct PatchSymbolTool;

impl PatchSymbolTool {
    /// Create a new symbol patch tool.
    pub fn new() -> Self {
        Self
    }

    /// Execute the patch-symbol tool with contract-shaped arguments.
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

        let edit = patch_symbol_in_content(
            Path::new(file_path),
            &original,
            symbol,
            replacement,
        )
        .map_err(map_patch_error)?;

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
            "message": format!("Patched symbol {symbol} in {file_path}"),
            "file_path": file_path,
            "symbol": edit.symbol,
            "language": edit.language,
            "definition_count": edit.definition_count,
            "range": edit.range,
            "md5": updated_md5,
            "line_count": edit.updated_content.lines().count(),
        }))
    }
}

impl Tool for PatchSymbolTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "patch_symbol".to_string(),
            description: "Replace exactly one AST-visible symbol definition inside a supported source file. Fails when the symbol is missing, ambiguous, or the replacement would introduce parse errors.".to_string(),
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
                        "description": "Exact symbol definition to patch."
                    },
                    "replacement": {
                        "type": "string",
                        "description": "Full replacement source for the matched symbol definition."
                    }
                }
            }),
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let file_path = output["file_path"].as_str().unwrap_or("file");
        let symbol = output["symbol"].as_str().unwrap_or("symbol");
        format!("Patched {symbol} in {file_path}")
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let input: PatchSymbolInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;

        PatchSymbolTool::execute(
            self,
            ctx,
            &input.file_path,
            &input.symbol,
            &input.replacement,
        )
        .await
    }
}

fn map_patch_error(error: PatchSymbolError) -> ToolError {
    match error {
        PatchSymbolError::AmbiguousSymbol { .. }
        | PatchSymbolError::SymbolNotFound(_)
        | PatchSymbolError::UnsupportedLanguage(_) => ToolError::InvalidInput(error.to_string()),
        PatchSymbolError::ReplacementHasErrors
        | PatchSymbolError::SourceHasErrors
        | PatchSymbolError::ParserConfiguration(_)
        | PatchSymbolError::ParseFailed => ToolError::ExecutionFailed(error.to_string()),
    }
}

inventory::submit! {
    ToolRegistration {
        name: "patch_symbol",
        factory: || Arc::new(PatchSymbolTool::new()) as Arc<dyn ErasedTool>,
    }
}

#[cfg(test)]
mod tests {
    use agentix::tooling::ToolContext;

    use super::*;

    #[tokio::test]
    async fn patches_symbol_in_file() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("lib.rs");
        fs::write(
            &file_path,
            "fn boot() -> i32 { 1 }\nfn keep() -> i32 { 3 }\n",
        )
        .unwrap();

        let output = PatchSymbolTool::new()
            .execute(
                &ToolContext {
                    execution_context: None,
                    available_tools: None,
                    spawner: None,
                },
                file_path.to_str().unwrap(),
                "boot",
                "fn boot() -> i32 { 2 }",
            )
            .await
            .unwrap();

        assert_eq!(output["symbol"], "boot");
        assert_eq!(
            fs::read_to_string(file_path).unwrap(),
            "fn boot() -> i32 { 2 }\nfn keep() -> i32 { 3 }\n"
        );
    }
}
