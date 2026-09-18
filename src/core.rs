//! Central engine state — coherence graph nodes, certified/isolated branches,
//! dream archive, and PQ-accelerated nearest-neighbour search.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use physis_core::coherence_query::{
    EpistemicQuery, EpistemicQueryResult, FailedPredictionSummary, HypothesisSummary,
};
use physis_core::contradiction::Contradiction;
use physis_core::epistemic::{EpistemicAuditTrail, EpistemicEvent, EpistemicEventType};
use physis_core::explanation::ExplanationReport;
use physis_core::hypothesis::{Hypothesis, HypothesisStatus};
use physis_core::process::ProcessCycle;
use physis_core::provenance::ProvenanceChain;
use physis_core::relation::{RelationType, TypedEdge};

use crate::graph::{HolonicGraph, NodePayload, RawNodeKey};
use crate::models::*;
use crate::quantize::ProductQuantizer;
use crate::trie::DynamicVectorTrie;

/// Bridges the two identity spaces: a coherence node's string id and its
/// holarchy `RawNodeKey` (a packed slotmap key). Without this, holarchy edges
/// (keyed by `RawNodeKey`) can't be mapped back to the labeled wiki nodes, so
/// edges can't be exported (Spanner) or browsed by label (TUI graph-walk).
/// Maps store `RawNodeKey.data` (u64) since `RawNodeKey` isn't `Hash`.
#[derive(Debug, Default)]
pub struct IdBridge {
    coh_to_holon: HashMap<String, u64>,
    holon_to_coh: HashMap<u64, String>,
}

impl IdBridge {
    pub fn link(&mut self, coh_id: &str, raw: RawNodeKey) {
        self.coh_to_holon.insert(coh_id.to_string(), raw.data);
        self.holon_to_coh.insert(raw.data, coh_id.to_string());
    }
    pub fn holon_for(&self, coh_id: &str) -> Option<RawNodeKey> {
        self.coh_to_holon
            .get(coh_id)
            .map(|&data| RawNodeKey { data })
    }
    pub fn coh_for(&self, raw: RawNodeKey) -> Option<&str> {
        self.holon_to_coh.get(&raw.data).map(String::as_str)
    }
    pub fn len(&self) -> usize {
        self.coh_to_holon.len()
    }
    pub fn is_empty(&self) -> bool {
        self.coh_to_holon.is_empty()
    }

    /// Drop both directions of a coherence-id ↔ holarchy mapping.
    pub fn unlink(&mut self, coh_id: &str) {
        if let Some(data) = self.coh_to_holon.remove(coh_id) {
            self.holon_to_coh.remove(&data);
        }
    }
    /// Drop the mapping by its raw holarchy key.
    pub fn unlink_raw(&mut self, raw: RawNodeKey) {
        if let Some(coh) = self.holon_to_coh.remove(&raw.data) {
            self.coh_to_holon.remove(&coh);
        }
    }
}

/// Central engine state — coherence graph, certified/isolated branches, dream archive,
/// and optional PQ-accelerated nearest-neighbour search.
#[derive(Debug)]
pub struct PhysisCore {
    /// Holonic decentralized graph (Phase I).
    pub holarchy: HolonicGraph,
    /// All coherence nodes keyed by ID.
    pub nodes: HashMap<String, CoherenceNode>,
    /// Dynamic vector trie for semantic search.
    pub wiki: DynamicVectorTrie,
    /// Branches that have been certified as coherent clusters.
    pub certified_branches: Vec<CertifiedBranch>,
    /// Branches flagged as low-coherence outliers.
    pub isolated_branches: Vec<IsolatedBranch>,
    /// Archive of dream evaluation results.
    pub dream_archive: Vec<DreamResult>,
    /// Optional product quantizer for ANN search.
    pub quantizer: Option<ProductQuantizer>,
    /// PQ-encoded nodes: node_id → codes.
    pub encoded_nodes: Vec<(String, Vec<u8>)>,
    /// Dimension used when training the quantizer.
    pub quantizer_dim: usize,
    /// label → node id, for O(1) dedup of labeled registrations (e.g. re-scanning
    /// the same dir must not create duplicate nodes). Only labeled nodes are
    /// indexed; unlabeled `register_node_vec` calls are never deduped.
    label_index: HashMap<String, String>,
    /// coherence id ↔ holarchy RawNodeKey. See `IdBridge`.
    pub bridge: IdBridge,
    /// When true, every newly registered node is auto-bridged into the holarchy
    /// (machines share one graph without manual `bridge_node`). Default false.
    pub auto_bridge: bool,
    /// Embedder id stamped onto every node registered here (vector provenance).
    /// Set once at startup via `set_embedder_id`. `None` = unstamped.
    pub embedder_id: Option<String>,
    /// First-class competing hypotheses.
    pub hypotheses: HashMap<String, Hypothesis>,
    /// Active contradictions held in non-destructive tension.
    pub contradictions: Vec<Contradiction>,
    /// Append-only epistemic audit trail of belief shifts.
    pub epistemic_audit: EpistemicAuditTrail,
    /// Provenance chains keyed by entity ID.
    pub provenance_chains: HashMap<String, ProvenanceChain>,
    /// Typed epistemic, causal, and procedural graph edges.
    pub edges: Vec<TypedEdge>,
    /// Industrial and operational process execution cycles.
    pub process_cycles: Vec<ProcessCycle>,
}

/// What an edit does to a node's semiotic cell pin.
///
/// A pin is the editor's testimony about where a node belongs — the same
/// species of claim as `asserted` is about whether it worked. Testimony is
/// retracted explicitly, never erased as a side effect of changing the text,
/// so `Keep` is what an edit that says nothing about the cell means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinEdit {
    /// Leave any existing pin exactly as it is.
    Keep,
    /// Pin (or re-pin) the node to this cell.
    Set(String, String),
    /// Retract the pin — the node's cell goes back to whatever the classifier
    /// derives from its content.
    Clear,
}

/// The observable effect of an "exchange-parts" edit.
#[derive(Debug, Clone, Serialize)]
pub struct NodeEditOutcome {
    pub node_id: String,
    pub label: String,
    /// The semiotic cell the editor pinned (`(domain, mode)`), when any.
    pub pinned_cell: Option<(String, String)>,
    /// The caller's verdict, preserved verbatim across the swap.
    pub asserted: Option<Score>,
    /// Derived density after the new content was re-embedded + scored.
    pub coherence_score: Score,
}

impl PhysisCore {
    /// Create an empty engine core with no nodes, no quantizer.
    pub fn new() -> Self {
        Self {
            holarchy: HolonicGraph::new(),
            nodes: HashMap::new(),
            wiki: DynamicVectorTrie::new(),
            certified_branches: vec![],
            isolated_branches: vec![],
            dream_archive: vec![],
            quantizer: None,
            encoded_nodes: vec![],
            quantizer_dim: 0,
            label_index: HashMap::new(),
            bridge: IdBridge::default(),
            auto_bridge: false,
            embedder_id: None,
            hypotheses: HashMap::new(),
            contradictions: Vec::new(),
            epistemic_audit: EpistemicAuditTrail::new(),
            provenance_chains: HashMap::new(),
            edges: Vec::new(),
            process_cycles: Vec::new(),
        }
    }

    // ── Epistemic & Hypothesis Methods ─────────────────────────────────

    /// Register a hypothesis into the core.
    pub fn register_hypothesis(&mut self, hypothesis: Hypothesis) -> String {
        let id = hypothesis.id.clone();
        self.epistemic_audit.record(
            EpistemicEvent::new(
                EpistemicEventType::HypothesisGenerated,
                &id,
                format!("Hypothesis registered: {}", hypothesis.statement),
            )
            .with_metric(hypothesis.fitness),
        );
        self.hypotheses.insert(id.clone(), hypothesis);
        id
    }

    /// Lookup hypothesis by ID.
    pub fn hypothesis(&self, id: &str) -> Option<&Hypothesis> {
        self.hypotheses.get(id)
    }

    /// Lookup mutable hypothesis by ID.
    pub fn hypothesis_mut(&mut self, id: &str) -> Option<&mut Hypothesis> {
        self.hypotheses.get_mut(id)
    }

    /// Explicitly transition hypothesis status with audit logging.
    pub fn transition_hypothesis(
        &mut self,
        id: &str,
        new_status: HypothesisStatus,
        reason: &str,
        trigger_id: Option<String>,
    ) -> bool {
        if let Some(h) = self.hypotheses.get_mut(id) {
            let prior = format!("{:?}", h.status);
            let posterior = format!("{:?}", new_status);
            h.status = new_status;

            let mut event = EpistemicEvent::new(
                EpistemicEventType::StatusTransition,
                id,
                format!("Transitioned {} -> {}: {}", prior, posterior, reason),
            )
            .with_transition(prior, posterior);
            if let Some(trig) = trigger_id {
                event = event.with_source(trig);
            }
            self.epistemic_audit.record(event);
            true
        } else {
            false
        }
    }

    /// Record a contradiction.
    pub fn record_contradiction(&mut self, contradiction: Contradiction) -> String {
        let id = contradiction.id.clone();
        self.epistemic_audit.record(EpistemicEvent::new(
            EpistemicEventType::ContradictionDetected,
            &id,
            format!(
                "Contradiction detected between '{}' ({}) and '{}' ({})",
                contradiction.claim_a.claim,
                contradiction.claim_a.source,
                contradiction.claim_b.claim,
                contradiction.claim_b.source
            ),
        ));
        self.contradictions.push(contradiction);
        id
    }

    /// Query or initialize provenance chain for an entity.
    pub fn provenance_chain(&mut self, entity_id: &str) -> &mut ProvenanceChain {
        self.provenance_chains
            .entry(entity_id.to_string())
            .or_default()
    }

    /// Add a typed graph edge.
    pub fn add_edge(&mut self, edge: TypedEdge) {
        self.edges.push(edge);
    }

