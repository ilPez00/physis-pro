//! Ontology-driven feedback loop — when user says "this is bad/wrong",
//! capture the negative signal through semiotic grid analysis + ontology mapping,
//! store as vector-space negative examples to bias future generations.
//!
//! The engine now lives in `physis-core`; this module is a thin facade that
//! hooks physis_pro's `VectorEmbed`/`OntologyLoader` types onto it, keeping
//! physis_pro's public API (2-arg `new`, field access) unchanged.

use std::path::Path;

use crate::config::OntologyLoader;
use crate::embed::VectorEmbed;

pub use physis_core::quality::QualityFailure;

/// Adapter making a physis_pro embedder satisfy physis_core's `VectorEmbed`
/// (which lacks `embed_image`; the engine never needs it).
struct CoreEmbed(Box<dyn VectorEmbed>);

impl physis_core::embed::VectorEmbed for CoreEmbed {
    fn embed(&self, text: &str) -> Vec<f32> {
        self.0.embed(text)
    }
    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        self.0.embed_batch(texts)
    }
    fn dimension(&self) -> usize {
        self.0.dimension()
    }
}

/// Tracks quality feedback and applies penalty/gain to semiotic cells. Facade
/// over `physis_core::quality::QualityTracker`; the core's public fields let
/// `tracker.cell_penalties` / `tracker.failures` access resolve through Deref.
pub struct QualityTracker(pub physis_core::quality::QualityTracker);

impl std::ops::Deref for QualityTracker {
    type Target = physis_core::quality::QualityTracker;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for QualityTracker {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl QualityTracker {
    pub fn new(embedder: Box<dyn VectorEmbed>, _ontology: OntologyLoader) -> Self {
        Self(physis_core::quality::QualityTracker::new(Box::new(
            CoreEmbed(embedder),
        )))
    }

    /// Look up `failures` type from the core without deref (associated fn).
    pub fn load(path: &Path) -> anyhow::Result<Vec<QualityFailure>> {
        physis_core::quality::QualityTracker::load(path)
    }
}

/// Load from disk, rebuilding penalties from the recorded failures.
pub fn load_or_new(path: &Path) -> QualityTracker {
    QualityTracker(physis_core::quality::QualityTracker::load_or_new(path))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::embed::RandomProjectionEmbedder;

    fn make_tracker() -> QualityTracker {
        let embedder: Box<dyn VectorEmbed> = Box::new(RandomProjectionEmbedder::new(384));
        let ontology = OntologyLoader::new();
        QualityTracker::new(embedder, ontology)
    }

    #[test]
    fn test_report_failure_creates_record() {
        let mut tracker = make_tracker();
        let mut centroids = HashMap::new();
        centroids.insert("WORK\x00DO".to_string(), vec![0.5; 384]);

        let (top_cell, top_score) = {
            let f =
                tracker.report_failure("this classification is completely wrong", &centroids, None);
            (f.top_cell.clone(), f.top_score)
        };
        assert_eq!(tracker.failures.len(), 1);
        assert_eq!(top_cell, "WORK\x00DO");
        assert!(top_score.is_finite());
    }

    #[test]
    fn test_cell_penalty_accumulates() {
        let mut tracker = make_tracker();
        let mut centroids = HashMap::new();
        centroids.insert("WORK\x00DO".to_string(), vec![0.5; 384]);

        tracker.report_failure("wrong", &centroids, None);
        assert!(tracker.cell_penalties.contains_key("WORK\x00DO"));
        assert!(*tracker.cell_penalties.get("WORK\x00DO").unwrap() > 0.0);
    }

    #[test]
    fn test_adjust_score_with_penalty() {
        let mut tracker = make_tracker();
        let mut centroids = HashMap::new();
        centroids.insert("WORK\x00DO".to_string(), vec![0.5; 384]);

        tracker.report_failure("wrong", &centroids, None);
        let adjusted = tracker.adjust_score("WORK\x00DO", 0.8);
        assert!(adjusted < 0.8, "penalty should reduce score");
    }

    #[test]
    fn test_adjust_score_with_boost() {
        let mut t = make_tracker();
        let decay_rate = t.boost_decay_rate;
        t.cell_boosts.insert(
            "WORK\x00DO".to_string(),
            physis_core::quality::BoostInfo {
                amount: 0.5,
                decay_rate,
                last_reinforced: std::time::Instant::now(),
            },
        );
        let adjusted = t.adjust_score("WORK\x00DO", 0.8);
        assert!(adjusted > 0.8, "boost should increase score");
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let mut tracker = make_tracker();
        let mut centroids = HashMap::new();
        centroids.insert("TEST\x00CELL".to_string(), vec![0.5; 384]);
        tracker.report_failure("test failure", &centroids, None);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("failures.json");
        tracker.save(&path).unwrap();

        let loaded = QualityTracker::load(&path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].feedback, "test failure");
    }

    #[test]
    fn test_summary_format() {
        let tracker = make_tracker();
        let summary = tracker.summary();
        assert!(summary.contains("No failures recorded"));
    }
}
