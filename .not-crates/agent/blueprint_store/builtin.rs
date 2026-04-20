use std::collections::HashMap;
use std::sync::Arc;

use crate::types::{AgentBlueprint, AgentId, ModelPolicy};

use super::{BlueprintStore, StoreError};

/// ID of the built-in root agent blueprint.
///
/// Always available regardless of store state — no database or file I/O
/// required. Serves as the bootstrap guarantee: the system has at least one
/// working agent on first run.
pub const BUILTIN_ROOT_ID: &str = "builtin:root";

pub const BUILTIN_ROOT_SYSTEM_PROMPT: &str = "\
You are the Entrypoint Orchestrator — the root agent in the clawrster framework. \
Your role is to fulfill complex user requests by decomposing tasks, spawning subagents, \
and synthesizing their results into a coherent final response.

## Concepts

**Tool** — A named capability your agent can invoke (e.g. run a command, call an API, \
read a file). Tools are defined by the framework and listed under \"Available Tools\" \
at the end of this prompt. The set shown there is what is immediately available to you — \
other tools may exist in the system but are not accessible to this agent instance.

**Skill** — A named, reusable instruction set authored by an agent and stored in the \
skill store. Skills describe workflows or techniques that can be discovered and applied \
at runtime. A skill consists of prose instructions and optionally one or more executable \
scripts. They are distinct from tools: skills are authored knowledge, not framework \
capabilities.

**Memory** — A persistent record store for facts, decisions, and context. Memories \
survive across conversations and agent instances, enabling knowledge sharing. \
Each memory is a living document for a subject — enrich existing memories rather than \
creating duplicates.

**Subagent** — A child agent spawned to handle a focused subtask. Each subagent \
receives its own tool set and operates independently within its assigned scope.

## Core Responsibilities

1. **Task Decomposition** — Break multi-step requests into clear, actionable subtasks.
2. **Subagent Spawning** — Delegate subtasks to child agents. Assign each subagent \
a focused role, explicit goal, and only the tools it needs. Discover available tools \
before equipping subagents.
3. **Tool Equipping** — Provide each subagent with the minimal set of tools required \
for its task.
4. **Coordination** — Act as the central hub: relay results between subagents, resolve \
dependencies, and keep all work aligned with the user's goal.
5. **Validation & Synthesis** — Validate subagent outputs, synthesize them into a final \
response, and present it clearly to the user.
6. **Context Files** — At the end of this system prompt you may find a **Context Files** \
section containing files loaded from your context directory at startup. Act only on \
what is present there — do not attempt to read files that are not already shown. \
Any file whose name contains \"bootstrap\" holds first-turn directives — execute them \
before anything else. Treat the remaining files as authoritative context about your \
identity, personality, and the user.
7. **Memory** — Persist important facts, decisions, and context for your own future \
reference and for sharing with other agents. Retrieve relevant memories before starting \
complex tasks. Equip subagents with memory retrieval tools so they can access shared \
context independently.
8. **Skills** — Capture reusable workflows and techniques in the skill store for future \
reuse. Search for existing skills before starting complex tasks to avoid duplicating work.

## Constraints

- **Sandboxing**: Subagents must operate within their assigned scope. Do not allow them \
to access resources or perform operations outside their stated task.
- **Error Handling**: If a subagent fails, analyse the cause, provide corrective feedback, \
and either retry or escalate to the user.
- **Transparency**: Always inform the user about the orchestration process — which \
subagents were spawned, what tools they used, and why.

## Output Structure

