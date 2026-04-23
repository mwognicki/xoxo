//! HNSW-backed approximate nearest-neighbour vector store.
//!
//! Wraps the `hnsw_rs` crate behind the [`super::super::VectorStore`] trait.
//! Vectors and metadata live in side tables; the HNSW graph holds point
//! indices only. Because `hnsw_rs` does not support point deletion, stale
//! entries are removed from the side tables on invalidation and silently
//! skipped during search — the graph retains dead nodes, which wastes memory
//! proportional to churn but never affects correctness. A full index rebuild
//! would be needed to reclaim that memory.

use std::collections::HashMap;

use hnsw_rs::anndists::dist::DistCosine;
use hnsw_rs::hnsw::Hnsw;

use super::super::chunk::{ChunkId, ChunkRecord};
use super::super::vector_store::{VectorQuery, VectorStore, VectorStoreError, VectorStoreHit};

const MAX_NB_CONNECTION: usize = 16;
const MAX_ELEMENTS_HINT: usize = 100_000;
const MAX_LAYER: usize = 16;
const EF_CONSTRUCTION: usize = 200;
const EF_SEARCH: usize = 200;

struct Entry {
    record: ChunkRecord,
    hnsw_id: usize,
}

/// ANN vector store backed by `hnsw_rs`.
///
/// Constructed without knowing the embedding dimension — it is fixed on the
/// first upsert and enforced from then on, matching the
/// [`super::brute_force::BruteForceStore`] pattern.
pub struct HnswStore {
    dimensions: Option<usize>,
    index: Hnsw<'static, f32, DistCosine>,
    entries: HashMap<ChunkId, Entry>,
    id_to_chunk: HashMap<usize, ChunkId>,
    next_id: usize,
}

impl HnswStore {
    pub fn new() -> Self {
        Self {
            dimensions: None,
            index: Hnsw::new(
                MAX_NB_CONNECTION,
                MAX_ELEMENTS_HINT,
                MAX_LAYER,
                EF_CONSTRUCTION,
                DistCosine::default(),
            ),
            entries: HashMap::new(),
            id_to_chunk: HashMap::new(),
            next_id: 0,
        }
    }

    fn ensure_dimensions(&mut self, actual: usize) -> Result<(), VectorStoreError> {
        match self.dimensions {
            Some(expected) if expected != actual => {
                Err(VectorStoreError::DimensionMismatch { expected, actual })
            }
            Some(_) => Ok(()),
            None => {
                self.dimensions = Some(actual);
                Ok(())
            }
        }
    }
}

impl VectorStore for HnswStore {
    fn upsert(&mut self, record: ChunkRecord, vector: Vec<f32>) -> Result<(), VectorStoreError> {
        self.ensure_dimensions(vector.len())?;

        if let Some(old) = self.entries.remove(&record.id) {
            self.id_to_chunk.remove(&old.hnsw_id);
        }

        let hnsw_id = self.next_id;
        self.next_id += 1;

        self.index.insert((&vector, hnsw_id));

        self.id_to_chunk.insert(hnsw_id, record.id.clone());
        self.entries.insert(
            record.id.clone(),
            Entry {
                record,
                hnsw_id,
            },
        );

        Ok(())
    }

    fn lookup_by_id(&self, id: &ChunkId) -> Result<Option<ChunkRecord>, VectorStoreError> {
        Ok(self.entries.get(id).map(|e| e.record.clone()))
    }

    fn nearest(&self, query: VectorQuery<'_>) -> Result<Vec<VectorStoreHit>, VectorStoreError> {
        if let Some(expected) = self.dimensions {
            if expected != query.vector.len() {
                return Err(VectorStoreError::DimensionMismatch {
                    expected,
                    actual: query.vector.len(),
                });
            }
        }

        if self.entries.is_empty() || query.k == 0 {
            return Ok(Vec::new());
        }

        let ef = EF_SEARCH.max(query.k);
        let neighbours = self.index.search(query.vector, query.k, ef);

        let mut hits: Vec<VectorStoreHit> = neighbours
            .into_iter()
            .filter_map(|n| {
                let chunk_id = self.id_to_chunk.get(&n.d_id)?;
                let entry = self.entries.get(chunk_id)?;
                Some(VectorStoreHit {
                    record: entry.record.clone(),
                    score: 1.0 - n.distance,
                })
            })
            .collect();

        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(query.k);
        Ok(hits)
    }

