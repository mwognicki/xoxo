use std::sync::Arc;

use futures::future::join_all;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::tooling::{
    ErasedTool, Tool, ToolContext, ToolError, ToolMetadata, ToolRegistration, ToolSchema,
};

#[derive(Debug, Deserialize)]
struct RunToolsConcurrentlyInput {
    tool_name: String,
    inputs: Vec<Value>,
}

/// Execute multiple invocations of the same eligible tool concurrently.
pub struct RunToolsConcurrentlyTool;

impl RunToolsConcurrentlyTool {
    pub fn new() -> Self {
        Self
    }

    fn resolve_target_tool<'a>(
        &self,
        ctx: &'a ToolContext,
        tool_name: &str,
    ) -> Result<&'a Arc<dyn ErasedTool>, ToolError> {
        let available_tools = ctx.available_tools.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed(
                "run_tools_concurrently requires the caller's available tool set".to_string(),
            )
        })?;

        let tool = available_tools.get(tool_name).ok_or_else(|| {
            ToolError::InvalidInput(format!(
                "tool {tool_name:?} is not available to the calling agent"
            ))
        })?;

        let metadata = tool.metadata();
        if !metadata.is_read_only {
            return Err(ToolError::InvalidInput(format!(
                "tool {tool_name:?} is not read-only and cannot be run concurrently"
            )));
        }
        if !metadata.supports_concurrent_invocation {
            return Err(ToolError::InvalidInput(format!(
                "tool {tool_name:?} is not marked safe for concurrent invocation"
            )));
        }

        if tool_name == Tool::schema(self).name {
            return Err(ToolError::InvalidInput(
                "run_tools_concurrently cannot invoke itself".to_string(),
            ));
        }

        Ok(tool)
    }
}

impl Tool for RunToolsConcurrentlyTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "run_tools_concurrently".to_string(),
            description: "Run multiple independent invocations of the same read-only tool concurrently. The target tool must be available to the calling agent and explicitly marked safe for concurrent invocation. Rejects the whole request up front if the tool is unavailable or not eligible.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["tool_name", "inputs"],
                "additionalProperties": false,
                "properties": {
                    "tool_name": {
                        "type": "string",
                        "description": "Name of the single tool to invoke concurrently across all inputs."
                    },
                    "inputs": {
                        "type": "array",
                        "minItems": 1,
                        "description": "Independent input payloads for repeated invocations of the same tool.",
                        "items": {
                            "type": "object"
                        }
                    }
                }
            }),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            is_read_only: true,
            supports_concurrent_invocation: false,
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let tool_name = output["tool_name"].as_str().unwrap_or("unknown");
        let total = output["results"].as_array().map_or(0, Vec::len);
        let success_count = output["results"]
            .as_array()
            .map_or(0, |results| {
                results
                    .iter()
                    .filter(|result| result["ok"].as_bool() == Some(true))
                    .count()
            });

        format!(
            "Ran {tool_name} concurrently ({success_count}/{total} invocation(s) succeeded)"
        )
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let input: RunToolsConcurrentlyInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;

        let tool = self.resolve_target_tool(ctx, &input.tool_name)?;
        let futures = input.inputs.into_iter().enumerate().map(|(index, tool_input)| async move {
            match tool.execute_erased(ctx, tool_input).await {
                Ok(output) => json!({
                    "index": index,
                    "ok": true,
                    "output": output,
                }),
                Err(error) => {
                    let message = match error {
                        ToolError::InvalidInput(message) => format!("invalid input: {message}"),
                        ToolError::ExecutionFailed(message) => message,
                    };
                    json!({
                        "index": index,
                        "ok": false,
                        "error": message,
                    })
                }
            }
        });

        let results = join_all(futures).await;
        Ok(json!({
            "tool_name": input.tool_name,
            "results": results,
        }))
    }
}

