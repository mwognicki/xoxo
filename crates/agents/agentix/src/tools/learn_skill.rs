use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::skills::learn_skill;
use crate::tooling::{ErasedTool, Tool, ToolContext, ToolError, ToolMetadata, ToolRegistration, ToolSchema};

#[derive(Debug, Deserialize)]
struct LearnSkillInput {
    skill_name: String,
    #[serde(default)]
    search_paths: Vec<PathBuf>,
}

/// Load the full markdown body of a named skill into the agent context.
pub struct LearnSkillTool;

impl LearnSkillTool {
    pub fn new() -> Self {
        Self
    }
}

impl Tool for LearnSkillTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "learn_skill".to_string(),
            description: "Load the full Markdown content of a named skill so it can extend the agent's prompt context. Looks in caller-provided absolute directories first, then falls back to ./.xoxo/skills, ./.agents/skills, and ~/.xoxo/skills. The first matching skill wins.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["skill_name"],
                "additionalProperties": false,
                "properties": {
                    "skill_name": {
                        "type": "string",
                        "description": "Skill directory name to load, for example rust-best-practices."
                    },
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
        let content = output.as_str().unwrap_or_default();
        format!("Loaded skill content ({} bytes)", content.len())
    }

    async fn execute(&self, _ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let input: LearnSkillInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;

        let content = learn_skill(&input.skill_name, &input.search_paths)
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        Ok(json!(content))
    }
}

inventory::submit! {
    ToolRegistration {
        name: "learn_skill",
        factory: || Arc::new(LearnSkillTool::new()) as Arc<dyn ErasedTool>,
    }
}

#[cfg(test)]
mod tests {
    use super::LearnSkillTool;
    use crate::tooling::{Tool, ToolContext};
    use serde_json::json;
    use std::fs;

    #[tokio::test]
    async fn returns_skill_content_as_a_string() {
        let custom = tempfile::tempdir().expect("custom tempdir");

        write_skill(
            custom.path(),
            "rust-best-practices/SKILL.md",
            "rust-best-practices",
            "Custom Rust guidance.",
        );

        let tool = LearnSkillTool::new();
        let output = tool
            .execute(
                &ToolContext {
                    execution_context: None,
                    available_tools: None,
                    spawner: None,
                },
                json!({
                    "skill_name": "rust-best-practices",
                    "search_paths": [custom.path().display().to_string()]
                }),
            )
            .await
            .expect("tool should succeed");

        assert_eq!(
            output.as_str().expect("string output"),
            "---\nname: rust-best-practices\ndescription: Custom Rust guidance.\n---\n# rust-best-practices\n"
        );
    }

    #[tokio::test]
    async fn returns_clear_error_when_skill_is_missing() {
        let tool = LearnSkillTool::new();
        let error = tool
            .execute(
                &ToolContext {
                    execution_context: None,
                    available_tools: None,
                    spawner: None,
                },
                json!({
                    "skill_name": "missing-skill-that-should-not-exist-anywhere-in-tests-4f0d8f2e"
                }),
            )
            .await
            .expect_err("missing skill should fail");

        match error {
            crate::tooling::ToolError::ExecutionFailed(message) => {
                assert!(message.contains("was not found"));
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