    /// Add a **named** typed link (Phase 16 I6) — the object-model counterpart to
    /// the bare `u32` holarchy edge. `RelationType` is the stable vocabulary
    /// (Supports, Contradicts, Causes, Precedes, …), and the edge gets a stable
    /// id + provenance so downstream surfaces (atlas, lineage, spanner) can walk
    /// it by name and backwards. Returns the edge id.
    ///
    /// `provenance` defaults to `"api"` when `None` (the pre-W1 behavior);
    /// `evidence` is an optional exact support pointer (span/path/ledger id).
    #[allow(clippy::too_many_arguments)]
    pub fn add_typed_edge(
        &mut self,
        source: &str,
        relation: physis_core::relation::RelationType,
        target: &str,
        confidence: f32,
        weight: f32,
        provenance: Option<String>,
        evidence: Option<String>,
    ) -> String {
        let mut edge = physis_core::relation::TypedEdge::new(relation, source, target);
        edge.confidence = confidence.clamp(0.0, 1.0);
        edge.weight = weight;
        edge.provenance = Some(provenance.unwrap_or_else(|| "api".into()));
        edge.evidence = evidence;
        let id = edge.id.clone();
        self.edges.push(edge);
        id
    }

    /// Outgoing typed edges from `id` — forward traversal by named link.
    pub fn edges_from(&self, id: &str) -> Vec<&physis_core::relation::TypedEdge> {
        self.edges.iter().filter(|e| e.source_id == id).collect()
    }

    /// Incoming typed edges to `id` — **inverse traversal**: everything that
    /// cites, causes, or precedes this node, regardless of edge direction.
    pub fn edges_to(&self, id: &str) -> Vec<&physis_core::relation::TypedEdge> {
        self.edges.iter().filter(|e| e.target_id == id).collect()
    }

    /// Reconstruct belief status at timestamp T.
    pub fn reconstruct_belief_at(
        &self,
        subject_id: &str,
        target_time: chrono::DateTime<chrono::Utc>,
    ) -> Option<HypothesisStatus> {
        self.epistemic_audit
            .reconstruct_status_at(subject_id, target_time)
    }

    /// Generate full structured explanation report.
    pub fn full_explanation_report(&self, hypothesis_id: &str) -> Option<ExplanationReport> {
        let hyp = self.hypotheses.get(hypothesis_id)?;
        let prov = self
            .provenance_chains
            .get(hypothesis_id)
            .cloned()
            .unwrap_or_default();

        let mut precedents = Vec::new();
        for node in self.nodes.values() {
            if let Some(verdict) = node.asserted {
                let sim = cosine_sim(&hyp.embedding, &node.embedding);
                if sim > 0.6 {
                    precedents.push(physis_core::explanation::HistoricalPrecedent {
                        case_id: node.id.clone(),
                        description: node
                            .label
                            .clone()
                            .unwrap_or_else(|| "unlabeled case".to_string()),
                        outcome_worked: verdict > 0.0,
                        similarity: sim,
                        context_match: None,
                    });
                }
            }
        }

        let mut report = ExplanationReport {
            subject_id: hyp.id.clone(),
            statement: hyp.statement.clone(),
            status: hyp.status,
            supporting_evidence: hyp.supporting_evidence.clone(),
            contradicting_evidence: hyp.contradicting_evidence.clone(),
            historical_precedents: precedents,
            expected_consequences: hyp.predictions.clone(),
            observed_consequences: hyp
                .predictions
                .iter()
                .filter(|p| p.actual_outcome.is_some())
                .cloned()
                .collect(),
            coherence_profile: hyp.coherence_profile.clone(),
            coherence_score: hyp.coherence,
            fitness_breakdown: hyp.fitness_breakdown.clone(),
            fitness_score: hyp.fitness,
            confidence: hyp.confidence,
            provenance_chain: prov,
            human_readable_summary: String::new(),
        };

        report.human_readable_summary = report.render_ascii();
        Some(report)
    }

