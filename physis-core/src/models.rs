//! Core data models: coherence nodes, ontology entries, the semiotic grid,
//! and vector helpers.

use serde::{Deserialize, Serialize};

pub type Score = f32;

/// Did this thing actually work? The **asserted** axis — a caller's verdict,
/// not something the engine can compute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoherenceRating {
    /// Worked, with a detected effect. `+1.0`
    Success,
    /// Ran, but produced no expected effect. `0.0`
    Inert,
    /// Refuted, errored, or a self-imposed rule broken. `-1.0`
    Failure,
}

impl CoherenceRating {
    pub fn weight(self) -> Score {
        match self {
            CoherenceRating::Success => 1.0,
            CoherenceRating::Inert => 0.0,
            CoherenceRating::Failure => -1.0,
        }
    }

    /// Bucket a continuous assertion.
    pub fn from_score(score: Score) -> Self {
        if score > 0.2 {
            CoherenceRating::Success
        } else if score < -0.2 {
            CoherenceRating::Failure
        } else {
            CoherenceRating::Inert
        }
    }
}

/// A node carries two coherence axes:
///
/// - `coherence_score` is *derived* — mean cosine to the k nearest neighbours,
///   clamped at `>= 0`. It answers "how typical is this?".
/// - `asserted` is *reported* — a caller's verdict in `[-1, 1]`, `None` until
///   someone says. It answers "did it work?".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceNode {
    pub id: String,
    pub embedding: Vec<f32>,
    /// Derived density. Never negative.
    pub coherence_score: Score,
    /// Asserted outcome in `[-1, 1]`; `None` = nobody has judged this yet.
    #[serde(default)]
    pub asserted: Option<Score>,
    #[serde(default)]
    pub label: Option<String>,
    /// Which embedder produced `embedding` (provenance).
    #[serde(default)]
    pub embedder: Option<String>,
}

impl CoherenceNode {
    pub fn new(embedding: Vec<f32>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            embedding,
            coherence_score: 0.0,
            asserted: None,
            label: None,
            embedder: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn rating(&self) -> Option<CoherenceRating> {
        self.asserted.map(CoherenceRating::from_score)
    }

    pub fn is_asserted_failure(&self) -> bool {
        matches!(self.rating(), Some(CoherenceRating::Failure))
    }
}

/// Result of a consistency check.
#[derive(Debug, Clone)]
pub enum ConsistencyResult {
    Clean,
    Conflict(ConstructiveRefutation),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstructiveRefutation {
    pub conflict_id: String,
    pub query_embedding: Vec<f32>,
    pub conflicting_node_ids: Vec<String>,
    pub suggestion: String,
    pub coherence_gap: Score,
}

impl ConstructiveRefutation {
    pub fn new(query_embedding: Vec<f32>, conflicting_ids: Vec<String>, suggestion: &str, gap: Score) -> Self {
        Self {
            conflict_id: uuid::Uuid::new_v4().to_string(),
            query_embedding,
            conflicting_node_ids: conflicting_ids,
            suggestion: suggestion.to_string(),
            coherence_gap: gap,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertifiedBranch {
    pub branch_id: String,
    pub node_ids: Vec<String>,
    pub centroid: Vec<f32>,
    pub stability_score: Score,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsolatedBranch {
    pub branch_id: String,
    pub node_ids: Vec<String>,
    pub outlier_score: Score,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamResult {
    pub dream_id: String,
    pub nodes_tested: Vec<String>,
    pub outcome: f32,
    pub prevented_failure: bool,
    pub coherence_delta: Score,
}

/// Snapshot of coherence health across the node graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceSnapshot {
    pub total_nodes: usize,
    pub high_coherence: usize,
    pub mid_coherence: usize,
    pub low_coherence: usize,
    pub certified_branches_count: usize,
    pub isolated_branches_count: usize,
    pub dream_cycle_count: usize,
    pub coherence_index: Score,
    pub cluster_count: usize,
    pub outlier_count: usize,
    pub asserted_success: usize,
    pub asserted_inert: usize,
    pub asserted_failure: usize,
    pub asserted_index: Option<Score>,
}

/// One row of an ontology file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyEntry {
    pub name: String,
    pub category: Option<String>,
    pub domain: String,
    pub mode: String,
    pub axis_kind: String,
    pub axis_name: String,
    pub unit: String,
    pub hints: Vec<String>,
}

/// An ontology file: kind + list of entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyConfig {
    pub kind: String,
    pub domains: Vec<OntologyEntry>,
}

/// The runtime form of an ontology entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainDef {
    pub name: String,
    pub category: Option<String>,
    pub domain: Option<String>,
    pub mode: Option<String>,
    pub axis_kind: Option<String>,
    pub axis_name: Option<String>,
    pub unit: String,
    pub hints: Vec<String>,
}

// ── Semiotic Grid ─────────────────────────────────────────────────────

/// The 5 human domains of the semiotic square.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum HumanDomain {
    Heal,
    Construct,
    Fabricate,
    Bond,
    Study,
}

impl HumanDomain {
    pub fn all() -> [HumanDomain; 5] {
        [HumanDomain::Heal, HumanDomain::Construct, HumanDomain::Fabricate, HumanDomain::Bond, HumanDomain::Study]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            HumanDomain::Heal => "HEAL",
            HumanDomain::Construct => "CONSTRUCT",
            HumanDomain::Fabricate => "FABRICATE",
            HumanDomain::Bond => "BOND",
            HumanDomain::Study => "STUDY",
        }
    }

