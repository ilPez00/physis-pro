//! Semiotic-grid classification: embed text, score against ontology entries,
//! rank DOMAIN×MODE cells.
//!
//! Scoring is **nearest-entry (max cosine)**: each cell's score is the cosine of
//! the query against its best-matching member entry, not a mean centroid. A small
//! top-k average smooths single-entry noise, and a domain prior disambiguates
//! same-domain modes.

use std::collections::HashMap;

use serde::{Serialize, Deserialize};

use crate::embed::VectorEmbed;
use crate::models::cosine_sim;
use crate::models::Facets;
use crate::ontology::OntologyLoader;

/// How many top member-entry similarities to average per cell.
/// Adaptive: small cells use TOP_K=1 (specificity), medium use 2, large use 3.
const TOP_K: usize = 2; // base value; adaptive logic applies at call site

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellScore {
    pub domain: String,
    pub mode: String,
    pub score: f32,
    pub entries: Vec<String>,
    /// Dominant facet profile across the cell's member entries.
    pub facets: Facets,
}

/// One populated cell with its member entry embeddings.
#[derive(Clone)]
pub struct Cell {
    pub domain: String,
    pub mode: String,
    /// Per-entry embeddings (one row per member entry).
    pub embeddings: Vec<Vec<f32>>,
    /// Member entry display names (parallel to `embeddings`).
    pub entries: Vec<String>,
    /// Per-entry facets (parallel to `embeddings`).
    pub facets: Vec<Facets>,
}

impl Cell {
    pub fn new(
        domain: String,
        mode: String,
        embeddings: Vec<Vec<f32>>,
        entries: Vec<String>,
        facets: Vec<Facets>,
    ) -> Self {
        Self { domain, mode, embeddings, entries, facets }
    }
}

/// Pre-computed cell member embeddings ready for classification.
pub struct CellClassifier {
    pub cells: Vec<Cell>,
}

/// A raw ontology entry feed for the classifier, decoupled from any specific
/// loader type so physis-pro can hand its own `DomainDef`s straight in.
#[derive(Debug, Clone, Default)]
pub struct EntrySeed {
    pub name: String,
    pub domain: String,
    pub mode: String,
    /// Space-joined text the entry is embedded from (name + hints).
    pub text: String,
    /// Orthogonal facets the entry carries (default: none).
    pub facets: Facets,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct ClassificationMetadata {
    /// Speaker who said/wrote the text
    pub speaker: Option<String>,
    /// Context or situation
    pub context: Option<String>,
    /// Source document or folder
    pub source_document: Option<String>,
    /// Custom tags for categorization
    pub tags: Vec<String>,
    /// Any additional key-value metadata
    pub extra: std::collections::HashMap<String, String>,
}

/// A sentence classified into a semiotic cell (domain x mode) with metadata
#[derive(Clone, Debug, Serialize)]
pub struct ClassifiedSentence {
    /// The original text
    pub text: String,
    /// The (domain, mode) cell it maps to
    pub cell: (String, String), // (domain, mode) pair
    /// Metadata enabling grid slicing and filtering
    pub metadata: ClassificationMetadata,
    /// Confidence score of the classification
    pub confidence: f32,
    /// When this was classified (ISO timestamp)
    pub classified_at: Option<String>,
}

impl Default for ClassifiedSentence {
    fn default() -> Self {
        Self {
            text: String::new(),
            cell: (String::new(), String::new()),
            metadata: ClassificationMetadata::default(),
            confidence: 0.0,
            classified_at: None,
        }
    }
}

/// Metadata filter for classifying with speaker/context awareness
#[derive(Debug, Clone, Default)]
pub struct MetadataFilter {
    /// Only classify texts from these speakers
    pub speakers: Vec<String>,
    /// Only classify texts with these contexts
    pub contexts: Vec<String>,
    /// Only classify texts with these tags
    pub tags: Vec<String>,
}

impl MetadataFilter {
    /// Check if this filter allows a given metadata
    pub fn allows(&self, metadata: &ClassificationMetadata) -> bool {
        // Speaker filter
        if let Some(ref want) = self.speakers.first() {
            if let Some(ref have) = metadata.speaker.as_ref() {
                if want != have {
                    return false;
                }
            } else {
                return false; // wants speaker but text has none
            }
        }
        // Context filter
        if let Some(ref want) = self.contexts.first() {
            if let Some(ref have) = metadata.context.as_ref() {
                if want != have {
                    return false;
                }
            } else {
                return false; // wants context but text has none
            }
        }
        // Tags filter (all specified tags must be present)
        for tag in &self.tags {
            if !metadata.tags.iter().any(|t| t == tag) {
                return false;
            }
        }
        true
    }
}



// A caller-supplied facet constraint. Cells whose member facets don't match the
/// specified dimensions are down-weighted so facet-known queries rank better.
#[derive(Debug, Clone, Default)]
pub struct FacetFilter {
    pub lifecycle: Option<crate::models::LifecyclePhase>,
    pub agency: Option<crate::models::Agency>,
    pub scale: Option<crate::models::Scale>,
    pub abstraction: Option<crate::models::Abstraction>,
    pub sub_domain: Option<String>,
    pub sub_mode: Option<String>,
}

impl FacetFilter {
    /// How many dimensions the caller constrained (0 ⇒ no filtering).
    fn weight(&self) -> usize {
        let mut n = 0;
        if self.lifecycle.is_some() { n += 1; }
        if self.agency.is_some() { n += 1; }
        if self.scale.is_some() { n += 1; }
        if self.abstraction.is_some() { n += 1; }
        if self.sub_domain.is_some() { n += 1; }
        if self.sub_mode.is_some() { n += 1; }
        n
    }

