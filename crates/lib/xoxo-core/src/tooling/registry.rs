use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use crate::tooling::{Tool, ToolContext, ToolError, ToolExecutionResult, ToolMetadata, ToolSchema};

/// Object-safe wrapper around [`Tool`].
///
/// `async fn` in traits is not object-safe in stable Rust, so `Arc<dyn Tool>`
/// cannot be used directly. `ErasedTool` boxes the future, enabling dynamic
/// dispatch. A blanket impl automatically covers every `T: Tool`.
pub trait ErasedTool: Send + Sync {

    fn schema(&self) -> ToolSchema;

    fn metadata(&self) -> ToolMetadata;

    fn map_to_preview(&self, output: &serde_json::Value) -> String;

    fn execute_erased<'a>(
        &'a self,
        ctx: &'a ToolContext,
        input: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, ToolError>> + Send + 'a>>;

    fn execute_erased_with_observability<'a>(
        &'a self,
        ctx: &'a ToolContext,
        input: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<ToolExecutionResult, ToolError>> + Send + 'a>>;
}

impl<T: Tool> ErasedTool for T {
    fn schema(&self) -> ToolSchema {
        Tool::schema(self)
    }

    fn metadata(&self) -> ToolMetadata {
        Tool::metadata(self)
    }

    fn map_to_preview(&self, output: &serde_json::Value) -> String {
        Tool::map_to_preview(self, output)
    }

    fn execute_erased<'a>(
        &'a self,
        ctx: &'a ToolContext,
        input: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, ToolError>> + Send + 'a>> {
        Box::pin(Tool::execute(self, ctx, input))
    }

    fn execute_erased_with_observability<'a>(
        &'a self,
        ctx: &'a ToolContext,
        input: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<ToolExecutionResult, ToolError>> + Send + 'a>> {
        Box::pin(Tool::execute_with_observability(self, ctx, input))
    }
}

/// Inventory item for self-registering tools.
///
/// Each tool module calls `inventory::submit!` with one of these, providing
/// a stable name and a zero-argument factory. The `ToolRegistry` collects
/// all submitted registrations automatically at startup — no central wiring
/// or enum required.
///
/// # Example
/// ```ignore
/// inventory::submit! {
///     ToolRegistration {
///         name: "http_request",
///         factory: || Arc::new(HttpTool::new()),
///     }
/// }
/// ```
pub struct ToolRegistration {
    /// Stable snake_case name used in agent blueprints and all persistence layers.
    pub name: &'static str,
    /// Zero-argument factory producing the tool implementation.
    pub factory: fn() -> Arc<dyn ErasedTool>,
}

inventory::collect!(ToolRegistration);

/// Errors produced by the tool registry.
#[derive(Debug)]
pub enum ToolRegistryError {
    /// The name does not match any registered tool.
    UnknownTool(String),
}

impl std::fmt::Display for ToolRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolRegistryError::UnknownTool(name) => write!(f, "unknown tool: {name:?}"),
        }
    }
}

impl std::error::Error for ToolRegistryError {}

/// Maps tool names to their implementations.
///
/// Auto-populated at construction time from all `inventory::submit!`
/// registrations in the binary. No explicit wiring in `main` needed —
/// adding a tool means adding a module with an `inventory::submit!` call.
///
/// # Example
/// ```ignore
/// let registry = ToolRegistry::new();
/// let set = registry.resolve_set(&["http_request".to_string()])?;
/// ```
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn ErasedTool>>,
}



impl ToolRegistry {
    /// Build the registry from all submitted [`ToolRegistration`] items,
    /// with no text overrides. Used in tests.
    pub fn new() -> Self {
        let tools = inventory::iter::<ToolRegistration>()
            .map(|r| (r.name.to_string(), (r.factory)()))
            .collect();
        Self { tools }
    }


    /// Look up the implementation for a tool by name.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn ErasedTool>> {
        self.tools.get(name)
    }

    /// Returns schemas for all registered tools.
    pub fn all_schemas(&self) -> Vec<ToolSchema> {
        self.tools.values().map(|tool| tool.schema()).collect()
    }

    /// Returns the names of all registered tools.
    pub fn all_tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Resolve a list of tool name strings (blueprint format) into a [`ToolSet`].
    ///
    /// Returns [`ToolRegistryError::UnknownTool`] if any name has no registered implementation.
    pub fn resolve_set(&self, names: &[String]) -> Result<ToolSet, ToolRegistryError> {
        let mut tools = HashMap::new();
        for name in names {
            let implementation = self
                .tools
                .get(name.as_str())
                .ok_or_else(|| ToolRegistryError::UnknownTool(name.clone()))?
                .clone();
            tools.insert(name.clone(), implementation);
        }
        Ok(ToolSet { tools })
    }
}

/// A resolved subset of tools for a specific agent instance.
///
/// Constructed from a `Vec<String>` (as stored in [`AgentBlueprint::tools`])
/// via [`ToolRegistry::resolve_set`]. Contains only the tools the agent is
/// configured to use.
///
/// [`AgentBlueprint::tools`]: crate::types::AgentBlueprint::tools
#[derive(Clone)]
pub struct ToolSet {
    tools: HashMap<String, Arc<dyn ErasedTool>>,
}

impl ToolSet {
    /// Look up a tool by name.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn ErasedTool>> {
        self.tools.get(name)
    }

    /// Iterate over all (name, implementation) pairs in this set.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Arc<dyn ErasedTool>)> {
        self.tools.iter()
    }
}
