//! Linear-scan cosine-similarity vector store.
//!
//! This is the zero-dependency default. It keeps the full set of records and
//! vectors in memory and iterates the whole collection on every query. ADR
//! 0003 explicitly accepts this as the v0/v1 shape while chunking and ranking
//! quality are validated; when chunk counts outgrow brute force, an ANN
//! implementation slots in as a sibling feature without reshaping the
//! [`super::super::VectorStore`] trait.

use super::super::chunk::{ChunkId, ChunkRecord};
use super::super::vector_store::{
    VectorQuery, VectorStore, VectorStoreError, VectorStoreHit,
};

/// In-memory `(record, vector)` pairs scanned linearly on each query.
#[derive(Debug, Default)]
pub struct BruteForceStore {
    dimensions: Option<usize>,
    entries: Vec<Entry>,
}

#[derive(Debug)]
struct Entry {
    record: ChunkRecord,
    vector: Vec<f32>,
}

impl BruteForceStore {
    pub fn new() -> Self {
        Self::default()
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

    fn position_of(&self, id: &ChunkId) -> Option<usize> {
        self.entries.iter().position(|e| &e.record.id == id)
    }
}

impl VectorStore for BruteForceStore {
    fn upsert(&mut self, record: ChunkRecord, vector: Vec<f32>) -> Result<(), VectorStoreError> {
        self.ensure_dimensions(vector.len())?;
        match self.position_of(&record.id) {
            Some(idx) => {
                self.entries[idx] = Entry { record, vector };
            }
            None => self.entries.push(Entry { record, vector }),
        }
        Ok(())
    }

    fn lookup_by_id(&self, id: &ChunkId) -> Result<Option<ChunkRecord>, VectorStoreError> {
        Ok(self
            .position_of(id)
            .map(|idx| self.entries[idx].record.clone()))
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

        let query_norm = norm(query.vector);
        if query_norm == 0.0 || query.k == 0 {
            return Ok(Vec::new());
        }

        let mut scored: Vec<VectorStoreHit> = self
            .entries
            .iter()
            .map(|entry| {
                let score = cosine(query.vector, &entry.vector, query_norm);
                VectorStoreHit {
                    record: entry.record.clone(),
                    score,
                }
            })
            .collect();

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(query.k);
        Ok(scored)
    }

    fn invalidate_by_path(&mut self, path: &str) -> Result<(), VectorStoreError> {
        self.entries.retain(|entry| entry.record.file_path != path);
        Ok(())
    }

    fn kind(&self) -> &'static str {
        "brute-force"
    }
}

fn norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

fn cosine(query: &[f32], other: &[f32], query_norm: f32) -> f32 {
    if query.len() != other.len() {
        return 0.0;
    }
    let other_norm = norm(other);
    if other_norm == 0.0 {
        return 0.0;
    }
    let dot: f32 = query.iter().zip(other.iter()).map(|(a, b)| a * b).sum();
    dot / (query_norm * other_norm)
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
            symbol_path: "facade::sym".to_string(),
            language: Some("rust".to_string()),
            byte_range: (0, 1),
            line_range: (1, 1),
            content_hash: ContentHash::new("h"),
            embedding_backend_id: "test".to_string(),
        }
    }

    #[test]
    fn upsert_and_lookup_round_trip() {
        let mut store = BruteForceStore::new();
        store
            .upsert(record("a", "a.rs"), vec![1.0, 0.0])
            .expect("upsert");
        let got = store.lookup_by_id(&ChunkId::new("a")).expect("lookup");
        assert_eq!(got.map(|r| r.file_path).as_deref(), Some("a.rs"));
    }

    #[test]
    fn nearest_orders_by_cosine_similarity() {
        let mut store = BruteForceStore::new();
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
        let mut store = BruteForceStore::new();
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
        let mut store = BruteForceStore::new();
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
}
