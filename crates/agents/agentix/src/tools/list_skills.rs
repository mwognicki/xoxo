use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::skills::list_skills;
use crate::tooling::{
    ErasedTool, Tool, ToolContext, ToolError, ToolMetadata, ToolRegistration, ToolSchema,
};

#[derive(Debug, Deserialize)]
struct ListSkillsInput {
    #[serde(default)]
    search_paths: Vec<PathBuf>,
}

/// List all available skills with their descriptions and source paths.
pub struct ListSkillsTool;

impl ListSkillsTool {
    pub fn new() -> Self {
        Self
    }
}

impl Tool for ListSkillsTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "list_skills".to_string(),
            description: "List all discovered skills with their names, descriptions, and source paths. Searches caller-provided absolute directories first, then falls back to ./.xoxo/skills, ./.agents/skills, and ~/.xoxo/skills. If the same skill exists in multiple locations, the first match wins.".to_string(),
            parameters: json!({
                "type": "object",
                "required": [],
                "additionalProperties": false,
                "properties": {
                    "search_paths": {
                        "type": "array",
                        "description": "Optional absolute directories to search before the default skill roots. Each directory should directly contain skill-name folders.",
                        "items": {
                            "type": "string"
                        }
                    }
                }
            }),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            is_read_only: true,
            supports_concurrent_invocation: true,
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let count = output.as_array().map_or(0, Vec::len);
        format!("Listed {count} skill(s)")
    }

    async fn execute(&self, _ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let input: ListSkillsInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;

        let skills = list_skills(&input.search_paths)
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        Ok(json!(
            skills
                .into_iter()
                .map(|skill| {
                    json!({
                        "skill_name": skill.skill_name,
                        "skill_description": skill.skill_description,
                        "path": skill.path.display().to_string(),
                    })
                })
                .collect::<Vec<_>>()
        ))
    }
}

inventory::submit! {
    ToolRegistration {
        name: "list_skills",
        factory: || Arc::new(ListSkillsTool::new()) as Arc<dyn ErasedTool>,
    }
}

#[cfg(test)]
mod tests {
    use super::ListSkillsTool;
    use crate::tooling::{Tool, ToolContext};
    use serde_json::json;
    use std::fs;

    #[tokio::test]
    async fn returns_listed_skills_with_expected_shape() {
        let custom = tempfile::tempdir().expect("custom tempdir");

        write_skill(
            custom.path(),
            "rust-best-practices/SKILL.md",
            "rust-best-practices",
            "Custom Rust guidance.",
        );

        let tool = ListSkillsTool::new();
        let output = tool
            .execute(
                &ToolContext {
                    execution_context: None,
                    available_tools: None,
                    spawner: None,
                },
                json!({
                    "search_paths": [custom.path().display().to_string()]
                }),
            )
            .await
            .expect("tool should succeed");

        let skills = output.as_array().expect("array output");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0]["skill_name"], "rust-best-practices");
        assert_eq!(skills[0]["skill_description"], "Custom Rust guidance.");
        assert_eq!(
            skills[0]["path"],
            json!(custom
                .path()
                .join("rust-best-practices")
                .join("SKILL.md")
                .display()
                .to_string())
        );
    }

    #[tokio::test]
    async fn returns_clear_error_for_relative_search_paths() {
        let tool = ListSkillsTool::new();
        let error = tool
            .execute(
                &ToolContext {
                    execution_context: None,
                    available_tools: None,
                    spawner: None,
                },
                json!({
                    "search_paths": ["./relative"]
                }),
            )
            .await
            .expect_err("relative path should fail");

        match error {
            crate::tooling::ToolError::ExecutionFailed(message) => {
                assert!(message.contains("search path must be absolute"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    fn write_skill(base: &std::path::Path, relative: &str, name: &str, description: &str) {
        let path = base.join(relative);
        fs::create_dir_all(path.parent().expect("skill dir")).expect("create skill dir");
        fs::write(
            path,
            format!(
                "---\nname: {name}\ndescription: {description}\n---\n# {name}\n"
            ),
        )
        .expect("write skill");
    }
}
