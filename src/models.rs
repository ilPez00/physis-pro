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
    /// Ontological review verdict, set when the dream was generated under the
    /// validation contract (see `dream::DreamContract`). Absent for legacy
    /// dreams and uncontracted generation — a dream that was never reviewed
    /// must stay distinguishable from one that was reviewed and accepted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<DreamReview>,
}

/// The ontological validation verdict for one generated dream.
///
/// Applies the Generative Ontology method (research_new): a generated output
/// must be nameable by the domain's ontology to count as coherent. Here the
/// grid is the schema — a dream whose embedding has no ontology entry within
/// `min_cosine` is structure-shaped noise (interpolating a machine fault
/// with an invoice lands where classification finds nothing), and a plan
/// that names nothing is not a plan. Rejections are counted, never hidden.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamReview {
    /// Whether the dream passed the contract (`cosine >= min_cosine`).
    pub accepted: bool,
    /// Closest ontology entry, if the grid had anything to say at all.
    pub best_entry: Option<String>,
    /// The owning cell of that entry, `DOMAIN×MODE`.
    pub best_cell: Option<String>,
    /// Best single-entry cosine found.
    pub cosine: f32,
    /// The contract threshold that was applied.
    pub min_cosine: f32,
}

impl Dream {
    pub fn new(
        source: Vec<f32>,
        embedding: Vec<f32>,
        dream_type: String,
        description: String,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source,
            embedding,
            grade: None,
            dream_type,
            description,
            review: None,
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
    /// When this node was first registered into the graph. Backward-compatible:
    /// rows written before the field existed deserialize to the epoch
    /// (1970-01-01), so `as_of` time-travel over legacy stores simply treats
    /// them as "always present". Stamped by `CoherenceNode::new` and preserved
    /// across content edits (structure survives, only content/score change).
    #[serde(default)]
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Optional structured template extracted during ingestion (PLAN 11).
    /// When present, `observable()` will populate `fragment` from it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<DocumentTemplate>,
    /// Typed-ontology object kind (Phase 16 I5); empty when untyped. Absent in
    /// rows written before the axis existed, hence `serde(default)`.
    #[serde(default)]
    pub kind: String,
    /// Typed-object properties (Phase 16 I5), keyed by name. Default-empty for
    /// rows written before the axis existed.
    #[serde(default)]
    pub properties: serde_json::Map<String, serde_json::Value>,
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
            created_at: chrono::Utc::now(),
            template: None,
            kind: String::new(),
            properties: serde_json::Map::new(),
        }
    }

    /// Attach the source text the vector was embedded from.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Stamp the typed-ontology object kind (Phase 16 I5).
    pub fn with_kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = kind.into();
        self
    }

    /// Stamp the typed-object properties (Phase 16 I5).
    pub fn with_properties(
        mut self,
        properties: serde_json::Map<String, serde_json::Value>,
    ) -> Self {
        self.properties = properties;
        self
    }

    /// Set a single typed-object property.
    pub fn property(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.properties.insert(key.into(), value);
        self
    }

    /// Attach a structured template (PLAN 11). When present, `observable()`
    /// will populate `fragment` from the template's fragment field.
    pub fn with_template(mut self, template: DocumentTemplate) -> Self {
        self.template = Some(template);
        self
    }

    /// Attach an embedder provenance stamp.
    pub fn with_embedder(mut self, embedder: impl Into<String>) -> Self {
        self.embedder = Some(embedder.into());
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
            created_at,
            template,
            kind,
            properties,
        } = self;
        NodeView {
            id: id.clone(),
            label: label.clone().unwrap_or_default(),
            score: *coherence_score,
            asserted: *asserted,
            rating: asserted.map(CoherenceRating::from_score),
            cell: cell_pin.clone().or(auto_cell),
            pinned: cell_pin.is_some(),
            embedder: embedder.clone(),
            dimension: embedding.len(),
            created_at: *created_at,
            fragment: template.as_ref().and_then(|t| t.as_fragment())
                .or_else(|| self.extract_fragment()),
            instances: Vec::new(),  // Populated by the server when serving nodes
            kind: kind.clone(),
            properties: properties.clone(),
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
    /// When the node was first registered — surfaces the time axis so clients
    /// can do "what did the graph know before <incident>" without re-deriving
    /// it. Epoch 1970 for legacy nodes written before the field existed.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// The exact text fragment that contributed most to this node's classification,
    /// extracted from the source document or label. Enables precise citation
    /// instead of opaque node references.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fragment: Option<String>,
    /// All instances sharing conceptual similarity — nodes whose embeddings land
    /// in the same domain×mode cell or whose cosine exceeds a threshold. Each is
    /// a lightweight (id, label, score, path) that the UI can render as a clickable
    /// hyperlink to the origin file.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instances: Vec<ConceptualInstance>,
    /// Typed-ontology object kind (Phase 16 I5); empty when untyped.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    /// Typed-object properties (Phase 16 I5); empty when untyped.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub properties: serde_json::Map<String, serde_json::Value>,
}

