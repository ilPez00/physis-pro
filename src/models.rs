use serde::{Deserialize, Serialize};

pub type Score = f32;

/// State of a goal in vector space. No human-readable qualia.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub embedding: Vec<f32>,
    pub progress: Score,
    pub domain_name: String,
}

impl Goal {
    pub fn new(embedding: Vec<f32>, domain_name: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            embedding,
            progress: 0.0,
            domain_name,
        }
    }
    pub fn new_vec(embedding: Vec<f32>) -> Self {
        Self::new(embedding, "unknown".to_string())
    }
}

/// A dream is a generated vector from source vectors. No types, descriptions, or paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dream {
    pub id: String,
    pub source: Vec<f32>,
    pub embedding: Vec<f32>,
    pub grade: Option<Score>,
    pub dream_type: String,
    pub description: String,
}

impl Dream {
    pub fn new(source: Vec<f32>, embedding: Vec<f32>, dream_type: String, description: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source,
            embedding,
            grade: None,
            dream_type,
            description,
        }
    }
}

/// Coherence is purely geometric — a node is just a vector with a density score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceNode {
    pub id: String,
    pub embedding: Vec<f32>,
    pub coherence_score: Score,
}

impl CoherenceNode {
    pub fn new(embedding: Vec<f32>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            embedding,
            coherence_score: 0.0,
        }
    }
}

/// An experience is a vector delta — before → after. No action text or rationale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experience {
    pub id: String,
    pub goal_id: String,
    pub before: Vec<f32>,
    pub after: Vec<f32>,
    pub delta: Vec<f32>,
    pub grade: Score,
    pub action: String,
    pub rationale: String,
    pub scores: std::collections::HashMap<String, f32>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Experience {
    pub fn new(goal_id: &str, before: Vec<f32>, after: Vec<f32>, action: String, rationale: String, scores: std::collections::HashMap<String, f32>) -> Self {
        let delta: Vec<f32> = before.iter().zip(after.iter()).map(|(b, a)| a - b).collect();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            goal_id: goal_id.to_string(),
            before,
            after,
            delta,
            grade: 0.0,
            action,
            rationale,
            scores,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Filtered context is a vector. No cleaned text, no vector_context string.
#[derive(Debug, Clone, Serialize)]
pub struct FilteredContext {
    pub embedding: Vec<f32>,
    pub valid: bool,
    pub token_estimate: usize,
}

/// Constructive refutation — kept for the consistency checker, but now vector-based.
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

/// CertifiedBranch — no label, no domain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertifiedBranch {
    pub branch_id: String,
    pub node_ids: Vec<String>,
    pub centroid: Vec<f32>,
    pub stability_score: Score,
}

/// IsolatedBranch — no label, no contradiction text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsolatedBranch {
    pub branch_id: String,
    pub node_ids: Vec<String>,
    pub outlier_score: Score,
}

/// DreamResult — no scenario text, no collapse_chain text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamResult {
    pub dream_id: String,
    pub nodes_tested: Vec<String>,
    pub outcome: f32,
    pub prevented_failure: bool,
    pub coherence_delta: Score,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyConfig {
    pub kind: String,
    pub domains: Vec<OntologyEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub description: Option<String>,
    pub attributes: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub source: String,
    pub target: String,
    pub predicate: String,
    pub weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OntologicalMap {
    pub entities: std::collections::HashMap<String, Entity>,
    pub relationships: Vec<Relationship>,
}

impl OntologicalMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_entity(&mut self, entity: Entity) {
        self.entities.insert(entity.id.clone(), entity);
    }

    pub fn add_relationship(&mut self, rel: Relationship) {
        self.relationships.push(rel);
    }

    pub fn merge(&mut self, other: OntologicalMap) {
        for (id, entity) in other.entities {
            if let Some(existing) = self.entities.get_mut(&id) {
                for (k, v) in entity.attributes {
                    existing.attributes.insert(k, v);
                }
                if entity.description.is_some() {
                    existing.description = entity.description;
                }
            } else {
                self.entities.insert(id, entity);
            }
        }
        self.relationships.extend(other.relationships);
    }
}

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

// ── Semiotic Types ──────────────────────────────────────────────────

/// The 6 human domains of the semiotic square.
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

    #[allow(clippy::should_implement_trait)]
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

    /// Peircean Firstness/Quality for each domain
    pub fn icon_type(&self) -> &'static str {
        match self {
            HumanDomain::Heal => "wholeness",
            HumanDomain::Construct => "structure",
            HumanDomain::Fabricate => "craft",
            HumanDomain::Bond => "connection",
            HumanDomain::Study => "truth",
        }
    }
}

