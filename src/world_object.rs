//! `WorldObject` — the one object the whole system navigates (P04).
//!
//! Every surface in the mission is a view over the same thing: a file, a
//! concept, a project, a module, a symbol, a person, a command, a process, a
//! machine, a website, a conversation, a service, a device. Today those live in
//! different stores — `CoherenceNode` in the engine, `ThemeNode`/`FileNode` in
//! the file-theme graph, notebook cells, ledger entries. This module defines the
//! shape they all project onto, so the TUI stream has exactly one item type to
//! render and the interop layer has exactly one schema to version.
//!
//! This is a *view type*, not a new store. It is composed from the existing
//! nodes and edges; it does not replace them. Its navigable identity is
//! [`crate::nav::ObjectId`]: the flat string ids on this view are its
//! `origin:key` spellings, so every projection addresses the same objects the
//! nav index owns (rename survival, content-derived identity, passage spans
//! included).
//!
//! The two fields that make the mission's claim checkable:
//!
//! * [`Provenance::route`] — was this value produced by a deterministic
//!   operator or by a model? Every answer carries it, so the UI can show it and
//!   the observation log can count it.
//! * [`WorldObject::provenance_chain`] — the "why did this appear" line. A
//!   navigational layer that cannot say why it said something is an oracle, and
//!   this project does not ship oracles (`ported/world.rs` states the same rule).

use serde::{Deserialize, Serialize};

use crate::models::CoherenceNode;
use crate::nav::{ObjectId, RelationOrigin};
use std::path::Path;

/// Stable identity of a world object. This is the `origin:key` spelling of
/// [`crate::nav::ObjectId`] — namespaced by the store that minted it, e.g.
/// `core:node:7f3a`, `fs:/home/gio/dev/aion/src/aion/ui/app.py`,
/// `fleet:node:pansa`, `cyclops:note:2026-09-19T14:00Z`. Stored as a string so
/// the view stays serde-stable; parse it back with [`parse_object_id`].
pub type WorldObjectId = String;

/// Parse a view identity back into the canonical navigable id. Every value
/// this view mints is written in `origin:key` form, so the round trip holds.
/// Pre-existing `fs:` identities use the whole path as the key (`fs` origin);
/// pass an explicit origin/key form for those when the content hash is known.
pub fn parse_object_id(id: &str) -> ObjectId {
    match id.split_once(':') {
        Some((origin, key)) => ObjectId::new(origin, key),
        None => ObjectId::new("unknown", id),
    }
}

/// What kind of thing this is. Drives the graph's iconography and the
/// affordances that are even offerable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    File,
    Concept,
    Project,
    Module,
    Symbol,
    Person,
    Command,
    Process,
    Machine,
    Website,
    Conversation,
    Service,
    Device,
    /// A model-backed agent run: identity, provider/model, status and the
    /// trace of what it did. Projected from plain strings, never from the
    /// provider runtime (see [`WorldObject::agent`]).
    Agent,
    /// A kind this build does not know yet. Carried verbatim so an older reader
    /// does not drop a newer writer's object.
    Other(String),
}

/// Who produced a value. The distinction the mission insists on: a result the
/// deterministic path can justify is not the same as a model's guess, and the
/// UI must never blur them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    /// Produced by an operator over stored state — reproducible, no model.
    Deterministic,
    /// Produced with a model in the loop.
    Model,
}

/// Where a value came from, in enough detail to re-derive it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    pub route: Route,
    /// The operator or tool, e.g. `embed:bge-base`, `operator:neighbors`,
    /// `operator:semantic-delta`, `model:jev-1.13-free`.
    pub source: String,
    /// Position in the append-only observation log, when the value is backed by
    /// one. Ordered by log position, never by clock.
    #[serde(default)]
    pub observation_seq: Option<u64>,
}

impl Provenance {
    pub fn deterministic(source: impl Into<String>) -> Self {
        Self {
            route: Route::Deterministic,
            source: source.into(),
            observation_seq: None,
        }
    }

    pub fn model(source: impl Into<String>) -> Self {
        Self {
            route: Route::Model,
            source: source.into(),
            observation_seq: None,
        }
    }

    pub fn at(mut self, seq: u64) -> Self {
        self.observation_seq = Some(seq);
        self
    }
}