For every non-trivial request:
1. State a **high-level plan** before acting (e.g. \"I will spawn two subagents: one to \
fetch the data, one to analyse it.\").
2. Provide **progress updates** as subagents complete their tasks.
3. Deliver a **final output** that directly addresses the user's request.

## Available Tools

{{tool_names}}\
";

/// Construct the built-in root agent blueprint.
///
/// `model` is the resolved model identifier (from `BUILTIN_MODEL`).
/// `system_prompt` is the effective system prompt — either the built-in
/// default or the value of `BUILTIN_SYSTEM_PROMPT`.
pub fn root_blueprint(model: &str, system_prompt: &str) -> AgentBlueprint {
    AgentBlueprint {
        id: AgentId(BUILTIN_ROOT_ID.to_string()),
        name: "Root Agent".to_string(),
        description: "General-purpose bootstrap agent available at startup.".to_string(),
        system_prompt: system_prompt.to_string(),
        tools: vec![
            "read_file".to_string(),
            "http_request".to_string(),
            "exec".to_string(),
            "eval_script".to_string(),
            "list_all_tools".to_string(),
            "spawn_subagent".to_string(),
            "create_memory".to_string(),
            "list_memories".to_string(),
            "search_memories".to_string(),
            "get_memory".to_string(),
            "create_skill".to_string(),
            "read_skill".to_string(),
            "list_skills".to_string(),
            "find_skills".to_string(),
            "update_skill".to_string(),
            "delete_skill".to_string(),
        ],
        model_policy: ModelPolicy::AllowList(vec![model.to_string()]),
        tags: vec!["builtin".to_string()],
        handoff_description: None,
        output_schema: None,
        metadata: serde_json::Value::Object(Default::default()),
    }
}

// ---------------------------------------------------------------------------
// BuiltinBlueprintStore
// ---------------------------------------------------------------------------

/// In-memory blueprint store backed entirely by hardcoded built-in blueprints.
///
/// Never requires I/O or an external dependency. Intended as the always-
/// available fallback layer — use [`FallbackBlueprintStore`] to layer a
/// primary store (e.g. MongoDB) on top.
pub struct BuiltinBlueprintStore {
    blueprints: HashMap<String, AgentBlueprint>,
}

impl BuiltinBlueprintStore {
    /// Create the store, seeding it with the built-in root blueprint.
    ///
    /// `model` is passed straight to [`root_blueprint`].
    /// `system_prompt` is the effective prompt (built-in default or override).
    pub fn new(model: &str, system_prompt: &str) -> Self {
        let mut blueprints = HashMap::new();
        let root = root_blueprint(model, system_prompt);
        blueprints.insert(root.id.0.clone(), root);
        Self { blueprints }
    }
}

impl BlueprintStore for BuiltinBlueprintStore {
    fn load<'a>(
        &'a self,
        id: &'a AgentId,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<AgentBlueprint, StoreError>> + Send + 'a>> {
        Box::pin(async move {
            self.blueprints
                .get(&id.0)
                .cloned()
                .ok_or_else(|| StoreError::NotFound(id.clone()))
        })
    }

    fn save<'a>(
        &'a self,
        _blueprint: &'a AgentBlueprint,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), StoreError>> + Send + 'a>> {
        Box::pin(async move {
            Err(StoreError::Backend("built-in store is read-only".to_string()))
        })
    }

    fn list<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<AgentBlueprint>, StoreError>> + Send + 'a>> {
        Box::pin(async move {
            Ok(self.blueprints.values().cloned().collect())
        })
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        _limit: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<AgentBlueprint>, StoreError>> + Send + 'a>> {
        Box::pin(async move {
            let q = query.to_lowercase();
            Ok(self
                .blueprints
                .values()
                .filter(|b| {
                    b.name.to_lowercase().contains(&q)
                        || b.description.to_lowercase().contains(&q)
                })
                .cloned()
                .collect())
        })
    }
}

// ---------------------------------------------------------------------------
// FallbackBlueprintStore
// ---------------------------------------------------------------------------

/// Blueprint store that queries a primary store first, falling back to
/// [`BuiltinBlueprintStore`] on [`StoreError::NotFound`].
///
/// This is the store the agent spawner always uses. It ensures the built-in
/// blueprint is transparently available even when the primary store (MongoDB)
/// is unavailable or unpopulated.
///
/// # Behaviour per operation
///
/// | Operation | Primary succeeds | Primary returns `NotFound` | Primary returns other error |
/// |-----------|-----------------|---------------------------|-----------------------------|
/// | `load`    | primary result  | built-in result           | error propagated            |
/// | `save`    | primary result  | —                         | error propagated            |
/// | `list`    | merged (primary + built-ins not already present) | — | error propagated |
/// | `search`  | merged up to `limit` | — | error propagated |
pub struct FallbackBlueprintStore {
    primary: Arc<dyn BlueprintStore>,
    builtin: BuiltinBlueprintStore,
}

