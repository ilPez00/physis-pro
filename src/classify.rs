//! Semiotic-grid classification: embed text/image, score against ontology
//! entries, rank DOMAIN×MODE cells.
//!
//! Scoring is **nearest-entry (max cosine)**: each cell's score is the cosine of
//! the query against its best-matching member entry, not against a mean
//! centroid. With weak/anisotropic embedders a mean centroid of a densely
//! populated cell (e.g. BOND) drifts toward the common direction and wins
//! regardless of the query; scoring by the single closest entry removes that
//! class-imbalance bias. A small top-k average smooths single-entry noise.
//!
//! The engine now lives in `physis-core`; this module is a thin facade that
//! hooks physis_pro's own `OntologyLoader`/`VectorEmbed` types onto it, keeping
//! physis_pro's public API unchanged.

use std::collections::HashMap;

use physis_core::classify::EntrySeed;

use crate::config::OntologyLoader;
use crate::embed::VectorEmbed;

pub use physis_core::classify::CellScore;

/// Pre-computed cell member embeddings ready for classification. Facade over
/// `physis_core::classify::CellClassifier`.
pub struct CellClassifier(pub physis_core::classify::CellClassifier);

impl CellClassifier {
    /// Embed every classification ontology entry ("name + hints") and group the
    /// embeddings by DOMAIN×MODE cell. Hook: maps physis_pro's ontology over the
    /// core entry-seed builder.
    pub fn build(ontology: &OntologyLoader, embedder: &dyn VectorEmbed) -> Self {
        let seeds = ontology.classification_domains().filter_map(|def| {
            let (domain, mode) = match (def.domain.as_deref(), def.mode.as_deref()) {
                (Some(d), Some(m)) => (d.to_string(), m.to_string()),
                _ => return None,
            };
            let mut text = def.name.clone();
            for hint in &def.hints {
                text.push(' ');
                text.push_str(hint);
            }
            Some(EntrySeed { name: def.name.clone(), domain, mode, text, facets: def.facets.clone() })
        });
        Self(physis_core::classify::CellClassifier::build_seeds(seeds, |t| embedder.embed(t)))
    }

    /// Number of populated cells.
    pub fn cell_count(&self) -> usize {
        self.0.cell_count()
    }

    /// Classify a pre-computed embedding with soft nested (domain→mode) scoring.
    pub fn classify(&self, embedding: &[f32]) -> Vec<CellScore> {
        self.0.classify(embedding)
    }

    /// Embed `text` and classify it in one call.
    pub fn classify_text(&self, text: &str, embedder: &dyn VectorEmbed) -> Vec<CellScore> {
        self.classify(&embedder.embed(text))
    }

    /// Mean-of-entries centroid per cell, keyed `"DOMAIN\x00MODE"`.
    pub fn cell_centroids(&self) -> HashMap<String, Vec<f32>> {
        self.0.cell_centroids()
    }

    /// Raw best single-entry cosine and its owning cell — the ontology coverage
    /// of an embedding, undiluted by blended scoring.
    pub fn best_entry_sim(&self, embedding: &[f32]) -> Option<(f32, String, String)> {
        self.0.best_entry_sim(embedding)
    }
}

/// Inject a pre-built classifier (used by tests and adapters).
pub fn from_cells(cells: Vec<physis_core::classify::Cell>) -> CellClassifier {
    CellClassifier(physis_core::classify::CellClassifier::from_cells(cells))
}

/// Re-export the core cell type so physis_pro can construct classifiers from
/// raw cells when needed (e.g. unit tests).
pub use physis_core::classify::Cell as CoreCell;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::RandomProjectionEmbedder;

    fn cell(domain: &str, mode: &str, embeddings: Vec<Vec<f32>>) -> CoreCell {
        let entries = (0..embeddings.len()).map(|i| format!("{domain}/{mode}#{i}")).collect();
        let facets = vec![physis_core::models::Facets::default(); embeddings.len()];
        CoreCell::new(domain.to_string(), mode.to_string(), embeddings, entries, facets)
    }

    fn basis(dim: usize, axis: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; dim];
        v[axis] = 1.0;
        v
    }

    #[test]
    fn build_from_ontology_and_embedder() {
        let ontology = OntologyLoader::load_all(&crate::config::PhysisConfig::default());
        let embedder = RandomProjectionEmbedder::new(384);
        let clf = CellClassifier::build(&ontology, &embedder);
        assert!(clf.cell_count() >= 20);
    }

    #[test]
    fn classify_ranks_matching_cell_first_and_sorts_desc() {
        let clf = from_cells(vec![
            cell("A", "X", vec![basis(4, 0)]),
            cell("A", "Y", vec![basis(4, 1)]),
            cell("B", "X", vec![basis(4, 2)]),
        ]);
        let results = clf.classify(&basis(4, 0));
        assert_eq!(results.len(), 3);
        assert_eq!((results[0].domain.as_str(), results[0].mode.as_str()), ("A", "X"));
        for w in results.windows(2) {
            assert!(w[0].score >= w[1].score);
        }
    }

    #[test]
    fn nearest_entry_scoring_resists_dense_cell_hub_bias() {
        let hub = basis(4, 3);
        let dense = cell("B", "X", vec![hub.clone(); 10]);
        let sparse = cell("A", "X", vec![basis(4, 0)]);
        let clf = from_cells(vec![dense, sparse]);
        let results = clf.classify(&basis(4, 0));
        assert_eq!(results[0].domain, "A");
    }

    #[test]
    fn best_entry_sim_returns_global_max() {
        let clf = from_cells(vec![
            cell("A", "X", vec![basis(4, 1), basis(4, 0)]),
            cell("B", "Y", vec![basis(4, 2)]),
        ]);
        let (score, domain, mode) = clf.best_entry_sim(&basis(4, 0)).unwrap();
        assert!((score - 1.0).abs() < 1e-6);
        assert_eq!((domain.as_str(), mode.as_str()), ("A", "X"));
    }

    #[test]
    fn cell_centroids_mean_and_key_format() {
        let clf = from_cells(vec![cell("A", "X", vec![vec![1.0, 0.0], vec![0.0, 1.0]])]);
        let centroids = clf.cell_centroids();
        let c = centroids.get("A\x00X").expect("key must be DOMAIN\\x00MODE");
        assert_eq!(c, &vec![0.5, 0.5]);
    }
}