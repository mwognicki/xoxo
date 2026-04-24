//! HNSW-backed approximate nearest-neighbour vector store via `instant-distance`.
//!
//! Unlike [`super::hnsw::HnswStore`] (which supports incremental insertion),
//! `instant-distance` builds its index in a single batch via [`Builder`]. Every
//! mutation (upsert or invalidation) triggers a full rebuild. This is
//! acceptable for v1 chunk counts (ADR 0003: "low 10⁵ is tolerable") and can be
//! optimised later with a dirty-flag + lazy-rebuild if profiling shows the
//! rebuild cost dominating.

use std::collections::HashMap;

use instant_distance::{Builder, HnswMap, Point, Search};

use super::super::chunk::{ChunkId, ChunkRecord};
use super::super::vector_store::{VectorQuery, VectorStore, VectorStoreError, VectorStoreHit};

const EF_CONSTRUCTION: usize = 200;
const EF_SEARCH: usize = 200;

#[derive(Clone)]
struct CosinePoint(Vec<f32>);

impl Point for CosinePoint {
    fn distance(&self, other: &Self) -> f32 {
        let norm_a = norm(&self.0);
        let norm_b = norm(&other.0);
        if norm_a == 0.0 || norm_b == 0.0 {
            return 1.0;
        }
        let dot: f32 = self.0.iter().zip(other.0.iter()).map(|(a, b)| a * b).sum();
        1.0 - dot / (norm_a * norm_b)
    }
}

fn norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

struct Entry {
    record: ChunkRecord,
    vector: Vec<f32>,
}

pub struct InstantDistanceStore {
    dimensions: Option<usize>,
    entries: HashMap<ChunkId, Entry>,
    index: Option<HnswMap<CosinePoint, ChunkId>>,
}

impl InstantDistanceStore {
    pub fn new() -> Self {
        Self {
            dimensions: None,
            entries: HashMap::new(),
            index: None,
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

    fn rebuild(&mut self) {
        if self.entries.is_empty() {
            self.index = None;
            return;
        }

        let mut points = Vec::with_capacity(self.entries.len());
        let mut values = Vec::with_capacity(self.entries.len());

        for (chunk_id, entry) in &self.entries {
            points.push(CosinePoint(entry.vector.clone()));
            values.push(chunk_id.clone());
        }

        self.index = Some(
            Builder::default()
                .ef_construction(EF_CONSTRUCTION)
                .ef_search(EF_SEARCH)
                .build(points, values),
        );
    }
}

impl VectorStore for InstantDistanceStore {
    fn upsert(&mut self, record: ChunkRecord, vector: Vec<f32>) -> Result<(), VectorStoreError> {
        self.ensure_dimensions(vector.len())?;
        self.entries.insert(
            record.id.clone(),
            Entry { record, vector },
        );
        self.rebuild();
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

        let index = match &self.index {
            Some(idx) => idx,
            None => return Ok(Vec::new()),
        };

        let mut search = Search::default();
        let query_point = CosinePoint(query.vector.to_vec());

        let hits: Vec<VectorStoreHit> = index
            .search(&query_point, &mut search)
            .map(|item| VectorStoreHit {
                record: self
                    .entries
                    .get(item.value)
                    .expect("index value must correspond to an entry")
                    .record
                    .clone(),
                score: 1.0 - item.distance,
            })
            .take(query.k)
            .collect();

        Ok(hits)
    }

    fn invalidate_by_path(&mut self, path: &str) -> Result<(), VectorStoreError> {
        let to_remove: Vec<ChunkId> = self
            .entries
            .iter()
            .filter(|(_, e)| e.record.file_path == path)
            .map(|(id, _)| id.clone())
            .collect();

        if !to_remove.is_empty() {
            for id in &to_remove {
                self.entries.remove(id);
            }
            self.rebuild();
        }

        Ok(())
    }

    fn kind(&self) -> &'static str {
        "instant-distance"
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
        let mut store = InstantDistanceStore::new();
        store
            .upsert(record("a", "a.rs"), vec![1.0, 0.0])
            .expect("upsert");
        let got = store.lookup_by_id(&ChunkId::new("a")).expect("lookup");
        assert_eq!(got.map(|r| r.file_path).as_deref(), Some("a.rs"));
    }

    #[test]
    fn nearest_orders_by_cosine_similarity() {
        let mut store = InstantDistanceStore::new();
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
        let mut store = InstantDistanceStore::new();
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
        let mut store = InstantDistanceStore::new();
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
        let mut store = InstantDistanceStore::new();
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
