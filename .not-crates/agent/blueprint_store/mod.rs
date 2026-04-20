pub mod builtin;
pub use builtin::{BUILTIN_ROOT_ID, BUILTIN_ROOT_SYSTEM_PROMPT, BuiltinBlueprintStore, FallbackBlueprintStore};

use crate::types::{AgentBlueprint, AgentId};

/// Error returned by blueprint store operations.
#[derive(Debug)]
pub enum StoreError {
    /// No blueprint exists for the given ID.
    NotFound(AgentId),
    /// The store backend failed (I/O error, parse error, etc.).
    Backend(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::NotFound(id) => write!(f, "blueprint not found: {id}"),
            StoreError::Backend(msg) => write!(f, "store backend error: {msg}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Persistent store for agent blueprints.
///
/// The single source of truth for what agents exist and how they are
/// configured. All access to blueprints must go through this trait — no
/// component reads blueprint data directly from the database.
///
/// # Backend agnosticism
///
/// Implementations may be backed by anything: MongoDB (production),
/// a Markdown file tree (development/testing), an in-memory map (tests).
/// The stable [`AgentBlueprint`] struct is the contract; backends must
/// preserve all fields on a round-trip through `save` → `load`.
///
/// Methods return boxed futures so the trait is dyn-compatible and can be
/// stored as `Arc<dyn BlueprintStore>` without an `async-trait` dependency.
pub trait BlueprintStore: Send + Sync {
    /// Load a single blueprint by agent ID.
    ///
    /// Returns [`StoreError::NotFound`] if no blueprint exists for `id`.
    fn load<'a>(
        &'a self,
        id: &'a AgentId,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<AgentBlueprint, StoreError>> + Send + 'a>>;

    /// Persist a blueprint, creating or replacing any existing entry for the
    /// same [`AgentId`].
    fn save<'a>(
        &'a self,
        blueprint: &'a AgentBlueprint,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), StoreError>> + Send + 'a>>;

    /// Return all blueprints in the store.
    ///
    /// Order is not guaranteed. An empty store returns `Ok(vec![])`.
    fn list<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<AgentBlueprint>, StoreError>> + Send + 'a>>;

    /// Semantic search over blueprint descriptions.
    ///
    /// Returns up to `limit` blueprints ranked by relevance to `query`.
    /// Backends may implement this via vector similarity (Qdrant),
    /// full-text search (MongoDB Atlas), or simple substring matching
    /// (Markdown/in-memory). An empty result is valid when nothing matches.
    fn search<'a>(
        &'a self,
        query: &'a str,
        limit: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<AgentBlueprint>, StoreError>> + Send + 'a>>;
}
