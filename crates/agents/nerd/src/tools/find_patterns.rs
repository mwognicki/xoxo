use std::fs;
use std::sync::Arc;

use ignore::WalkBuilder;
use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};
use xoxo_core::tooling::{ErasedTool, Tool, ToolContext, ToolError, ToolRegistration, ToolSchema};

#[derive(Debug, Deserialize)]
struct SearchProjectInput {
    pattern: String,
}

pub struct FindPatternsTool;

impl FindPatternsTool {
    pub fn new() -> Self {
        Self
    }
}

impl Tool for FindPatternsTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "find_patterns".to_string(),
            description: "Search recursively through project files for a regex pattern. Respects .gitignore and other standard ignore files.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["pattern"],
                "additionalProperties": false,
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Regex pattern to search for in project files."
                    }
                }
            }),
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let hit_count = output["hit_count"].as_u64();
        let file_count = output["file_count"].as_u64();

        match (hit_count, file_count) {
            (Some(hit_count), Some(file_count)) => {
                format!("Found {hit_count} hit(s) across {file_count} file(s)")
            }
            (Some(hit_count), None) => format!("Found {hit_count} hit(s)"),
            _ => "Search completed".to_string(),
        }
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        input: Value,
    ) -> Result<Value, ToolError> {
        let input: SearchProjectInput = serde_json::from_value(input)
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
            let content = match fs::read_to_string(path) {
                Ok(content) => content,
                Err(_) => continue,
            };

            for (index, line) in content.lines().enumerate() {
                if regex.is_match(line) {
                    hits.push(json!({
                        "file_path": relative_to(&cwd, path),
                        "line_number": index + 1,
                        "line": line,
                    }));
                }
            }
        }

        let mut files = std::collections::BTreeSet::new();
        for hit in &hits {
            if let Some(file_path) = hit["file_path"].as_str() {
                files.insert(file_path.to_string());
            }
        }

        Ok(json!({
            "pattern": input.pattern,
            "hit_count": hits.len(),
            "file_count": files.len(),
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
        name: "find_patterns",
        factory: || Arc::new(FindPatternsTool::new()) as Arc<dyn ErasedTool>,
    }
}