    /// Query the coherence and hypothesis state.
    pub fn query_coherence(&self, query: &EpistemicQuery) -> EpistemicQueryResult {
        match query {
            EpistemicQuery::WhyIsBelieved { id } => {
                if let Some(rep) = self.full_explanation_report(id) {
                    EpistemicQueryResult::Explanation(Box::new(rep))
                } else {
                    EpistemicQueryResult::Explanation(Box::new(ExplanationReport {
                        subject_id: id.clone(),
                        statement: "Subject not found".to_string(),
                        status: HypothesisStatus::Isolated,
                        supporting_evidence: Vec::new(),
                        contradicting_evidence: Vec::new(),
                        historical_precedents: Vec::new(),
                        expected_consequences: Vec::new(),
                        observed_consequences: Vec::new(),
                        coherence_profile: physis_core::coherence_dimensions::CoherenceProfile::new(
                        ),
                        coherence_score: 0.0,
                        fitness_breakdown: Default::default(),
                        fitness_score: 0.0,
                        confidence: 0.0,
                        provenance_chain: Default::default(),
                        human_readable_summary: "Subject not found".to_string(),
                    }))
                }
            }
            EpistemicQuery::WhatContradicts { id } => {
                let contradicting_claims: Vec<String> = self
                    .contradictions
                    .iter()
                    .filter(|c| c.claim_a.claim.contains(id) || c.claim_b.claim.contains(id))
                    .map(|c| format!("A: {} vs B: {}", c.claim_a.claim, c.claim_b.claim))
                    .collect();
                let conflicting_edges: Vec<TypedEdge> = self
                    .edges
                    .iter()
                    .filter(|e| {
                        (e.source_id == *id || e.target_id == *id)
                            && e.relation_type == RelationType::Contradicts
                    })
                    .cloned()
                    .collect();
                EpistemicQueryResult::Contradictions {
                    subject_id: id.clone(),
                    contradicting_claims,
                    conflicting_edges,
                }
            }
            EpistemicQuery::WhatHappenedAfterPrediction { id } => {
                let preds = self
                    .hypotheses
                    .get(id)
                    .map(|h| h.predictions.clone())
                    .unwrap_or_default();
                EpistemicQueryResult::PredictionsOutcome {
                    subject_id: id.clone(),
                    predictions: preds,
                }
            }
            EpistemicQuery::SimilarCasesSucceeded { embedding, limit } => {
                let mut matches: Vec<(String, Score, bool)> = self
                    .nodes
                    .values()
                    .filter_map(|n| {
                        if let Some(v) = n.asserted {
                            if v > 0.0 {
                                let sim = cosine_sim(embedding, &n.embedding);
                                return Some((
                                    n.label.clone().unwrap_or_else(|| n.id.clone()),
                                    sim,
                                    true,
                                ));
                            }
                        }
                        None
                    })
                    .collect();
                matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                matches.truncate(*limit);
                EpistemicQueryResult::SimilarCases { cases: matches }
            }
            EpistemicQuery::SimilarCasesFailed { embedding, limit } => {
                let mut matches: Vec<(String, Score, bool)> = self
                    .nodes
                    .values()
                    .filter_map(|n| {
                        if let Some(v) = n.asserted {
                            if v < 0.0 {
                                let sim = cosine_sim(embedding, &n.embedding);
                                return Some((
                                    n.label.clone().unwrap_or_else(|| n.id.clone()),
                                    sim,
                                    false,
                                ));
                            }
                        }
                        None
                    })
                    .collect();
                matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                matches.truncate(*limit);
                EpistemicQueryResult::SimilarCases { cases: matches }
            }
            EpistemicQuery::WhyChangedInterpretation { id } => {
                let (status, revs) = self
                    .hypotheses
                    .get(id)
                    .map(|h| (h.status, h.revision_history.clone()))
                    .unwrap_or((HypothesisStatus::Isolated, Vec::new()));
                EpistemicQueryResult::RevisionHistory {
                    subject_id: id.clone(),
                    current_status: status,
                    revisions: revs,
                }
            }
            EpistemicQuery::InsufficientOntologyConcepts => {
                let gaps: Vec<String> = self
                    .isolated_branches
                    .iter()
                    .flat_map(|b| b.node_ids.clone())
                    .collect();
                EpistemicQueryResult::OntologyGaps {
                    uncovered_cells: gaps,
                    candidate_domains: Vec::new(),
                }
            }
            EpistemicQuery::StrongestHypotheses { limit } => {
                let mut hyps: Vec<HypothesisSummary> = self
                    .hypotheses
                    .values()
                    .map(|h| HypothesisSummary {
                        id: h.id.clone(),
                        statement: h.statement.clone(),
                        status: h.status,
                        fitness: h.fitness,
                        coherence: h.coherence,
                        confidence: h.confidence,
                        evidence_count: h.evidence_count(),
                        predictions_count: h.predictions.len(),
                    })
                    .collect();
                hyps.sort_by(|a, b| {
                    b.fitness
                        .partial_cmp(&a.fitness)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                hyps.truncate(*limit);
                EpistemicQueryResult::RankedHypotheses { hypotheses: hyps }
            }
            EpistemicQuery::FailedPredictions { limit } => {
                let mut failed_preds = Vec::new();
                for h in self.hypotheses.values() {
                    for p in &h.predictions {
                        if p.correct == Some(false) {
                            failed_preds.push(FailedPredictionSummary {
                                hypothesis_id: h.id.clone(),
                                hypothesis_statement: h.statement.clone(),
                                prediction_statement: p.statement.clone(),
                                expected_outcome: p.expected_outcome.clone(),
                                actual_outcome: p.actual_outcome.clone(),
                                made_at: p.made_at,
                                observed_at: p.observed_at,
                            });
                        }
                    }
                }
                failed_preds.truncate(*limit);
                EpistemicQueryResult::FailedPredictionsList {
                    failures: failed_preds,
                }
            }
            EpistemicQuery::HighestEmpiricalFitness { limit } => {
                let mut hyps: Vec<HypothesisSummary> = self
                    .hypotheses
                    .values()
                    .map(|h| HypothesisSummary {
                        id: h.id.clone(),
                        statement: h.statement.clone(),
                        status: h.status,
                        fitness: h.fitness_breakdown.empirical_support,
                        coherence: h.coherence,
                        confidence: h.confidence,
                        evidence_count: h.evidence_count(),
                        predictions_count: h.predictions.len(),
                    })
                    .collect();
                hyps.sort_by(|a, b| {
                    b.fitness
                        .partial_cmp(&a.fitness)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                hyps.truncate(*limit);
                EpistemicQueryResult::RankedHypotheses { hypotheses: hyps }
            }
        }
    }

    /// Stamp the embedder id onto every node registered after this call (vector
    /// provenance). Call once at startup with the loaded embedder's kind.
    pub fn set_embedder_id(&mut self, id: impl Into<String>) {
        self.embedder_id = Some(id.into());
    }

    /// Join every newly registered coherence node into the holarchy
    /// automatically (see `auto_bridge`). The web server enables this at
    /// startup so the live graph grows without per-write bridge calls.
    pub fn set_auto_bridge(&mut self, on: bool) {
        self.auto_bridge = on;
    }

    /// Assert a relation between two coherence nodes. Both are bridged into the
    /// holarchy (idempotent), then a holarchy edge is added between their keys.
    /// This is how a machine's reducer composes a graph — not just nodes, but
    /// the relations between them. Returns false if either id is unknown.
    pub fn add_edge_between(
        &mut self,
        coh_a: &str,
        coh_b: &str,
        weight: f32,
        semantic_type: u32,
    ) -> bool {
        let (Some(a), Some(b)) = (self.bridge_node(coh_a), self.bridge_node(coh_b)) else {
            return false;
        };
        self.holarchy.edges.push(crate::graph::Edge {
            source: a,
            target: b,
            weight,
            semantic_type,
        });
        true
    }

    /// Ensure the coherence node `coh_id` has a corresponding holarchy node and
    /// is linked to it; returns the holarchy key. Idempotent — a second call
    /// returns the existing key without adding a duplicate holarchy node. This
    /// is the one place the two identity spaces get joined, so edges and durable
    /// rows can share a stable id. Returns `None` if `coh_id` is unknown.
    pub fn bridge_node(&mut self, coh_id: &str) -> Option<RawNodeKey> {
        if let Some(raw) = self.bridge.holon_for(coh_id) {
            return Some(raw);
        }
        let embedding = self.nodes.get(coh_id)?.embedding.clone();
        let payload = NodePayload {
            activation_energy: 1.0,
            decay_metrics: 0.0,
            prune_flag: 0,
            compress_flag: 0,
            _padding: [0; 2],
        };
        let key = self.holarchy.add_node(payload, embedding);
        let raw = RawNodeKey::from(key);
        self.bridge.link(coh_id, raw);
        Some(raw)
    }

    /// Holarchy edges mapped back to coherence ids via the bridge. Endpoints not
    /// yet bridged resolve to `None`. Lets clients export edges with stable ids
    /// (Spanner) or render a label-walkable graph instead of opaque keys.
    pub fn resolved_edges(&self) -> Vec<(Option<String>, Option<String>, f32)> {
        self.holarchy
            .edges
            .iter()
            .map(|e| {
                (
                    self.bridge.coh_for(e.source).map(str::to_string),
                    self.bridge.coh_for(e.target).map(str::to_string),
                    e.weight,
                )
            })
            .collect()
    }

    /// Create an engine core pre-populated with an existing wiki trie.
    pub fn with_wiki(wiki: DynamicVectorTrie) -> Self {
        let mut g = Self::new();
        g.wiki = wiki;
        g
    }

    /// Enable PQ compression for ANN search. Call once with the vector dimension.
    pub fn enable_quantizer(&mut self, dim: usize) {
        self.quantizer = Some(ProductQuantizer::new(dim));
        self.quantizer_dim = dim;
        self.encoded_nodes.clear();
    }

    /// Register a coherence node from a vector.
    pub fn register_node_vec(&mut self, embedding: Vec<f32>) -> String {
        self.register_node_vec_labeled(embedding, None)
    }

    /// Register a vector, optionally retaining the source text as a browsable label.
    /// Labeled registrations are deduped: re-registering the same label returns the
    /// existing node id instead of creating a duplicate (so re-scanning a dir is
    /// idempotent). Unlabeled registrations are always inserted.
    pub fn register_node_vec_labeled(
        &mut self,
        embedding: Vec<f32>,
        label: Option<String>,
    ) -> String {
        if let Some(ref l) = label {
            if let Some(existing) = self.label_index.get(l) {
                return existing.clone();
            }
        }
        let mut node = CoherenceNode::new(embedding.clone());
        node.label = label.clone();
        node.embedder = self.embedder_id.clone();
        let id = node.id.clone();
        if let Some(l) = label {
            self.label_index.insert(l, id.clone());
        }
        // Train PQ quantizer if enabled
        if let Some(ref mut pq) = self.quantizer {
            pq.train_one(&embedding);
            let codes = pq.quantize(&embedding);
            self.encoded_nodes.push((id.clone(), codes));
        }
        // Insert BEFORE scoring: `update_coherence` looks the node up in
        // `self.nodes` (both branches), so scoring first silently left every
        // node at the 0.0 default and pinned `coherence_index` to 0.0.
        self.nodes.insert(id.clone(), node);
        self.update_coherence(&id);
        // Auto-bridge: when enabled, every new node is joined into the holarchy
        // so machines populate one shared graph without calling bridge_node by
        // hand. Off by default (back-compat with the separate ingest pipeline).
        if self.auto_bridge {
            self.bridge_node(&id);
        }
        id
    }

    /// Register from text (embed first using an embedder). The text is kept as
    /// the node's label so it can be recalled by content later.
    pub fn register_node_from_text(
        &mut self,
        text: &str,
        embedder: &dyn crate::embed::VectorEmbed,
    ) -> String {
        let embedding = embedder.embed(text);
        self.register_node_vec_labeled(embedding, Some(text.to_string()))
    }

    /// Report whether a node actually worked. `score` is clamped to `[-1, 1]`;
    /// returns false if the id is unknown.
    ///
    /// This writes the *asserted* axis and deliberately leaves `coherence_score`
    /// (derived density) alone — a caller's verdict and a geometric measurement
    /// answer different questions, and overwriting one with the other is how the
    /// outcome signal was lost before. Re-asserting an id replaces the verdict,
    /// so a task that later fails can correct an earlier optimistic report.
    pub fn assert_coherence(&mut self, id: &str, score: Score) -> bool {
        match self.nodes.get_mut(id) {
            Some(node) => {
                node.asserted = Some(score.clamp(-1.0, 1.0));
                true
            }
            None => false,
        }
    }

    /// **Exchange-parts edit** — replace a node's *content* while preserving its
    /// *structure*. Because the two are independent, swapping `label` + `embedding`
    /// keeps the node functional everywhere it is referenced: the same `id` is
    /// used by the dedupe index, holarchy edges (`IdBridge`), dreams, PDCA, and
    /// persistence — none of which need to change for the swap to take.
    ///
    /// The caller supplies the re-embedded vector and says what should happen to
    /// the cell pin (see [`PinEdit`] — the default is that a pin *survives* the
    /// edit). The asserted verdict and embedder provenance stamp are preserved;
    /// stale PQ codes for the node are dropped.
    ///
    /// Returns `Err` for an unknown id, else the post-edit outcome.
    pub fn edit_node(
        &mut self,
        id: &str,
        new_label: &str,
        new_embedding: Vec<f32>,
        pin: PinEdit,
        embedder_id: Option<&str>,
    ) -> anyhow::Result<NodeEditOutcome> {
        let node = self
            .nodes
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("unknown node {id}"))?;
        // The dedupe index maps label → id. Swap the alias atomically so the old
        // label stops resolving (re-scan dedupes on the new label, not the dead
        // one) and the new label starts resolving.
        if let Some(old) = node.label.take() {
            if self.label_index.get(&old).map(String::as_str) == Some(id) {
                self.label_index.remove(&old);
            }
        }
        node.label = Some(new_label.to_string());
        node.embedding = new_embedding;
        node.embedder = embedder_id.map(str::to_string);
        match pin {
            // Testimony survives a content change. Someone said "this belongs in
            // FABRICATE×MAINTAIN"; fixing a typo in the label is not a retraction
            // of that claim, so silently dropping it would lose a human judgement
            // to a keystroke.
            PinEdit::Keep => {}
            PinEdit::Set(domain, mode) => node.cell_pin = Some((domain, mode)),
            PinEdit::Clear => node.cell_pin = None,
        }
        self.label_index
            .insert(new_label.to_string(), id.to_string());
        // Drop any stale PQ codes for this node so DBANN never reads the old vector.
        self.encoded_nodes.retain(|(nid, _)| nid != id);
        self.update_coherence(id);
        let node = &self.nodes[id];
        Ok(NodeEditOutcome {
            node_id: id.to_string(),
            label: node.label.clone().unwrap_or_default(),
            pinned_cell: node.cell_pin.clone(),
            asserted: node.asserted,
            coherence_score: node.coherence_score,
        })
    }

    /// Remove a node and every structure that references it: the label dedupe
    /// index, the holarchy bridge, the holarchy node + its edges, any PQ
    /// codes, and any typed edges touching it. The caller is responsible for
    /// removing the persisted sled rows.
    /// Returns false when the id is unknown.
    pub fn delete_node(&mut self, id: &str) -> bool {
        let Some(node) = self.nodes.remove(id) else {
            return false;
        };
        if let Some(label) = node.label {
            if self.label_index.get(&label).map(String::as_str) == Some(id) {
                self.label_index.remove(&label);
            }
        }
        if let Some(raw) = self.bridge.holon_for(id) {
            self.holarchy.nodes.remove(raw.into());
            self.holarchy.embeddings.remove(raw.into());
            self.holarchy
                .edges
                .retain(|e| e.source.data != raw.data && e.target.data != raw.data);
            self.bridge.unlink_raw(raw);
        }
        self.encoded_nodes.retain(|(nid, _)| nid != id);
        self.edges
            .retain(|e| e.source_id != id && e.target_id != id);
        true
    }

    /// Delete every node whose label is `label` or begins with `label → `.
    /// Used by watch/ingest when a file is removed: its nodes were registered
    /// under the structural path (`dir → file`) plus one per structural line
    /// (`dir → file → fn`), so a single prefix sweep drops them all. Returns
    /// the removed ids so callers can drop the persisted sled rows (the nodes
    /// themselves are already gone from memory / holarchy / PQ / dedupe index).
    pub fn delete_nodes_by_label_prefix(&mut self, label: &str) -> Vec<String> {
        let prefix = format!("{label} → ");
        let ids: Vec<String> = self
            .nodes
            .iter()
            .filter(|(_, n)| {
                n.label
                    .as_deref()
                    .is_some_and(|l| l == label || l.starts_with(&prefix))
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in &ids {
            self.delete_node(id);
        }
        ids
    }

    /// Mean of the reported verdicts, or `None` when nothing has been judged.
    ///
    /// Distinct from `coherence_index`, which averages derived density over
    /// *every* node: unjudged nodes must not read as failures, so they are
    /// excluded rather than counted as zero.
    pub fn asserted_index(&self) -> Option<Score> {
        let judged: Vec<Score> = self.nodes.values().filter_map(|n| n.asserted).collect();
        if judged.is_empty() {
            return None;
        }
        Some(judged.iter().sum::<Score>() / judged.len() as Score)
    }

    /// Nodes someone has reported as not working, worst first.
    pub fn asserted_failures(&self) -> Vec<&CoherenceNode> {
        let mut out: Vec<&CoherenceNode> = self
            .nodes
            .values()
            .filter(|n| n.is_asserted_failure())
            .collect();
        out.sort_by(|a, b| {
            a.asserted
                .unwrap_or(0.0)
                .partial_cmp(&b.asserted.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }

    /// sled key for one coherence node: `coh:<uid>:<node_id>`.
    /// `pub(crate)` so the web layer can remove the persisted row on delete.
    pub(crate) fn node_key(uid: &str, id: &str) -> String {
        format!("coh:{uid}:{id}")
    }

    /// sled key for one typed edge: `edge:<uid>:<edge_id>`. Edges carry no
    /// vectors, so they load regardless of the embedder (unlike nodes).
    /// `pub(crate)` so the web layer can remove the persisted row on delete.
    pub(crate) fn edge_key(uid: &str, id: &str) -> String {
        format!("edge:{uid}:{id}")
    }

    /// Persist a single typed edge so it survives a restart. No-op for an
    /// unknown id.
    pub fn persist_edge(&self, db: &sled::Db, uid: &str, id: &str) -> anyhow::Result<()> {
        if let Some(edge) = self.edges.iter().find(|e| e.id == id) {
            db.insert(Self::edge_key(uid, id), serde_json::to_vec(edge)?)?;
        }
        Ok(())
    }

    /// Rehydrate a tenant's persisted typed edges into this core. Returns
    /// `(loaded, skipped)`. Edges whose endpoints are absent (nodes deleted
    /// or never loaded) are skipped — an edge to nothing fails loudly at
    /// write time and must not resurrect silently at load time.
    pub fn load_edges(&mut self, db: &sled::Db, uid: &str) -> (usize, usize) {
        let (mut loaded, mut skipped) = (0, 0);
        for item in db.scan_prefix(Self::edge_key(uid, "")) {
            let Ok((_, raw)) = item else { continue };
            let Ok(edge) = serde_json::from_slice::<TypedEdge>(&raw) else {
                skipped += 1;
                continue;
            };
            if !self.nodes.contains_key(&edge.source_id)
                || !self.nodes.contains_key(&edge.target_id)
            {
                skipped += 1;
                continue;
            }
            if self.edges.iter().any(|e| e.id == edge.id) {
                continue;
            }
            self.edges.push(edge);
            loaded += 1;
        }
        (loaded, skipped)
    }

    /// Drop every typed edge touching `id` from memory. The caller removes
    /// the persisted sled rows. Returns the dropped edge ids.
    pub fn delete_edges_for_node(&mut self, id: &str) -> Vec<String> {
        let ids: Vec<String> = self
            .edges
            .iter()
            .filter(|e| e.source_id == id || e.target_id == id)
            .map(|e| e.id.clone())
            .collect();
        self.edges
            .retain(|e| e.source_id != id && e.target_id != id);
        ids
    }

    /// Persist a single node so it survives a restart. No-op for an unknown id.
    ///
    /// One key per node, not one blob per tenant (the `OeeBoard`/`LifeOSTracker`
    /// pattern): a board holds a bounded set of machines, but the wiki grows
    /// without limit, and re-serialising every node on every registration would
    /// make a scan quadratic.
    pub fn persist_node(&self, db: &sled::Db, uid: &str, id: &str) -> anyhow::Result<()> {
        if let Some(node) = self.nodes.get(id) {
            db.insert(Self::node_key(uid, id), serde_json::to_vec(node)?)?;
        }
        Ok(())
    }

    /// Rehydrate a tenant's persisted nodes into this core. Returns
    /// `(loaded, skipped)`.
    ///
    /// A node whose `embedder` provenance disagrees with this core's is
    /// **skipped**: its vector was produced by a different model, so admitting it
    /// would let cosine compare across two embedding spaces and quietly corrupt
    /// every coherence score and search result. Unstamped (`None`) nodes predate
    /// provenance tracking and are accepted.
    pub fn load_nodes(&mut self, db: &sled::Db, uid: &str) -> (usize, usize) {
        let (mut loaded, mut skipped) = (0, 0);
        for item in db.scan_prefix(Self::node_key(uid, "")) {
            let Ok((_, raw)) = item else { continue };
            let Ok(node) = serde_json::from_slice::<CoherenceNode>(&raw) else {
                skipped += 1;
                continue;
            };
            if node.embedder.is_some() && node.embedder != self.embedder_id {
                skipped += 1;
                continue;
            }
            if let Some(label) = node.label.clone() {
                self.label_index.insert(label, node.id.clone());
            }
            self.nodes.insert(node.id.clone(), node);
            loaded += 1;
        }
        (loaded, skipped)
    }

    /// Recall labeled coherence nodes by similarity to `query`. Embeddings are
    /// L2-normalized, so cosine reduces to a dot product. Returns up to `max`
    /// matches as `(id, label, score)`, highest score first. Unlabeled nodes are
    /// skipped. This is how a machine recalls what the shared graph already holds
    /// — across every domain that registered into it.
    pub fn search_nodes(&self, query: &[f32], max: usize) -> Vec<(String, String, Score)> {
        let mut scored: Vec<(String, String, Score)> = self
            .nodes
            .values()
            .filter_map(|n| {
                let label = n.label.as_ref()?;
                let dot: f32 = n.embedding.iter().zip(query).map(|(a, b)| a * b).sum();
                Some((n.id.clone(), label.clone(), dot))
            })
            .collect();
        scored.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max);
        scored
    }

    /// Find all nodes that share conceptual similarity with `target_id`.
    /// Returns `Vec<ConceptualInstance>` — lightweight projections with id,
    /// label, score, origin path, and cell. This powers the "see all instances"
    /// feature in the Studio UI.
    pub fn find_conceptual_instances(
        &self,
        target_id: &str,
        classifier: &crate::classify::CellClassifier,
        threshold: Score,
        limit: usize,
    ) -> Vec<crate::models::ConceptualInstance> {
        let target = match self.nodes.get(target_id) {
            Some(n) => n,
            None => return Vec::new(),
        };
        let target_cell = target
            .cell_pin
            .clone()
            .or_else(|| {
                classifier
                    .classify(&target.embedding)
                    .first()
                    .map(|c| (c.domain.clone(), c.mode.clone()))
            });

        let mut results: Vec<crate::models::ConceptualInstance> = self
            .nodes
            .values()
            .filter(|n| n.id != target_id)
            .filter_map(|n| {
                let label = n.label.as_ref()?;
                let sim: f32 = target.embedding.iter().zip(&n.embedding).map(|(a, b)| a * b).sum();

                // Determine this node's cell
                let node_cell = n.cell_pin.clone().or_else(|| {
                    classifier.classify(&n.embedding)
                        .first()
                        .map(|c| (c.domain.clone(), c.mode.clone()))
                });

                // Same cell with a minimum similarity floor to avoid hash-based noise.
                // The floor is higher than the typical collision rate (~0.20 for
                // 32-dim random projection with shared tokens).
                let same_cell = target_cell.as_ref().is_some_and(|tc| {
                    node_cell.as_ref().map(|nc| nc == tc).unwrap_or(false)
                });

                // Include if cosine similarity exceeds threshold OR they share
                // a cell (with a minimum similarity floor to avoid hash-based noise).
                let min_floor = 0.30; // floor for same-cell inclusion
                if sim < threshold && !(same_cell && sim >= min_floor) {
                    return None;
                }

                // Determine origin path from label if it looks like a file path
                let origin = if label.contains('/') || label.contains('\\') ||
                             label.starts_with("http") ||
                             std::path::PathBuf::from(label).exists() {
                    Some(label.clone())
                } else {
                    None
                };

                Some(crate::models::ConceptualInstance {
                    id: n.id.clone(),
                    label: label.clone(),
                    score: Some(sim),
                    origin,
                    position: None,
                    cell: node_cell,
                })
            })
            .collect();

        results.sort_by(|a, b| {
            b.score.unwrap_or(0.0).partial_cmp(&a.score.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        results
    }

    /// Update coherence score for a node (mean cosine to k nearest neighbors).
    fn update_coherence(&mut self, node_id: &str) {
        if self.nodes.len() <= 1 {
            if let Some(node) = self.nodes.get_mut(node_id) {
                node.coherence_score = 1.0;
            }
            return;
        }

        let query = match self.nodes.get(node_id) {
            Some(n) => n.embedding.clone(),
            None => return,
        };

        let k = 5.min(self.nodes.len() - 1);
        let mut sims: Vec<f32> = self
            .nodes
            .values()
            .filter(|n| n.id != node_id)
            .map(|n| cosine_sim(&query, &n.embedding))
            .collect();
        sims.sort_by(|a, b| b.partial_cmp(a).unwrap());
        sims.truncate(k);

        let avg = if sims.is_empty() {
            1.0
        } else {
            sims.iter().sum::<f32>() / sims.len() as f32
        };

        if let Some(node) = self.nodes.get_mut(node_id) {
            node.coherence_score = avg.max(0.0);
        }
    }

    /// High-level holarchy orchestration.
    pub fn tick_holarchy(
        &mut self,
        onnx: &crate::ai::onnx_worker::OnnxHolon,
        rachmaninov: &mut crate::rachmaninov::RachmaninovHolon,
    ) {
        // 1. Dreaming/Pruning
        self.holarchy.dream_cycle(0.85);

        // 2. GNN Context Extraction
        let (reply_tx, reply_rx) = crossbeam_channel::unbounded();

        let mut nodes_for_gnn = Vec::new();
        for (key, payload) in self.holarchy.nodes.iter() {
            if let Some(emb) = self.holarchy.embeddings.get(key) {
                nodes_for_gnn.push((
                    crate::graph::RawNodeKey::from(key).data,
                    emb.clone(),
                    *payload,
                ));
            }
        }

        onnx.send(crate::ai::onnx_worker::OnnxRequest::GnnContext {
            nodes: nodes_for_gnn,
            edges: self.holarchy.edges.clone(),
            reply_to: reply_tx,
        });

        if let Ok(crate::ai::onnx_worker::OnnxResponse::GnnResult { context_vector }) =
            reply_rx.recv()
        {
            // 3. Feed to Rachmaninov — but only once the holarchy holds enough
            // nodes for a similarity to mean anything. QA 8.6: below this
            // threshold the context vector is degenerate and every "drift"
            // detection was noise.
            if self.holarchy.nodes.len() >= 3 {
                rachmaninov.tick(&context_vector);
            }
        }
    }

    /// Register a behavioural vector (convenience: same as register_node_vec).
    pub fn register_behavioural_vector(&mut self, embedding: Vec<f32>) -> String {
        self.register_node_vec(embedding)
    }

    /// Consistency check in vector space.
    pub fn check_consistency(&self, query_embedding: &[f32], threshold: f32) -> ConsistencyResult {
        for node in self.nodes.values() {
            let sim = cosine_sim(query_embedding, &node.embedding);
            if sim > threshold {
                let gap = 1.0 - sim;
                let refutation = ConstructiveRefutation::new(
                    query_embedding.to_vec(),
                    vec![node.id.clone()],
                    "",
                    gap,
                );
                return ConsistencyResult::Conflict(refutation);
            }
        }
        ConsistencyResult::Clean
    }

    /// filter_context: input text → vector. No more qualia text output.
    pub fn filter_context(
        &self,
        raw_input: &str,
        embedder: &dyn crate::embed::VectorEmbed,
    ) -> FilteredContext {
        let embedding = embedder.embed(raw_input);
        let token_estimate = raw_input.split_whitespace().count();
        self.filter_context_embedded(embedding, token_estimate)
    }

    /// Same as `filter_context` but takes a pre-computed embedding, so callers
    /// holding a lock can embed outside it (the embed is the expensive part).
    pub fn filter_context_embedded(
        &self,
        embedding: Vec<f32>,
        token_estimate: usize,
    ) -> FilteredContext {
        let consistency = self.check_consistency(&embedding, 0.85);
        let valid = matches!(consistency, ConsistencyResult::Clean);
        FilteredContext {
            embedding,
            valid,
            token_estimate,
        }
    }

    /// Certify branches: cluster nodes by geometric proximity.
    pub fn certify_branches(&mut self) -> Vec<CertifiedBranch> {
        if self.nodes.len() < 2 {
            return vec![];
        }

        let mut newly_certified = Vec::new();
        let mut visited = std::collections::HashSet::new();

        for (id, node) in &self.nodes {
            if visited.contains(id) {
                continue;
            }

            let mut cluster = vec![id.clone()];
            visited.insert(id.clone());

            for (other_id, other) in &self.nodes {
                if visited.contains(other_id) {
                    continue;
                }
                let sim = cosine_sim(&node.embedding, &other.embedding);
                if sim > 0.7 {
                    cluster.push(other_id.clone());
                    visited.insert(other_id.clone());
                }
            }

            if cluster.len() >= 2 {
                let centroid: Vec<f32> = (0..node.embedding.len())
                    .map(|i| {
                        cluster
                            .iter()
                            .filter_map(|cid| self.nodes.get(cid))
                            .map(|n| n.embedding[i])
                            .sum::<f32>()
                            / cluster.len() as f32
                    })
                    .collect();

                let stability = cluster
                    .iter()
                    .filter_map(|cid| self.nodes.get(cid))
                    .map(|n| n.coherence_score)
                    .sum::<f32>()
                    / cluster.len() as f32;

                let branch = CertifiedBranch {
                    branch_id: uuid::Uuid::new_v4().to_string(),
                    node_ids: cluster,
                    centroid,
                    stability_score: stability,
                };
                newly_certified.push(branch.clone());
                self.certified_branches.push(branch);
            }
        }

        newly_certified
    }

    /// Detect outliers: nodes with low coherence relative to their nearest cluster.
    pub fn detect_contradictions(&mut self) -> Vec<IsolatedBranch> {
        let mut isolated = Vec::new();
        let threshold = 0.3;

        for (id, node) in &self.nodes {
            if node.coherence_score < threshold {
                let branch = IsolatedBranch {
                    branch_id: uuid::Uuid::new_v4().to_string(),
                    node_ids: vec![id.clone()],
                    outlier_score: 1.0 - node.coherence_score,
                };
                isolated.push(branch.clone());
                self.isolated_branches.push(branch);
            }
        }

        isolated
    }

    /// PQ-based approximate nearest neighbors.
    /// Returns (node_id, approximate_squared_distance) sorted ascending.
    /// Falls back to empty vec if quantizer is not enabled or not trained.
    pub fn pq_find_neighbors(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        let pq = match &self.quantizer {
            Some(pq) if pq.is_trained() && !self.encoded_nodes.is_empty() => pq,
            _ => return vec![],
        };
        let mut results = pq.adc_search(query, &self.encoded_nodes);
        results.truncate(k);
        results
    }

    /// Compress logs into dense causal rules (unchanged).
    pub fn compress_logs(&self, raw_logs: &[String]) -> String {
        let mut rules = Vec::new();
        for log in raw_logs {
            let cleaned: Vec<&str> = log.split_whitespace().filter(|w| w.len() > 2).collect();
            if !cleaned.is_empty() {
                rules.push(cleaned.join(" "));
            }
        }
        let mut seen = std::collections::HashSet::new();
        rules.retain(|r| seen.insert(r.clone()));
        rules.truncate(200);
        rules.join("│")
    }

    /// Dream simulation over the nodes worth replaying.
    ///
    /// A reported failure is always replayed, however dense its neighbourhood —
    /// that is the point of the loop. Nodes nobody has judged fall back to the
    /// geometric proxy (sparse neighbourhood = suspicious), and a node reported
    /// as *working* is skipped even if it sits alone in the space, because a
    /// caller's verdict outranks a guess drawn from how typical it looks.
    pub fn dream(&mut self) -> Vec<DreamResult> {
        let low_coherence: Vec<CoherenceNode> = self
            .nodes
            .values()
            .filter(|n| match n.rating() {
                Some(CoherenceRating::Failure) | Some(CoherenceRating::Inert) => true,
                Some(CoherenceRating::Success) => false,
                None => n.coherence_score < 0.5,
            })
            .cloned()
            .collect();

        let mut results = Vec::new();
        for node in &low_coherence {
            if self
                .dream_archive
                .iter()
                .any(|d| d.nodes_tested.contains(&node.id))
            {
                continue;
            }

            let outcome = if node.coherence_score < 0.2 { 0.0 } else { 1.0 };
            let result = DreamResult {
                dream_id: uuid::Uuid::new_v4().to_string(),
                nodes_tested: vec![node.id.clone()],
                outcome,
                prevented_failure: outcome < 0.5,
                coherence_delta: node.coherence_score,
            };
            results.push(result.clone());
            self.dream_archive.push(result);
        }
        results
    }

    /// Mean coherence across all nodes.
    pub fn coherence_index(&self) -> Score {
        if self.nodes.is_empty() {
            return 1.0;
        }
        let sum: Score = self.nodes.values().map(|n| n.coherence_score).sum();
        sum / self.nodes.len() as Score
    }

    pub fn snapshot(&self) -> CoherenceSnapshot {
        let total = self.nodes.len();
        let high = self
            .nodes
            .values()
            .filter(|n| n.coherence_score > 0.7)
            .count();
        let mid = self
            .nodes
            .values()
            .filter(|n| n.coherence_score > 0.3 && n.coherence_score <= 0.7)
            .count();
        let low = self
            .nodes
            .values()
            .filter(|n| n.coherence_score <= 0.3)
            .count();
        let rated = |want: CoherenceRating| {
            self.nodes
                .values()
                .filter(|n| n.rating() == Some(want))
                .count()
        };
        CoherenceSnapshot {
            total_nodes: total,
            high_coherence: high,
            mid_coherence: mid,
            low_coherence: low,
            certified_branches_count: self.certified_branches.len(),
            isolated_branches_count: self.isolated_branches.len(),
            dream_cycle_count: self.dream_archive.len(),
            coherence_index: self.coherence_index(),
            cluster_count: self.certified_branches.len(),
            outlier_count: self.isolated_branches.len(),
            asserted_success: rated(CoherenceRating::Success),
            asserted_inert: rated(CoherenceRating::Inert),
            asserted_failure: rated(CoherenceRating::Failure),
            asserted_index: self.asserted_index(),
        }
    }
}

impl Default for PhysisCore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
/// Result of a consistency check: no conflict, or a constructive refutation.
pub enum ConsistencyResult {
    /// No conflicting nodes found above the threshold.
    Clean,
    /// A conflict was detected with a suggested refutation.
    Conflict(ConstructiveRefutation),
}

/// Snapshot of coherence health across the node graph, broken into bands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceSnapshot {
    pub total_nodes: usize,
    pub high_coherence: usize,
    pub mid_coherence: usize,
    pub low_coherence: usize,
    pub certified_branches_count: usize,
    pub isolated_branches_count: usize,
    pub dream_cycle_count: usize,
    /// Mean *derived* density over every node. Never negative.
    pub coherence_index: Score,
    pub cluster_count: usize,
    pub outlier_count: usize,
    /// Counts on the *asserted* axis. Nodes nobody has judged appear in none of
    /// these — "unknown" must not read as "inert".
    #[serde(default)]
    pub asserted_success: usize,
    #[serde(default)]
    pub asserted_inert: usize,
    #[serde(default)]
    pub asserted_failure: usize,
    /// Mean verdict over judged nodes only; `None` when nothing is judged.
    #[serde(default)]
    pub asserted_index: Option<Score>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::{RandomProjectionEmbedder, VectorEmbed};

    fn fixture_embedder() -> RandomProjectionEmbedder {
        RandomProjectionEmbedder::new(32)
    }

    fn fixture_core() -> PhysisCore {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        g.register_node_from_text("exercise running success", &emb);
        g.register_node_from_text("diet no sugar success", &emb);
        g.register_node_from_text("compile physis core", &emb);
        g
    }

    #[test]
    fn test_filter_context() {
        let g = fixture_core();
        let emb = fixture_embedder();
        let result = g.filter_context("swimming is good exercise", &emb);
        assert!(result.valid);
        assert_eq!(result.embedding.len(), 32);
    }

    #[test]
    fn test_register_node_returns_id() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let id = g.register_node_from_text("test node", &emb);
        assert!(g.nodes.contains_key(&id));
    }

    #[test]
    fn test_search_nodes_ranks_by_similarity() {
        let g = fixture_core();
        let emb = fixture_embedder();
        let q = emb.embed("exercise running success");
        let hits = g.search_nodes(&q, 3);
        assert_eq!(hits.len(), 3, "all labeled nodes returned, capped at max");
        // Exact-text node ranks first (cosine 1.0 to itself).
        assert_eq!(hits[0].1, "exercise running success");
        // Sorted descending by score.
        assert!(hits[0].2 >= hits[1].2 && hits[1].2 >= hits[2].2);
        // max caps the result count.
        assert_eq!(g.search_nodes(&q, 1).len(), 1);
    }

    #[test]
    fn test_labeled_registration_is_deduped() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        // Same label twice (e.g. re-scanning the same file) → one node, same id.
        let id1 = g.register_node_from_text("src/main.rs", &emb);
        let id2 = g.register_node_from_text("src/main.rs", &emb);
        assert_eq!(id1, id2);
        assert_eq!(g.nodes.len(), 1);
        // A different label adds a distinct node.
        g.register_node_from_text("src/lib.rs", &emb);
        assert_eq!(g.nodes.len(), 2);
        // Unlabeled registrations are never deduped.
        g.register_node_vec(vec![0.1; 32]);
        g.register_node_vec(vec![0.1; 32]);
        assert_eq!(g.nodes.len(), 4);
    }

    #[test]
    fn test_edit_node_swaps_content_keeps_structure() {
        let mut g = PhysisCore::new();
        g.set_embedder_id("rp32");
        g.auto_bridge = true;
        let emb = fixture_embedder();
        let a = g.register_node_from_text("milling spindle bearing", &emb);
        let b = g.register_node_from_text("furnace temperature profile", &emb);
        // Verdict + a holarchy relation referencing the node (structure).
        assert!(g.assert_coherence(&a, -1.0));
        assert!(g.add_edge_between(&a, &b, 0.7, 1));
        let bridge_before = g.bridge.holon_for(&a).expect("node bridged").data;

        // Exchange the content: new label + re-embedded vector, no pin.
        let new_vec = emb.embed("welding robot joint recalibration");
        let out = g
            .edit_node(
                &a,
                "welding robot joint recalibration",
                new_vec,
                PinEdit::Keep,
                Some("rp32"),
            )
            .expect("edit succeeds");

        // Identity + structure survive the swap.
        assert_eq!(out.node_id, a);
        assert_eq!(out.label, "welding robot joint recalibration");
        assert_eq!(
            out.asserted,
            Some(-1.0),
            "verdict preserved across content swap"
        );
        assert_eq!(
            g.nodes[&a].embedder.as_deref(),
            Some("rp32"),
            "provenance re-stamped"
        );
        assert_eq!(
            g.bridge.holon_for(&a).expect("still bridged").data,
            bridge_before,
            "holarchy bridge survives the swap"
        );
        assert_eq!(g.resolved_edges().len(), 1, "edges survive the swap");

        // Dedupe index moved with the label: old label no longer resolves, new one does.
        let dup_old = g.register_node_from_text("milling spindle bearing", &emb);
        assert_ne!(dup_old, a, "old label must not dedupe to the edited node");
        let dup_new = g.register_node_from_text("welding robot joint recalibration", &emb);
        assert_eq!(dup_new, a, "new label dedupes to the edited node");
    }

    #[test]
    fn test_edit_node_unknown_id_errors() {
        let mut g = PhysisCore::new();
        let err = g.edit_node("missing", "x", vec![0.1; 32], PinEdit::Keep, None);
        assert!(err.is_err());
    }

    #[test]
    fn test_delete_nodes_by_label_prefix() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        g.register_node_vec_labeled(emb.embed("a"), Some("src → main.rs".into()));
        g.register_node_vec_labeled(emb.embed("b"), Some("src → main.rs → fn main".into()));
        g.register_node_vec_labeled(emb.embed("c"), Some("src → lib.rs".into()));

        let removed = g.delete_nodes_by_label_prefix("src → main.rs");
        assert_eq!(
            removed.len(),
            2,
            "structural path + its per-line labels drop together"
        );
        assert!(g
            .nodes
            .values()
            .all(|n| n.label.as_deref() != Some("src → main.rs")));
        assert!(g.nodes.len() == 1, "unrelated prefix survives");

        // The label is free to register again (dedupe index cleaned up).
        let again = g.register_node_vec_labeled(emb.embed("d"), Some("src → main.rs".into()));
        assert!(!removed.contains(&again), "deleted label is free again");
    }

    #[test]
    fn test_edit_node_pin_survives_content_edits_until_retracted() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let id = g.register_node_from_text("autoclave cycle log", &emb);
        let pinned = ("FABRICATE".to_string(), "MAINTAIN".to_string());

        let new_vec = emb.embed("autoclave cycle log (edited)");
        let out = g
            .edit_node(
                &id,
                "autoclave cycle log (edited)",
                new_vec,
                PinEdit::Set("FABRICATE".into(), "MAINTAIN".into()),
                None,
            )
            .expect("edit succeeds");
        assert_eq!(out.pinned_cell.as_ref(), Some(&pinned));
        assert_eq!(
            g.nodes[&id].cell_pin.as_ref(),
            Some(&pinned),
            "pin persists on the node"
        );

        // A later edit that says nothing about the cell must NOT drop it. A pin
        // is the editor's testimony about where this belongs; fixing a typo is
        // not a retraction of that claim.
        g.edit_node(
            &id,
            "autoclave cycle log (again)",
            emb.embed("autoclave"),
            PinEdit::Keep,
            None,
        )
        .expect("edit succeeds");
        assert_eq!(
            g.nodes[&id].cell_pin.as_ref(),
            Some(&pinned),
            "an edit with no opinion about the cell leaves the pin standing"
        );

        // Re-pinning replaces.
        g.edit_node(
            &id,
            "autoclave cycle log",
            emb.embed("autoclave"),
            PinEdit::Set("STUDY".into(), "LEARN".into()),
            None,
        )
        .expect("edit succeeds");
        assert_eq!(
            g.nodes[&id].cell_pin.as_ref(),
            Some(&("STUDY".to_string(), "LEARN".to_string()))
        );

        // Retraction is explicit, and only then does the classifier take over.
        g.edit_node(
            &id,
            "autoclave cycle log",
            emb.embed("autoclave"),
            PinEdit::Clear,
            None,
        )
        .expect("edit succeeds");
        assert_eq!(
            g.nodes[&id].cell_pin, None,
            "an explicit Clear retracts the pin"
        );
    }

    #[test]
    fn test_observable_reports_the_pin_over_the_classifier() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let id = g.register_node_from_text("autoclave cycle log", &emb);
        let auto = Some(("STUDY".to_string(), "WORK".to_string()));

        // Unpinned: the caller's auto cell is what shows, and `pinned` says so.
        let view = g.nodes[&id].observable(auto.clone());
        assert_eq!(view.cell, auto);
        assert!(!view.pinned);
        assert_eq!(
            view.label, "autoclave cycle log",
            "the projection carries the text"
        );

        // Pinned: the pin wins over the classifier, and the flag reports which
        // one the caller is looking at. Before this existed `cell_pin` was
        // stored, persisted, and visible from no endpoint at all.
        g.edit_node(
            &id,
            "autoclave cycle log",
            emb.embed("autoclave cycle log"),
            PinEdit::Set("FABRICATE".into(), "MAINTAIN".into()),
            None,
        )
        .expect("edit succeeds");
        let view = g.nodes[&id].observable(auto);
        assert_eq!(
            view.cell,
            Some(("FABRICATE".to_string(), "MAINTAIN".to_string())),
            "the pin outranks the auto-derived cell"
        );
        assert!(view.pinned);
    }

    #[test]
    fn test_edit_node_recomputes_coherence_and_drops_pq_codes() {
        let mut g = PhysisCore::new();
        g.enable_quantizer(32);
        let emb = fixture_embedder();
        let a = g.register_node_from_text("alpha", &emb);
        g.register_node_from_text("beta", &emb);
        g.register_node_from_text("gamma", &emb);
        assert_eq!(g.encoded_nodes.len(), 3, "all nodes PQ-encoded");

        g.edit_node(
            &a,
            "alpha edited",
            emb.embed("alpha edited"),
            PinEdit::Keep,
            None,
        )
        .expect("edit");
        assert_eq!(
            g.encoded_nodes.iter().filter(|(id, _)| id == &a).count(),
            0,
            "stale PQ codes for the edited node are dropped"
        );
        assert!(g.nodes[&a].coherence_score.is_finite());
    }

    #[test]
    fn test_delete_node_removes_all_references() {
        let mut g = PhysisCore::new();
        g.auto_bridge = true;
        let emb = fixture_embedder();
        let a = g.register_node_from_text("alpha", &emb);
        let b = g.register_node_from_text("beta", &emb);
        let c = g.register_node_from_text("gamma", &emb);
        assert!(g.add_edge_between(&a, &b, 0.5, 1));
        assert!(g.add_edge_between(&b, &c, 0.5, 1));

        assert!(g.delete_node(&b));
        assert!(!g.nodes.contains_key(&b), "node removed from the store");
        assert!(!g.delete_node(&b), "second delete is a no-op/false");
        // Holarchy node + edges are gone, bridge unlinked.
        assert!(g.bridge.holon_for(&b).is_none(), "bridge unlinked");
        assert_eq!(
            g.resolved_edges().len(),
            0,
            "edges touching the removed node are gone"
        );
        assert_eq!(g.holarchy.nodes.len(), 2, "holarchy node removed");
        // Label dedupe index no longer resolves the removed label.
        let dup = g.register_node_from_text("beta", &emb);
        assert_ne!(dup, b, "deleted label is free to register again");
    }

    #[test]
    fn test_id_bridge_links_and_resolves_edges() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let a = g.register_node_from_text("alpha", &emb);
        let b = g.register_node_from_text("beta", &emb);

        let ka = g.bridge_node(&a).expect("alpha bridged");
        let kb = g.bridge_node(&b).expect("beta bridged");
        // Idempotent: re-bridging returns the same holarchy key, no new node.
        assert_eq!(g.bridge_node(&a).unwrap().data, ka.data);
        assert_eq!(g.bridge.len(), 2);
        assert_eq!(g.holarchy.nodes.len(), 2);
        assert!(g.bridge_node("does-not-exist").is_none());

        // A holarchy edge between two bridged nodes resolves back to coherence ids.
        g.holarchy.edges.push(crate::graph::Edge {
            source: ka,
            target: kb,
            weight: 0.7,
            semantic_type: 0,
        });
        let resolved = g.resolved_edges();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0.as_deref(), Some(a.as_str()));
        assert_eq!(resolved[0].1.as_deref(), Some(b.as_str()));
        assert_eq!(resolved[0].2, 0.7);
    }

    #[test]
    fn test_auto_bridge_and_edge_write() {
        let mut g = PhysisCore::new();
        g.auto_bridge = true;
        let emb = fixture_embedder();
        let a = g.register_node_from_text("alpha", &emb);
        let b = g.register_node_from_text("beta", &emb);
        // auto-bridged at registration time
        assert!(g.bridge.holon_for(&a).is_some());
        assert!(g.bridge.holon_for(&b).is_some());
        assert_eq!(g.holarchy.nodes.len(), 2);

        // edge-write API composes a relation
        assert!(g.add_edge_between(&a, &b, 0.5, 1));
        assert!(!g.add_edge_between(&a, "missing", 0.5, 1));
        let resolved = g.resolved_edges();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0.as_deref(), Some(a.as_str()));
        assert_eq!(resolved[0].1.as_deref(), Some(b.as_str()));
    }

    #[test]
    fn test_coherence_index_empty() {
        let g = PhysisCore::new();
        assert!((g.coherence_index() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_coherence_index_with_nodes() {
        let g = fixture_core();
        let idx = g.coherence_index();
        assert!(idx >= 0.0);
        assert!(idx <= 1.0);
    }

    #[test]
    fn test_snapshot() {
        let g = fixture_core();
        let snap = g.snapshot();
        assert_eq!(snap.total_nodes, 3);
        assert!(snap.high_coherence > 0 || snap.mid_coherence > 0 || snap.low_coherence > 0);
    }

    #[test]
    fn test_detect_contradictions() {
        let mut g = fixture_core();
        g.detect_contradictions();
    }

    #[test]
    fn test_compress_logs() {
        let g = fixture_core();
        let logs = vec![
            "went for run today felt good".to_string(),
            "studied three hours".to_string(),
        ];
        let compressed = g.compress_logs(&logs);
        assert!(!compressed.is_empty());
        assert!(compressed.contains("went"));
    }

    #[test]
    fn test_register_behavioural_vector() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let vec = emb.embed("morning yoga completed");
        let id = g.register_behavioural_vector(vec);
        assert!(g.nodes.contains_key(&id));
    }

    #[test]
    fn test_registration_scores_coherence() {
        // Regression: `update_coherence` was called before the node was inserted
        // into `self.nodes`, so its lookup missed and every node kept the 0.0
        // default forever — `coherence_index` was pinned at 0.0 and `dream()`
        // (which selects on `score < 0.5`) treated the whole graph as failing.
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();

        let lone = g.register_node_from_text("morning yoga completed", &emb);
        assert_eq!(
            g.nodes[&lone].coherence_score, 1.0,
            "the only node has nothing to disagree with"
        );

        // A near-synonym must land close to it, not at the 0.0 default.
        let near = g.register_node_from_text("completed morning yoga session", &emb);
        assert!(
            g.nodes[&near].coherence_score > 0.0,
            "a scored node must not keep the default 0.0 (got {})",
            g.nodes[&near].coherence_score
        );

        // The index is the mean over nodes, so it moves off 0.0 too.
        assert!(
            g.coherence_index() > 0.0,
            "coherence_index must not be pinned at 0.0"
        );

        // Score is the mean cosine to the k nearest, so it stays a similarity.
        assert!(g.nodes[&near].coherence_score <= 1.0);
    }

    #[test]
    fn test_asserted_axis_is_separate_from_density() {
        // The two axes answer different questions and must not overwrite each
        // other -- collapsing them into one number is what erased the outcome
        // signal before: density is clamped at >= 0, so failure was unsayable.
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let a = g.register_node_from_text("deploy script that always works", &emb);
        let b = g.register_node_from_text("deploy script that always works, but fails", &emb);

        // Unjudged: no verdict, and the index has nothing to average.
        assert_eq!(g.nodes[&a].rating(), None);
        assert_eq!(g.asserted_index(), None);

        let density_before = g.nodes[&b].coherence_score;
        assert!(g.assert_coherence(&b, -1.0));
        assert_eq!(g.nodes[&b].rating(), Some(CoherenceRating::Failure));
        assert!(g.nodes[&b].is_asserted_failure());
        // Density is a measurement, not a verdict -- asserting must not touch it.
        assert_eq!(g.nodes[&b].coherence_score, density_before);
        // ...and the derived index still can't go negative.
        assert!(g.coherence_index() >= 0.0);

        // The asserted index averages judged nodes ONLY. `a` is unjudged, so a
        // single -1.0 verdict must read as -1.0, not be diluted toward zero.
        assert_eq!(g.asserted_index(), Some(-1.0));

        g.assert_coherence(&a, 1.0);
        assert_eq!(g.asserted_index(), Some(0.0)); // mean of +1 and -1

        // Re-asserting corrects a verdict (a task reported at spawn, then failed).
        g.assert_coherence(&a, -0.5);
        assert_eq!(g.nodes[&a].rating(), Some(CoherenceRating::Failure));
        // Out-of-range verdicts are clamped, never stored raw.
        g.assert_coherence(&a, 9.0);
        assert_eq!(g.nodes[&a].asserted, Some(1.0));

        assert!(!g.assert_coherence("no-such-node", 1.0));

        let snap = g.snapshot();
        assert_eq!((snap.asserted_success, snap.asserted_failure), (1, 1));
        assert_eq!(snap.asserted_inert, 0);
    }

    #[test]
    fn test_dream_replays_asserted_failures() {
        // The thesis: the dream loop replays what DIDN'T WORK. A reported
        // failure must be picked up however typical it looks, and a reported
        // success must be left alone however isolated it is -- a caller's
        // verdict outranks a guess drawn from neighbourhood density.
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let failed = g.register_node_from_text("migration that corrupted the index", &emb);
        let worked = g.register_node_from_text("migration that corrupted the index, fixed", &emb);

        g.assert_coherence(&failed, -1.0);
        g.assert_coherence(&worked, 1.0);
        // Force both into the dense band so density alone would skip them.
        g.nodes.get_mut(&failed).unwrap().coherence_score = 0.9;
        g.nodes.get_mut(&worked).unwrap().coherence_score = 0.1;

        let dreams = g.dream();
        let tested: Vec<&String> = dreams.iter().flat_map(|d| &d.nodes_tested).collect();
        assert!(
            tested.contains(&&failed),
            "a reported failure must be replayed"
        );
        assert!(
            !tested.contains(&&worked),
            "a reported success must not be replayed on density alone"
        );
    }

    #[test]
    fn test_nodes_survive_a_restart() {
        // Regression: tenant coherence state was in-memory only, so every node
        // aion/praxis registered was lost on the next deploy.
        let db = sled::Config::new().temporary(true).open().unwrap();
        let emb = fixture_embedder();

        let (id, score) = {
            let mut g = PhysisCore::new();
            g.set_embedder_id("fixture");
            let id = g.register_node_from_text("morning yoga completed", &emb);
            g.register_node_from_text("completed morning yoga session", &emb);
            g.assert_coherence(&id, -1.0); // a verdict must persist with the node
            for n in g.nodes.keys().cloned().collect::<Vec<_>>() {
                g.persist_node(&db, "tenant-a", &n).unwrap();
            }
            (id.clone(), g.nodes[&id].coherence_score)
        }; // drop the core — this is the "restart"

        let mut restored = PhysisCore::new();
        restored.set_embedder_id("fixture");
        let (loaded, skipped) = restored.load_nodes(&db, "tenant-a");
        assert_eq!((loaded, skipped), (2, 0));
        assert_eq!(
            restored.nodes[&id].coherence_score, score,
            "scores survive too"
        );
        assert_eq!(
            restored.nodes[&id].asserted,
            Some(-1.0),
            "the verdict survives too"
        );
        assert_eq!(restored.nodes[&id].rating(), Some(CoherenceRating::Failure));
        assert_eq!(
            restored.nodes[&id].label.as_deref(),
            Some("morning yoga completed")
        );

        // The label index is rebuilt, so dedupe still holds after a restart --
        // otherwise a restart would silently duplicate every re-registered node.
        let again = restored.register_node_from_text("morning yoga completed", &emb);
        assert_eq!(again, id);
        assert_eq!(restored.nodes.len(), 2);

        // Tenants stay isolated: another tenant's prefix sees nothing.
        let mut other = PhysisCore::new();
        assert_eq!(other.load_nodes(&db, "tenant-b"), (0, 0));

        // A core on a different embedder must refuse the vectors: comparing
        // across embedding spaces would corrupt every score silently.
        let mut swapped = PhysisCore::new();
        swapped.set_embedder_id("some-other-model");
        assert_eq!(swapped.load_nodes(&db, "tenant-a"), (0, 2));
    }

    #[test]
    fn test_edges_survive_a_restart_and_die_with_their_node() {
        // W1: typed edges have the same durability as nodes — persist,
        // rehydrate, cascade on delete. Edges carry no vectors, so no
        // embedder-mismatch skip applies.
        use physis_core::relation::RelationType;
        let db = sled::Config::new().temporary(true).open().unwrap();
        let emb = fixture_embedder();

        let (a, b, eid) = {
            let mut g = PhysisCore::new();
            g.set_embedder_id("fixture");
            let a = g.register_node_from_text("press seized bearing", &emb);
            let b = g.register_node_from_text("work-order-41 open", &emb);
            let eid = g.add_typed_edge(
                &a,
                RelationType::Causes,
                &b,
                0.9,
                0.7,
                Some("human".into()),
                Some("ledger:3".into()),
            );
            for n in g.nodes.keys().cloned().collect::<Vec<_>>() {
                g.persist_node(&db, "tenant-a", &n).unwrap();
            }
            g.persist_edge(&db, "tenant-a", &eid).unwrap();
            (a, b, eid)
        };

        let mut restored = PhysisCore::new();
        restored.set_embedder_id("fixture");
        assert_eq!(restored.load_nodes(&db, "tenant-a"), (2, 0));
        assert_eq!(restored.load_edges(&db, "tenant-a"), (1, 0));
        let e = restored.edges.iter().find(|e| e.id == eid).expect("edge");
        assert_eq!(e.relation_type, RelationType::Causes);
        assert_eq!((e.source_id.as_str(), e.target_id.as_str()), (a.as_str(), b.as_str()));
        assert_eq!(e.provenance.as_deref(), Some("human"));
        assert_eq!(e.evidence.as_deref(), Some("ledger:3"));

        // Deleting a node drops its edges in memory; rows go with it.
        let dropped = restored.delete_edges_for_node(&a);
        assert_eq!(dropped, vec![eid.clone()]);
        assert!(restored.edges.is_empty());
        for eid in &dropped {
            let _ = db.remove(PhysisCore::edge_key("tenant-a", eid));
        }
        let mut again = PhysisCore::new();
        again.set_embedder_id("fixture");
        assert_eq!(again.load_nodes(&db, "tenant-a"), (2, 0));
        assert_eq!(again.load_edges(&db, "tenant-a"), (0, 0));

        // `delete_node` itself cascades: edges never outlive an endpoint.
        let mut g = PhysisCore::new();
        g.set_embedder_id("fixture");
        let x = g.register_node_from_text("node x here", &emb);
        let y = g.register_node_from_text("node y there", &emb);
        g.add_typed_edge(&x, RelationType::Supports, &y, 1.0, 1.0, None, None);
        assert!(g.delete_node(&x));
        assert!(g.edges.is_empty(), "typed edges die with their node");

        // Tenant isolation holds for edges too.
        let mut other = PhysisCore::new();
        other.set_embedder_id("fixture");
        assert_eq!(other.load_edges(&db, "tenant-b"), (0, 0));
    }

    #[test]
    fn test_certify_branches() {
        let mut g = fixture_core();
        let certified = g.certify_branches();
        // Should create clusters of similar nodes
        assert!(certified.len() <= 3);
    }

    #[test]
    fn test_pq_integration() {
        let mut g = PhysisCore::new();
        g.enable_quantizer(32);
        let emb = fixture_embedder();
        g.register_node_from_text("exercise running fitness", &emb);
        g.register_node_from_text("diet nutrition health", &emb);
        g.register_node_from_text("coding rust programming", &emb);
        assert!(g.quantizer.as_ref().unwrap().is_trained());
        assert_eq!(g.encoded_nodes.len(), 3);
        let query = emb.embed("morning run");
        let pq_results = g.pq_find_neighbors(&query, 2);
        assert_eq!(pq_results.len(), 2);
        assert!(pq_results[0].1 <= pq_results[1].1); // sorted by distance asc
    }

    #[test]
    fn test_find_conceptual_instances() {
        use crate::classify::CellClassifier;
        use crate::config::OntologyLoader;
        use crate::config::PhysisConfig;
        use crate::embed::RandomProjectionEmbedder;

        let mut g = PhysisCore::new();
        let emb = RandomProjectionEmbedder::new(32);
        let config = PhysisConfig::default();
        let ontology = OntologyLoader::load_all(&config);
        let classifier = CellClassifier::build(&ontology, &emb);

        // Register several related nodes
        let a = g.register_node_from_text("machine bearing check alignment procedure", &emb);
        let b = g.register_node_from_text("bearing alignment calibration routine procedure", &emb);
        let totally_unrelated = g.register_node_from_text("recipe for chocolate cake with butter and sugar flour eggs", &emb);
        let d = g.register_node_from_text("bearing inspection protocol document standard", &emb);

        // With a threshold high enough, unrelated nodes are excluded.
        // (RandomProjectionEmbedder gives hash-based similarity, so we verify
        // the ranking: related nodes score higher than unrelated.)
        let instances_low = g.find_conceptual_instances(&a, &classifier, 0.0, 100);
        // All non-empty-label nodes should be included at threshold 0.0
        assert!(instances_low.iter().any(|i| i.id == b));
        assert!(instances_low.iter().any(|i| i.id == d));
        assert!(instances_low.iter().any(|i| i.id == totally_unrelated));

        // Related instances should rank higher than unrelated
        let b_score = instances_low.iter().find(|i| i.id == b).map(|i| i.score.unwrap_or(0.0));
        let u_score = instances_low.iter().find(|i| i.id == totally_unrelated).map(|i| i.score.unwrap_or(0.0));
        assert!(b_score.is_some() && u_score.is_some(), "both should be found at threshold 0");
        assert!(b_score.unwrap() >= u_score.unwrap(),
            "related score should exceed unrelated: related={}, unrelated={}",
            b_score.unwrap(), u_score.unwrap());

        // With a sufficient threshold, the unrelated node is filtered
        let threshold = b_score.unwrap().min(0.95);
        let instances_high = g.find_conceptual_instances(&a, &classifier, threshold, 100);
        assert!(instances_high.iter().any(|i| i.id == b), "b should still be found at threshold {}", threshold);
        // The unrelated node may or may not be found depending on cosine,
        // but we verify the ranking is stable:
        if let Some(u) = instances_high.iter().find(|i| i.id == totally_unrelated) {
            if let Some(b) = instances_high.iter().find(|i| i.id == b) {
                assert!(b.score.unwrap() >= u.score.unwrap(),
                    "ranking must be stable: b={}, u={}", b.score.unwrap(), u.score.unwrap());
            }
        }

        // Each instance should have a label and score
        for inst in &instances_low {
            assert!(!inst.label.is_empty(), "instance should have a label");
            assert!(inst.score.is_some(), "instance should have a score");
        }

        // Test with non-existent node returns empty
        let empty = g.find_conceptual_instances("nonexistent-id", &classifier, 0.5, 100);
        assert!(empty.is_empty(), "non-existent node should return empty");

        // Verify cell info is populated for instances
        assert!(instances_low.iter().any(|i| i.cell.is_some()), "instances should have cell info");
    }

    #[test]
    fn test_node_fragment_extraction() {
        let emb = fixture_embedder();
        let mut g = PhysisCore::new();
        let id = g.register_node_from_text("Hydraulic press tonnage drift when cold. Check calibration.", &emb);
        let node = &g.nodes[&id];
        let fragment = node.extract_fragment();
        assert!(fragment.is_some(), "should extract a fragment");
        let f = fragment.unwrap();
        assert!(f.contains("Hydraulic press") || f.contains("drift"), "fragment should contain key text");
        assert!(f.len() <= 120, "fragment should be bounded in length");
    }
}