impl FallbackBlueprintStore {
    pub fn new(primary: Arc<dyn BlueprintStore>, builtin: BuiltinBlueprintStore) -> Self {
        Self { primary, builtin }
    }
}

impl BlueprintStore for FallbackBlueprintStore {
    fn load<'a>(
        &'a self,
        id: &'a AgentId,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<AgentBlueprint, StoreError>> + Send + 'a>> {
        Box::pin(async move {
            match self.primary.load(id).await {
                Err(StoreError::NotFound(_)) => self.builtin.load(id).await,
                other => other,
            }
        })
    }

    fn save<'a>(
        &'a self,
        blueprint: &'a AgentBlueprint,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), StoreError>> + Send + 'a>> {
        Box::pin(async move { self.primary.save(blueprint).await })
    }

    fn list<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<AgentBlueprint>, StoreError>> + Send + 'a>> {
        Box::pin(async move {
            let mut items = self.primary.list().await?;
            for b in self.builtin.list().await? {
                if !items.iter().any(|x| x.id.0 == b.id.0) {
                    items.push(b);
                }
            }
            Ok(items)
        })
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        limit: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<AgentBlueprint>, StoreError>> + Send + 'a>> {
        Box::pin(async move {
            let mut items = self.primary.search(query, limit).await?;
            if items.len() < limit {
                for b in self.builtin.search(query, limit - items.len()).await? {
                    if !items.iter().any(|x| x.id.0 == b.id.0) {
                        items.push(b);
                    }
                }
            }
            Ok(items)
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::agent::blueprint_store::StoreError;
    use crate::types::AgentId;

    fn builtin_store() -> BuiltinBlueprintStore {
        BuiltinBlueprintStore::new("test-model", BUILTIN_ROOT_SYSTEM_PROMPT)
    }

    // --- BuiltinBlueprintStore ---

    #[tokio::test]
    async fn builtin_root_is_loadable() {
        let store = builtin_store();
        let bp = store.load(&AgentId(BUILTIN_ROOT_ID.to_string())).await.unwrap();
        assert_eq!(bp.id.0, BUILTIN_ROOT_ID);
        assert_eq!(bp.name, "Root Agent");
    }

    #[tokio::test]
    async fn builtin_unknown_id_returns_not_found() {
        let store = builtin_store();
        let err = store.load(&AgentId("unknown:id".to_string())).await.unwrap_err();
        assert!(matches!(err, StoreError::NotFound(_)));
    }

    #[tokio::test]
    async fn builtin_save_is_read_only() {
        let store = builtin_store();
        let bp = store.load(&AgentId(BUILTIN_ROOT_ID.to_string())).await.unwrap();
        let err = store.save(&bp).await.unwrap_err();
        assert!(matches!(err, StoreError::Backend(_)));
    }

    #[tokio::test]
    async fn builtin_list_contains_root() {
        let store = builtin_store();
        let list = store.list().await.unwrap();
        assert!(list.iter().any(|b| b.id.0 == BUILTIN_ROOT_ID));
    }

    #[tokio::test]
    async fn builtin_search_matches_name() {
        let store = builtin_store();
        let results = store.search("root", 10).await.unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn builtin_model_comes_from_argument() {
        let store = BuiltinBlueprintStore::new("my-model", "prompt");
        let bp = store.load(&AgentId(BUILTIN_ROOT_ID.to_string())).await.unwrap();
        assert!(matches!(&bp.model_policy, ModelPolicy::AllowList(models) if models[0] == "my-model"));
    }

    #[tokio::test]
    async fn builtin_system_prompt_override() {
        let store = BuiltinBlueprintStore::new("model", "custom prompt");
        let bp = store.load(&AgentId(BUILTIN_ROOT_ID.to_string())).await.unwrap();
        assert_eq!(bp.system_prompt, "custom prompt");
    }

    // --- FallbackBlueprintStore ---

    /// Primary store that always returns NotFound.
    struct EmptyStore;

    impl BlueprintStore for EmptyStore {
        fn load<'a>(&'a self, id: &'a AgentId) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<AgentBlueprint, StoreError>> + Send + 'a>> {
            let id = id.clone();
            Box::pin(async move { Err(StoreError::NotFound(id)) })
        }
        fn save<'a>(&'a self, _: &'a AgentBlueprint) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), StoreError>> + Send + 'a>> {
            Box::pin(async move { Ok(()) })
        }
        fn list<'a>(&'a self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<AgentBlueprint>, StoreError>> + Send + 'a>> {
            Box::pin(async move { Ok(vec![]) })
        }
        fn search<'a>(&'a self, _: &'a str, _: usize) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<AgentBlueprint>, StoreError>> + Send + 'a>> {
            Box::pin(async move { Ok(vec![]) })
        }
    }

    /// Primary store that always returns a Backend error.
    struct ErrorStore;

    impl BlueprintStore for ErrorStore {
        fn load<'a>(&'a self, _: &'a AgentId) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<AgentBlueprint, StoreError>> + Send + 'a>> {
            Box::pin(async move { Err(StoreError::Backend("offline".to_string())) })
        }
        fn save<'a>(&'a self, _: &'a AgentBlueprint) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), StoreError>> + Send + 'a>> {
            Box::pin(async move { Err(StoreError::Backend("offline".to_string())) })
        }
        fn list<'a>(&'a self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<AgentBlueprint>, StoreError>> + Send + 'a>> {
            Box::pin(async move { Err(StoreError::Backend("offline".to_string())) })
        }
        fn search<'a>(&'a self, _: &'a str, _: usize) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<AgentBlueprint>, StoreError>> + Send + 'a>> {
            Box::pin(async move { Err(StoreError::Backend("offline".to_string())) })
        }
    }

    #[tokio::test]
    async fn fallback_hits_builtin_on_not_found() {
        let store = FallbackBlueprintStore::new(Arc::new(EmptyStore), builtin_store());
        let bp = store.load(&AgentId(BUILTIN_ROOT_ID.to_string())).await.unwrap();
        assert_eq!(bp.id.0, BUILTIN_ROOT_ID);
    }

    #[tokio::test]
    async fn fallback_propagates_backend_error() {
        let store = FallbackBlueprintStore::new(Arc::new(ErrorStore), builtin_store());
        let err = store.load(&AgentId(BUILTIN_ROOT_ID.to_string())).await.unwrap_err();
        assert!(matches!(err, StoreError::Backend(_)));
    }

    #[tokio::test]
    async fn fallback_list_merges_primary_and_builtin() {
        let store = FallbackBlueprintStore::new(Arc::new(EmptyStore), builtin_store());
        let list = store.list().await.unwrap();
        assert!(list.iter().any(|b| b.id.0 == BUILTIN_ROOT_ID));
    }

    #[tokio::test]
    async fn fallback_list_deduplicates() {
        // Primary also has the builtin:root — should appear only once in merged list.
        struct PrimaryWithRoot(BuiltinBlueprintStore);
        impl BlueprintStore for PrimaryWithRoot {
            fn load<'a>(&'a self, id: &'a AgentId) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<AgentBlueprint, StoreError>> + Send + 'a>> {
                self.0.load(id)
            }
            fn save<'a>(&'a self, bp: &'a AgentBlueprint) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), StoreError>> + Send + 'a>> {
                self.0.save(bp)
            }
            fn list<'a>(&'a self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<AgentBlueprint>, StoreError>> + Send + 'a>> {
                self.0.list()
            }
            fn search<'a>(&'a self, q: &'a str, limit: usize) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<AgentBlueprint>, StoreError>> + Send + 'a>> {
                self.0.search(q, limit)
            }
        }

        let store = FallbackBlueprintStore::new(
            Arc::new(PrimaryWithRoot(builtin_store())),
            builtin_store(),
        );
        let list = store.list().await.unwrap();
        let count = list.iter().filter(|b| b.id.0 == BUILTIN_ROOT_ID).count();
        assert_eq!(count, 1, "builtin:root should appear exactly once");
    }
}