/// A typed edge. The edge carries meaning; an untyped edge is a bug. Two
/// orthogonal axes: `kind` is *what* the edge means, `origin` is *how we know*
/// (a [`crate::nav::RelationOrigin`]). Similarity projections become
/// `SimilarTo` + `Inferred`; recorded graph edges keep their own origin, so
/// the view never implies a nearest-neighbour similarity is a proven relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    Contains,
    DependsOn,
    ProducedBy,
    SimilarTo,
    Contradicts,
    Calls,
    Controls,
    DerivedFrom,
    ModifiedBy,
    Requires,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    pub kind: RelationKind,
    pub target: WorldObjectId,
    #[serde(default)]
    pub provenance: Option<Provenance>,
    /// How this edge is known. Similarity-derived edges are always
    /// `Inferred`; asserted and scanned edges carry their own origin. Defaults
    /// to `Inferred` for view-built edges, so a reader can never mistake them
    /// for recorded truth.
    #[serde(default = "inferred_origin")]
    pub origin: RelationOrigin,
}

fn inferred_origin() -> RelationOrigin {
    RelationOrigin::Inferred
}

/// The state axis, mirroring the engine's two axes: `coherence` is derived and
/// never negative; `asserted` is a verdict someone reported and may be absent.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ObjectState {
    #[serde(default)]
    pub coherence: Option<f32>,
    #[serde(default)]
    pub asserted: Option<f32>,
    /// Semiotic cell `(domain, mode)`, when pinned.
    #[serde(default)]
    pub cell: Option<(String, String)>,
}

/// One object in the world, projected from whatever store owns it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldObject {
    pub identity: WorldObjectId,
    pub kind: ObjectKind,
    /// Human-readable content: a label, a path, a fragment. Not a copy of the
    /// file body — content lives where it lives; this is what the stream shows.
    pub content: String,
    pub state: ObjectState,
    pub provenance: Provenance,
    #[serde(default)]
    pub relations: Vec<Relation>,
    /// What this object sits inside, if anything.
    #[serde(default)]
    pub containment: Option<WorldObjectId>,
    /// What a user or agent can *do* with this object.
    #[serde(default)]
    pub affordances: Vec<String>,
    /// What must hold for those affordances to be valid.
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<WorldObjectId>,
    /// The embedding, when the store has one. Named by the embedder in
    /// `provenance.source`, never anonymous.
    #[serde(default)]
    pub semantic_position: Option<Vec<f32>>,
    /// Provenance entries in log order: how this object came to be and change.
    #[serde(default)]
    pub temporal_history: Vec<Provenance>,
}

impl WorldObject {
    pub fn new(identity: impl Into<String>, kind: ObjectKind, content: impl Into<String>) -> Self {
        Self {
            identity: identity.into(),
            kind,
            content: content.into(),
            state: ObjectState::default(),
            provenance: Provenance::deterministic("operator:new"),
            relations: Vec::new(),
            containment: None,
            affordances: Vec::new(),
            constraints: Vec::new(),
            dependencies: Vec::new(),
            semantic_position: None,
            temporal_history: Vec::new(),
        }
    }

    pub fn with_provenance(mut self, provenance: Provenance) -> Self {
        self.provenance = provenance;
        self
    }

    /// Builder-style edge with an explicit origin. The plain [`Self::relate`]
    /// marks the edge `Inferred`; use this when the caller knows the edge was
    /// recorded (a user assertion, a deterministic scan) so the view does not
    /// over-claim what it knows.
    pub fn relate_with_origin(
        mut self,
        kind: RelationKind,
        target: impl Into<String>,
        origin: RelationOrigin,
    ) -> Self {
        self.relations.push(Relation {
            kind,
            target: target.into(),
            provenance: None,
            origin,
        });
        self
    }

    pub fn relate(mut self, kind: RelationKind, target: impl Into<String>) -> Self {
        self.relations.push(Relation {
            kind,
            target: target.into(),
            provenance: None,
            origin: RelationOrigin::Inferred,
        });
        self
    }

    pub fn contained_in(mut self, parent: impl Into<String>) -> Self {
        self.containment = Some(parent.into());
        self
    }

