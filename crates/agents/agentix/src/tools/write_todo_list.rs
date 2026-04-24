use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::tooling::{
    ErasedTool, Tool, ToolContext, ToolError, ToolExecutionTodoList,
    ToolExecutionTodoListWriteAction, ToolExecutionTodoPriority, ToolExecutionTodoState,
    ToolExecutionTodoTask, ToolRegistration, ToolSchema,
};

#[derive(Debug, Deserialize)]
struct WriteTodoListInput {
    tasks: Vec<WriteTodoTaskInput>,
}

#[derive(Debug, Deserialize)]
struct WriteTodoTaskInput {
    content: String,
    priority: ToolExecutionTodoPriority,
    #[serde(default = "default_task_state")]
    state: ToolExecutionTodoState,
}

fn default_task_state() -> ToolExecutionTodoState {
    ToolExecutionTodoState::Pending
}

/// Write the session-scoped structured todo list for the current coding task.
pub struct WriteTodoListTool;

impl WriteTodoListTool {
    pub fn new() -> Self {
        Self
    }
}

impl Tool for WriteTodoListTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "write_todo_list".to_string(),
            description: "Use this tool to create and manage a structured task list for your current coding session. This helps you track progress, organize complex tasks, and demonstrate thoroughness to the user. Each call writes the full todo-list snapshot, creating it on first use or updating the previous value afterward. Supports task states pending, in_progress, completed, cancelled; priority levels high, medium, low; and the guidelines that only one task should be in_progress at a time, tasks should be completed before starting new ones, and irrelevant tasks should be cancelled. Use it for complex multistep tasks with 3 or more distinct steps, when the user explicitly requests a todo list, or proactively for non-trivial feature work with multiple operations. Do not use it for single straightforward tasks, trivial work completable in fewer than 3 steps, or purely conversational or informational requests.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["tasks"],
                "additionalProperties": false,
                "properties": {
                    "tasks": {
                        "type": "array",
                        "minItems": 1,
                        "description": "Ordered task list for the current coding session.",
                        "items": {
                            "type": "object",
                            "required": ["content", "priority"],
                            "additionalProperties": false,
                            "properties": {
                                "content": {
                                    "type": "string",
                                    "description": "Short task description."
                                },
                                "priority": {
                                    "type": "string",
                                    "enum": ["high", "medium", "low"],
                                    "description": "Task priority level."
                                },
                                "state": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed", "cancelled"],
                                    "description": "Optional task state. Defaults to pending."
                                }
                            }
                        }
                    }
                }
            }),
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let task_count = output["todo_list"]["tasks"].as_array().map_or(0, Vec::len);
        let action = output["action"].as_str().unwrap_or("wrote");
        serde_json::json!({
            "kind": "write_todo_list_preview",
            "action": action,
            "task_count": task_count,
            "tasks": output["todo_list"]["tasks"].clone(),
        })
        .to_string()
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        input: Value,
    ) -> Result<Value, ToolError> {
        let exec_ctx = ctx.execution_context.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed(
                "write_todo_list requires an execution context".to_string(),
            )
        })?;

        let input: WriteTodoListInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;

        let todo_list = ToolExecutionTodoList {
            tasks: input
                .tasks
                .into_iter()
                .enumerate()
                .map(|(index, task)| ToolExecutionTodoTask {
                    id: format!("task_{}", index + 1),
                    content: task.content.trim().to_string(),
                    priority: task.priority,
                    state: task.state,
                })
                .collect(),
        };

        let mut state = exec_ctx.todo_list.lock().await;
        let action = state
            .write(todo_list.clone())
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;

        let action = match action {
            ToolExecutionTodoListWriteAction::Created => "created",
            ToolExecutionTodoListWriteAction::Updated => "updated",
        };

        Ok(json!({
            "ok": true,
            "action": action,
            "message": format!("{action} todo list with {} task(s).", todo_list.tasks.len()),
            "guidelines": {
                "only_one_in_progress_at_a_time": true,
                "complete_tasks_before_starting_new_ones": true,
                "cancel_irrelevant_tasks": true
            },
            "todo_list": todo_list,
        }))
    }
}

inventory::submit! {
    ToolRegistration {
        name: "write_todo_list",
        factory: || Arc::new(WriteTodoListTool::new()) as Arc<dyn ErasedTool>,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::tooling::{BashOptions, ToolExecutionContext};

    #[tokio::test]
    async fn write_todo_list_persists_tasks_in_execution_context() {
        let exec_ctx = Arc::new(ToolExecutionContext::new(BashOptions::default()).await.unwrap());
        let ctx = ToolContext {
            execution_context: Some(exec_ctx.clone()),
            spawner: None,
        };
        let tool = WriteTodoListTool::new();

        let output = tool
            .execute(
                &ctx,
                json!({
                    "tasks": [
                        {
                            "content": "Inspect the shared agent tool registry",
                            "priority": "high"
                        },
                        {
                            "content": "Implement the todo list creation tool",
                            "priority": "medium",
                            "state": "in_progress"
                        }
                    ]
                }),
            )
            .await
            .unwrap();

        let stored = exec_ctx.todo_list.lock().await.get().cloned().unwrap();

        assert_eq!(output["ok"], json!(true));
        assert_eq!(output["action"], json!("created"));
        assert_eq!(stored.tasks.len(), 2);
        assert_eq!(stored.tasks[0].id, "task_1");
        assert_eq!(stored.tasks[1].state, ToolExecutionTodoState::InProgress);

        exec_ctx.shutdown().await;
    }

    #[tokio::test]
    async fn write_todo_list_updates_existing_snapshot() {
        let exec_ctx = Arc::new(ToolExecutionContext::new(BashOptions::default()).await.unwrap());
        let ctx = ToolContext {
            execution_context: Some(exec_ctx.clone()),
            spawner: None,
        };
        let tool = WriteTodoListTool::new();

        tool.execute(
            &ctx,
            json!({
                "tasks": [
                    {
                        "content": "Create the initial plan",
                        "priority": "high"
                    }
                ]
            }),
        )
        .await
        .unwrap();

        let output = tool
            .execute(
                &ctx,
                json!({
                    "tasks": [
                        {
                            "content": "Replace the existing plan snapshot",
                            "priority": "low"
                        }
                    ]
                }),
            )
            .await
            .unwrap();

        let stored = exec_ctx.todo_list.lock().await.get().cloned().unwrap();

        assert_eq!(output["action"], json!("updated"));
        assert_eq!(stored.tasks.len(), 1);
        assert_eq!(stored.tasks[0].content, "Replace the existing plan snapshot");

        exec_ctx.shutdown().await;
    }
}
