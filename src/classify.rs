//! Semiotic-grid classification: embed text/image, score against per-cell
//! centroids, rank DOMAIN×MODE cells.
//!
//! Centroids are pre-computed once from the ontology hints (name + hints,
//! averaged per cell) using whatever embedder is active. Classification is then
//! a cosine comparison of an input embedding against every cell centroid.
//!
//! Extracted from the web server so the same path is unit-testable (see
//! `tests/classification.rs`) and reusable by the CLI.

use std::collections::HashMap;

use serde::Serialize;

use crate::config::OntologyLoader;
use crate::embed::VectorEmbed;
use crate::models::cosine_sim;

/// Score for a single DOMAIN×MODE cell.
#[derive(Debug, Clone, Serialize)]
pub struct CellScore {
    pub domain: String,
    pub mode: String,
    pub score: f32,
    pub entries: Vec<String>,
}

/// One populated cell: (domain, mode, centroid vector, member entry names).
type Cell = (String, String, Vec<f32>, Vec<String>);

/// Pre-computed cell centroids ready for classification.
pub struct CellClassifier {
    cells: Vec<Cell>,
}

impl CellClassifier {
    /// Build centroids from every classification ontology entry. Each cell's
    /// centroid is the mean embedding of its members' "name + hints" text.
    pub fn build(ontology: &OntologyLoader, embedder: &dyn VectorEmbed) -> Self {
        // (domain, mode) → (summed vec, count, entry names)
        type Acc = (Vec<f32>, usize, Vec<String>);
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
            let entry = acc
                .entry((domain, mode))
                .or_insert_with(|| (vec![0.0f32; emb.len()], 0, Vec::new()));
            if entry.0.len() != emb.len() {
                // Dimension mismatch (shouldn't happen with one embedder) — skip.
                continue;
            }
            for (i, v) in emb.iter().enumerate() {
                entry.0[i] += v;
            }
            entry.1 += 1;
            entry.2.push(def.name.clone());
        }
        let cells = acc
            .into_iter()
            .map(|((domain, mode), (sum, count, entries))| {
                let n = count.max(1) as f32;
                let centroid: Vec<f32> = sum.into_iter().map(|v| v / n).collect();
                (domain, mode, centroid, entries)
            })
            .collect();
        Self { cells }
    }

    /// Number of populated cells.
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Classify a pre-computed embedding: cosine vs every cell centroid,
    /// returned sorted by descending score.
    pub fn classify(&self, embedding: &[f32]) -> Vec<CellScore> {
        let mut results: Vec<CellScore> = self
            .cells
            .iter()
            .map(|(domain, mode, centroid, entries)| CellScore {
                domain: domain.clone(),
                mode: mode.clone(),
                score: cosine_sim(embedding, centroid),
                entries: entries.clone(),
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