/// A pointer to a node that shares conceptual similarity with the current node.
/// Used for "see all instances" browsing — each one is hyperlinked-clickable.
#[derive(Debug, Clone, Serialize)]
pub struct ConceptualInstance {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<Score>,
    /// Where this instance lives — file path, node id, or other origin reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// Line or position within the origin for precise navigation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    /// The semiotic cell this instance occupies.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell: Option<(String, String)>,
}

impl CoherenceNode {
    /// Extract the best citation fragment for this node — the source text or the
    /// most relevant portion of it. Falls back to the full label if no template
    /// is attached.
    pub fn extract_fragment(&self) -> Option<String> {
        let label = self.label.as_deref()?;
        // Prefer the first complete sentence or clause for readability in UI.
        let fragment = label
            .split('.')
            .next()
            .unwrap_or(label)
            .trim()
            .strip_suffix(',')
            .unwrap_or(label.trim())
            .to_string();
        if fragment.len() <= label.len().min(120) && !fragment.is_empty() {
            Some(fragment)
        } else {
            Some(label.trim().to_string())
        }
    }
}

// ── Document Templates (PLAN 11) ──────────────────────────────────────
// Structured output templates per file type. Each extractor produces one of these
// alongside the body text, so the Studio UI can render type-specific metadata
// ("Vendor: Acme · 2026-05-12") while the engine just consumes `body`.

/// Structured template extracted from an invoice or procurement document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InvoiceTemplate {
    /// Vendor/issuer name extracted from the document header.
    pub vendor: Option<String>,
    /// Invoice or document number (e.g. "INV-2026-088").
    pub doc_number: Option<String>,
    /// Issue date, ISO `YYYY-MM-DD` when parseable.
    pub date: Option<String>,
    /// Total amount as parsed currency + amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_amount: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// Line-level detail for reconstruction and auditing.
    #[serde(default)]
    pub line_items: Vec<InvoiceLine>,
    /// The most relevant text fragment that triggered classification.
    pub fragment: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InvoiceLine {
    pub description: String,
    #[serde(default)]
    pub quantity: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_price: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<f32>,
}

/// Structured template extracted from an email message.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmailTemplate {
    pub from: Option<String>,
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cc: Option<String>,
    pub subject: String,
    /// Thread ID for grouping replies.
    pub thread_id: String,
    /// Plain-text body (after quoted-reply stripping).
    pub body_plain: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_html: Option<String>,
    #[serde(default)]
    pub attachments: Vec<AttachmentMeta>,
    pub fragment: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttachmentMeta {
    pub filename: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    pub size_bytes: usize,
}

/// Structured template extracted from a spreadsheet.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpreadsheetTemplate {
    pub sheet_name: String,
    pub headers: Vec<String>,
    /// First N rows as string cells.
    #[serde(default)]
    pub rows: Vec<Vec<String>>,
    /// Auto-detected category (finance, procurement, etc.).
    pub detected_kind: String,
    pub fragment: Option<String>,
}

