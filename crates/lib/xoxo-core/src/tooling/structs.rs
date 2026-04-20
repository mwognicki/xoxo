use std::sync::Arc;
use crate::agents::Spawner;
use crate::chat::structs::CostObservability;
use serde::{Deserialize, Serialize};
use crate::tooling::execution_context::{ToolExecutionContext};

pub struct ToolContext {
    pub execution_context: Option<Arc<ToolExecutionContext>>,
    pub spawner: Option<Arc<dyn Spawner>>,
}

#[derive(Debug)]
pub enum ToolError {
    /// The input JSON did not match the tool's expected schema.
    InvalidInput(String),
    /// The tool ran but encountered a runtime failure.
    ExecutionFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    pub output: serde_json::Value,
    pub observability: Option<CostObservability>,
}

impl ToolExecutionResult {
    pub fn without_observability(output: serde_json::Value) -> Self {
        Self {
            output,
            observability: None,
        }
    }
}



/// OpenAI function-calling compatible schema for a [`Tool`].
///
/// [`Tool`]: crate::tool::Tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    /// Identifier used by the LLM to select this tool (snake_case).
    pub name: String,
    /// Human-readable description of what the tool does; shown to the LLM.
    pub description: String,
    /// JSON Schema object describing the tool's input parameters.
    /// Must conform to the OpenAI function-calling parameter schema format.
    pub parameters: serde_json::Value,
}

pub trait Tool: Send + Sync {

    fn schema(&self) -> ToolSchema;

    /// Map a full tool result into a client-facing preview string.
    ///
    /// The default behavior preserves the current runtime contract by
    /// serializing the full JSON value. Tools that return sensitive or
    /// very large payloads can override this to emit a bounded summary
    /// for bus events and TUI history while still returning the full
    /// result to the agent runtime.
    fn map_to_preview(&self, output: &serde_json::Value) -> String {
        output.to_string()
    }

    /// Execute the tool and optionally return observability metadata together
    /// with the output payload.
    fn execute_with_observability(
        &self,
        ctx: &ToolContext,
        input: serde_json::Value,
    ) -> impl Future<Output = Result<ToolExecutionResult, ToolError>> + Send {
        async move {
            self.execute(ctx, input)
                .await
                .map(ToolExecutionResult::without_observability)
        }
    }

    /// Execute the tool with the given context and JSON input.
    ///
    /// Input must conform to the JSON Schema declared in [`schema`].
    /// Output is an arbitrary JSON value returned to the LLM as the tool
    /// result. Errors are propagated to the agent runtime for handling.
    ///
    /// [`schema`]: Tool::schema
    fn execute(
        &self,
        ctx: &ToolContext,
        input: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, ToolError>> + Send;
}
