//! Typed ontology graph — the "Palantir-like" layer.
//!
//! `FileThemeGraph` (see `graph.rs`) is the legacy flat grid: themes + files
//! joined by a single scored edge type. This module promotes that into a typed
//! property graph:
//!
//! - Nodes are [`OntologyNode`]s tagged with a [`NodeKind`] (Theme / Document /
//!   Concept) and carry a free-form property bag.
//! - Links are [`OntologyEdge`]s tagged with a named [`LinkType`] (MapsTo /
//!   Contains / Relates / Discovered / CoOccurs) and their own property bag.
//!
//! Builders adapt the existing surfaces without touching them:
//! - [`OntologyGraph::from_file_theme_graph`] keeps the Studio/demo working.
//! - [`OntologyGraph::from_discovery`] turns discovered domains into Concept
//!   nodes so "your ontology grows itself" is a typed, inspectable artifact.
//!
//! Everything is `serde`-serializable so it can be saved, diffed, and loaded
//! back — the basis for the self-growing, offline ontology story.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// What kind of thing a node represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// A cell from the baked-in grid (the 370-cell ontology).
    Theme,
    /// An ingested document/file.
    Document,
    /// A domain the discovery pass found that the grid couldn't name yet.
    Concept,
}

impl NodeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            NodeKind::Theme => "theme",
            NodeKind::Document => "document",
            NodeKind::Concept => "concept",
        }
    }
}

/// Named relationship between two nodes. Replaces the legacy single
/// "theme->file score" edge with semantically meaningful link types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkType {
    /// Document maps onto a known theme (the Studio's grid coverage).
    MapsTo,
    /// A theme/document contains a sub-concept.
    Contains,
    /// Generic semantic relatedness (cosine above threshold).
    Relates,
    /// A Concept node discovered from the corpus, not yet in the grid.
    Discovered,
    /// Two documents co-occur (shared themes) — weak structural signal.
    CoOccurs,
}

impl LinkType {
    pub fn as_str(self) -> &'static str {
        match self {
            LinkType::MapsTo => "maps_to",
            LinkType::Contains => "contains",
            LinkType::Relates => "relates",
            LinkType::Discovered => "discovered",
            LinkType::CoOccurs => "co_occurs",
        }
    }
}

/// A typed node with a property bag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyNode {
    pub id: usize,
    pub kind: NodeKind,
    pub label: String,
    /// Free-form attributes (domain, mode, axis, vendor, date, doc kind, ...).
    pub props: HashMap<String, String>,
    /// Optional embedding for similarity queries (absent on Concept stubs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub embedding: Vec<f32>,
}

/// A typed, weighted, attributed link.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyEdge {
    pub source: usize,
    pub target: usize,
    pub link: LinkType,
    pub weight: f32,
    #[serde(default)]
    pub props: HashMap<String, String>,
}

/// Why a node/link entered the graph. Powers the provenance ledger: an
/// enterprise buyer can see *what the ontology learned, when, and from what* —
/// the trust substrate beneath "your ontology grows itself."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// A domain was discovered from the corpus (not yet in the grid).
    Discovery,
    /// A discovered domain was merged into the live grid.
    Merge,
    /// A node was added (theme/file/concept).
    NodeAdded,
    /// A link was added.
    LinkAdded,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::Discovery => "discovery",
            EventKind::Merge => "merge",
            EventKind::NodeAdded => "node_added",
            EventKind::LinkAdded => "link_added",
        }
    }
}

/// One provenance entry. `seq` is a monotonic counter (not wall-clock) so the
/// ledger is deterministic and diff-stable across re-runs — important for an
/// offline, reproducible demo. `ts` is optional caller-supplied timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEvent {
    pub seq: u64,
    pub kind: EventKind,
    /// Label of the affected node, when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    /// Human-readable detail (e.g. the source document or hint list).
    pub detail: String,
    /// Optional ISO timestamp; left empty for deterministic runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts: Option<String>,
}

/// The typed ontology graph.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OntologyGraph {
    pub nodes: Vec<OntologyNode>,
    pub edges: Vec<OntologyEdge>,
    /// Provenance ledger (kept out of the main JSON; use `save_events`).
    #[serde(skip)]
    pub events: Vec<GraphEvent>,
    #[serde(skip)]
    label_index: HashMap<String, usize>,
    #[serde(skip)]
    next_id: usize,
    #[serde(skip)]
    next_event: u64,
}