/// Structured template extracted from an audio transcript (Whisper).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TranscriptTemplate {
    /// Timestamped speaker labels (seconds, speaker name/id).
    #[serde(default)]
    pub speaker_labels: Vec<(f32, String)>,
    /// Full transcribed text.
    pub text: String,
    /// Duration in seconds.
    pub duration_secs: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub fragment: Option<String>,
}

/// Structured template for image files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImageTemplate {
    pub width_px: u32,
    pub height_px: u32,
    #[serde(default)]
    pub color_space: String,
    /// Top 5 dominant colors as (hex, proportion).
    #[serde(default)]
    pub dominant_colors: Vec<(String, f32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_text: Option<String>,
    pub fragment: Option<String>,
}

/// Structured template for video files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VideoTemplate {
    pub width_px: u32,
    pub height_px: u32,
    pub duration_secs: f32,
    pub fps: f32,
    /// Number of frames extracted for embedding.
    pub frame_count: usize,
    #[serde(default)]
    pub motion_score: f32,
    pub fragment: Option<String>,
}

/// Generic template fallback for unclassified files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GenericTemplate {
    pub file_size_bytes: u64,
    pub mime_type: Option<String>,
    pub first_chars: String,
    pub fragment: Option<String>,
}

/// Union of all template types; attached to IngestedDoc.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum DocumentTemplate {
    Invoice(InvoiceTemplate),
    Email(EmailTemplate),
    Spreadsheet(SpreadsheetTemplate),
    Transcript(TranscriptTemplate),
    Image(ImageTemplate),
    Video(VideoTemplate),
    Generic(GenericTemplate),
}

impl Default for DocumentTemplate {
    fn default() -> Self {
        DocumentTemplate::Generic(GenericTemplate::default())
    }
}

impl DocumentTemplate {
    /// Extract the best citation fragment from any template type.
    pub fn as_fragment(&self) -> Option<String> {
        match self {
            DocumentTemplate::Invoice(t) => t.fragment.clone(),
            DocumentTemplate::Email(t) => {
                t.fragment.clone().or_else(|| Some(t.subject.clone()))
            }
            DocumentTemplate::Spreadsheet(t) => t.fragment.clone(),
            DocumentTemplate::Transcript(t) => t.fragment.clone(),
            DocumentTemplate::Image(t) => t.fragment.clone(),
            DocumentTemplate::Video(t) => t.fragment.clone(),
            DocumentTemplate::Generic(t) => t.fragment.clone(),
        }
    }