    /// Match ratio in `[0, 1]` of this filter against a cell's aggregated facets.
    fn match_ratio(&self, cell: &Facets) -> f32 {
        let total = self.weight();
        if total == 0 {
            return 1.0;
        }
        let mut hits = 0;
        if let (Some(want), Some(have)) = (self.lifecycle, cell.lifecycle) {
            if want == have { hits += 1; }
        }
        if let (Some(want), Some(have)) = (self.agency, cell.agency) {
            if want == have { hits += 1; }
        }
        if let (Some(want), Some(have)) = (self.scale, cell.scale) {
            if want == have { hits += 1; }
        }
        if let (Some(want), Some(have)) = (self.abstraction, cell.abstraction) {
            if want == have { hits += 1; }
        }
        if let (Some(want), Some(have)) = (&self.sub_domain, &cell.sub_domain) {
            if want.eq_ignore_ascii_case(have) { hits += 1; }
        }
        if let (Some(want), Some(have)) = (&self.sub_mode, &cell.sub_mode) {
            if want.eq_ignore_ascii_case(have) { hits += 1; }
        }
        hits as f32 / total as f32
    }
}

impl CellClassifier {
    /// Embed every entry seed (its `text`), group embeddings by DOMAIN×MODE cell.
    /// Takes an embed closure instead of the `VectorEmbed` trait so external
    /// crates with their own embedder traits (physis-pro) can hook in directly.
    pub fn build_seeds<F>(seeds: impl IntoIterator<Item = EntrySeed>, embed: F) -> Self
    where
        F: Fn(&str) -> Vec<f32>,
    {
        type Acc = (Vec<Vec<f32>>, Vec<String>, Vec<Facets>);
        let mut acc: HashMap<(String, String), Acc> = HashMap::new();
        for seed in seeds {
            let emb = embed(&seed.text);
            let entry = acc
                .entry((seed.domain.clone(), seed.mode.clone()))
                .or_insert_with(|| (Vec::new(), Vec::new(), Vec::new()));
            entry.0.push(emb);
            entry.1.push(seed.name);
            entry.2.push(seed.facets);
        }
        let cells = acc
            .into_iter()
            .map(|((domain, mode), (embeddings, entries, facets))| Cell { domain, mode, embeddings, entries, facets })
            .collect();
        Self { cells }
    }

    /// Rebuild from pre-built cells (used by tests / adapters that already
    /// constructed the internal shape).
    pub fn from_cells(cells: Vec<Cell>) -> Self {
        Self { cells }
    }

    /// Embed every classification ontology entry ("name + hints") and group the
    /// embeddings by DOMAIN×MODE cell, carrying each entry's facets.
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
        Self::build_seeds(seeds, |t| embedder.embed(t))
    }

    /// Number of populated cells.
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Adaptive TOP_K based on cell entry count for optimal precision.
/// Uses `TOP_K` as the medium/default value (4-10 entries → TOP_K=2).
/// - 1-3 entries: TOP_K=1 (maximize specificity, no averaging noise)
/// - 4-10 entries: TOP_K=2 (current balanced behavior, via TOP_K constant)
/// - 11-30 entries: TOP_K=3 (smooth noise while keeping top entries)
/// - 31+ entries: TOP_K=4 (significant smoothing)
///
    /// Used by [`CellClassifier::classify`](Self::classify) to select the number of
    /// top similarities to average per cell.
    pub fn adaptive_top_k(num_entries: usize) -> usize {
        let base = TOP_K; // = 2, the medium default
        match num_entries {
            1..=3 => 1,
            4..=10 => base, // = 2
            11..=30 => base + 1, // = 3
            _ => base + 2, // = 4
        }
    }

/// Classify a pre-computed embedding with soft nested (domain→mode) scoring.
/// Uses adaptive TOP_K per cell size for improved precision:
/// Small cells preserve specificity, large cells get noise smoothing.
pub fn classify(&self, embedding: &[f32]) -> Vec<CellScore> {
    let mut domain_cell_scores: HashMap<&str, Vec<f32>> = HashMap::new();
    let mut cell_scores: Vec<f32> = Vec::with_capacity(self.cells.len());
    for cell in &self.cells {
        let k = Self::adaptive_top_k(cell.embeddings.len());
        let mut sims: Vec<f32> = cell.embeddings.iter().map(|e| cosine_sim(embedding, e)).collect();
        sims.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let cs = topk_mean(&sims, k);
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
            // Population weight: larger cells have more reliable top entries,
            // so their scores get a modest boost; small cells get a small boost
            // to prevent them from being drowned out by big cells.
            let pop_weight = if cell.embeddings.len() >= 10 {
                1.0 + (cell.embeddings.len() as f32 / 100.0).min(0.2)
            } else if cell.embeddings.len() <= 3 {
                1.0 + (3.0 - cell.embeddings.len() as f32) / 10.0
            } else {
                1.0
            };
            let score = (ALPHA * ds + (1.0 - ALPHA) * cell_score) * pop_weight;
            CellScore {
                domain: cell.domain.clone(),
                mode: cell.mode.clone(),
                score,
                entries: cell.entries.clone(),
                facets: Facets::aggregate(&cell.facets),
            }
        })
        .collect();
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results
}