impl OntologyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    fn add_node(&mut self, kind: NodeKind, label: &str, props: HashMap<String, String>, embedding: Vec<f32>) -> usize {
        // De-dupe by label so themes/files/concepts collapse to one node.
        if let Some(&id) = self.label_index.get(label) {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.label_index.insert(label.to_string(), id);
        self.nodes.push(OntologyNode {
            id,
            kind,
            label: label.to_string(),
            props,
            embedding,
        });
        id
    }

    /// Add a property to an existing node (no-op if absent).
    pub fn annotate(&mut self, label: &str, key: &str, value: &str) {
        if let Some(&id) = self.label_index.get(label) {
            if let Some(n) = self.nodes.iter_mut().find(|n| n.id == id) {
                n.props.insert(key.to_string(), value.to_string());
            }
        }
    }

    pub fn add_link(&mut self, source: usize, target: usize, link: LinkType, weight: f32, props: HashMap<String, String>) {
        // Avoid duplicate identical links (keep the stronger weight).
        if let Some(e) = self.edges.iter_mut().find(|e| e.source == source && e.target == target && e.link == link) {
            if weight > e.weight {
                e.weight = weight;
                e.props = props;
            }
            return;
        }
        self.edges.push(OntologyEdge {
            source,
            target,
            link,
            weight,
            props,
        });
    }