/// The 6 human modes of operation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum HumanMode {
    Lift,
    Rest,
    Walk,
    Work,
    Create,
    Learn,
}

impl HumanMode {
    pub fn all() -> [HumanMode; 6] {
        [HumanMode::Lift, HumanMode::Rest, HumanMode::Walk, HumanMode::Work, HumanMode::Create, HumanMode::Learn]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            HumanMode::Lift => "LIFT",
            HumanMode::Rest => "REST",
            HumanMode::Walk => "WALK",
            HumanMode::Work => "WORK",
            HumanMode::Create => "CREATE",
            HumanMode::Learn => "LEARN",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<HumanMode> {
        match s.to_uppercase().as_str() {
            "LIFT" => Some(HumanMode::Lift),
            "REST" => Some(HumanMode::Rest),
            "WALK" => Some(HumanMode::Walk),
            "WORK" => Some(HumanMode::Work),
            "CREATE" => Some(HumanMode::Create),
            "LEARN" => Some(HumanMode::Learn),
            _ => None,
        }
    }
}

/// A grid position mapping an ontology entry to one of the 36 cells.
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
        d * 6 + m
    }
}

/// Peircean sign classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeirceanSign {
    pub representamen: String,
    pub object: String,
    pub interpretant: String,
    pub trichotomy: String,
}

/// A cell in the 6×6 semiotic grid with all its metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemioticCell {
    pub domain: HumanDomain,
    pub mode: HumanMode,
    pub entries: Vec<String>,
    pub activation: f32,
    pub peircean: Option<PeirceanSign>,
    pub greimas: Option<String>,
}

/// The semiotic grid — a 5×6 matrix of domain×mode cells.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemioticGrid {
    pub cells: Vec<SemioticCell>,
}

impl SemioticGrid {
    pub fn new() -> Self {
        let mut cells = Vec::with_capacity(30);
        for d in HumanDomain::all() {
            for m in HumanMode::all() {
                cells.push(SemioticCell {
                    domain: d,
                    mode: m,
                    entries: Vec::new(),
                    activation: 0.0,
                    peircean: None,
                    greimas: None,
                });
            }
        }
        SemioticGrid { cells }
    }

