use serde::{Deserialize, Serialize};

pub type Score = f32;

/// Orthogonal action-space facets (lifecycle / agency / scale / abstraction /
/// tree-nesting). Defined in `physis-core`; re-exported so the monolith and the
/// engine share one Facets type across the classification bridge.
pub use physis_core::models::{Abstraction, Agency, Facets, LifecyclePhase, Scale};

/// State of a goal in vector space.
///
/// The vector is the state; `label` optionally retains the text it was embedded
/// from. This mirrors `CoherenceNode`: the "no human-readable qualia" rule was
/// already dropped there because it left nodes unbrowsable, and it cost goals
/// the same way — every dream proposal read "Mutate unknown", because
/// `new_vec` stamps `domain_name: "unknown"` and the description interpolates
/// it. Keeping the source text costs one string and makes the engine able to
/// say what it is reasoning about.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub embedding: Vec<f32>,
    pub progress: Score,
    pub domain_name: String,
    /// Source text this goal was embedded from. Absent on goals written before
    /// the field existed, hence `serde(default)`.
    #[serde(default)]
    pub label: Option<String>,
}

impl Goal {
    pub fn new(embedding: Vec<f32>, domain_name: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            embedding,
            progress: 0.0,
            domain_name,
            label: None,
        }
    }
    pub fn new_vec(embedding: Vec<f32>) -> Self {
        Self::new(embedding, "unknown".to_string())
    }

    /// Attach the text the vector came from.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        let label = label.into();
        if !label.trim().is_empty() {
            self.label = Some(label);
        }
        self
    }

    /// What to call this goal in human-facing output: the source text when we
    /// have it, else the domain, else the placeholder.
    pub fn name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.domain_name)
    }
}

#[cfg(test)]
mod goal_tests {
    use super::*;

    #[test]
    fn a_goal_can_say_what_it_is_about() {
        // Every dream proposal interpolates this name. Without a label it read
        // "Mutate unknown", which is the engine failing to describe its own
        // reasoning rather than a limit of the reasoning.
        let bare = Goal::new_vec(vec![0.1; 8]);
        assert_eq!(bare.name(), "unknown");
        assert_eq!(bare.label, None);

        let named = Goal::new_vec(vec![0.1; 8]).with_label("reduce nozzle clogs");
        assert_eq!(named.name(), "reduce nozzle clogs");

        // A blank prompt is not a name — it would render as an empty gap in
        // every proposal, which reads worse than the domain placeholder.
        let blank = Goal::new_vec(vec![0.1; 8]).with_label("   ");
        assert_eq!(blank.name(), "unknown");
    }