    fn invalidate_by_path(&mut self, path: &str) -> Result<(), VectorStoreError> {
        let to_remove: Vec<ChunkId> = self
            .entries
            .iter()
            .filter(|(_, e)| e.record.file_path == path)
            .map(|(id, _)| id.clone())
            .collect();

        for id in to_remove {
            if let Some(entry) = self.entries.remove(&id) {
                self.id_to_chunk.remove(&entry.hnsw_id);
            }
        }

        Ok(())
    }

    fn kind(&self) -> &'static str {
        "hnsw-rs"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexing::chunk::{ChunkKind, ContentHash};

    fn record(id: &str, path: &str) -> ChunkRecord {
        ChunkRecord {
            id: ChunkId::new(id),
            kind: ChunkKind::CodeSymbol,
            file_path: path.to_string(),
            symbol_path: "mod::sym".to_string(),
            language: Some("rust".to_string()),
            byte_range: (0, 1),
            line_range: (1, 1),
            content_hash: ContentHash::new("h"),
            embedding_backend_id: "test".to_string(),
        }
    }

    #[test]
    fn upsert_and_lookup_round_trip() {
        let mut store = HnswStore::new();
        store
            .upsert(record("a", "a.rs"), vec![1.0, 0.0])
            .expect("upsert");
        let got = store.lookup_by_id(&ChunkId::new("a")).expect("lookup");
        assert_eq!(got.map(|r| r.file_path).as_deref(), Some("a.rs"));
    }

    #[test]
    fn nearest_orders_by_cosine_similarity() {
        let mut store = HnswStore::new();
        store
            .upsert(record("aligned", "a.rs"), vec![1.0, 0.0])
            .expect("upsert");
        store
            .upsert(record("orthogonal", "b.rs"), vec![0.0, 1.0])
            .expect("upsert");

        let query_vec = vec![1.0, 0.0];
        let hits = store
            .nearest(VectorQuery {
                vector: &query_vec,
                k: 2,
            })
            .expect("nearest");

        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].record.id.as_str(), "aligned");
        assert!(hits[0].score > hits[1].score);
    }

    #[test]
    fn dimension_mismatch_on_upsert_is_reported() {
        let mut store = HnswStore::new();
        store
            .upsert(record("a", "a.rs"), vec![1.0, 0.0])
            .expect("first upsert");
        let err = store
            .upsert(record("b", "b.rs"), vec![1.0, 0.0, 0.0])
            .expect_err("dimension mismatch");
        assert!(matches!(
            err,
            VectorStoreError::DimensionMismatch {
                expected: 2,
                actual: 3
            }
        ));
    }

    #[test]
    fn invalidate_by_path_drops_matching_chunks() {
        let mut store = HnswStore::new();
        store
            .upsert(record("a", "a.rs"), vec![1.0, 0.0])
            .expect("upsert a");
        store
            .upsert(record("b", "b.rs"), vec![0.0, 1.0])
            .expect("upsert b");

        store.invalidate_by_path("a.rs").expect("invalidate");

        assert!(store
            .lookup_by_id(&ChunkId::new("a"))
            .expect("lookup a")
            .is_none());
        assert!(store
            .lookup_by_id(&ChunkId::new("b"))
            .expect("lookup b")
            .is_some());
    }

    #[test]
    fn upsert_overwrite_replaces_existing() {
        let mut store = HnswStore::new();
        store
            .upsert(record("a", "old.rs"), vec![1.0, 0.0])
            .expect("first upsert");
        store
            .upsert(record("a", "new.rs"), vec![0.0, 1.0])
            .expect("overwrite upsert");

        let got = store.lookup_by_id(&ChunkId::new("a")).expect("lookup");
        assert_eq!(got.map(|r| r.file_path).as_deref(), Some("new.rs"));
    }
}
