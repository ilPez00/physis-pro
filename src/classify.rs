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
//! Extracted from the web server so the same path is unit-testable (see
//! `tests/classification.rs`) and reusable by the CLI.

use std::collections::HashMap;

use serde::Serialize;

use crate::config::OntologyLoader;
use crate::embed::VectorEmbed;
use crate::models::cosine_sim;

/// How many top member-entry similarities to average per cell. 1 = pure
/// nearest-entry; >1 smooths noise from a single outlier entry.
const TOP_K: usize = 2;

/// Score for a single DOMAIN×MODE cell.
#[derive(Debug, Clone, Serialize)]
pub struct CellScore {
    pub domain: String,
    pub mode: String,
    pub score: f32,
    pub entries: Vec<String>,
}

/// One populated cell with its member entry embeddings.
struct Cell {
    domain: String,
    mode: String,
    /// Per-entry embeddings (one row per member entry).
    embeddings: Vec<Vec<f32>>,
    /// Member entry display names (parallel to `embeddings`).
    entries: Vec<String>,
}

/// Pre-computed cell member embeddings ready for classification.
pub struct CellClassifier {
    cells: Vec<Cell>,
}

impl CellClassifier {
    /// Embed every classification ontology entry ("name + hints") and group the
    /// embeddings by DOMAIN×MODE cell.
    pub fn build(ontology: &OntologyLoader, embedder: &dyn VectorEmbed) -> Self {
        // (domain, mode) → (embeddings, entry names)
        type Acc = (Vec<Vec<f32>>, Vec<String>);
        let mut acc: HashMap<(String, String), Acc> = HashMap::new();
        for def in ontology.classification_domains() {
            let (domain, mode) = match (def.domain.as_deref(), def.mode.as_deref()) {
                (Some(d), Some(m)) => (d.to_string(), m.to_string()),
                _ => continue,
            };
            let mut text = def.name.clone();
            for hint in &def.hints {
                text.push(' ');
                text.push_str(hint);
            }
            let emb = embedder.embed(&text);
            let entry = acc.entry((domain, mode)).or_insert_with(|| (Vec::new(), Vec::new()));
            entry.0.push(emb);
            entry.1.push(def.name.clone());
        }
        let cells = acc
            .into_iter()
            .map(|((domain, mode), (embeddings, entries))| Cell { domain, mode, embeddings, entries })
            .collect();
        Self { cells }
    }

    /// Number of populated cells.
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Classify a pre-computed embedding. Each cell scores as the mean of its
    /// top-`TOP_K` member-entry cosine similarities; results sorted descending.
    pub fn classify(&self, embedding: &[f32]) -> Vec<CellScore> {
        let mut results: Vec<CellScore> = self
            .cells
            .iter()
            .map(|cell| {
                let mut sims: Vec<f32> = cell
                    .embeddings
                    .iter()
                    .map(|e| cosine_sim(embedding, e))
                    .collect();
                sims.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
                let k = TOP_K.min(sims.len()).max(1);
                let score = sims.iter().take(k).sum::<f32>() / k as f32;
                CellScore {
                    domain: cell.domain.clone(),
                    mode: cell.mode.clone(),
                    score,
                    entries: cell.entries.clone(),
                }
            })
            .collect();
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Embed `text` and classify it in one call.
    pub fn classify_text(&self, text: &str, embedder: &dyn VectorEmbed) -> Vec<CellScore> {
        self.classify(&embedder.embed(text))
    }
}