    #[test]
    fn goals_written_before_the_field_still_load() {
        // Persisted goals predate `label`; they must deserialize, not fail the
        // whole tenant's rehydration.
        let legacy = r#"{"id":"g1","embedding":[0.1,0.2],"progress":0.5,"domain_name":"Body"}"#;
        let g: Goal = serde_json::from_str(legacy).expect("legacy goal loads");
        assert_eq!(g.label, None);
        assert_eq!(g.name(), "Body");
        assert_eq!(g.progress, 0.5);
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

/// Did this thing actually work? The **asserted** axis — a caller's verdict,
/// not something the engine can compute.
///
/// The same three states apply to a machine function and a human behaviour,
/// which is the symmetry the whole model rests on: a compile that produces no
/// effect and a study session with no retention are both `Inert`.
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

    /// Bucket a continuous assertion. Callers report confidence (praxis sends a
    /// reliability score, aion a loop outcome), so the stored value stays a
    /// float and this is only the reading of it.
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

/// A node carries **two** coherence axes, and conflating them is what hollowed
/// the model out once already:
///
/// - `coherence_score` is *derived* — mean cosine to the k nearest neighbours,
///   clamped at `>= 0`. It answers "how typical is this?" and every vector store
///   can compute it. It cannot express failure.
/// - `asserted` is *reported* — a caller's verdict in `[-1, 1]`, `None` until
///   someone says. It answers "did it work?", which is the only question the
///   dream loop can actually act on.
///
/// `label` optionally retains the human source text the vector came from, so the
/// node can be browsed by content (otherwise the id is the only readable handle).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceNode {
    pub id: String,
    pub embedding: Vec<f32>,
    /// Derived density. Never negative — see `asserted` for outcome.
    pub coherence_score: Score,
    /// Asserted outcome in `[-1, 1]`; `None` = nobody has judged this yet.
    /// Absent in rows written before the axis existed, hence `serde(default)`.
    #[serde(default)]
    pub asserted: Option<Score>,
    #[serde(default)]
    pub label: Option<String>,
    /// Which embedder produced `embedding` (provenance). Lets a loader detect a
    /// model swap (mismatched id ⇒ stale vector, re-embed) instead of silently
    /// comparing vectors from different spaces. `None` for legacy/unstamped nodes.
    #[serde(default)]
    pub embedder: Option<String>,
    /// Semiotic cell the editor has manually pinned this node to, as
    /// `(domain, mode)` — the "hybrid" edit semantics. Structure (id, asserted,
    /// edges) is preserved across a content edit; the cell is re-derived from
    /// the new content; a pin overrides the auto result and survives restarts.
    /// `None` for nodes never pinned (and rows written before the field existed,
    /// hence `serde(default)`).
    #[serde(default)]
    pub cell_pin: Option<(String, String)>,
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
            cell_pin: None,
        }
    }

    /// Attach the source text the vector was embedded from.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The reported verdict, if one has been made.
    pub fn rating(&self) -> Option<CoherenceRating> {
        self.asserted.map(CoherenceRating::from_score)
    }

    /// True when someone has reported this as not working. This — not a low
    /// density — is what the dream loop exists to replay.
    pub fn is_asserted_failure(&self) -> bool {
        matches!(self.rating(), Some(CoherenceRating::Failure))
    }

    /// Everything currently true of this node, in the single form every surface
    /// renders from.
    ///
    /// The destructure below is **exhaustive on purpose** — no `..`. Adding a
    /// field to `CoherenceNode` stops compiling here until someone decides
    /// whether it is observable. `cell_pin` shipped stored, persisted, and read
    /// by nothing precisely because each surface hand-assembled its own DTO,
    /// and a hand-assembled projection can silently omit state where a derived
    /// one cannot.
    ///
    /// `auto_cell` is passed in rather than computed: classification needs the
    /// classifier, which lives in the server, not in the model layer.
    pub fn observable(&self, auto_cell: Option<(String, String)>) -> NodeView {
        let CoherenceNode {
            id,
            embedding,
            coherence_score,
            asserted,
            label,
            embedder,
            cell_pin,
        } = self;
        NodeView {
            id: id.clone(),
            label: label.clone().unwrap_or_default(),
            score: *coherence_score,
            asserted: *asserted,
            rating: asserted.map(CoherenceRating::from_score),
            // A pin outranks the classifier: it is the editor's testimony about
            // where this belongs, and testimony is not overruled by a re-embed.
            cell: cell_pin.clone().or(auto_cell),
            pinned: cell_pin.is_some(),
            embedder: embedder.clone(),
            // The vector itself is never projected — it is not renderable and
            // callers that want it hold the node. Its width is the useful part.
            dimension: embedding.len(),
        }
    }
}

/// The projection of a [`CoherenceNode`] — what a client can see.
///
/// Every surface (`/coherence/nodes`, `/coherence/nodes/all`, the TUI, the
/// graph UIs) selects a subset of this instead of building its own shape, so
/// state added to the engine cannot stay invisible in some of them and not
/// others. Field names are kept stable (`score`, not `coherence_score`) because
/// existing clients — the TUI's `parse_hits` among them — read them.
#[derive(Debug, Clone, Serialize)]
pub struct NodeView {
    pub id: String,
    /// The source text, or empty for a node registered as a bare vector.
    pub label: String,
    /// Derived density — mean cosine to nearest neighbours, never negative.
    pub score: Score,
    /// Reported outcome in `[-1, 1]`; absent until someone judges it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asserted: Option<Score>,
    /// The same verdict as a rating, so a client need not re-derive the bands.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<CoherenceRating>,
    /// The cell this node actually occupies: the editor's pin when there is
    /// one, else the classifier's top hit. `pinned` says which it was.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell: Option<(String, String)>,
    /// True when `cell` came from a human pin rather than the classifier.
    pub pinned: bool,
    /// Which embedder produced the vector.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedder: Option<String>,
    pub dimension: usize,
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
    #[serde(default)]
    pub facets: Facets,
}