/// Classify with metadata tracking, returning a `ClassifiedSentence` for the top result.
/// Uses the top-scoring cell and incorporates metadata for grid slicing.
/// The `filter` can restrict which texts/metadata are considered; pass `Default::default()`
/// for no filtering.
pub fn classify_with_metadata(
    &self,
    embedding: &[f32],
    metadata: &ClassificationMetadata,
    filter: &MetadataFilter,
) -> Option<ClassifiedSentence> {
    let results = self.classify(embedding);
    if results.is_empty() {
        return None;
    }
    let top = &results[0];

    // Apply metadata filter - if filter is set, ensure the top result's cell matches
    // In the future, this could weight cells by metadata relevance
    if !filter.allows(metadata) {
        // Filter doesn't match; return None
        return None;
    }

    Some(ClassifiedSentence {
        text: "classified text".to_string(), // TODO: pass text in future
        cell: (top.domain.clone(), top.mode.clone()),
        metadata: metadata.clone(),
        confidence: top.score,
        classified_at: Some(
            chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3f").to_string(),
        ),
    })
}



    /// Classify with an optional facet constraint. Cells whose aggregated facets
    /// match the filter are up-weighted; the more dimensions match, the higher
    /// the factor (1.0 when the filter is empty, so this degrades to `classify`).
    pub fn classify_filtered(&self, embedding: &[f32], filter: &FacetFilter) -> Vec<CellScore> {
        let mut results = self.classify(embedding);
        for r in &mut results {
            if let Some(cell) = self.cells.iter().find(|c| c.domain == r.domain && c.mode == r.mode) {
                let ratio = filter.match_ratio(&Facets::aggregate(&cell.facets));
                r.score *= 0.5 + 0.5 * ratio;
            }
        }
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
    use crate::models::{Agency, LifecyclePhase};

    fn basis(dim: usize, axis: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; dim];
        v[axis] = 1.0;
        v
    }

    fn cell(domain: &str, mode: &str, embeddings: Vec<Vec<f32>>) -> Cell {
        let entries = (0..embeddings.len()).map(|i| format!("{domain}/{mode}#{i}")).collect();
        let facets = vec![Facets::default(); embeddings.len()];
        Cell::new(domain.to_string(), mode.to_string(), embeddings, entries, facets)
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

    #[test]
    fn classify_carries_aggregated_facets() {
        let mut c = cell("A", "X", vec![basis(4, 0), basis(4, 0), basis(4, 0)]);
        c.facets = vec![
            Facets { lifecycle: Some(LifecyclePhase::Operate), agency: Some(Agency::SelfActor), ..Default::default() },
            Facets { lifecycle: Some(LifecyclePhase::Operate), agency: Some(Agency::SelfActor), ..Default::default() },
            Facets { lifecycle: Some(LifecyclePhase::Design), agency: Some(Agency::Automated), ..Default::default() },
        ];
        let clf = CellClassifier { cells: vec![c] };
        let r = &clf.classify(&basis(4, 0))[0];
        assert_eq!(r.domain, "A");
        assert_eq!(r.facets.lifecycle, Some(LifecyclePhase::Operate));
        // 2/3 SelfActor, 2/3 Operate → clear majorities.
        assert_eq!(r.facets.agency, Some(Agency::SelfActor));
    }

    #[test]
    fn classify_filtered_upweights_matching_facets() {
        let mut c = cell("A", "X", vec![basis(4, 0)]);
        c.facets = vec![Facets { lifecycle: Some(LifecyclePhase::Operate), ..Default::default() }];
        let other = cell("B", "Y", vec![basis(4, 0)]);
        let clf = CellClassifier { cells: vec![c, other.clone()] };

        // Without a filter both cells tie on embedding; A should still surface via facets.
        let filter = FacetFilter { lifecycle: Some(LifecyclePhase::Operate), ..Default::default() };
        let ranked = clf.classify_filtered(&basis(4, 0), &filter);
        assert_eq!((ranked[0].domain.as_str(), ranked[0].mode.as_str()), ("A", "X"));
    }

    #[test]
    fn classify_filtered_empty_is_identity() {
        let clf = CellClassifier { cells: vec![cell("A", "X", vec![basis(4, 0)])] };
        let a = clf.classify(&basis(4, 0));
        let b = clf.classify_filtered(&basis(4, 0), &FacetFilter::default());
        assert!((a[0].score - b[0].score).abs() < 1e-6);
    }
}
