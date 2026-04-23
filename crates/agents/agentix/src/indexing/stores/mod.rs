//! Pluggable [`super::VectorStore`] implementations.
//!
//! Each implementation is gated behind a sibling Cargo feature. The runtime
//! [`selector`] picks one based on config, mirroring the LLM-backend pattern
//! in `xoxo-core::llm::backends::selector`.
//!
//! Currently shipped:
//!
//! - `vector-store-brute-force` (default) — linear cosine scan. Zero extra
//!   deps, fine up to the "low 10⁵ chunks" range ADR 0003 calls tolerable.
//!
//! Reserved for future sibling features (no code yet; adding one is a new
//! file + a new `[features]` entry + a new [`VectorStoreKind`] variant +
//! a new selector branch):
//!
//! - `vector-store-hnsw-rs` — HNSW ANN via the `hnsw_rs` crate.
//! - `vector-store-instant-distance` — HNSW ANN via `instant-distance`.

#[cfg(feature = "vector-store-brute-force")]
pub mod brute_force;

#[cfg(feature = "vector-store-hnsw-rs")]
pub mod hnsw;

#[cfg(feature = "vector-store-instant-distance")]
pub mod instant_distance;

pub mod selector;

pub use selector::{VectorStoreKind, select_vector_store};