    /// Outgoing edges of one kind.
    pub fn relations_of(&self, kind: RelationKind) -> impl Iterator<Item = &Relation> {
        self.relations.iter().filter(move |r| r.kind == kind)
    }

    /// The "why did this appear" line. Always names the route and the source,
    /// and the observation position when there is one.
    pub fn provenance_chain(&self) -> String {
        let route = match self.provenance.route {
            Route::Deterministic => "deterministic",
            Route::Model => "model",
        };
        let mut line = format!("route={route} source={}", self.provenance.source);
        if let Some(seq) = self.provenance.observation_seq {
            line.push_str(&format!(" obs={seq}"));
        }
        line
    }

    /// Build a project object from a directory: the directory is the object,
    /// its immediate children are `Contains` edges. This is the smallest real
    /// world object the TUI can show without a model or a network — the visible
    /// slice of the first sprint.
    ///
    /// Read-only: it never writes, and it skips entries it cannot stat.
    pub fn from_directory(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();
        let display = root.display().to_string();
        let name = root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| display.clone());

        let mut obj = Self::new(format!("fs:{display}"), ObjectKind::Project, name)
            .with_provenance(Provenance::deterministic("operator:fs-scan"));
        obj.affordances = vec!["open".into(), "scan".into(), "edit-content".into()];
        obj.constraints = vec!["read-only view (no writes)".into()];
        if let Some(parent) = root.parent() {
            obj.containment = Some(format!("fs:{}", parent.display()));
        }

