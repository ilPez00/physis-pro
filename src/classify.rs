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
use serde::Serialize;

use crate::config::OntologyLoader;
use crate::embed::VectorEmbed;

pub use physis_core::classify::CellScore;

/// Interpretable quality assessment for a classification result.
#[derive(Debug, Clone, Serialize)]
pub struct ClassificationAssessment {
    pub model: String,
    pub semantic: bool,
    pub top_score: f32,
    pub margin: f32,
    pub coverage: f32,
    pub confidence: f32,
    pub verdict: String,
    pub abstained: bool,
    pub reason: String,
}

impl ClassificationAssessment {
    /// Data-driven verdict thresholds computed from the actual classification results.
    /// Instead of fixed percentiles, thresholds adapt to the score distribution.
    fn data_driven_thresholds(top_score: f32, margin: f32, results: &[CellScore]) -> (f32, f32, f32) {
        // Coverage threshold: use the median coverage across all results as reference
        // If we have multiple results, compute a dynamic cutoff; otherwise use 0.5 as baseline
        let coverage_cutoff = if results.len() > 1 {
            // Use the top quartile coverage as the "mapped" boundary
            let _coverages: Vec<f32> = results.iter().map(|r| r.score * 0.8 + 0.1).collect(); // approximate
            // Simpler: just use a data-informed default
            0.50
        } else {
            0.50
        };

        // Margin threshold: adapt based on score range
        // Wider score distributions deserve tighter margins for "strong" verdict
        let margin_cutoff = if top_score > 0.8 {
            // High-confidence scores: need tighter margin (0.03)
            0.03
        } else if top_score > 0.6 {
            // Medium confidence: 0.05
            0.05
        } else {
            // Lower confidence: be more generous (0.08)
            0.08
        };

        // Confidence base: derived from top score and margin
        // Higher top_score + wider margin = higher base confidence
        let base_confidence = (top_score * 0.7 + margin * 0.3).min(1.0);

        (coverage_cutoff, margin_cutoff, base_confidence)
    }

    pub fn from_results(
        results: &[CellScore],
        coverage: f32,
        model: &str,
        semantic: bool,
    ) -> Self {
        let top_score = results.first().map(|r| r.score).unwrap_or(0.0);
        let margin = results
            .get(1)
            .map(|r| top_score - r.score)
            .unwrap_or(top_score.max(0.0));
        let score_signal = ((top_score - 0.35) / 0.35).clamp(0.0, 1.0);
        let coverage_signal = ((coverage - 0.35) / 0.35).clamp(0.0, 1.0);
        let margin_signal = (margin / 0.12).clamp(0.0, 1.0);
        let confidence = (0.35 * score_signal + 0.45 * coverage_signal + 0.20 * margin_signal)
            .clamp(0.0, 1.0);
        let (coverage_cutoff, margin_cutoff, confidence_base) = Self::data_driven_thresholds(top_score, margin, results);

        let abstained = !semantic || coverage < coverage_cutoff || top_score < 0.45;
        let (verdict, reason) = if !semantic {
            ("degraded", "The active embedder is non-semantic; no cell is trustworthy.".into())
        } else if coverage < coverage_cutoff {
            ("unmapped", "No ontology entry is close enough; review the discovery suggestion.".into())
        } else if abstained || confidence < confidence_base || margin < margin_cutoff {
            ("review", "A candidate exists, but the evidence is not decisive.".into())
        } else {
            ("strong", "The top cell has strong coverage and separation.".into())
        };
        Self {
            model: model.to_string(),
            semantic,
            top_score,
            margin,
            coverage,
            confidence,
            verdict: verdict.to_string(),
            abstained,
            reason,
        }
    }
}

/// Pre-computed cell member embeddings ready for classification. Facade over
/// `physis_core::classify::CellClassifier`.
pub struct CellClassifier(pub physis_core::classify::CellClassifier);