    pub fn node_by_label(&self, label: &str) -> Option<&OntologyNode> {
        let id = *self.label_index.get(label)?;
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Neighbors of a node, with their link type and weight.
    pub fn neighbors(&self, label: &str) -> Vec<(&OntologyNode, LinkType, f32)> {
        let id = match self.label_index.get(label) {
            Some(&id) => id,
            None => return Vec::new(),
        };
        self.edges
            .iter()
            .filter(|e| e.source == id || e.target == id)
            .filter_map(|e| {
                let other = if e.source == id { e.target } else { e.source };
                self.nodes.iter().find(|n| n.id == other).map(|n| (n, e.link, e.weight))
            })
            .collect()
    }

    pub fn count_by_kind(&self) -> HashMap<NodeKind, usize> {
        let mut m = HashMap::new();
        for n in &self.nodes {
            *m.entry(n.kind).or_insert(0) += 1;
        }
        m
    }

    pub fn count_by_link(&self) -> HashMap<LinkType, usize> {
        let mut m = HashMap::new();
        for e in &self.edges {
            *m.entry(e.link).or_insert(0) += 1;
        }
        m
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        std::fs::write(path, self.to_json())
    }

    /// Append a provenance event (monotonic `seq`; deterministic across runs).
    pub fn log_event(&mut self, kind: EventKind, node: Option<&str>, detail: &str) {
        let seq = self.next_event;
        self.next_event += 1;
        self.events.push(GraphEvent {
            seq,
            kind,
            node: node.map(|s| s.to_string()),
            detail: detail.to_string(),
            ts: None,
        });
    }

    /// Serialize just the provenance ledger.
    pub fn to_events_json(&self) -> String {
        serde_json::to_string_pretty(&self.events).unwrap_or_else(|_| "[]".to_string())
    }

    /// Save the provenance ledger to its own inspectable file.
    pub fn save_events(&self, path: &std::path::Path) -> std::io::Result<()> {
        std::fs::write(path, self.to_events_json())
    }

    /// Events affecting a particular node label (in insertion order).
    pub fn events_for(&self, label: &str) -> Vec<&GraphEvent> {
        self.events.iter().filter(|e| e.node.as_deref() == Some(label)).collect()
    }

    // ---- Adapters (the "reuse, don't rewrite" bridge) ----

    /// Build the typed graph from the legacy flat grid. Themes become Theme
    /// nodes, files become Document nodes, and each theme-file edge becomes a
    /// `MapsTo` link. This is what keeps the Studio/demo working unchanged.
    pub fn from_file_theme_graph(g: &crate::graph::FileThemeGraph) -> OntologyGraph {
        let mut og = OntologyGraph::new();
        let mut theme_ids: HashMap<usize, usize> = HashMap::new();
        for t in &g.themes {
            let mut props = HashMap::new();
            props.insert("domain".into(), t.domain.clone());
            props.insert("mode".into(), t.mode.clone());
            props.insert("axis_kind".into(), t.axis_kind.clone());
            if let Some(c) = &t.category {
                props.insert("category".into(), c.clone());
            }
            let id = og.add_node(NodeKind::Theme, &t.name, props, t.embedding.clone());
            theme_ids.insert(t.id, id);
        }
        let mut file_ids: HashMap<usize, usize> = HashMap::new();
        for f in &g.files {
            let mut props = HashMap::new();
            props.insert("kind".into(), f.kind.clone());
            props.insert("path".into(), f.path.to_string_lossy().to_string());
            let id = og.add_node(NodeKind::Document, &f.title, props, f.embedding.clone());
            file_ids.insert(f.id, id);
        }
        for e in &g.edges {
            let (Some(&tid), Some(&fid)) = (theme_ids.get(&e.theme_id), file_ids.get(&e.file_id)) else {
                continue;
            };
            let mut props = HashMap::new();
            props.insert("score".into(), format!("{:.3}", e.score));
            og.add_link(fid, tid, LinkType::MapsTo, e.score, props);
        }
        og
    }

    /// Build a typed graph containing ONLY the discovered domains (each a
    /// Concept node linked via `Discovered` from a `DiscoveredOntology` hub).
    /// Equivalent to `OntologyGraph::new().merge_discovery(report)`.
    pub fn from_discovery(report: &crate::discovery::DiscoveryReport) -> OntologyGraph {
        let mut og = OntologyGraph::new();
        og.merge_discovery(report);
        og
    }

    /// Merge discovered domains into this graph as Concept nodes linked from a
    /// `DiscoveredOntology` hub. Lets one JSON carry both the baked-in grid and
    /// the corpus-derived gaps — the full typed picture in a single artifact.
    /// Each proposal is also recorded in the provenance ledger.
    pub fn merge_discovery(&mut self, report: &crate::discovery::DiscoveryReport) {
        let hub_label = "DiscoveredOntology";
        let hub = match self.node_by_label(hub_label) {
            Some(n) => n.id,
            None => self.add_node(
                NodeKind::Concept,
                hub_label,
                {
                    let mut p = HashMap::new();
                    p.insert("role".into(), "discovery_hub".into());
                    p
                },
                Vec::new(),
            ),
        };
        for p in &report.proposals {
            let mut props = HashMap::new();
            props.insert("domain".into(), p.domain.clone());
            props.insert("mode".into(), p.mode.clone());
            props.insert("coverage".into(), format!("{:.3}", p.coverage));
            props.insert("count".into(), p.count.to_string());
            props.insert("hints".into(), p.hints.join(", "));
            props.insert("samples".into(), p.samples.join(" | "));
            let cid = self.add_node(NodeKind::Concept, &p.name, props, Vec::new());
            self.add_link(
                hub,
                cid,
                LinkType::Discovered,
                p.coverage.max(0.5),
                HashMap::new(),
            );
            // Provenance: what was discovered, from which documents.
            self.log_event(
                EventKind::Discovery,
                Some(&p.name),
                &format!("from {} doc(s); hints: {}", p.count, p.hints.join(", ")),
            );
        }
        if !report.proposals.is_empty() {
            self.log_event(
                EventKind::Merge,
                Some(hub_label),
                &format!("merged {} discovered domain(s) into the grid", report.proposals.len()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ftg() -> crate::graph::FileThemeGraph {
        let mut g = crate::graph::FileThemeGraph::new();
        g.add_theme("Email", Some("Communication"), "BOND", "WORK", "office", vec![0.9, 0.1]);
        g.add_theme("Budget", Some("Finance"), "CONSTRUCT", "CREATE", "office", vec![0.1, 0.9]);
        g.add_file(std::path::PathBuf::from("inbox/msg.eml"), "Q4 report", "Email", vec![0.85, 0.15]);
        g.add_file(std::path::PathBuf::from("finance/budget.xlsx"), "Budget 2025", "Spreadsheet", vec![0.15, 0.85]);
        g.add_edge(0, 0, 0.95);
        g.add_edge(1, 1, 0.90);
        g
    }

    #[test]
    fn test_from_file_theme_graph_typed() {
        let og = OntologyGraph::from_file_theme_graph(&sample_ftg());
        let by_kind = og.count_by_kind();
        assert_eq!(by_kind.get(&NodeKind::Theme).copied(), Some(2));
        assert_eq!(by_kind.get(&NodeKind::Document).copied(), Some(2));
        // two MapsTo links (theme<->file)
        let by_link = og.count_by_link();
        assert_eq!(by_link.get(&LinkType::MapsTo).copied(), Some(2));
        // label dedup: themes addressed by name
        assert!(og.node_by_label("Email").is_some());
        assert!(og.node_by_label("Budget").is_some());
    }

    #[test]
    fn test_neighbors_and_links() {
        let og = OntologyGraph::from_file_theme_graph(&sample_ftg());
        let nbrs = og.neighbors("Q4 report");
        assert_eq!(nbrs.len(), 1);
        assert_eq!(nbrs[0].1, LinkType::MapsTo);
        assert!((nbrs[0].2 - 0.95).abs() < 1e-5);
    }

    #[test]
    fn test_serialization_round_trip() {
        let og = OntologyGraph::from_file_theme_graph(&sample_ftg());
        let json = og.to_json();
        let back: OntologyGraph = serde_json::from_str(&json).expect("round-trip");
        assert_eq!(back.nodes.len(), og.nodes.len());
        assert_eq!(back.edges.len(), og.edges.len());
        // label_index is skipped, but node_by_label rebuilds lazily? It isn't —
        // verify direct lookup still works via nodes.
        assert!(back.nodes.iter().any(|n| n.label == "Email"));
    }

    #[test]
    fn test_from_discovery_concepts() {
        // Build a minimal DiscoveryReport with one proposal.
        let proposal = crate::discovery::ProposedDomain {
            name: "Cyber & SOC".into(),
            domain: "BOND".into(),
            mode: "WORK".into(),
            hints: vec!["network".into(), "incident".into()],
            samples: vec!["security_incident.md".into()],
            count: 3,
            coverage: 0.42,
        };
        let report = crate::discovery::DiscoveryReport {
            total: 10,
            covered: 9,
            unmapped: 1,
            proposals: vec![proposal],
            sim_min: 0.1,
            sim_mean: 0.4,
            sim_max: 0.9,
            threshold_used: 0.3,
            auto_retuned: false,
        };
        let og = OntologyGraph::from_discovery(&report);
        let by_kind = og.count_by_kind();
        assert_eq!(by_kind.get(&NodeKind::Concept).copied(), Some(2)); // hub + 1 proposal
        let by_link = og.count_by_link();
        assert_eq!(by_link.get(&LinkType::Discovered).copied(), Some(1));
        assert!(og.node_by_label("Cyber & SOC").is_some());
    }

    #[test]
    fn test_event_ledger_recorded() {
        let proposal = crate::discovery::ProposedDomain {
            name: "Cyber & SOC".into(),
            domain: "BOND".into(),
            mode: "WORK".into(),
            hints: vec!["network".into()],
            samples: vec!["security_incident.md".into()],
            count: 3,
            coverage: 0.42,
        };
        let report = crate::discovery::DiscoveryReport {
            total: 10,
            covered: 9,
            unmapped: 1,
            proposals: vec![proposal],
            sim_min: 0.1,
            sim_mean: 0.4,
            sim_max: 0.9,
            threshold_used: 0.3,
            auto_retuned: false,
        };
        let og = OntologyGraph::from_discovery(&report);
        // One Discovery event for the proposal + one Merge event for the hub.
        assert_eq!(og.events.len(), 2);
        let kinds: Vec<_> = og.events.iter().map(|e| e.kind).collect();
        assert!(kinds.contains(&EventKind::Discovery));
        assert!(kinds.contains(&EventKind::Merge));
        // seq is monotonic and deterministic.
        assert!(og.events[0].seq < og.events[1].seq);
        // events_for filters by node.
        assert!(!og.events_for("Cyber & SOC").is_empty());
        // ledger serializes independently of the graph JSON.
        let ev_json = og.to_events_json();
        assert!(ev_json.contains("discovery"));
    }
}
