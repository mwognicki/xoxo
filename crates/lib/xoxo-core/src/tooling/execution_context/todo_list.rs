use serde::{Deserialize, Serialize};

/// Session-scoped todo list retained in the tool execution context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolExecutionTodoList {
    /// Ordered tasks for the current coding session.
    pub tasks: Vec<ToolExecutionTodoTask>,
}

impl ToolExecutionTodoList {
    /// Validate a todo list before storing it in the execution context.
    ///
    /// # Errors
    /// Returns an error when the list is empty, contains blank tasks, or has
    /// more than one task in progress.
    ///
    /// # Panics
    /// Never panics.
    ///
    /// # Examples
    /// ```rust
    /// use xoxo_core::tooling::{
    ///     ToolExecutionTodoList, ToolExecutionTodoPriority, ToolExecutionTodoState,
    ///     ToolExecutionTodoTask,
    /// };
    ///
    /// let list = ToolExecutionTodoList {
    ///     tasks: vec![ToolExecutionTodoTask {
    ///         id: "task_1".to_string(),
    ///         content: "Inspect the current tool registry".to_string(),
    ///         priority: ToolExecutionTodoPriority::High,
    ///         state: ToolExecutionTodoState::Pending,
    ///     }],
    /// };
    ///
    /// assert!(list.validate().is_ok());
    /// ```
    pub fn validate(&self) -> Result<(), ToolExecutionTodoListError> {
        if self.tasks.is_empty() {
            return Err(ToolExecutionTodoListError::EmptyList);
        }

        let in_progress_count = self
            .tasks
            .iter()
            .filter(|task| task.state == ToolExecutionTodoState::InProgress)
            .count();

        if in_progress_count > 1 {
            return Err(ToolExecutionTodoListError::MultipleInProgressTasks {
                count: in_progress_count,
            });
        }

        for task in &self.tasks {
            if task.content.trim().is_empty() {
                return Err(ToolExecutionTodoListError::BlankTaskContent {
                    task_id: task.id.clone(),
                });
            }
        }

        Ok(())
    }
}

/// One persisted task inside a session-scoped todo list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolExecutionTodoTask {
    /// Stable task identifier generated when the list is created.
    pub id: String,
    /// User-facing task description.
    pub content: String,
    /// Relative importance for ordering and triage.
    pub priority: ToolExecutionTodoPriority,
    /// Current execution status.
    pub state: ToolExecutionTodoState,
}

/// Allowed todo task states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionTodoState {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

/// Allowed todo task priorities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionTodoPriority {
    High,
    Medium,
    Low,
}

/// Mutable session state for a single todo list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolExecutionTodoListState {
    todo_list: Option<ToolExecutionTodoList>,
}

impl ToolExecutionTodoListState {
    /// Write the todo list for the current execution context.
    ///
    /// Creates the list when none exists yet, otherwise replaces the previous
    /// value with the new snapshot.
    ///
    /// # Errors
    /// Returns an error when the input list is invalid.
    ///
    /// # Panics
    /// Never panics.
    ///
    /// # Examples
    /// ```rust
    /// use xoxo_core::tooling::{
    ///     ToolExecutionTodoList, ToolExecutionTodoListState, ToolExecutionTodoPriority,
    ///     ToolExecutionTodoState, ToolExecutionTodoTask,
    /// };
    ///
    /// let mut state = ToolExecutionTodoListState::default();
    /// let list = ToolExecutionTodoList {
    ///     tasks: vec![ToolExecutionTodoTask {
    ///         id: "task_1".to_string(),
    ///         content: "Add the shared tool".to_string(),
    ///         priority: ToolExecutionTodoPriority::High,
    ///         state: ToolExecutionTodoState::Pending,
    ///     }],
    /// };
    ///
    /// state.write(list).unwrap();
    /// assert!(state.get().is_some());
    /// ```
    pub fn write(
        &mut self,
        todo_list: ToolExecutionTodoList,
    ) -> Result<ToolExecutionTodoListWriteAction, ToolExecutionTodoListError> {
        todo_list.validate()?;
        let action = if self.todo_list.is_some() {
            ToolExecutionTodoListWriteAction::Updated
        } else {
            ToolExecutionTodoListWriteAction::Created
        };
        self.todo_list = Some(todo_list);
        Ok(action)
    }

    /// Return the stored todo list, if one exists.
    ///
    /// # Errors
    /// Never returns an error.
    ///
    /// # Panics
    /// Never panics.
    ///
    /// # Examples
    /// ```rust
    /// use xoxo_core::tooling::ToolExecutionTodoListState;
    ///
    /// let state = ToolExecutionTodoListState::default();
    /// assert!(state.get().is_none());
    /// ```
    pub fn get(&self) -> Option<&ToolExecutionTodoList> {
        self.todo_list.as_ref()
    }
}

/// Errors for session todo-list lifecycle and validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolExecutionTodoListError {
    EmptyList,
    BlankTaskContent { task_id: String },
    MultipleInProgressTasks { count: usize },
}

/// Outcome of writing the current session todo list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolExecutionTodoListWriteAction {
    Created,
    Updated,
}

impl std::fmt::Display for ToolExecutionTodoListError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyList => write!(f, "Todo list must contain at least one task"),
            Self::BlankTaskContent { task_id } => {
                write!(f, "Todo task {task_id} must have non-empty content")
            }
            Self::MultipleInProgressTasks { count } => write!(
                f,
                "Todo list may have only one in_progress task, found {count}"
            ),
        }
    }
}

impl std::error::Error for ToolExecutionTodoListError {}
