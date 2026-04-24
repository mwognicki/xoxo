use std::fs;
use std::sync::Arc;

use agentix::tooling::{
    ErasedTool, Tool, ToolContext, ToolError, ToolRegistration, ToolSchema,
};
use serde_json::{Value, json};

use super::apply::{patch_file_impl, read_file_md5, split_lines};
use super::diff_preview::build_diff_preview;
use super::types::{PatchFile, PatchFileInput};

/// Tool implementation for patching an existing file with line-based updates.
pub struct PatchFileTool;

impl PatchFileTool {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(
        &self,
        ctx: &ToolContext,
        file_path: &str,
        updates: Vec<PatchFile>,
    ) -> Result<Value, ToolError> {
        let original_md5 = if ctx.execution_context.is_some() {
            Some(read_file_md5(file_path).map_err(ToolError::ExecutionFailed)?)
        } else {
            None
        };
        let original_content =
            fs::read_to_string(file_path).map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        let diff_preview = build_diff_preview(file_path, &original_content, &updates)
            .map_err(ToolError::ExecutionFailed)?;

        if let (Some(exec_ctx), Some(original_md5)) = (&ctx.execution_context, original_md5.as_deref())
        {
            exec_ctx
                .file_registry
                .lock()
                .await
                .ensure_read(file_path, original_md5)
                .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        }

        let updated =
            patch_file_impl(file_path, updates, None, None).map_err(ToolError::ExecutionFailed)?;
        let updated_md5 = format!("{:x}", md5::compute(&updated));

        if let (Some(exec_ctx), Some(original_md5)) = (&ctx.execution_context, original_md5.as_deref())
        {
            exec_ctx
                .file_registry
                .lock()
                .await
                .update(file_path, original_md5, updated_md5.clone())
                .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        }

        Ok(json!({
            "message": format!("File patched: {file_path}"),
            "file_path": file_path,
            "exists": true,
            "md5": updated_md5,
            "line_count": split_lines(&updated).len(),
            "diff_preview": diff_preview,
        }))
    }
}

impl Tool for PatchFileTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "patch_file".to_string(),
            description: "Apply a batch of line-based edits to an existing file atomically. Requires file state to match the tracked baseline verified via MD5. Multiple sequential calls require re-reading the file after each patch to get fresh line numbers and MD5. All line numbers in a single call are interpreted against the original file before any edits, not incrementally, so operations cannot reference lines added or removed by other updates in the same batch. For multiple unrelated edits, batch them in a single call. Multiple calls require fresh file state from `read_file` to avoid MD5 mismatch failures. Pass a real structured object with `file_path` and `updates` fields, not a JSON-formatted string. Fails with 'MD5 mismatch' if the file changed externally or between calls, and with invalid input errors if updates are provided as JSON strings instead of raw array objects.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["file_path", "updates"],
                "additionalProperties": false,
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to an existing file, absolute or relative to the current working directory. The path must resolve against the same file state tracked by the current MD5 baseline. Provide the raw string path value, not a stringified JSON object."
                    },
                    "updates": {
                        "type": "array",
                        "description": "Batch of line-based update operations, all referenced against the original file before any edits are applied. Provide a real array of operation objects, not JSON-formatted text. Do not split unrelated edits across multiple calls unless you re-read the file first to refresh line numbers and MD5.",
                        "items": {
                            "oneOf": [
                                {
                                    "type": "object",
                                    "required": ["kind", "start", "end"],
                                    "additionalProperties": false,
                                    "properties": {
                                        "kind": { "const": "remove" },
                                        "start": { "type": "integer", "minimum": 1 },
                                        "end": { "type": "integer", "minimum": 1 }
                                    }
                                },
                                {
                                    "type": "object",
                                    "required": ["kind", "line", "content"],
                                    "additionalProperties": false,
                                    "properties": {
                                        "kind": { "const": "replace" },
                                        "line": { "type": "integer", "minimum": 1 },
                                        "content": { "type": "string" }
                                    }
                                },
                                {
                                    "type": "object",
                                    "required": ["kind", "after_line", "lines"],
                                    "additionalProperties": false,
                                    "properties": {
                                        "kind": { "const": "insert" },
                                        "after_line": { "type": "integer", "minimum": 0 },
                                        "lines": {
                                            "type": "array",
                                            "minItems": 1,
                                            "items": { "type": "string" }
                                        }
                                    }
                                }
                            ]
                        }
                    }
                }
            }),
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        if let Some(diff_preview) = output.get("diff_preview") {
            return diff_preview.to_string();
        }

        match (output["file_path"].as_str(), output["md5"].as_str()) {
            (Some(file_path), Some(checksum)) => {
                format!("File patched: {file_path} (MD5: {checksum})")
            }
            (Some(file_path), None) => format!("File patched: {file_path}"),
            _ => "File patched".to_string(),
        }
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let input: PatchFileInput =
            serde_json::from_value(input).map_err(|err| ToolError::InvalidInput(err.to_string()))?;

        PatchFileTool::execute(self, ctx, &input.file_path, input.updates).await
    }
}

inventory::submit! {
    ToolRegistration {
        name: "patch_file",
        factory: || Arc::new(PatchFileTool::new()) as Arc<dyn ErasedTool>,
    }
}