    pub fn from_str(s: &str) -> Option<HumanDomain> {
        match s.to_uppercase().as_str() {
            "HEAL" => Some(HumanDomain::Heal),
            "CONSTRUCT" => Some(HumanDomain::Construct),
            "FABRICATE" => Some(HumanDomain::Fabricate),
            "BOND" => Some(HumanDomain::Bond),
            "STUDY" => Some(HumanDomain::Study),
            _ => None,
        }
    }
}

/// The 14 human modes of operation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum HumanMode {
    Lift,
    Rest,
    Walk,
    Work,
    Create,
    Learn,
    Destroy,
    Sense,
    Guide,
    Play,
    Brainstorm,
    Maintain,
    Move,
    Plan,
}

impl HumanMode {
    pub fn all() -> [HumanMode; 14] {
        [
            HumanMode::Lift, HumanMode::Rest, HumanMode::Walk, HumanMode::Work,
            HumanMode::Create, HumanMode::Learn, HumanMode::Destroy, HumanMode::Sense,
            HumanMode::Guide, HumanMode::Play, HumanMode::Brainstorm, HumanMode::Maintain,
            HumanMode::Move, HumanMode::Plan,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            HumanMode::Lift => "LIFT",
            HumanMode::Rest => "REST",
            HumanMode::Walk => "WALK",
            HumanMode::Work => "WORK",
            HumanMode::Create => "CREATE",
            HumanMode::Learn => "LEARN",
            HumanMode::Destroy => "DESTROY",
            HumanMode::Sense => "SENSE",
            HumanMode::Guide => "GUIDE",
            HumanMode::Play => "PLAY",
            HumanMode::Brainstorm => "BRAINSTORM",
            HumanMode::Maintain => "MAINTAIN",
            HumanMode::Move => "MOVE",
            HumanMode::Plan => "PLAN",
        }
    }

    pub fn from_str(s: &str) -> Option<HumanMode> {
        match s.to_uppercase().as_str() {
            "LIFT" => Some(HumanMode::Lift),
            "REST" => Some(HumanMode::Rest),
            "WALK" => Some(HumanMode::Walk),
            "WORK" => Some(HumanMode::Work),
            "CREATE" => Some(HumanMode::Create),
            "LEARN" => Some(HumanMode::Learn),
            "DESTROY" => Some(HumanMode::Destroy),
            "SENSE" => Some(HumanMode::Sense),
            "GUIDE" => Some(HumanMode::Guide),
            "PLAY" => Some(HumanMode::Play),
            "BRAINSTORM" => Some(HumanMode::Brainstorm),
            "MAINTAIN" => Some(HumanMode::Maintain),
            "MOVE" => Some(HumanMode::Move),
            "PLAN" => Some(HumanMode::Plan),
            _ => None,
        }
    }
}

/// A grid position mapping an ontology entry to one of the 70 cells.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridPosition {
    pub domain: HumanDomain,
    pub mode: HumanMode,
    pub axis_kind: String,
    pub axis_name: String,
}

impl GridPosition {
    pub fn from_ontology_entry(e: &OntologyEntry) -> Option<GridPosition> {
        let domain = HumanDomain::from_str(&e.domain)?;
        let mode = HumanMode::from_str(&e.mode)?;
        Some(GridPosition {
            domain,
            mode,
            axis_kind: e.axis_kind.clone(),
            axis_name: e.axis_name.clone(),
        })
    }

    pub fn cell_index(&self) -> usize {
        let d = HumanDomain::all().iter().position(|x| *x == self.domain).unwrap_or(0);
        let m = HumanMode::all().iter().position(|x| *x == self.mode).unwrap_or(0);
        d * HumanMode::all().len() + m
    }
}