        if let Ok(entries) = std::fs::read_dir(root) {
            let mut children: Vec<String> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path().display().to_string())
                .collect();
            children.sort();
            for path in children.iter().take(200) {
                // `origin` is Deterministic: the edge came from an actual
                // directory read, not a similarity projection.
                obj.relations.push(Relation {
                    kind: RelationKind::Contains,
                    target: format!("fs:{path}"),
                    provenance: None,
                    origin: RelationOrigin::Deterministic,
                });
            }
        }
        obj
    }

    /// Project any path: a directory becomes a `Project` (see
    /// [`Self::from_directory`]), a file becomes a `File` whose `Contains`
    /// edges are its deterministic structural symbols. Read-only, offline, no
    /// model — the projection the navigator descends through.
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        if path.is_dir() {
            return Self::from_directory(path);
        }
        let display = path.display().to_string();
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| display.clone());
        let mut obj = Self::new(format!("fs:{display}"), ObjectKind::File, name)
            .with_provenance(Provenance::deterministic("operator:fs-scan"));
        obj.affordances = vec![
            "open".into(),
            "read".into(),
            "symbols".into(),
            "edit-content".into(),
        ];
        obj.constraints = vec!["read-only view (no writes)".into()];
        if !path.is_file() {
            obj.constraints
                .push("not a regular file (stat failed)".into());
        }
        if let Some(parent) = path.parent() {
            obj.containment = Some(format!("fs:{}", parent.display()));
        }
        for line in structural_lines(path) {
            // Symbol edges come from the one structural extractor — a
            // deterministic operator over file bytes, not a similarity guess.
            obj.relations.push(Relation {
                kind: RelationKind::Contains,
                target: symbol_identity(&display, &line),
                provenance: None,
                origin: RelationOrigin::Deterministic,
            });
        }
        obj
    }

    /// The objects one step inside this one: a directory's immediate children,
    /// or a file's structural symbols. Pure, and read from the object's own
    /// `Contains` edges rather than from disk again, so a file is parsed
    /// exactly once — when it is projected. This is what makes the object a
    /// navigation payload rather than a single rendered page.
    pub fn children(&self) -> Vec<WorldObject> {
        if self.kind == ObjectKind::File {
            return self
                .relations_of(RelationKind::Contains)
                .filter_map(|r| r.target.strip_prefix("symbol:"))
                .filter_map(|rest| rest.split_once('#'))
                .map(|(path, structural)| symbol_object(path, structural))
                .collect();
        }
        self.relations_of(RelationKind::Contains)
            .filter_map(|r| r.target.strip_prefix("fs:"))
            .map(Self::from_path)
            .collect()
    }

    /// Project an engine node into a world object. The join between the
    /// engine's stored state and the one object the TUI navigates.
    ///
    /// Edges are *not* added here: they live in the holarchy, not on the node,
    /// so the caller supplies them. This maps what the node owns — identity,
    /// label, the two coherence axes, the pinned cell, the embedder and the
    /// vector.
    pub fn from_coherence_node(node: &CoherenceNode) -> Self {
        let content = node.label.clone().unwrap_or_else(|| node.id.clone());
        let provenance = match &node.embedder {
            Some(e) => Provenance::deterministic(format!("embed:{e}")),
            None => Provenance::deterministic("operator:core-node"),
        };
        let mut obj = Self::new(
            format!("core:node:{}", node.id),
            ObjectKind::Concept,
            content,
        )
        .with_provenance(provenance);
        obj.state.coherence = Some(node.coherence_score);
        obj.state.asserted = node.asserted;
        obj.state.cell = node.cell_pin.clone();
        obj.semantic_position = Some(node.embedding.clone());
        obj
    }

    /// Project an agent run as a world object. Takes plain strings on purpose:
    /// the view type must not depend on the provider runtime (`ai::provider`),
    /// only on what a caller observed about the run. The route is `Model` — an
    /// agent's output is a model's, and the UI must never blur that with a
    /// deterministic operator.
    pub fn agent(id: impl Into<String>, provider: &str, model: &str, status: &str) -> Self {
        let label = match (provider.is_empty(), model.is_empty()) {
            (false, false) => format!("{provider}:{model}"),
            (false, true) => provider.to_string(),
            _ => "agent".to_string(),
        };
        let content = if status.is_empty() {
            label
        } else {
            format!("{label} — {status}")
        };
        let mut obj = Self::new(id, ObjectKind::Agent, content)
            .with_provenance(Provenance::model("agent:run"));
        obj.affordances = vec!["inspect".into(), "abort".into()];
        obj.constraints = vec!["route=model".into(), "requires a provider (network)".into()];
        obj
    }

    /// Plain-text lines for the TUI stream. Kept UI-agnostic on purpose: the
    /// library must not depend on ratatui, and a line list is testable without
    /// a terminal.
    pub fn render(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(self.provenance_chain());
        lines.push(format!("{:?}  {}", self.kind, self.identity).to_lowercase());
        lines.push(self.content.clone());

        let mut state = Vec::new();
        if let Some(c) = self.state.coherence {
            state.push(format!("coherence={c:.2}"));
        }
        if let Some(a) = self.state.asserted {
            state.push(format!("asserted={a:.2}"));
        }
        if let Some((d, m)) = &self.state.cell {
            state.push(format!("cell={d}/{m}"));
        }
        if !state.is_empty() {
            lines.push(format!("state: {}", state.join(" ")));
        }

        if let Some(parent) = &self.containment {
            lines.push(format!("inside: {parent}"));
        }
        for kind in [
            RelationKind::Contains,
            RelationKind::DependsOn,
            RelationKind::Requires,
            RelationKind::Contradicts,
        ] {
            let targets: Vec<&str> = self.relations_of(kind).map(|r| r.target.as_str()).collect();
            if !targets.is_empty() {
                lines.push(format!(
                    "{:?} ({}) -> {}",
                    kind,
                    targets.len(),
                    targets.join(", ")
                ));
            }
        }
        if !self.affordances.is_empty() {
            lines.push(format!("can: {}", self.affordances.join(", ")));
        }
        if !self.constraints.is_empty() {
            lines.push(format!("must: {}", self.constraints.join(", ")));
        }
        lines
    }
}

/// Stable identity for a symbol: the file path plus the structural line the
/// scanner produced. Deduped by the scanner, so it is unique within a file.
fn symbol_identity(path: &str, structural: &str) -> String {
    format!("symbol:{path}#{structural}")
}

/// Materialise one symbol from the identity already stored on its file's
/// `Contains` edge. No disk read: the structural line came from the scanner
/// when the file was projected.
fn symbol_object(path: &str, structural: &str) -> WorldObject {
    let mut sym = WorldObject::new(
        symbol_identity(path, structural),
        ObjectKind::Symbol,
        structural,
    )
    .with_provenance(Provenance::deterministic("operator:structure-scan"));
    sym.containment = Some(format!("fs:{path}"));
    sym.relations.push(Relation {
        kind: RelationKind::DerivedFrom,
        target: format!("fs:{path}"),
        provenance: None,
        origin: RelationOrigin::Deterministic,
    });
    sym.affordances = vec!["open".into()];
    sym
}

