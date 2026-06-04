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

/// Nested-classification blend weight. The final cell score is
/// `ALPHA·domain_score + (1-ALPHA)·cell_score`, where `domain_score` is the
/// top-2 mean of the cell's DOMAIN's per-cell scores. Domain is the reliable
/// 5-way signal; blending it as a prior disambiguates same-domain modes (lifts
/// cell accuracy 0.795 → 0.821 on MiniLM) without changing domain top-1.
/// Empirically flat across 0.35–0.5; >0.5 starts to hurt domain. 0.0 = flat.
const ALPHA: f32 = 0.4;

/// Mean of the top-`k` values of an already-descending-sorted slice.
fn topk_mean(sorted_desc: &[f32], k: usize) -> f32 {
    if sorted_desc.is_empty() {
        return 0.0;
    }
    let k = k.min(sorted_desc.len()).max(1);
    sorted_desc.iter().take(k).sum::<f32>() / k as f32
}

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

    /// Classify a pre-computed embedding with soft nested (domain→mode) scoring.
    ///
    /// Per cell: `cell_score` = mean of its top-`TOP_K` member cosines.
    /// Per domain: `domain_score` = top-2 mean of that domain's per-cell scores
    /// (cell-level, not entry-pooled — pooling all entries would favour
    /// entry-rich domains). Final score = `ALPHA·domain_score +
    /// (1-ALPHA)·cell_score`, so the strongest domain acts as a prior and the
    /// mode is disambiguated within it. Results sorted by final score, desc.
    pub fn classify(&self, embedding: &[f32]) -> Vec<CellScore> {
        // First pass: per-cell top-k cell score + collect per-domain cell scores.
        let mut domain_cell_scores: HashMap<&str, Vec<f32>> = HashMap::new();
        let mut cell_scores: Vec<f32> = Vec::with_capacity(self.cells.len());
        for cell in &self.cells {
            let mut sims: Vec<f32> = cell.embeddings.iter().map(|e| cosine_sim(embedding, e)).collect();
            sims.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
            let cs = topk_mean(&sims, TOP_K);
            cell_scores.push(cs);
            domain_cell_scores.entry(cell.domain.as_str()).or_default().push(cs);
        }

        // Per-domain score: top-2 mean of the domain's *cell* scores (not entry-
        // pooled — pooling all entries favours entry-rich domains by sampling).
        let domain_score: HashMap<&str, f32> = domain_cell_scores
            .into_iter()
            .map(|(d, mut cs)| {
                cs.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
                (d, topk_mean(&cs, 2))
            })
            .collect();

        // Second pass: blend.
        let mut results: Vec<CellScore> = self
            .cells
            .iter()
            .zip(cell_scores)
            .map(|(cell, cell_score)| {
                let ds = *domain_score.get(cell.domain.as_str()).unwrap_or(&0.0);
                CellScore {
                    domain: cell.domain.clone(),
                    mode: cell.mode.clone(),
                    score: ALPHA * ds + (1.0 - ALPHA) * cell_score,
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