/// A cell in the 5×14 semiotic grid with all its metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemioticCell {
    pub domain: HumanDomain,
    pub mode: HumanMode,
    pub entries: Vec<String>,
    pub activation: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemioticGrid {
    pub cells: Vec<SemioticCell>,
}

impl SemioticGrid {
    pub fn new() -> Self {
        let mut cells = Vec::with_capacity(HumanDomain::all().len() * HumanMode::all().len());
        for d in HumanDomain::all() {
            for m in HumanMode::all() {
                cells.push(SemioticCell {
                    domain: d,
                    mode: m,
                    entries: Vec::new(),
                    activation: 0.0,
                });
            }
        }
        SemioticGrid { cells }
    }

    pub fn get_cell(&self, domain: HumanDomain, mode: HumanMode) -> Option<&SemioticCell> {
        self.cells.iter().find(|c| c.domain == domain && c.mode == mode)
    }

    pub fn get_cell_mut(&mut self, domain: HumanDomain, mode: HumanMode) -> Option<&mut SemioticCell> {
        self.cells.iter_mut().find(|c| c.domain == domain && c.mode == mode)
    }

    pub fn classify(&mut self, entry_name: &str, domain: HumanDomain, mode: HumanMode) {
        if let Some(cell) = self.get_cell_mut(domain, mode) {
            cell.entries.push(entry_name.to_string());
            cell.activation += 0.1;
        }
    }

    /// Heatmap as a 5×14 matrix.
    pub fn heatmap_matrix(&self) -> Vec<Vec<f32>> {
        let mut matrix = vec![vec![0.0_f32; HumanMode::all().len()]; HumanDomain::all().len()];
        for cell in &self.cells {
            let di = HumanDomain::all().iter().position(|d| *d == cell.domain).unwrap_or(0);
            let mi = HumanMode::all().iter().position(|m| *m == cell.mode).unwrap_or(0);
            matrix[di][mi] = cell.activation;
        }
        matrix
    }

    pub fn reset_activations(&mut self) {
        for cell in &mut self.cells {
            cell.activation = 0.0;
        }
    }
}

impl Default for SemioticGrid {
    fn default() -> Self {
        Self::new()
    }
}

// ── Vector helpers ────────────────────────────────────────────────────

pub fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
    (dot / (na * nb)).clamp(-1.0, 1.0)
}

pub fn cosine_dist(a: &[f32], b: &[f32]) -> f32 {
    1.0 - cosine_sim(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coherence_rating_buckets() {
        assert_eq!(CoherenceRating::from_score(1.0), CoherenceRating::Success);
        assert_eq!(CoherenceRating::from_score(0.0), CoherenceRating::Inert);
        assert_eq!(CoherenceRating::from_score(-1.0), CoherenceRating::Failure);
        assert_eq!(CoherenceRating::from_score(0.1), CoherenceRating::Inert);
    }

    #[test]
    fn test_node_assertion_and_failure() {
        let mut n = CoherenceNode::new(vec![0.1, 0.2]);
        assert_eq!(n.rating(), None);
        n.asserted = Some(-1.0);
        assert!(n.is_asserted_failure());
    }

    #[test]
    fn test_grid_places_entry() {
        let mut grid = SemioticGrid::new();
        grid.classify("running", HumanDomain::Heal, HumanMode::Work);
        let cell = grid.get_cell(HumanDomain::Heal, HumanMode::Work).unwrap();
        assert_eq!(cell.entries, vec!["running"]);
        assert!((cell.activation - 0.1).abs() < 1e-6);
        assert_eq!(grid.cells.len(), 70);
    }

    #[test]
    fn test_grid_position_from_entry() {
        let e = OntologyEntry {
            name: "x".into(),
            category: None,
            domain: "HEAL".into(),
            mode: "WORK".into(),
            axis_kind: "k".into(),
            axis_name: "n".into(),
            unit: "u".into(),
            hints: vec![],
        };
        let gp = GridPosition::from_ontology_entry(&e).expect("parses");
        assert_eq!(gp.domain, HumanDomain::Heal);
        assert_eq!(gp.mode, HumanMode::Work);
        let bad = OntologyEntry { domain: "NOPE".into(), ..e };
        assert!(GridPosition::from_ontology_entry(&bad).is_none());
    }

    #[test]
    fn test_cosine_helpers() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!((cosine_sim(&a, &a) - 1.0).abs() < 1e-6);
        assert!(cosine_sim(&a, &b).abs() < 1e-6);
        assert!((cosine_dist(&a, &b) - 1.0).abs() < 1e-6);
    }
}