inventory::submit! {
    ToolRegistration {
        name: "run_tools_concurrently",
        factory: || Arc::new(RunToolsConcurrentlyTool::new()) as Arc<dyn ErasedTool>,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::{Value, json};

    use super::RunToolsConcurrentlyTool;
    use crate::tooling::{
        ErasedTool, Tool, ToolContext, ToolError, ToolMetadata, ToolRegistration, ToolRegistry,
        ToolSchema,
    };

    struct TestReadOnlyTool;

    impl Tool for TestReadOnlyTool {
        fn schema(&self) -> ToolSchema {
            ToolSchema {
                name: "test_read_only_tool".to_string(),
                description: "Test tool that can succeed or fail without mutating state.".to_string(),
                parameters: json!({
                    "type": "object",
                    "required": ["value"],
                    "additionalProperties": false,
                    "properties": {
                        "value": {
                            "type": "integer"
                        },
                        "fail": {
                            "type": "boolean"
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

        async fn execute(&self, _ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
            let value = input["value"]
                .as_i64()
                .ok_or_else(|| ToolError::InvalidInput("missing integer value".to_string()))?;

            if input["fail"].as_bool() == Some(true) {
                return Err(ToolError::ExecutionFailed(format!("forced failure for {value}")));
            }

            Ok(json!({ "value": value }))
        }
    }

    struct TestMutatingTool;

    impl Tool for TestMutatingTool {
        fn schema(&self) -> ToolSchema {
            ToolSchema {
                name: "test_mutating_tool".to_string(),
                description: "Test tool that is intentionally ineligible.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {}
                }),
            }
        }

        async fn execute(&self, _ctx: &ToolContext, _input: Value) -> Result<Value, ToolError> {
            Ok(json!({ "ok": true }))
        }
    }

    inventory::submit! {
        ToolRegistration {
            name: "test_read_only_tool",
            factory: || Arc::new(TestReadOnlyTool) as Arc<dyn ErasedTool>,
        }
    }

    inventory::submit! {
        ToolRegistration {
            name: "test_mutating_tool",
            factory: || Arc::new(TestMutatingTool) as Arc<dyn ErasedTool>,
        }
    }

    fn test_context(tool_names: &[&str]) -> ToolContext {
        let registry = ToolRegistry::new();
        let names = tool_names.iter().map(|name| (*name).to_string()).collect::<Vec<_>>();
        let available_tools = registry.resolve_set(&names).unwrap();

        ToolContext {
            execution_context: None,
            available_tools: Some(Arc::new(available_tools)),
            spawner: None,
        }
    }

    #[tokio::test]
    async fn rejects_tools_not_available_to_the_calling_agent() {
        let tool = RunToolsConcurrentlyTool::new();
        let error = tool
            .execute(
                &test_context(&["run_tools_concurrently"]),
                json!({
                    "tool_name": "test_read_only_tool",
                    "inputs": [{ "value": 1 }]
                }),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ToolError::InvalidInput(message) if message.contains("not available")));
    }

    #[tokio::test]
    async fn rejects_tools_that_are_not_marked_safe_for_concurrency() {
        let tool = RunToolsConcurrentlyTool::new();
        let error = tool
            .execute(
                &test_context(&["run_tools_concurrently", "test_mutating_tool"]),
                json!({
                    "tool_name": "test_mutating_tool",
                    "inputs": [{}]
                }),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ToolError::InvalidInput(message) if message.contains("not read-only")));
    }

    #[tokio::test]
    async fn returns_compound_results_with_partial_failures() {
        let tool = RunToolsConcurrentlyTool::new();
        let output = tool
            .execute(
                &test_context(&["run_tools_concurrently", "test_read_only_tool"]),
                json!({
                    "tool_name": "test_read_only_tool",
                    "inputs": [
                        { "value": 1 },
                        { "value": 2, "fail": true },
                        { "value": 3 }
                    ]
                }),
            )
            .await
            .unwrap();

        assert_eq!(output["tool_name"], json!("test_read_only_tool"));
        assert_eq!(output["results"].as_array().unwrap().len(), 3);
        assert_eq!(output["results"][0]["ok"], json!(true));
        assert_eq!(output["results"][0]["output"]["value"], json!(1));
        assert_eq!(output["results"][1]["ok"], json!(false));
        assert!(output["results"][1]["error"]
            .as_str()
            .unwrap()
            .contains("forced failure for 2"));
        assert_eq!(output["results"][2]["ok"], json!(true));
        assert_eq!(output["results"][2]["output"]["value"], json!(3));
    }
}
