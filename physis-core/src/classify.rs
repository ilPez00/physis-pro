//! Semiotic-grid classification: embed text, score against ontology entries,
//! rank DOMAIN×MODE cells.
//!
//! Scoring is **nearest-entry (max cosine)**: each cell's score is the cosine of
//! the query against its best-matching member entry, not a mean centroid. A small
//! top-k average smooths single-entry noise, and a domain prior disambiguates
//! same-domain modes.

use std::collections::HashMap;

use serde::Serialize;

use crate::embed::VectorEmbed;
use crate::models::cosine_sim;
use crate::ontology::OntologyLoader;

/// How many top member-entry similarities to average per cell.
const TOP_K: usize = 2;

/// Nested-classification blend weight: final = ALPHA·domain + (1-ALPHA)·cell.
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
pub struct Cell {
    pub domain: String,
    pub mode: String,
    /// Per-entry embeddings (one row per member entry).
    pub embeddings: Vec<Vec<f32>>,
    /// Member entry display names (parallel to `embeddings`).
    pub entries: Vec<String>,
}

impl Cell {
    pub fn new(domain: String, mode: String, embeddings: Vec<Vec<f32>>, entries: Vec<String>) -> Self {
        Self { domain, mode, embeddings, entries }
    }
}

/// Pre-computed cell member embeddings ready for classification.
pub struct CellClassifier {
    pub cells: Vec<Cell>,
}

/// A raw ontology entry feed for the classifier, decoupled from any specific
/// loader type so physis-pro can hand its own `DomainDef`s straight in.
pub struct EntrySeed {
    pub name: String,
    pub domain: String,
    pub mode: String,
    /// Space-joined text the entry is embedded from (name + hints).
    pub text: String,
}

impl CellClassifier {
    /// Embed every entry seed (its `text`), group embeddings by DOMAIN×MODE cell.
    /// Takes an embed closure instead of the `VectorEmbed` trait so external
    /// crates with their own embedder traits (physis-pro) can hook in directly.
    pub fn build_seeds<F>(seeds: impl IntoIterator<Item = EntrySeed>, embed: F) -> Self
    where
        F: Fn(&str) -> Vec<f32>,
    {
        type Acc = (Vec<Vec<f32>>, Vec<String>);
        let mut acc: HashMap<(String, String), Acc> = HashMap::new();
        for seed in seeds {
            let emb = embed(&seed.text);
            let entry = acc.entry((seed.domain.clone(), seed.mode.clone())).or_insert_with(|| (Vec::new(), Vec::new()));
            entry.0.push(emb);
            entry.1.push(seed.name);
        }
        let cells = acc
            .into_iter()
            .map(|((domain, mode), (embeddings, entries))| Cell { domain, mode, embeddings, entries })
            .collect();
        Self { cells }
    }

    /// Rebuild from pre-built cells (used by tests / adapters that already
    /// constructed the internal shape).
    pub fn from_cells(cells: Vec<Cell>) -> Self {
        Self { cells }
    }

    /// Embed every classification ontology entry ("name + hints") and group the
    /// embeddings by DOMAIN×MODE cell.
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
            Some(EntrySeed { name: def.name.clone(), domain, mode, text })
        });
        Self::build_seeds(seeds, |t| embedder.embed(t))
    }

    /// Number of populated cells.
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Classify a pre-computed embedding with soft nested (domain→mode) scoring.
    pub fn classify(&self, embedding: &[f32]) -> Vec<CellScore> {
        let mut domain_cell_scores: HashMap<&str, Vec<f32>> = HashMap::new();
        let mut cell_scores: Vec<f32> = Vec::with_capacity(self.cells.len());
        for cell in &self.cells {
            let mut sims: Vec<f32> = cell.embeddings.iter().map(|e| cosine_sim(embedding, e)).collect();
            sims.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
            let cs = topk_mean(&sims, TOP_K);
            cell_scores.push(cs);
            domain_cell_scores.entry(cell.domain.as_str()).or_default().push(cs);
        }

        let domain_score: HashMap<&str, f32> = domain_cell_scores
            .into_iter()
            .map(|(d, mut cs)| {
                cs.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
                (d, topk_mean(&cs, 2))
            })
            .collect();

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

    /// Mean-of-entries centroid per cell, keyed `"DOMAIN\x00MODE"` — the shape
    /// the quality tracker consumes for penalty targeting.
    pub fn cell_centroids(&self) -> HashMap<String, Vec<f32>> {
        self.cells
            .iter()
            .filter(|c| !c.embeddings.is_empty())
            .map(|c| {
                let dim = c.embeddings[0].len();
                let n = c.embeddings.len() as f32;
                let mut mean = vec![0.0f32; dim];
                for e in &c.embeddings {
                    for (i, v) in e.iter().enumerate() {
                        mean[i] += v;
                    }
                }
                mean.iter_mut().for_each(|v| *v /= n);
                (format!("{}\x00{}", c.domain, c.mode), mean)
            })
            .collect()
    }

    /// Raw best single-entry cosine and its owning cell — the ontology coverage
    /// of an embedding, undiluted by blended scoring. Low values mean "no
    /// existing entry is near this text".
    pub fn best_entry_sim(&self, embedding: &[f32]) -> Option<(f32, String, String)> {
        let mut best: Option<(f32, String, String)> = None;
        for cell in &self.cells {
            for e in &cell.embeddings {
                let s = cosine_sim(embedding, e);
                if best.as_ref().map(|(b, _, _)| s > *b).unwrap_or(true) {
                    best = Some((s, cell.domain.clone(), cell.mode.clone()));
                }
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn basis(dim: usize, axis: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; dim];
        v[axis] = 1.0;
        v
    }

    fn cell(domain: &str, mode: &str, embeddings: Vec<Vec<f32>>) -> Cell {
        let entries = (0..embeddings.len()).map(|i| format!("{domain}/{mode}#{i}")).collect();
        Cell { domain: domain.to_string(), mode: mode.to_string(), embeddings, entries }
    }

    #[test]
    fn classify_ranks_matching_cell_first_and_sorts_desc() {
        let clf = CellClassifier {
            cells: vec![
                cell("A", "X", vec![basis(4, 0)]),
                cell("A", "Y", vec![basis(4, 1)]),
                cell("B", "X", vec![basis(4, 2)]),
            ],
        };
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
        let clf = CellClassifier { cells: vec![dense, sparse] };
        let results = clf.classify(&basis(4, 0));
        assert_eq!(results[0].domain, "A");
    }

    #[test]
    fn best_entry_sim_returns_global_max() {
        let clf = CellClassifier {
            cells: vec![
                cell("A", "X", vec![basis(4, 1), basis(4, 0)]),
                cell("B", "Y", vec![basis(4, 2)]),
            ],
        };
        let (score, domain, mode) = clf.best_entry_sim(&basis(4, 0)).unwrap();
        assert!((score - 1.0).abs() < 1e-6);
        assert_eq!((domain.as_str(), mode.as_str()), ("A", "X"));
    }

    #[test]
    fn cell_centroids_mean_and_key_format() {
        let clf = CellClassifier {
            cells: vec![cell("A", "X", vec![vec![1.0, 0.0], vec![0.0, 1.0]])],
        };
        let centroids = clf.cell_centroids();
        let c = centroids.get("A\x00X").expect("key must be DOMAIN\\x00MODE");
        assert_eq!(c, &vec![0.5, 0.5]);
    }
}