    /// Get a display name for the template type.
    pub fn display_name(&self) -> &'static str {
        match self {
            DocumentTemplate::Invoice(_) => "Invoice",
            DocumentTemplate::Email(_) => "Email",
            DocumentTemplate::Spreadsheet(_) => "Spreadsheet",
            DocumentTemplate::Transcript(_) => "Audio Transcript",
            DocumentTemplate::Image(_) => "Image",
            DocumentTemplate::Video(_) => "Video",
            DocumentTemplate::Generic(_) => "Document",
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
    pub fn new(
        goal_id: &str,
        before: Vec<f32>,
        after: Vec<f32>,
        action: String,
        rationale: String,
        scores: std::collections::HashMap<String, f32>,
    ) -> Self {
        let delta: Vec<f32> = before
            .iter()
            .zip(after.iter())
            .map(|(b, a)| a - b)
            .collect();
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
    pub fn new(
        query_embedding: Vec<f32>,
        conflicting_ids: Vec<String>,
        suggestion: &str,
        gap: Score,
    ) -> Self {
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
            return Err(format!(
                "outcome {both:?} is listed as both confirming and refuting"
            ));
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
        [
            HumanDomain::Heal,
            HumanDomain::Construct,
            HumanDomain::Fabricate,
            HumanDomain::Bond,
            HumanDomain::Study,
        ]
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
            HumanMode::Lift,
            HumanMode::Rest,
            HumanMode::Walk,
            HumanMode::Work,
            HumanMode::Create,
            HumanMode::Learn,
            HumanMode::Destroy,
            HumanMode::Sense,
            HumanMode::Guide,
            HumanMode::Play,
            HumanMode::Brainstorm,
            HumanMode::Maintain,
            HumanMode::Move,
            HumanMode::Plan,
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
        let d = HumanDomain::all()
            .iter()
            .position(|x| *x == self.domain)
            .unwrap_or(0);
        let m = HumanMode::all()
            .iter()
            .position(|x| *x == self.mode)
            .unwrap_or(0);
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
        self.cells
            .iter()
            .find(|c| c.domain == domain && c.mode == mode)
    }

    pub fn get_cell_mut(
        &mut self,
        domain: HumanDomain,
        mode: HumanMode,
    ) -> Option<&mut SemioticCell> {
        self.cells
            .iter_mut()
            .find(|c| c.domain == domain && c.mode == mode)
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

    pub fn compose(
        &self,
        d1: HumanDomain,
        _m1: HumanMode,
        d2: HumanDomain,
        m2: HumanMode,
    ) -> Option<GridPosition> {
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
            let di = HumanDomain::all()
                .iter()
                .position(|d| *d == cell.domain)
                .unwrap_or(0);
            let mi = HumanMode::all()
                .iter()
                .position(|m| *m == cell.mode)
                .unwrap_or(0);
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

// The typed-ontology axis started with a `kind` on CoherenceNode (I5). This
// resolver completes it: it maps the 70-cell classification grid back to the
// kind of thing a node *is* — a machine, a work order, a sensor reading, an
// invoice-like record. Classification (domain × mode) already applied overlays
// and rerank, so this is a deterministic lookup, not a second classifier.

/// The object-type vocabulary a classified node can denote. Used to validate
/// user-supplied kinds and to auto-stamp `kind` from the winning cell.
pub const OBJECT_TYPES: &[&str] = &[
    "machine",
    "asset",
    "sensor_reading",
    "work_order",
    "maintenance_record",
    "inspection",
    "incident",
    "goal",
    "person",
    "contact",
    "document",
    "knowledge",
    "note",
    "observation",
    "prototype",
];

/// Resolve the 70-cell grid to an object type. **Total over the grid**: every
/// (domain, mode) cell denotes a thing, so a winning cell always yields a type.
/// Mode refinements beat the per-domain default.
///
/// The five domains: HEAL (restore/care), CONSTRUCT (build), FABRICATE
/// (make/produce), BOND (relate/people/exchange), STUDY (learn/know).
pub fn object_type_for(domain: &str, mode: &str) -> Option<&'static str> {
    let d = domain.trim().to_ascii_uppercase();
    let m = mode.trim().to_ascii_uppercase();
    Some(match (d.as_str(), m.as_str()) {
        // FABRICATE — the plant floor.
        ("FABRICATE", "SENSE") => "sensor_reading",
        ("FABRICATE", "PLAN") => "work_order",
        ("FABRICATE", "MAINTAIN") => "maintenance_record",
        ("FABRICATE", "CREATE") => "prototype",
        ("FABRICATE", _) => "machine",
        // CONSTRUCT — build / erect.
        ("CONSTRUCT", "SENSE") => "inspection",
        ("CONSTRUCT", "PLAN") => "work_order",
        ("CONSTRUCT", "MAINTAIN") => "maintenance_record",
        ("CONSTRUCT", _) => "asset",
        // HEAL — health, recovery, incidents.
        ("HEAL", "SENSE") => "sensor_reading",
        ("HEAL", "MAINTAIN") => "maintenance_record",
        ("HEAL", "PLAN") => "work_order",
        ("HEAL", _) => "incident",
        // BOND — people, relations, goals, exchange.
        ("BOND", "PLAN") | ("BOND", "WORK") => "goal",
        ("BOND", "SENSE") => "contact",
        ("BOND", _) => "person",
        // STUDY — knowledge work.
        ("STUDY", "SENSE") => "observation",
        ("STUDY", "LEARN") => "knowledge",
        ("STUDY", "BRAINSTORM") => "note",
        ("STUDY", _) => "document",
        _ => return None,
    })
}

/// Whether `kind` is in the known object vocabulary (or empty/untyped).
pub fn is_valid_object_kind(kind: &str) -> bool {
    kind.is_empty() || OBJECT_TYPES.contains(&kind)
}

// ── Typed actions (Phase 16 I8) ─────────────────────────────────────────────
// The "decide" half of the dream loop: a replayed failure node proposes one
// *concrete* action an operator — or, later, a harness via the outbound layer
// (I11/I12) — can take, derived from the node's object kind. Not free text that
// only a human can interpret: `OpenWorkOrder`/`ScheduleMaintenance` are actions
// an agent can dispatch.

/// The kinds of concrete action the dream loop can propose for a failing node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// Open (or re-plan) a work order for the failing object.
    OpenWorkOrder,
    /// Schedule maintenance for a machine/asset.
    ScheduleMaintenance,
    /// Update the record (maintenance/sensor log) for this object.
    UpdateRecord,
    /// Review the failing document/knowledge node.
    ReviewDocument,
    /// Escalate to a person or goal.
    Escalate,
    /// Acknowledge an incident or a resolved item.
    Acknowledge,
}

impl ActionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ActionKind::OpenWorkOrder => "open_work_order",
            ActionKind::ScheduleMaintenance => "schedule_maintenance",
            ActionKind::UpdateRecord => "update_record",
            ActionKind::ReviewDocument => "review_document",
            ActionKind::Escalate => "escalate",
            ActionKind::Acknowledge => "acknowledge",
        }
    }
}