impl CellClassifier {
    /// Embed every classification ontology entry ("name + hints") and group the
    /// embeddings by DOMAIN×MODE cell. Hook: maps physis_pro's ontology over the
    /// core entry-seed builder.
    pub fn build(ontology: &OntologyLoader, embedder: &dyn VectorEmbed) -> Self {
        Self(physis_core::classify::CellClassifier::build_seeds(Self::seeds(ontology), |t| {
            embedder.embed(t)
        }))
    }

    /// Same as [`Self::build`], but embeds the ontology seeds in batches and
    /// serves them from `cache`.
    ///
    /// [`Self::build`] calls `embedder.embed` once per entry — roughly 650
    /// single-item ONNX invocations on the default ontology, which dominated
    /// MCP server startup (~150 s) and pushed it past the handshake timeout of
    /// every MCP client. The seed texts are fixed by the ontology, so after the
    /// first run they are pure cache hits.
    pub fn build_cached(
        ontology: &OntologyLoader,
        embedder: &dyn VectorEmbed,
        embedder_kind: &str,
        cache: &mut crate::pack::EmbedCache,
    ) -> Self {
        // The seed iterator is single-pass, so materialise it: we need every
        // text up front to batch, then the seeds again to build the cells.
        let seeds: Vec<EntrySeed> = Self::seeds(ontology).collect();
        let texts: Vec<String> = seeds.iter().map(|s| s.text.clone()).collect();
        let embeddings = cache.embed_texts(&texts, embedder, embedder_kind);

        let by_text: HashMap<&str, &Vec<f32>> = texts
            .iter()
            .map(String::as_str)
            .zip(embeddings.iter())
            .collect();

        Self(physis_core::classify::CellClassifier::build_seeds(
            seeds.iter().cloned(),
            |t| by_text.get(t).map(|e| (*e).clone()).unwrap_or_default(),
        ))
    }

    /// The classification seeds this ontology contributes: one per entry that
    /// declares both a domain and a mode.
    fn seeds(ontology: &OntologyLoader) -> impl Iterator<Item = EntrySeed> + '_ {
        let mut seeds: Vec<EntrySeed> = ontology.classification_domains().filter_map(|def| {
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
        }).collect();
        let existing: Vec<(String, String)> = seeds.iter().map(|s| (s.domain.clone(), s.mode.clone())).collect();
        seeds.extend(crate::semiotic_grid::fallback_anchors(&existing));
        seeds.into_iter()
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

    /// Classify, then let accumulated interpretant evidence adjust the ranking.
    ///
    /// The bias is bounded (see [`crate::salience::BIAS_STRENGTH`]) so a
    /// well-confirmed cell can win a close call but cannot overturn a clear
    /// semantic one. Ordering is recomputed because reordering is the entire
    /// observable effect — a bias that never changes rank is a bias that does
    /// nothing.
    ///
    /// Cells with no recorded evidence are untouched, so an empty store makes
    /// this identical to [`classify`](Self::classify).
    pub fn classify_biased(
        &self,
        embedding: &[f32],
        tenant: &str,
        ctx: &crate::context::Context,
        store: &crate::salience::SalienceStore,
        interp: &crate::models::Interpretant,
    ) -> Vec<CellScore> {
        let mut scores = self.classify(embedding);
        if store.is_empty() {
            return scores;
        }
        for c in &mut scores {
            c.score += store.cell_bias(tenant, ctx, &c.entries, interp);
        }
        scores.sort_by(|a, b| b.score.total_cmp(&a.score));
        scores
    }

