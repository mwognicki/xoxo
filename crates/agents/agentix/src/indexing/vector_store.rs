//! Vendor-neutral vector-store trait.
//!
//! Every pluggable implementation (brute-force cosine today; `hnsw_rs` and
//! `instant-distance` as feature-flagged siblings later) sits behind this
//! trait. The surface is pinned to the lowest common denominator of the
//! candidate ANN crates — filtered search and incremental graph updates that
//! not all backends support honestly are intentionally left out of the trait
//! and will be exposed as capability methods if and when they're needed.

use thiserror::Error;

use super::chunk::{ChunkId, ChunkRecord};

/// A single nearest-neighbour hit returned by [`VectorStore::nearest`].
#[derive(Debug, Clone, PartialEq)]
pub struct VectorStoreHit {
    pub record: ChunkRecord,
    /// Similarity score in `[0.0, 1.0]` (cosine by default). Higher is closer.
    pub score: f32,
}

/// Query shape for a nearest-neighbour lookup.
///
/// Deliberately narrow for v1. Metadata filters (per-language, per-repo) are
/// the obvious next axis; they're left out until at least one caller actually
/// needs them, so the trait doesn't grow ahead of demand.
#[derive(Debug, Clone)]
pub struct VectorQuery<'a> {
    pub vector: &'a [f32],
    pub k: usize,
}

/// Errors surfaced by vector-store implementations.
#[derive(Debug, Error)]
pub enum VectorStoreError {
    /// The query or upsert vector did not match the index's dimensionality.
    #[error("vector dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// A chunk id referenced a record that isn't present in the store.
    #[error("chunk not found: {0}")]
    NotFound(ChunkId),

    /// Wrapper for any backend-specific error (I/O, sled, ANN crate, ...).
    #[error("vector store backend error: {0}")]
    Backend(#[from] Box<dyn std::error::Error + Send + Sync>),
}

/// Durable-plus-queryable store of embedding vectors keyed by [`ChunkId`].
///
/// Trait methods are synchronous to keep the LCD honest — brute-force cosine
/// and every candidate ANN crate in ADR 0003 are synchronous in-memory
/// operations. If a future implementation needs async (e.g. to page vectors
/// off disk), we'll add an `async` sibling trait rather than making every
/// impl pay the cost.
pub trait VectorStore: Send + Sync {
    /// Insert or overwrite the record + vector for `record.id`.
    fn upsert(&mut self, record: ChunkRecord, vector: Vec<f32>) -> Result<(), VectorStoreError>;

    /// Look up a single record by id.
    fn lookup_by_id(&self, id: &ChunkId) -> Result<Option<ChunkRecord>, VectorStoreError>;

    /// Return the top-`k` nearest records for the given query vector.
    fn nearest(&self, query: VectorQuery<'_>) -> Result<Vec<VectorStoreHit>, VectorStoreError>;

    /// Drop every chunk whose `file_path` matches `path`.
    ///
    /// Used by the lazy-on-read invalidation path: when a file's content hash
    /// has changed, its stale chunks are cleared before fresh ones are
    /// upserted.
    fn invalidate_by_path(&mut self, path: &str) -> Result<(), VectorStoreError>;

    /// Stable identifier of the backing implementation
    /// (e.g. `"brute-force"`, `"hnsw_rs"`). Written to the index header so
    /// `xoxo` can refuse-on-mismatch if the selected backend changes — see
    /// ADR 0003 § "Observability and cost" + the reserved
    /// `vector_store_kind` header field.
    fn kind(&self) -> &'static str;
}