/// What counts, *in this ontology*, as one of its signs having meant something.
///
/// Peirce's third leg. An entry pairs a signifier (its hints) with a signified
/// (its embedding centroid); the interpretant is the effect the sign has on
/// whoever used it — and it is not universal. "This worked" means a passing
/// test in a software ontology, a completed session in a fitness one, and a
/// machine that kept running in an industrial one. Declaring it per ontology
/// makes the universe of discourse explicit instead of assuming one global
/// rule fits all of them.
///
/// Deliberately declarative rather than learned: the outcomes are the existing
/// [`CoherenceRating`] vocabulary, so the feedback loop stays deterministic and
/// auditable — the same reason the engine avoids an LLM in the scoring path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Interpretant {
    /// Outcomes that reinforce an entry's salience. Default: `["success"]`.
    #[serde(default = "Interpretant::default_confirms")]
    pub confirms: Vec<String>,
    /// Outcomes that weaken it. Default: `["failure"]`.
    #[serde(default = "Interpretant::default_refutes")]
    pub refutes: Vec<String>,
    /// Days for accumulated salience to halve. `0` disables decay.
    ///
    /// Decay is what keeps the ontology a record of what is currently true
    /// rather than an ever-growing tally: an entry that stops being confirmed
    /// should fade, not hold its rank forever.
    #[serde(default = "Interpretant::default_half_life")]
    pub half_life_days: f32,
    /// How strongly this ontology's salience biases classification. `0` records
    /// outcomes without letting them influence scoring — useful for observing a
    /// signal before trusting it.
    #[serde(default = "Interpretant::default_weight")]
    pub weight: f32,
    /// Free-text statement of what confirmation means here, for the studio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Interpretant {
    fn default_confirms() -> Vec<String> {
        vec!["success".to_string()]
    }
    fn default_refutes() -> Vec<String> {
        vec!["failure".to_string()]
    }
    fn default_half_life() -> f32 {
        90.0
    }
    fn default_weight() -> f32 {
        1.0
    }

    /// The outcome names an interpretant may reference.
    pub const OUTCOMES: [&'static str; 3] = ["success", "inert", "failure"];

    /// Reject outcome names that will never fire, and weights that would make
    /// salience meaningless. A typo like `"succeeded"` would otherwise sit in
    /// the config silently matching nothing — the failure mode this codebase
    /// keeps hitting, so it is checked at load rather than discovered later.
    pub fn validate(&self) -> Result<(), String> {
        for name in self.confirms.iter().chain(self.refutes.iter()) {
            if !Self::OUTCOMES.contains(&name.as_str()) {
                return Err(format!(
                    "unknown outcome {name:?}; expected one of {:?}",
                    Self::OUTCOMES
                ));
            }
        }
        if let Some(both) = self.confirms.iter().find(|c| self.refutes.contains(c)) {
            return Err(format!("outcome {both:?} is listed as both confirming and refuting"));
        }
        if !self.half_life_days.is_finite() || self.half_life_days < 0.0 {
            return Err("half_life_days must be finite and >= 0".to_string());
        }
        if !self.weight.is_finite() || self.weight < 0.0 {
            return Err("weight must be finite and >= 0".to_string());
        }
        Ok(())
    }

    /// Salience delta for one observed outcome: `+1` confirming, `-1` refuting,
    /// `0` for anything this ontology does not care about.
    pub fn delta(&self, outcome: &str) -> f32 {
        if self.confirms.iter().any(|c| c == outcome) {
            1.0
        } else if self.refutes.iter().any(|r| r == outcome) {
            -1.0
        } else {
            0.0
        }
    }

    /// Multiplier applied to salience accumulated `days` ago.
    pub fn decay(&self, days: f32) -> f32 {
        if self.half_life_days <= 0.0 {
            return 1.0;
        }
        0.5f32.powf(days / self.half_life_days)
    }
}

impl Default for Interpretant {
    fn default() -> Self {
        Self {
            confirms: Self::default_confirms(),
            refutes: Self::default_refutes(),
            half_life_days: Self::default_half_life(),
            weight: Self::default_weight(),
            note: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyConfig {
    pub kind: String,
    /// How this ontology judges its own signs.
    ///
    /// `None` when the file declares nothing, which is deliberately kept
    /// distinct from an explicit copy of the defaults: the studio reports an
    /// absent interpretant as an open question rather than a decision, and
    /// that only works if absence survives deserialisation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interpretant: Option<Interpretant>,
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
    #[serde(default)]
    pub facets: Facets,
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

/// The 14 human modes of operation.
///
/// First six are the original activity-energy axis (LIFT…LEARN). The rest extend
/// it: DESTROY completes the Greimas opposition with CREATE; SENSE is
/// input/perception; GUIDE is leading others (vs LEARN, self-study); PLAY is
/// low-stakes generative; BRAINSTORM is divergent ideation; MAINTAIN is
/// upkeep/repair/preserve (keep-existing); MOVE is relocate/transport/logistics;
/// PLAN is convergent sequencing/organizing (the converge to BRAINSTORM's
/// diverge). New variants are appended so existing `cell_index` ordinals are
/// stable for the earlier modes.
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

    #[allow(clippy::should_implement_trait)]
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
    #[serde(default)]
    pub facets: Facets,
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
            facets: e.facets.clone(),
        })
    }

