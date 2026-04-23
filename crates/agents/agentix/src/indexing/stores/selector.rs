//! Runtime selection of a [`VectorStore`] implementation.
//!
//! Mirrors `xoxo-core::llm::backends::selector`: config carries a kind tag,
//! this module maps the tag to a concrete, feature-gated impl, and callers
//! receive a trait object so nothing downstream depends on which backend was
//! chosen.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::super::vector_store::VectorStore;

/// Configured vector-store implementation.
///
/// Adding a new sibling feature is a three-line change here: one variant, one
/// `serde` rename, one branch in [`select_vector_store`]. The rest of the
/// workspace keeps talking to `dyn VectorStore`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum VectorStoreKind {
    /// Linear cosine scan. Default when the `vector-store-brute-force`
    /// feature is enabled.
    BruteForce,
    #[cfg(feature = "vector-store-hnsw-rs")]
    Hnsw,
    #[cfg(feature = "vector-store-instant-distance")]
    InstantDistance,
}

impl VectorStoreKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BruteForce => "brute-force",
            #[cfg(feature = "vector-store-hnsw-rs")]
            Self::Hnsw => "hnsw-rs",
            #[cfg(feature = "vector-store-instant-distance")]
            Self::InstantDistance => "instant-distance",
        }
    }
}

/// Failures that can surface when picking a vector store at startup.
#[derive(Debug, Error)]
pub enum VectorStoreSelectionError {
    /// The requested kind is known to the enum but its backing feature was
    /// not enabled for this build.
    #[error("vector store kind `{0}` is not available: the corresponding Cargo feature is disabled")]
    FeatureDisabled(&'static str),
}

/// Instantiate the implementation requested by config.
///
/// Returns a boxed trait object so the selected backend is opaque to callers,
/// matching the LLM facade. If the matching feature is off, the error is
/// actionable ("enable `vector-store-brute-force`") rather than a silent
/// fallback to a different backend — silent mismatches are exactly the class
/// of bug ADR 0003 wants to avoid.
pub fn select_vector_store(
    kind: VectorStoreKind,
) -> Result<Box<dyn VectorStore>, VectorStoreSelectionError> {
    match kind {
        VectorStoreKind::BruteForce => {
            #[cfg(feature = "vector-store-brute-force")]
            {
                Ok(Box::new(super::brute_force::BruteForceStore::new()))
            }
            #[cfg(not(feature = "vector-store-brute-force"))]
            {
                Err(VectorStoreSelectionError::FeatureDisabled(
                    "vector-store-brute-force",
                ))
            }
        }
        #[cfg(feature = "vector-store-hnsw-rs")]
        VectorStoreKind::Hnsw => Ok(Box::new(super::hnsw::HnswStore::new())),
        #[cfg(feature = "vector-store-instant-distance")]
        VectorStoreKind::InstantDistance => {
            Ok(Box::new(super::instant_distance::InstantDistanceStore::new()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_str_is_stable_for_config_round_trip() {
        assert_eq!(VectorStoreKind::BruteForce.as_str(), "brute-force");
    }

    #[cfg(feature = "vector-store-brute-force")]
    #[test]
    fn selector_returns_brute_force_when_feature_enabled() {
        let store = select_vector_store(VectorStoreKind::BruteForce).expect("brute-force available");
        assert_eq!(store.kind(), "brute-force");
    }
}