/// A concrete, typed action proposed by the dream loop for one node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedAction {
    pub kind: ActionKind,
    /// The node the action targets (the failing object's id).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    /// The object kind (I5) that selected the action, e.g. `machine`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_kind: Option<String>,
    /// One-line instruction both a human and an agent can act on.
    pub summary: String,
}

/// Derive the typed action for a replayed node (Phase 16 I8). A replayed node
/// that the dream fixed (or a positive outcome) is an acknowledgement; a node
/// still failing gets an action chosen by its object kind — machine → schedule
/// maintenance, work order → open a work order, sensor → investigate, etc.
pub fn typed_action_for(
    node_kind: &str,
    object_id: Option<String>,
    outcome: f32,
    prevented_failure: bool,
) -> TypedAction {
    let (kind, verb, fallback_target) = if prevented_failure || outcome >= 0.0 {
        (ActionKind::Acknowledge, "acknowledge", "the resolved item")
    } else {
        match node_kind {
            "machine" | "asset" => {
                (ActionKind::ScheduleMaintenance, "schedule maintenance for", "the machine")
            }
            "work_order" => (ActionKind::OpenWorkOrder, "open a work order for", "the work order"),
            "sensor_reading" => {
                (ActionKind::OpenWorkOrder, "investigate the reading from", "the sensor")
            }
            "maintenance_record" | "inspection" => {
                (ActionKind::UpdateRecord, "update the record for", "the record")
            }
            "document" | "knowledge" | "note" => {
                (ActionKind::ReviewDocument, "review", "the document")
            }
            "person" | "goal" => (ActionKind::Escalate, "escalate", "the person/goal"),
            "incident" => (ActionKind::Acknowledge, "acknowledge", "the incident"),
            _ => (ActionKind::Acknowledge, "acknowledge", "the item"),
        }
    };
    let target = object_id.clone().unwrap_or_else(|| fallback_target.to_string());
    TypedAction {
        kind,
        object_id,
        object_kind: if node_kind.is_empty() {
            None
        } else {
            Some(node_kind.to_string())
        },
        summary: format!("{verb} {target}"),
    }
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
        let d = Dream::new(
            vec![1.0, 0.0],
            vec![0.5, 0.5],
            "test".to_string(),
            "test dream".to_string(),
        );
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
    fn typed_axis_defaults_empty_and_stamps() {
        let c = CoherenceNode::new(vec![1.0, 0.0]);
        assert_eq!(c.kind, "");
        assert!(c.properties.is_empty());

        let mut props = serde_json::Map::new();
        props.insert("oee".into(), serde_json::json!(0.82));
        let typed = c
            .with_kind("machine")
            .with_properties(props.clone())
            .property("down", serde_json::json!(true));
        assert_eq!(typed.kind, "machine");
        assert_eq!(typed.properties.get("oee"), Some(&serde_json::json!(0.82)));
        assert_eq!(typed.properties.get("down"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn legacy_node_json_without_typed_axis_deserializes() {
        // Rows written before the axis existed have no kind/properties fields;
        // they must load as untyped, not fail.
        let legacy = r#"{
            "id": "legacy-1",
            "embedding": [0.1, 0.2],
            "coherence_score": 0.5,
            "asserted": null,
            "label": "old node",
            "created_at": "2026-01-01T00:00:00Z"
        }"#;
        let node: CoherenceNode = serde_json::from_str(legacy).unwrap();
        assert_eq!(node.kind, "");
        assert!(node.properties.is_empty());
        assert_eq!(node.label.as_deref(), Some("old node"));
    }

    #[test]
    fn object_type_resolver_is_total_over_the_grid() {
        // Every one of the 5×14 cells resolves to a known object type.
        for d in HumanDomain::all() {
            for m in HumanMode::all() {
                let t = object_type_for(d.as_str(), m.as_str())
                    .unwrap_or_else(|| panic!("no object type for {} / {}", d.as_str(), m.as_str()));
                assert!(
                    OBJECT_TYPES.contains(&t),
                    "{}/{} resolved to {} which is not in the vocabulary",
                    d.as_str(),
                    m.as_str(),
                    t
                );
            }
        }
        // Unknown domains yield nothing; unknown modes fall through to the
        // domain default (the grid is total over its own 14 modes).
        assert_eq!(object_type_for("NOT_A_DOMAIN", "WORK"), None);
        assert_eq!(object_type_for("FABRICATE", "NOT_A_MODE"), Some("machine"));
    }

    #[test]
    fn object_type_refinements_beat_domain_defaults() {
        assert_eq!(object_type_for("FABRICATE", "WORK"), Some("machine"));
        assert_eq!(object_type_for("FABRICATE", "SENSE"), Some("sensor_reading"));
        assert_eq!(object_type_for("FABRICATE", "PLAN"), Some("work_order"));
        assert_eq!(object_type_for("CONSTRUCT", "WORK"), Some("asset"));
        assert_eq!(object_type_for("CONSTRUCT", "SENSE"), Some("inspection"));
        assert_eq!(object_type_for("HEAL", "LIFT"), Some("incident"));
        assert_eq!(object_type_for("BOND", "PLAN"), Some("goal"));
        assert_eq!(object_type_for("BOND", "PLAY"), Some("person"));
        assert_eq!(object_type_for("STUDY", "LEARN"), Some("knowledge"));
        assert_eq!(object_type_for("STUDY", "BRAINSTORM"), Some("note"));
        // Case-insensitive.
        assert_eq!(object_type_for("fabricate", "sense"), Some("sensor_reading"));
    }

    #[test]
    fn object_kind_validation() {
        assert!(is_valid_object_kind("machine"));
        assert!(is_valid_object_kind("sensor_reading"));
        assert!(is_valid_object_kind("")); // untyped is fine
        assert!(!is_valid_object_kind("not-a-real-kind"));
    }

    #[test]
    fn typed_action_maps_node_kind_to_action() {
        let a = typed_action_for("machine", Some("cnc-01".into()), -0.8, false);
        assert_eq!(a.kind, ActionKind::ScheduleMaintenance);
        assert_eq!(a.summary, "schedule maintenance for cnc-01");
        assert_eq!(a.object_kind.as_deref(), Some("machine"));

        let a = typed_action_for("work_order", Some("wo-41".into()), -0.8, false);
        assert_eq!(a.kind, ActionKind::OpenWorkOrder);
        assert_eq!(a.summary, "open a work order for wo-41");

        let a = typed_action_for("sensor_reading", None, -0.8, false);
        assert_eq!(a.kind, ActionKind::OpenWorkOrder);
        assert_eq!(a.summary, "investigate the reading from the sensor");

        let a = typed_action_for("document", Some("d-1".into()), -0.8, false);
        assert_eq!(a.kind, ActionKind::ReviewDocument);

        let a = typed_action_for("person", None, -0.8, false);
        assert_eq!(a.kind, ActionKind::Escalate);
    }

    #[test]
    fn typed_action_acknowledges_when_dream_prevented_failure() {
        let a = typed_action_for("machine", Some("cnc-01".into()), 0.5, true);
        assert_eq!(a.kind, ActionKind::Acknowledge);
        assert_eq!(a.summary, "acknowledge cnc-01");

        // Positive outcome without explicit prevention also closes the loop.
        let b = typed_action_for("work_order", None, 0.3, false);
        assert_eq!(b.kind, ActionKind::Acknowledge);
    }

    #[test]
    fn typed_action_untyped_node_falls_back_to_acknowledge() {
        let a = typed_action_for("", Some("x".into()), -0.8, false);
        assert_eq!(a.kind, ActionKind::Acknowledge);
        assert_eq!(a.object_kind, None);
    }

    #[test]
    fn test_experience_new_computes_delta() {
        let e = Experience::new(
            "goal-1",
            vec![1.0, 0.0, 0.0],
            vec![1.0, 0.5, 0.3],
            "action".to_string(),
            "rationale".to_string(),
            std::collections::HashMap::new(),
        );
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
        let i = cfg
            .interpretant
            .as_ref()
            .expect("declared interpretant is preserved");
        i.validate().unwrap();
        assert_eq!(i.delta("inert"), 1.0, "inert confirms in this ontology");
        assert_eq!(i.delta("failure"), -1.0);
        assert_eq!(i.weight, 0.5);
        assert!(i.note.is_some());
    }

    #[test]
    fn a_misspelled_outcome_is_rejected_rather_than_silently_matching_nothing() {
        let i = Interpretant {
            confirms: vec!["succeeded".into()],
            ..Default::default()
        };
        let err = i.validate().unwrap_err();
        assert!(
            err.contains("succeeded"),
            "error should name the bad outcome: {err}"
        );
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
        let i = Interpretant {
            half_life_days: 30.0,
            ..Default::default()
        };
        assert!((i.decay(0.0) - 1.0).abs() < 1e-6);
        assert!((i.decay(30.0) - 0.5).abs() < 1e-6);
        assert!((i.decay(60.0) - 0.25).abs() < 1e-6);

        let forever = Interpretant {
            half_life_days: 0.0,
            ..Default::default()
        };
        assert_eq!(forever.decay(3650.0), 1.0, "half_life 0 means no decay");
    }

    #[test]
    fn a_zero_weight_interpretant_still_records_but_cannot_bias() {
        // Observing a signal before trusting it: deltas are unchanged, weight
        // is what gates the influence on scoring.
        let i = Interpretant {
            weight: 0.0,
            ..Default::default()
        };
        i.validate().unwrap();
        assert_eq!(i.delta("success"), 1.0);
        assert_eq!(i.weight, 0.0);
    }

    // ── Document Template Tests (PLAN 11) ──────────────────────────────

    #[test]
    fn test_invoice_template_fragment() {
        let template = DocumentTemplate::Invoice(InvoiceTemplate {
            vendor: Some("ACME Corp".into()),
            doc_number: Some("INV-2026-001".into()),
            date: Some("2026-05-10".into()),
            total_amount: Some(1250.00),
            currency: Some("USD".into()),
            line_items: vec![InvoiceLine {
                description: "Bearing assembly".into(),
                quantity: 2.0,
                unit_price: Some(625.00),
                total: Some(1250.00),
            }],
            fragment: Some("Bearing assembly for CNC spindle repair".into()),
        });
        assert_eq!(template.display_name(), "Invoice");
        let frag = template.as_fragment();
        assert_eq!(frag.as_deref(), Some("Bearing assembly for CNC spindle repair"));
    }

    #[test]
    fn test_email_template_fragment() {
        let template = DocumentTemplate::Email(EmailTemplate {
            from: Some("supplier@example.com".into()),
            to: Some("procurement@factory.com".into()),
            subject: "Delay update on shipment".into(),
            thread_id: "msg-123".into(),
            body_plain: "The shipment will be delayed by 2 days.".into(),
            fragment: None, // No explicit fragment — should fall back to subject
            ..Default::default()
        });
        assert_eq!(template.display_name(), "Email");
        let frag = template.as_fragment();
        assert_eq!(frag.as_deref(), Some("Delay update on shipment"));
    }

    #[test]
    fn test_spreadsheet_template() {
        let template = DocumentTemplate::Spreadsheet(SpreadsheetTemplate {
            sheet_name: "Inventory".into(),
            headers: vec!["Item".into(), "Quantity".into(), "Location".into()],
            rows: vec![vec!["Bearing".into(), "12".into(), "Warehouse A".into()]],
            detected_kind: "inventory".into(),
            fragment: Some("Bearing inventory at 12 units in Warehouse A".into()),
        });
        assert_eq!(template.display_name(), "Spreadsheet");
    }

    #[test]
    fn test_transcript_template() {
        let template = DocumentTemplate::Transcript(TranscriptTemplate {
            text: "Operator: Machine is running hot today.".into(),
            duration_secs: 45.5,
            language: Some("en".into()),
            fragment: Some("Operator: Machine is running hot today.".into()),
            ..Default::default()
        });
        assert_eq!(template.display_name(), "Audio Transcript");
    }

    #[test]
    fn test_image_template() {
        let template = DocumentTemplate::Image(ImageTemplate {
            width_px: 1920,
            height_px: 1080,
            color_space: "sRGB".into(),
            dominant_colors: vec![("#ff0000".into(), 0.45), ("#00ff00".into(), 0.32)],
            detected_text: Some("WARNING: HIGH PRESSURE".into()),
            fragment: Some("WARNING: HIGH PRESSURE".into()),
        });
        assert_eq!(template.display_name(), "Image");
    }

    #[test]
    fn test_video_template() {
        let template = DocumentTemplate::Video(VideoTemplate {
            width_px: 1280,
            height_px: 720,
            duration_secs: 120.5,
            fps: 30.0,
            frame_count: 3615,
            motion_score: 0.23,
            fragment: None,
        });
        assert_eq!(template.display_name(), "Video");
        assert!(!template.as_fragment().is_some(), "no fragment set");
    }

    #[test]
    fn test_generic_template_default() {
        let template = DocumentTemplate::default();
        match template {
            DocumentTemplate::Generic(_) => {}
            _ => panic!("default should be Generic"),
        }
    }

    #[test]
    fn test_coherence_node_with_template() {
        let template = DocumentTemplate::Invoice(InvoiceTemplate {
            vendor: Some("ACME".into()),
            fragment: Some("Bearing assembly for spindle repair".into()),
            ..Default::default()
        });
        let node = CoherenceNode::new(vec![0.1; 8])
            .with_label("invoice doc")
            .with_template(template);
        let view = node.observable(None);
        assert_eq!(
            view.fragment.as_deref(),
            Some("Bearing assembly for spindle repair")
        );
    }
}