    pub fn cell_index(&self) -> usize {
        let d = HumanDomain::all().iter().position(|x| *x == self.domain).unwrap_or(0);
        let m = HumanMode::all().iter().position(|x| *x == self.mode).unwrap_or(0);
        d * HumanMode::all().len() + m
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

/// A cell in the 5×14 semiotic grid with all its metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemioticCell {
    pub domain: HumanDomain,
    pub mode: HumanMode,
    pub entries: Vec<String>,
    pub activation: f32,
    pub peircean: Option<PeirceanSign>,
    pub greimas: Option<String>,
}

/// The semiotic grid — a 5×14 matrix of domain×mode cells.
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
            facets: Facets::default(),
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
            // Create/Destroy = make↔unmake; Learn(self-study)↔Guide(teach others);
            // Sense(observe real)↔Brainstorm(imagine); Play(unstructured)↔Plan(structured);
            // Maintain(keep-in-place)↔Move(relocate).
            HumanMode::Create => HumanMode::Destroy,
            HumanMode::Destroy => HumanMode::Create,
            HumanMode::Learn => HumanMode::Guide,
            HumanMode::Guide => HumanMode::Learn,
            HumanMode::Sense => HumanMode::Brainstorm,
            HumanMode::Brainstorm => HumanMode::Sense,
            HumanMode::Play => HumanMode::Plan,
            HumanMode::Plan => HumanMode::Play,
            HumanMode::Maintain => HumanMode::Move,
            HumanMode::Move => HumanMode::Maintain,
        };
        Some((d, m))
    }

    /// Activation heatmap as a 5×14 matrix
    pub fn heatmap_matrix(&self) -> Vec<Vec<f32>> {
        let mut matrix = vec![vec![0.0_f32; HumanMode::all().len()]; HumanDomain::all().len()];
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

    // ── Interpretants ──────────────────────────────────────────────────────

    #[test]
    fn an_ontology_without_an_interpretant_gets_the_default() {
        let json = r#"{"kind":"software_dev","domains":[]}"#;
        let cfg: OntologyConfig = serde_json::from_str(json).unwrap();
        // Absence must survive deserialisation: the studio distinguishes an
        // unanswered question from an explicit choice of the defaults.
        assert!(cfg.interpretant.is_none());
        let d = Interpretant::default();
        assert_eq!(d.delta("success"), 1.0);
        assert_eq!(d.delta("failure"), -1.0);
        assert_eq!(d.delta("inert"), 0.0);
    }

    #[test]
    fn an_ontology_can_declare_its_own_interpretant() {
        // A maintenance ontology where merely running the check is the point:
        // an inert outcome still confirms, and only an outright failure refutes.
        let json = r#"{
            "kind":"machine",
            "interpretant":{
                "confirms":["success","inert"],
                "refutes":["failure"],
                "half_life_days":30,
                "weight":0.5,
                "note":"a check that ran and found nothing still confirms the routine"
            },
            "domains":[]
        }"#;
        let cfg: OntologyConfig = serde_json::from_str(json).unwrap();
        let i = cfg.interpretant.as_ref().expect("declared interpretant is preserved");
        i.validate().unwrap();
        assert_eq!(i.delta("inert"), 1.0, "inert confirms in this ontology");
        assert_eq!(i.delta("failure"), -1.0);
        assert_eq!(i.weight, 0.5);
        assert!(i.note.is_some());
    }

    #[test]
    fn a_misspelled_outcome_is_rejected_rather_than_silently_matching_nothing() {
        let i = Interpretant { confirms: vec!["succeeded".into()], ..Default::default() };
        let err = i.validate().unwrap_err();
        assert!(err.contains("succeeded"), "error should name the bad outcome: {err}");
    }

    #[test]
    fn an_outcome_cannot_both_confirm_and_refute() {
        let i = Interpretant {
            confirms: vec!["success".into()],
            refutes: vec!["success".into()],
            ..Default::default()
        };
        assert!(i.validate().is_err());
    }

    #[test]
    fn salience_halves_over_one_half_life_and_can_be_disabled() {
        let i = Interpretant { half_life_days: 30.0, ..Default::default() };
        assert!((i.decay(0.0) - 1.0).abs() < 1e-6);
        assert!((i.decay(30.0) - 0.5).abs() < 1e-6);
        assert!((i.decay(60.0) - 0.25).abs() < 1e-6);

        let forever = Interpretant { half_life_days: 0.0, ..Default::default() };
        assert_eq!(forever.decay(3650.0), 1.0, "half_life 0 means no decay");
    }

    #[test]
    fn a_zero_weight_interpretant_still_records_but_cannot_bias() {
        // Observing a signal before trusting it: deltas are unchanged, weight
        // is what gates the influence on scoring.
        let i = Interpretant { weight: 0.0, ..Default::default() };
        i.validate().unwrap();
        assert_eq!(i.delta("success"), 1.0);
        assert_eq!(i.weight, 0.0);
    }
}
