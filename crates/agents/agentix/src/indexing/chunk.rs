//! Chunk identity and metadata shared across all indexing backends.
//!
//! The types here are deliberately minimal: enough to let the
//! [`super::VectorStore`] trait be vendor-neutral, without prescribing how
//! `nerd` (or any future agent) decides what to chunk. Concrete chunking
//! strategies (AST-symbol-bounded for code, paragraph-bounded for docs, etc.)
//! live in the consuming agent crates.

use serde::{Deserialize, Serialize};

/// Stable identifier for a single indexed chunk.
///
/// Callers are expected to derive this deterministically from
/// `(file_path, symbol_path, language, content_hash)` — see ADR 0003 §
/// "Chunk-level invalidation rule". Keeping the id opaque at this layer lets
/// individual agents choose their own derivation without leaking it into the
/// storage surface.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkId(pub String);

impl std::fmt::Display for ChunkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl ChunkId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Content-hash of the chunk body at embedding time.
///
/// Used to skip re-embedding chunks whose `(symbol_path, hash)` pair has not
/// changed between reindex runs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentHash(pub String);

impl ContentHash {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What kind of content a chunk contains.
///
/// Intentionally starts narrow (`CodeSymbol` only, matching ADR 0003's v1
/// scope). Additional variants (docs, chat transcripts, memory) will be added
/// as those milestones land — see ADR 0003 § "Alternatives considered".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ChunkKind {
    /// An AST-symbol-bounded slice of source code.
    CodeSymbol,
}

/// Durable metadata for a single chunk.
///
/// The vector itself lives alongside this record but is not embedded in the
/// struct: vector storage is impl-specific (raw `[f32; D]` bytes in sled
/// today; potentially a packed ANN graph on disk later) and must not leak
/// into the trait surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkRecord {
    pub id: ChunkId,
    pub kind: ChunkKind,
    /// Absolute path of the file the chunk was extracted from.
    pub file_path: String,
    /// Agent-defined symbol path (e.g. `module::Struct::method`). Empty for
    /// chunk kinds that don't have a symbol notion.
    pub symbol_path: String,
    /// Detected language, when applicable.
    pub language: Option<String>,
    /// Inclusive-start, exclusive-end byte range inside `file_path`.
    pub byte_range: (usize, usize),
    /// 1-based inclusive-start, inclusive-end line range.
    pub line_range: (usize, usize),
    /// Hash of the chunk body captured when the vector was produced.
    pub content_hash: ContentHash,
    /// Backend id of the embedding model that produced the vector — must match
    /// the index header, see ADR 0003 § "Hard constraint".
    pub embedding_backend_id: String,
}