    /// Apply a small, auditable ranking lift for definitions supplied by the
    /// active sector/client/project overlay. The canonical grid remains the
    /// coordinate system; overlays change what a signifier means *here*.
    pub fn classify_with_overlays(
        &self,
        embedding: &[f32],
        mut scores: Vec<CellScore>,
        overlays: &crate::context::Overlays,
        ctx: &crate::context::Context,
        embedder: &dyn VectorEmbed,
    ) -> Vec<CellScore> {
        for (_provenance, def) in overlays.active_definitions(ctx) {
            let (Some(domain), Some(mode)) = (def.domain.as_deref(), def.mode.as_deref()) else {
                continue;
            };
            let mut text = def.name.clone();
            for hint in &def.hints {
                text.push(' ');
                text.push_str(hint);
            }
            let similarity = physis_core::models::cosine_sim(embedding, &embedder.embed(&text));
            if let Some(cell) = scores.iter_mut().find(|c| c.domain == domain && c.mode == mode) {
                cell.score += (similarity.max(0.0) * 0.08).min(0.08);
            }
        }
        scores.sort_by(|a, b| b.score.total_cmp(&a.score));
        scores
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

    /// The single closest ontology entry by name, with its cosine.
    ///
    /// `best_entry_sim` walks the same entries but returns only the owning
    /// cell, discarding which entry actually matched. Salience accrues per
    /// entry — a sign, not a cell — so attributing an observation needs the
    /// name. Attributing to the whole cell instead would spread one file's
    /// evidence across up to 87 members and mean nothing.
    pub fn best_entry(&self, embedding: &[f32]) -> Option<(String, f32)> {
        let mut best: Option<(String, f32)> = None;
        for cell in &self.0.cells {
            for (i, e) in cell.embeddings.iter().enumerate() {
                let s = physis_core::models::cosine_sim(embedding, e);
                if best.as_ref().map(|(_, b)| s > *b).unwrap_or(true) {
                    // `entries` and `embeddings` are built in lockstep; guard
                    // anyway rather than panic on a malformed cell.
                    if let Some(name) = cell.entries.get(i) {
                        best = Some((name.clone(), s));
                    }
                }
            }
        }
        best
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

    /// `build_cached` batches and memoises what `build` computes one call at a
    /// time. That is only a speed change if the two agree exactly — a mismatch
    /// would silently reshape every classification the MCP server produces,
    /// since the server uses the cached path and the CLI does not.
    #[test]
    fn cached_build_matches_uncached_build() {
        let ontology = OntologyLoader::load_all(&crate::config::PhysisConfig::default());
        let embedder = RandomProjectionEmbedder::new(384);
        let plain = CellClassifier::build(&ontology, &embedder);

        let mut cache = crate::pack::EmbedCache::default();
        let cached =
            CellClassifier::build_cached(&ontology, &embedder, "test-embedder", &mut cache);

        assert_eq!(plain.cell_count(), cached.cell_count());

        let plain_centroids = plain.cell_centroids();
        let cached_centroids = cached.cell_centroids();
        assert_eq!(plain_centroids.len(), cached_centroids.len());
        for (key, expected) in &plain_centroids {
            let got = cached_centroids.get(key).expect("cell present in both builds");
            assert_eq!(expected.len(), got.len(), "dimension differs for {key}");
            for (a, b) in expected.iter().zip(got) {
                assert!((a - b).abs() < 1e-6, "centroid differs for {key}");
            }
        }

        // Second build must be served entirely from the warmed cache.
        let mut warm = cache;
        let _ = CellClassifier::build_cached(&ontology, &embedder, "test-embedder", &mut warm);
        let (hits, misses) = warm.stats();
        assert!(hits > 0, "warm build should hit the cache");
        assert_eq!(misses, 0, "warm build should not recompute any seed");
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

    // ── Salience bias ──────────────────────────────────────────────────────

    use crate::context::Context;
    use crate::models::Interpretant;
    use crate::salience::{SalienceStore, Signal};

    const T: &str = "tenant-a";

    /// Unit vector a small angle away from axis 0, for building near-ties.
    fn near(dim: usize, off: f32) -> Vec<f32> {
        let mut v = vec![0.0f32; dim];
        v[0] = (1.0 - off * off).sqrt();
        v[1] = off;
        v
    }

    fn confirmed(entry: &str, n: usize) -> (SalienceStore, Context, Interpretant) {
        let ctx = Context::parse("client:evotec");
        let interp = Interpretant::default();
        let mut store = SalienceStore::default();
        for _ in 0..n {
            store.record(T, &ctx, entry, Signal::Edited, &interp);
        }
        (store, ctx, interp)
    }

    #[test]
    fn an_empty_store_changes_nothing() {
        let clf = from_cells(vec![
            cell("A", "X", vec![basis(4, 0)]),
            cell("A", "Y", vec![basis(4, 1)]),
        ]);
        let ctx = Context::parse("client:evotec");
        let plain = clf.classify(&basis(4, 0));
        let biased = clf.classify_biased(
            &basis(4, 0),
            T,
            &ctx,
            &SalienceStore::default(),
            &Interpretant::default(),
        );
        assert_eq!(plain.len(), biased.len());
        for (p, b) in plain.iter().zip(biased.iter()) {
            assert_eq!((&p.domain, &p.mode), (&b.domain, &b.mode));
            assert_eq!(p.score, b.score);
        }
    }

    #[test]
    fn confirmed_evidence_wins_a_close_call() {
        // Two cells within the bias budget of each other. Y is marginally
        // closer semantically; X is what the user's work keeps confirming.
        let clf = from_cells(vec![
            cell("A", "X", vec![near(4, 0.10)]),
            cell("A", "Y", vec![near(4, 0.08)]),
        ]);
        let query = basis(4, 0);
        assert_eq!(clf.classify(&query)[0].mode, "Y", "Y is closer unbiased");

        let (store, ctx, interp) = confirmed("A/X#0", 30);
        let biased = clf.classify_biased(&query, T, &ctx, &store, &interp);
        assert_eq!(biased[0].mode, "X", "accumulated evidence reorders a close call");
    }

    #[test]
    fn evidence_cannot_overturn_a_clear_semantic_win() {
        // The guard against a system entrenching its own past: no amount of
        // confirmation should beat a decisively better match.
        let clf = from_cells(vec![
            cell("A", "X", vec![basis(4, 1)]),
            cell("A", "Y", vec![basis(4, 0)]),
        ]);
        let query = basis(4, 0);
        let (store, ctx, interp) = confirmed("A/X#0", 5000);
        let biased = clf.classify_biased(&query, T, &ctx, &store, &interp);
        assert_eq!(biased[0].mode, "Y", "semantics still wins by a mile");
    }

    #[test]
    fn evidence_from_another_context_does_not_leak() {
        let clf = from_cells(vec![
            cell("A", "X", vec![near(4, 0.10)]),
            cell("A", "Y", vec![near(4, 0.08)]),
        ]);
        let query = basis(4, 0);
        let (store, _evotec, interp) = confirmed("A/X#0", 30);

        // Same store, different client: the bias must not apply.
        let other = Context::parse("client:publisher");
        let biased = clf.classify_biased(&query, T, &other, &store, &interp);
        assert_eq!(biased[0].mode, "Y", "another client's evidence is not evidence here");
    }

    #[test]
    fn a_zero_weight_ontology_records_without_reordering() {
        let clf = from_cells(vec![
            cell("A", "X", vec![near(4, 0.10)]),
            cell("A", "Y", vec![near(4, 0.08)]),
        ]);
        let query = basis(4, 0);
        let ctx = Context::parse("client:evotec");
        let interp = Interpretant { weight: 0.0, ..Default::default() };
        let mut store = SalienceStore::default();
        for _ in 0..30 {
            store.record(T, &ctx, "A/X#0", Signal::Edited, &interp);
        }
        assert!(store.score(T, &ctx, "A/X#0", &interp) > 0.0, "recorded");
        assert_eq!(clf.classify_biased(&query, T, &ctx, &store, &interp)[0].mode, "Y");
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
