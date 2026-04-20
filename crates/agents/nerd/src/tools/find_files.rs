use std::sync::Arc;

use ignore::WalkBuilder;
use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};
use xoxo_core::tooling::{ErasedTool, Tool, ToolContext, ToolError, ToolRegistration, ToolSchema};

#[derive(Debug, Deserialize)]
struct FindFilesInput {
    pattern: String,
}

pub struct FindFilesTool;

impl FindFilesTool {
    pub fn new() -> Self {
        Self
    }
}

impl Tool for FindFilesTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "find_files".to_string(),
            description: "Search recursively through project file names for a regex pattern. Respects .gitignore and other standard ignore files.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["pattern"],
                "additionalProperties": false,
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Regex pattern to match against file names in the project."
                    }
                }
            }),
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let hit_count = output["hit_count"].as_u64();

        match hit_count {
            Some(hit_count) => format!("Found {hit_count} matching file(s)"),
            None => "Search completed".to_string(),
        }
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        input: Value,
    ) -> Result<Value, ToolError> {
        let input: FindFilesInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;
        let regex = Regex::new(&input.pattern)
            .map_err(|err| ToolError::InvalidInput(format!("invalid regex pattern: {err}")))?;
        let cwd = std::env::current_dir()
            .map_err(|err| ToolError::ExecutionFailed(format!("failed to get current directory: {err}")))?;

        let mut hits = Vec::new();

        for result in WalkBuilder::new(&cwd).standard_filters(true).build() {
            let entry = match result {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }

            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };

            if regex.is_match(file_name) {
                hits.push(json!({
                    "file_path": relative_to(&cwd, path),
                    "file_name": file_name,
                }));
            }
        }

        Ok(json!({
            "pattern": input.pattern,
            "hit_count": hits.len(),
            "hits": hits,
        }))
    }
}

fn relative_to(base: &std::path::Path, path: &std::path::Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

inventory::submit! {
    ToolRegistration {
        name: "find_files",
        factory: || Arc::new(FindFilesTool::new()) as Arc<dyn ErasedTool>,
    }
}
