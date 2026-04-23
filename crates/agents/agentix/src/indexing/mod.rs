//! Shared indexing contracts for agents that embed and retrieve content.
//!
//! This module defines the vendor-neutral chunk + vector-store surface used by
//! `nerd` (and later `concierge`) to plug embedding-backed discovery tools on
//! top of the deterministic layer. See
//! `docs/adr/0003-embeddings-and-vector-search.md` for the design rationale.
//!
//! # Layering
//!
//! - [`chunk`] — data types (`ChunkRecord`, `ChunkKind`, `ContentHash`) shared
//!   across every agent that embeds content. No backend-specific types here.
//! - [`vector_store`] — the [`VectorStore`] trait plus query/error types.
//!   Callers depend only on this seam.
//! - [`stores`] — pluggable implementations gated behind sibling Cargo features
//!   (`vector-store-brute-force`, and — as they land — `vector-store-hnsw-rs`,
//!   `vector-store-instant-distance`). A runtime selector picks one based on
//!   config, mirroring the LLM facade in `xoxo-core::llm`.
//!
//! Switching implementations never changes the on-disk chunk layout: the raw
//! `[f32; D]` vectors remain the durable source of truth and any in-memory ANN
//! graph is hydrated from them at startup.

pub mod chunk;
pub mod stores;
pub mod vector_store;

pub use chunk::{ChunkId, ChunkKind, ChunkRecord, ContentHash};
pub use vector_store::{VectorQuery, VectorStore, VectorStoreError, VectorStoreHit};