    pub fn get_cell(&self, domain: HumanDomain, mode: HumanMode) -> Option<&SemioticCell> {
        let _idx = domain.as_str().len() * mode.as_str().len();
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

    /// Activate grid cells from a vector embedding using cosine similarity to domain centroids
    pub fn activate_from_embedding(&mut self, _embedding: &[f32]) {
        // Future: use stored centroid embeddings per cell for soft classification
    }

    pub fn compose(&self, d1: HumanDomain, _m1: HumanMode, d2: HumanDomain, m2: HumanMode) -> Option<GridPosition> {
        // Composition: mode of second follows domain of first applied to mode of second
        let domain = d1;
        let mode = m2;
        Some(GridPosition {
            domain,
            mode,
            axis_kind: "composite".into(),
            axis_name: format!("{}/{}", d1.as_str(), d2.as_str()),
        })
    }

    /// Dual (opposite) cell on the Greimas square
    pub fn dual(&self, domain: HumanDomain, mode: HumanMode) -> Option<(HumanDomain, HumanMode)> {
        let d = match domain {
            HumanDomain::Heal => HumanDomain::Fabricate,
            HumanDomain::Construct => HumanDomain::Study,
            HumanDomain::Fabricate => HumanDomain::Heal,
            HumanDomain::Bond => HumanDomain::Study,
            HumanDomain::Study => HumanDomain::Bond,
        };
        let m = match mode {
            HumanMode::Lift => HumanMode::Rest,
            HumanMode::Rest => HumanMode::Lift,
            HumanMode::Walk => HumanMode::Work,
            HumanMode::Work => HumanMode::Walk,
            HumanMode::Create => HumanMode::Learn,
            HumanMode::Learn => HumanMode::Create,
        };
        Some((d, m))
    }

    /// Activation heatmap as a 5×6 matrix
    pub fn heatmap_matrix(&self) -> Vec<Vec<f32>> {
        let mut matrix = vec![vec![0.0_f32; 6]; 5];
        for cell in &self.cells {
            let di = HumanDomain::all().iter().position(|d| *d == cell.domain).unwrap_or(0);
            let mi = HumanMode::all().iter().position(|m| *m == cell.mode).unwrap_or(0);
            matrix[di][mi] = cell.activation;
        }
        matrix
    }

    /// Clear all activations
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

// ── Helper: cosine similarity (used throughout) ───────────────────────

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
    fn test_goal_new_vec() {
        let g = Goal::new_vec(vec![0.1, 0.2, 0.3]);
        assert!(!g.id.is_empty());
        assert_eq!(g.embedding, vec![0.1, 0.2, 0.3]);
        assert_eq!(g.progress, 0.0);
    }

    #[test]
    fn test_dream_new() {
        let d = Dream::new(vec![1.0, 0.0], vec![0.5, 0.5], "test".to_string(), "test dream".to_string());
        assert!(!d.id.is_empty());
        assert_eq!(d.source, vec![1.0, 0.0]);
        assert_eq!(d.embedding, vec![0.5, 0.5]);
        assert!(d.grade.is_none());
    }

    #[test]
    fn test_coherence_node_new() {
        let c = CoherenceNode::new(vec![0.9, 0.1]);
        assert!(!c.id.is_empty());
        assert_eq!(c.embedding, vec![0.9, 0.1]);
        assert_eq!(c.coherence_score, 0.0);
    }

    #[test]
    fn test_experience_new_computes_delta() {
        let e = Experience::new("goal-1", vec![1.0, 0.0, 0.0], vec![1.0, 0.5, 0.3], "action".to_string(), "rationale".to_string(), std::collections::HashMap::new());
        assert_eq!(e.goal_id, "goal-1");
        assert_eq!(e.before, vec![1.0, 0.0, 0.0]);
        assert_eq!(e.after, vec![1.0, 0.5, 0.3]);
        assert_eq!(e.delta, vec![0.0, 0.5, 0.3]);
        assert_eq!(e.grade, 0.0);
    }

    #[test]
    fn test_constructive_refutation_new() {
        let cr = ConstructiveRefutation::new(
            vec![0.1, 0.2],
            vec!["n1".into(), "n2".into()],
            "merge them",
            0.85,
        );
        assert!(!cr.conflict_id.is_empty());
        assert_eq!(cr.conflicting_node_ids, vec!["n1", "n2"]);
        assert_eq!(cr.suggestion, "merge them");
        assert_eq!(cr.coherence_gap, 0.85);
    }

    #[test]
    fn test_ontological_map_add_and_merge() {
        let mut m = OntologicalMap::new();
        assert!(m.entities.is_empty());
        assert!(m.relationships.is_empty());

        let e = Entity {
            id: "e1".into(),
            name: "Test".into(),
            kind: "concept".into(),
            description: Some("desc".into()),
            attributes: [("color".into(), "red".into())].into(),
        };
        m.add_entity(e.clone());
        assert_eq!(m.entities.len(), 1);

        let r = Relationship {
            source: "e1".into(),
            target: "e2".into(),
            predicate: "connects_to".into(),
            weight: 1.0,
        };
        m.add_relationship(r);
        assert_eq!(m.relationships.len(), 1);

        let mut other = OntologicalMap::new();
        other.add_entity(Entity {
            id: "e1".into(),
            name: "Test".into(),
            kind: "concept".into(),
            description: Some("updated".into()),
            attributes: [("size".into(), "big".into())].into(),
        });
        other.add_entity(Entity {
            id: "e3".into(),
            name: "New".into(),
            kind: "thing".into(),
            description: None,
            attributes: Default::default(),
        });
        other.add_relationship(Relationship {
            source: "e3".into(),
            target: "e1".into(),
            predicate: "depends_on".into(),
            weight: 0.5,
        });

        m.merge(other);
        assert_eq!(m.entities.len(), 2);
        assert_eq!(m.relationships.len(), 2);
        assert_eq!(m.entities["e1"].description.as_deref(), Some("updated"));
        assert!(m.entities["e1"].attributes.contains_key("color"));
        assert!(m.entities["e1"].attributes.contains_key("size"));
    }

    #[test]
    fn test_cosine_sim_identical() {
        let v = vec![0.5, 0.5, 0.5, 0.5];
        let sim = cosine_sim(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_sim_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_sim(&a, &b);
        assert!((sim + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_sim_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_sim(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_sim_zero_vector() {
        let a = vec![0.0, 0.0];
        let b = vec![1.0, 0.0];
        let sim = cosine_sim(&a, &b);
        assert!(!sim.is_nan());
        assert!((-1.0..=1.0).contains(&sim));
    }

    #[test]
    fn test_cosine_dist() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let d = cosine_dist(&a, &b);
        assert!((d - 1.0).abs() < 1e-6);
        assert_eq!(cosine_dist(&a, &a), 0.0);
    }
}
