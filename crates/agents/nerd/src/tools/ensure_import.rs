use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentix::tooling::{
    ErasedTool, Tool, ToolContext, ToolError, ToolRegistration, ToolSchema,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::coding::{EnsureImportError, ensure_import_in_content};

#[derive(Debug, Deserialize)]
struct EnsureImportInput {
    file_path: String,
    import_spec: String,
}

/// Tool implementation for deterministic AST-backed import insertion.
pub struct EnsureImportTool;

impl EnsureImportTool {
    /// Create a new import insertion tool.
    pub fn new() -> Self {
        Self
    }

    /// Execute the ensure-import tool with contract-shaped arguments.
    pub async fn execute(
        &self,
        ctx: &ToolContext,
        file_path: &str,
        import_spec: &str,
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

        let edit = ensure_import_in_content(Path::new(file_path), &original, import_spec)
            .map_err(map_ensure_import_error)?;

        if edit.changed {
            fs::write(file_path, &edit.updated_content)
                .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        }
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
            "message": if edit.changed {
                format!("Inserted import in {file_path}")
            } else {
                format!("Import already present in {file_path}")
            },
            "file_path": file_path,
            "import_spec": edit.import_spec,
            "language": edit.language,
            "changed": edit.changed,
            "inserted_at_byte": edit.inserted_at_byte,
            "md5": updated_md5,
            "line_count": edit.updated_content.lines().count(),
        }))
    }
}

impl Tool for EnsureImportTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "ensure_import".to_string(),
            description: "Ensure a raw import/use/include statement or block exists in one supported source file using AST-backed import placement. Existing exact imports are left unchanged.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["file_path", "import_spec"],
                "additionalProperties": false,
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the source file, absolute or relative to the current working directory."
                    },
                    "import_spec": {
                        "type": "string",
                        "description": "Full import/use/include source to ensure exists, for example `use std::path::Path;` or `import \"fmt\"`."
                    }
                }
            }),
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let file_path = output["file_path"].as_str().unwrap_or("file");
        let changed = output["changed"].as_bool().unwrap_or(false);
        if changed {
            format!("Inserted import in {file_path}")
        } else {
            format!("Import already present in {file_path}")
        }
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let input: EnsureImportInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;

        EnsureImportTool::execute(self, ctx, &input.file_path, &input.import_spec).await
    }
}

fn map_ensure_import_error(error: EnsureImportError) -> ToolError {
    match error {
        EnsureImportError::EmptyImportSpec | EnsureImportError::UnsupportedLanguage(_) => {
            ToolError::InvalidInput(error.to_string())
        }
        EnsureImportError::SourceHasErrors
        | EnsureImportError::ImportIntroducesErrors
        | EnsureImportError::ParserConfiguration(_)
        | EnsureImportError::ParseFailed => ToolError::ExecutionFailed(error.to_string()),
    }
}

inventory::submit! {
    ToolRegistration {
        name: "ensure_import",
        factory: || Arc::new(EnsureImportTool::new()) as Arc<dyn ErasedTool>,
    }
}

#[cfg(test)]
mod tests {
    use agentix::tooling::ToolContext;

    use super::*;

    #[tokio::test]
    async fn inserts_import_in_file() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("lib.rs");
        fs::write(&file_path, "fn main() {}\n").unwrap();

        let output = EnsureImportTool::new()
            .execute(
                &ToolContext {
                    execution_context: None,
                    available_tools: None,
                    spawner: None,
                },
                file_path.to_str().unwrap(),
                "use std::path::Path;",
            )
            .await
            .unwrap();

        assert_eq!(output["changed"], true);
        assert_eq!(
            fs::read_to_string(file_path).unwrap(),
            "use std::path::Path;\nfn main() {}\n"
        );
    }
}