/// Reuse the one structural extractor in the crate rather than parsing again
/// here; the scanner already covers Rust, Python, JS/TS, Go, Ruby, Kotlin,
/// Java, Swift, C, Markdown, JSON, TOML/YAML and CSV.
fn structural_lines(path: &Path) -> Vec<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .unwrap_or_default();
    crate::scanner::extract_structural_lines(path, &ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_chain_names_route_source_and_log_position() {
        let obj = WorldObject::new("core:node:7f3a", ObjectKind::Concept, "sled vs postgres")
            .with_provenance(Provenance::deterministic("embed:bge-base").at(1234));
        assert_eq!(
            obj.provenance_chain(),
            "route=deterministic source=embed:bge-base obs=1234"
        );
    }

    #[test]
    fn model_produced_values_are_never_labelled_deterministic() {
        let obj = WorldObject::new("core:node:7f3a", ObjectKind::Concept, "a decision")
            .with_provenance(Provenance::model("model:jev-1.13-free"));
        assert_eq!(
            obj.provenance_chain(),
            "route=model source=model:jev-1.13-free"
        );
    }

    #[test]
    fn relations_are_typed_and_filterable() {
        let obj = WorldObject::new("project:aion", ObjectKind::Project, "aion")
            .relate(RelationKind::Contains, "module:ui")
            .relate(RelationKind::DependsOn, "service:physis")
            .relate(RelationKind::Contains, "module:deck");
        assert_eq!(obj.relations_of(RelationKind::Contains).count(), 2);
        assert_eq!(obj.relations_of(RelationKind::DependsOn).count(), 1);
        assert_eq!(obj.relations_of(RelationKind::Contradicts).count(), 0);
    }

    #[test]
    fn an_unknown_kind_round_trips_instead_of_being_dropped() {
        let obj = WorldObject::new("x:1", ObjectKind::Other("hologram".into()), "new thing");
        let json = serde_json::to_string(&obj).expect("serializes");
        let back: WorldObject = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back, obj);
    }

    #[test]
    fn a_directory_becomes_a_project_with_typed_children() {
        // Cargo runs lib tests from the package root, so `src` exists.
        let obj = WorldObject::from_directory("src");
        assert_eq!(obj.kind, ObjectKind::Project);
        assert_eq!(obj.provenance.route, Route::Deterministic);
        assert!(obj.identity.starts_with("fs:"));
        assert!(obj.relations_of(RelationKind::Contains).count() > 0);
    }

    #[test]
    fn view_identities_parse_back_to_canonical_nav_ids() {
        // The gate: WorldObject is a view over nav identity — every identity
        // the view mints must resolve to the canonical ObjectId it names.
        for id in [
            "core:node:7f3a",
            "fs:src/world_object.rs",
            "concept:budget-planning",
            "agent:hermes",
        ] {
            let oid = parse_object_id(id);
            let (origin, key) = id.split_once(':').expect("test ids carry an origin");
            assert_eq!(oid.origin, origin, "{id}");
            assert_eq!(oid.key, key, "{id}");
        }
    }

    #[test]
    fn scanned_edges_are_deterministic_not_inferred() {
        // The gate: a directory scan and a symbol extraction are recorded
        // facts, so their edges must not read as similarity guesses.
        let obj = WorldObject::from_directory("src");
        let scanned: Vec<&Relation> = obj.relations_of(RelationKind::Contains).collect();
        assert!(!scanned.is_empty());
        assert!(
            scanned.iter().all(|r| r.origin == RelationOrigin::Deterministic),
            "scan edges carry their origin"
        );

        let file = WorldObject::from_path("src/world_object.rs");
        let symbols: Vec<&Relation> = file.relations_of(RelationKind::Contains).collect();
        assert!(!symbols.is_empty());
        assert!(
            symbols.iter().all(|r| r.origin == RelationOrigin::Deterministic),
            "symbol edges carry their origin"
        );

        // …while a bare relate() stays Inferred: an unproven edge must never
        // borrow the grammar of a recorded one.
        let guessed = WorldObject::new("x:1", ObjectKind::Concept, "guess")
            .relate(RelationKind::SimilarTo, "core:node:7f3a");
        assert_eq!(
            guessed.relations_of(RelationKind::SimilarTo).next().unwrap().origin,
            RelationOrigin::Inferred
        );
    }

    #[test]
    fn an_engine_node_projects_to_a_world_object() {
        let node = CoherenceNode {
            id: "7f3a".into(),
            embedding: vec![0.1, 0.2, 0.3],
            coherence_score: 0.42,
            asserted: Some(-1.0),
            label: Some("sled vs postgres".into()),
            cell_pin: Some(("storage".into(), "decision".into())),
            embedder: Some("bge-base".into()),
            ..CoherenceNode::new(vec![0.1, 0.2, 0.3])
        };
        let obj = WorldObject::from_coherence_node(&node);
        assert_eq!(obj.identity, "core:node:7f3a");
        assert_eq!(obj.content, "sled vs postgres");
        assert_eq!(
            obj.provenance_chain(),
            "route=deterministic source=embed:bge-base"
        );
        assert_eq!(obj.state.coherence, Some(0.42));
        assert_eq!(obj.state.asserted, Some(-1.0));
        assert_eq!(obj.state.cell, Some(("storage".into(), "decision".into())));
        assert_eq!(obj.semantic_position, Some(vec![0.1, 0.2, 0.3]));
    }

    #[test]
    fn an_agent_run_is_a_model_route_object_not_a_deterministic_one() {
        let obj = WorldObject::agent("agent:run-1", "openai", "gpt-4o", "running");
        assert_eq!(obj.kind, ObjectKind::Agent);
        assert_eq!(obj.provenance.route, Route::Model);
        assert_eq!(obj.provenance_chain(), "route=model source=agent:run");
        assert_eq!(obj.content, "openai:gpt-4o — running");
        assert!(obj.constraints.iter().any(|c| c == "route=model"));
        assert!(obj.children().is_empty(), "an agent run has no fs children");
    }

    #[test]
    fn render_names_the_route_the_container_and_the_relations() {
        let obj = WorldObject::new("core:node:1", ObjectKind::Concept, "a concept")
            .with_provenance(Provenance::model("model:jev-1.13-free"))
            .relate(RelationKind::DependsOn, "service:physis")
            .contained_in("project:aion");
        let text = obj.render().join("\n");
        assert!(text.contains("route=model"), "{text}");
        assert!(text.contains("inside: project:aion"), "{text}");
        assert!(text.contains("DependsOn (1) -> service:physis"), "{text}");
    }

    #[test]
    fn a_file_projects_with_its_symbols_as_children() {
        // Cargo runs lib tests from the package root, so this file exists.
        let file = WorldObject::from_path("src/world_object.rs");
        assert_eq!(file.kind, ObjectKind::File);
        assert_eq!(file.provenance.route, Route::Deterministic);
        assert_eq!(file.containment.as_deref(), Some("fs:src"));

        let symbols = file.children();
        // The scanner matches unqualified `^struct`/`^fn` only, so this
        // repo's `pub`-qualified declarations do not project as symbols —
        // pinned here so a scanner-coverage change shows up as a test diff,
        // not a silent vocabulary shift.
        assert!(
            !symbols.is_empty(),
            "this repo's extractor finds at least one symbol-bearing line"
        );
        assert!(symbols.iter().all(|s| s.kind == ObjectKind::Symbol));
        assert!(symbols
            .iter()
            .all(|s| s.containment.as_deref() == Some("fs:src/world_object.rs")));
        assert!(symbols.iter().all(|s| s
            .relations_of(RelationKind::DerivedFrom)
            .any(|r| r.target == "fs:src/world_object.rs")));
    }

    #[test]
    fn a_directory_navigates_down_to_a_file_then_to_symbols() {
        let project = WorldObject::from_path("src");
        assert_eq!(project.kind, ObjectKind::Project);

        let file = project
            .children()
            .into_iter()
            .find(|c| c.content == "world_object.rs")
            .expect("module file is a child of the project");
        assert_eq!(file.kind, ObjectKind::File);

        assert!(file
            .children()
            .iter()
            .any(|s| s.content.starts_with("fn ") || s.content.starts_with("struct ")));
    }
}
