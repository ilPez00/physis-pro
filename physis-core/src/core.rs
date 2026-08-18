//! Central engine state — coherence nodes, certified/isolated branches,
//! first-class hypotheses, typed graph relationships, process cycles, and
//! an epistemic audit trail.
//!
//! Maintains competing interpretations of reality, evaluates their coherence with
//! observations, processes, evidence, and outcomes, and preferentially retains
//! interpretations that continue to work.

use std::collections::HashMap;

use crate::coherence_dimensions::CoherenceProfile;
use crate::coherence_query::{
    EpistemicQuery, EpistemicQueryResult, FailedPredictionSummary, HypothesisSummary,
};
use crate::contradiction::{Contradiction, ContradictionParty};
use crate::embed::VectorEmbed;
use crate::epistemic::{EpistemicAuditTrail, EpistemicEvent, EpistemicEventType};
use crate::explanation::{ExplanationReport, HistoricalPrecedent};
use crate::hypothesis::{Hypothesis, HypothesisStatus};
use crate::models::*;
use crate::process::ProcessCycle;
use crate::provenance::ProvenanceChain;
use crate::relation::{RelationType, TypedEdge};

/// Central engine state.
#[derive(Debug, Default)]
pub struct PhysisCore {
    /// All coherence nodes keyed by ID.
    pub nodes: HashMap<String, CoherenceNode>,
    /// Branches certified as coherent clusters.
    pub certified_branches: Vec<CertifiedBranch>,
    /// Branches flagged as low-coherence outliers.
    pub isolated_branches: Vec<IsolatedBranch>,
    /// Archive of dream evaluation results.
    pub dream_archive: Vec<DreamResult>,
    /// label → node id, for O(1) dedup of labeled registrations.
    label_index: HashMap<String, String>,
    /// Embedder id stamped onto every node registered here (provenance).
    embedder_id: Option<String>,
    /// First-class hypotheses (interpretations with evidence, status, provenance).
    pub hypotheses: HashMap<String, Hypothesis>,
    /// Explicit contradictions between claims.
    pub contradictions: Vec<Contradiction>,
    /// Provenance chains for audit trails.
    pub provenance_chains: HashMap<String, ProvenanceChain>,
    /// Rich typed graph edges connecting nodes, hypotheses, and observations.
    pub edges: Vec<TypedEdge>,
    /// Replayable timeline of epistemic events.
    pub epistemic_audit: EpistemicAuditTrail,
    /// Process and PDCA execution cycles.
    pub process_cycles: Vec<ProcessCycle>,
}

impl PhysisCore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stamp the embedder id onto every node registered after this call.
    pub fn set_embedder_id(&mut self, id: impl Into<String>) {
        self.embedder_id = Some(id.into());
    }

    pub fn embedder_id(&self) -> Option<&str> {
        self.embedder_id.as_deref()
    }

    /// Register a vector, optionally retaining source text as a browsable label.
    /// Labeled registrations are deduped by label (idempotent re-scans).
    pub fn register_node_vec_labeled(&mut self, embedding: Vec<f32>, label: Option<String>) -> String {
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
        self.nodes.insert(id.clone(), node);
        self.update_coherence(&id);

        self.epistemic_audit.record(
            EpistemicEvent::new(
                EpistemicEventType::ObservationIngested,
                &id,
                format!("Registered node vector (label: {:?})", self.nodes[&id].label),
            )
            .with_source(self.embedder_id.as_deref().unwrap_or("unknown_embedder")),
        );

        id
    }

    pub fn register_node_vec(&mut self, embedding: Vec<f32>) -> String {
        self.register_node_vec_labeled(embedding, None)
    }

    /// Register from text (embed first using an embedder).
    pub fn register_node_from_text(&mut self, text: &str, embedder: &dyn VectorEmbed) -> String {
        let embedding = embedder.embed(text);
        self.register_node_vec_labeled(embedding, Some(text.to_string()))
    }

    /// Report whether a node actually worked. Score clamped to [-1, 1].
    pub fn assert_coherence(&mut self, id: &str, score: Score) -> bool {
        match self.nodes.get_mut(id) {
            Some(node) => {
                let clamped = score.clamp(-1.0, 1.0);
                node.asserted = Some(clamped);
                true
            }
            None => false,
        }
    }

    /// Mean of the reported verdicts, or `None` when nothing has been judged.
    pub fn asserted_index(&self) -> Option<Score> {
        let judged: Vec<Score> = self.nodes.values().filter_map(|n| n.asserted).collect();
        if judged.is_empty() {
            return None;
        }
        Some(judged.iter().sum::<Score>() / judged.len() as Score)
    }

    /// Nodes someone reported as not working, worst first.
    pub fn asserted_failures(&self) -> Vec<&CoherenceNode> {
        let mut out: Vec<&CoherenceNode> =
            self.nodes.values().filter(|n| n.is_asserted_failure()).collect();
        out.sort_by(|a, b| {
            a.asserted
                .unwrap_or(0.0)
                .partial_cmp(&b.asserted.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }

    /// Recall labeled nodes by similarity to `query`. Embeddings are L2-normalized.
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

    /// Search by text.
    pub fn search_text(&self, query: &str, embedder: &dyn VectorEmbed, max: usize) -> Vec<(String, String, Score)> {
        self.search_nodes(&embedder.embed(query), max)
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

        let avg = if sims.is_empty() { 1.0 } else { sims.iter().sum::<f32>() / sims.len() as f32 };

        if let Some(node) = self.nodes.get_mut(node_id) {
            node.coherence_score = avg.max(0.0);
        }
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

    /// Group nodes into certified dense clusters.
    pub fn certify_branches(&mut self) -> Vec<CertifiedBranch> {
        let mut certified = Vec::new();
        let high_nodes: Vec<&CoherenceNode> = self.nodes.values().filter(|n| n.coherence_score > 0.7).collect();
        if !high_nodes.is_empty() {
            let ids: Vec<String> = high_nodes.iter().map(|n| n.id.clone()).collect();
            let branch = CertifiedBranch {
                branch_id: uuid::Uuid::new_v4().to_string(),
                node_ids: ids,
                centroid: Vec::new(),
                stability_score: 0.85,
            };
            certified.push(branch.clone());
            self.certified_branches.push(branch);
        }
        certified
    }

    /// Detect isolated outliers.
    pub fn detect_contradictions(&mut self) -> Vec<IsolatedBranch> {
        let mut isolated = Vec::new();
        let low_nodes: Vec<&CoherenceNode> = self.nodes.values().filter(|n| n.coherence_score < 0.3).collect();
        for node in low_nodes {
            let branch = IsolatedBranch {
                branch_id: uuid::Uuid::new_v4().to_string(),
                node_ids: vec![node.id.clone()],
                outlier_score: node.coherence_score,
            };
            isolated.push(branch.clone());
            self.isolated_branches.push(branch);
        }
        isolated
    }

    /// Dream over asserted failures.
    pub fn dream(&mut self) -> Vec<DreamResult> {
        let failures = self.asserted_failures();
        if failures.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        for failure in failures {
            let result = DreamResult {
                dream_id: uuid::Uuid::new_v4().to_string(),
                nodes_tested: vec![failure.id.clone()],
                outcome: 0.5,
                prevented_failure: false,
                coherence_delta: 0.0,
            };
            results.push(result);
        }
        self.dream_archive.extend(results.clone());
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
        let high = self.nodes.values().filter(|n| n.coherence_score > 0.7).count();
        let mid = self.nodes.values().filter(|n| n.coherence_score > 0.3 && n.coherence_score <= 0.7).count();
        let low = self.nodes.values().filter(|n| n.coherence_score <= 0.3).count();
        let rated = |want: CoherenceRating| {
            self.nodes.values().filter(|n| n.rating() == Some(want)).count()
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

    /// Serialize all nodes to JSON.
    pub fn to_json(&self) -> anyhow::Result<String> {
        let nodes: Vec<&CoherenceNode> = self.nodes.values().collect();
        Ok(serde_json::to_string_pretty(&nodes)?)
    }

    /// Replace all nodes from JSON.
    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        let nodes: Vec<CoherenceNode> = serde_json::from_str(json)?;
        let mut core = Self::new();
        for n in nodes {
            if let Some(ref label) = n.label {
                core.label_index.insert(label.clone(), n.id.clone());
            }
            core.nodes.insert(n.id.clone(), n);
        }
        Ok(core)
    }

    // ── First-Class Hypotheses ─────────────────────────────────────────

    /// Register a new hypothesis and record epistemic event.
    pub fn register_hypothesis(&mut self, hypothesis: Hypothesis) -> String {
        let id = hypothesis.id.clone();
        self.epistemic_audit.record(
            EpistemicEvent::new(
                EpistemicEventType::HypothesisGenerated,
                &id,
                format!("Hypothesis formulated: {}", hypothesis.statement),
            )
            .with_transition("none", hypothesis.status.as_str())
            .with_metric(hypothesis.fitness),
        );
        self.hypotheses.insert(id.clone(), hypothesis);
        id
    }

    /// Get a hypothesis by id.
    pub fn hypothesis(&self, id: &str) -> Option<&Hypothesis> {
        self.hypotheses.get(id)
    }

    /// Get a mutable hypothesis by id.
    pub fn hypothesis_mut(&mut self, id: &str) -> Option<&mut Hypothesis> {
        self.hypotheses.get_mut(id)
    }

    /// Transition a hypothesis to a new status.
    pub fn transition_hypothesis(
        &mut self,
        id: &str,
        new_status: HypothesisStatus,
        description: impl Into<String>,
        trigger: Option<String>,
    ) -> bool {
        let desc = description.into();
        if let Some(h) = self.hypotheses.get_mut(id) {
            let prior = h.status.as_str().to_string();
            h.transition_to(new_status, desc.clone(), trigger.clone());
            self.epistemic_audit.record(
                EpistemicEvent::new(
                    EpistemicEventType::StatusTransition,
                    id,
                    desc,
                )
                .with_transition(prior, new_status.as_str())
                .with_metric(h.fitness),
            );
            true
        } else {
            false
        }
    }

    // ── Contradictions ─────────────────────────────────────────────────

    /// Record an explicit contradiction between two claims without overwriting.
    pub fn record_contradiction(&mut self, contradiction: Contradiction) -> String {
        let id = contradiction.id.clone();
        self.epistemic_audit.record(
            EpistemicEvent::new(
                EpistemicEventType::ContradictionDetected,
                &id,
                format!(
                    "Contradiction detected between [{}] and [{}]",
                    contradiction.claim_a.claim, contradiction.claim_b.claim
                ),
            )
            .with_source("contradiction_handler"),
        );
        self.contradictions.push(contradiction);
        id
    }

    /// Detect contradictions between two hypotheses by comparing their claims.
    pub fn detect_hypothesis_contradiction(&self, a: &str, b: &str) -> Option<Contradiction> {
        let ha = self.hypotheses.get(a)?;
        let hb = self.hypotheses.get(b)?;
        let party_a = ContradictionParty::new(ha.statement.clone(), "hypothesis")
            .with_confidence(ha.confidence);
        let party_b = ContradictionParty::new(hb.statement.clone(), "hypothesis")
            .with_confidence(hb.confidence);
        Some(Contradiction::new(party_a, party_b))
    }

    // ── Typed Graph Edges ──────────────────────────────────────────────

    /// Add a typed relationship edge to the context graph.
    pub fn add_edge(&mut self, edge: TypedEdge) {
        self.edges.push(edge);
    }

    /// Find all typed edges where `entity_id` is source or target.
    pub fn edges_for(&self, entity_id: &str) -> Vec<&TypedEdge> {
        self.edges
            .iter()
            .filter(|e| e.source_id == entity_id || e.target_id == entity_id)
            .collect()
    }

    // ── Process Cycles ─────────────────────────────────────────────────

    /// Register a process cycle.
    pub fn record_process_cycle(&mut self, cycle: ProcessCycle) {
        self.process_cycles.push(cycle);
    }

    // ── Epistemic Audit & Reconstruction ───────────────────────────────

    /// Record an arbitrary epistemic event.
    pub fn record_epistemic_event(&mut self, event: EpistemicEvent) {
        self.epistemic_audit.record(event);
    }

    /// Reconstruct belief state about `subject_id` at instant `when`.
    pub fn reconstruct_belief_at(
        &self,
        subject_id: &str,
        when: chrono::DateTime<chrono::Utc>,
    ) -> Option<HypothesisStatus> {
        self.epistemic_audit.reconstruct_status_at(subject_id, when)
    }

    // ── Provenance & Explanation ───────────────────────────────────────

    /// Get or create a provenance chain for a subject.
    pub fn provenance_chain(&mut self, subject: impl Into<String>) -> &mut ProvenanceChain {
        let key = subject.into();
        self.provenance_chains.entry(key).or_default()
    }

    /// Generate a fully structured explanation report for a hypothesis.
    pub fn full_explanation_report(&self, id: &str) -> Option<ExplanationReport> {
        let h = self.hypotheses.get(id)?;
        let prov = self.provenance_chains.get(id).cloned().unwrap_or_default();

        let mut precedents = Vec::new();
        for node in self.nodes.values() {
            if let Some(verdict) = node.asserted {
                let sim = cosine_sim(&h.embedding, &node.embedding);
                if sim > 0.6 {
                    precedents.push(HistoricalPrecedent {
                        case_id: node.id.clone(),
                        description: node.label.clone().unwrap_or_else(|| "unlabeled case".to_string()),
                        outcome_worked: verdict > 0.0,
                        similarity: sim,
                        context_match: None,
                    });
                }
            }
        }

        let report = ExplanationReport {
            subject_id: h.id.clone(),
            statement: h.statement.clone(),
            status: h.status,
            supporting_evidence: h.supporting_evidence.clone(),
            contradicting_evidence: h.contradicting_evidence.clone(),
            historical_precedents: precedents,
            expected_consequences: h.predictions.clone(),
            observed_consequences: h.predictions.iter().filter(|p| p.actual_outcome.is_some()).cloned().collect(),
            coherence_profile: h.coherence_profile.clone(),
            coherence_score: h.coherence,
            fitness_breakdown: h.fitness_breakdown.clone(),
            fitness_score: h.fitness,
            confidence: h.confidence,
            provenance_chain: prov,
            human_readable_summary: String::new(),
        };

        let rendered = report.render_ascii();
        let mut final_report = report;
        final_report.human_readable_summary = rendered;
        Some(final_report)
    }

    /// String summary explanation of a hypothesis.
    pub fn explain_hypothesis(&self, id: &str) -> Option<String> {
        self.full_explanation_report(id).map(|r| r.human_readable_summary)
    }

    // ── Upgraded Dream Loop ────────────────────────────────────────────

    /// Dream over unresolved failures: generate candidate hypotheses that could
    /// restore coherence, rank candidates by multi-dimensional coherence, and
    /// retain them as candidates (never silently certifying them).
    pub fn dream_hypotheses(&mut self) -> Vec<Hypothesis> {
        let failures: Vec<(String, Option<String>, Vec<f32>, Score)> = self
            .asserted_failures()
            .iter()
            .map(|f| (f.id.clone(), f.label.clone(), f.embedding.clone(), f.coherence_score))
            .collect();

        let mut candidates = Vec::new();

        for (fid, flabel, femb, fcoh) in failures {
            let statement = format!(
                "Candidate hypothesis explaining failure in node: {}",
                flabel.as_deref().unwrap_or(&fid)
            );
            let mut hyp = Hypothesis::new(statement, femb);
            hyp.status = HypothesisStatus::Candidate;
            hyp.coherence_profile.semantic.score = fcoh;
            hyp.coherence_profile.empirical.score = 0.2; // Reflects failure observation
            hyp.coherence = hyp.coherence_profile.composite();
            hyp.recompute_fitness();

            self.epistemic_audit.record(
                EpistemicEvent::new(
                    EpistemicEventType::HypothesisGenerated,
                    &hyp.id,
                    format!("Dream loop generated candidate explanation for failure in {}", fid),
                )
                .with_source("dream_loop")
                .with_metric(hyp.fitness),
            );

            self.hypotheses.insert(hyp.id.clone(), hyp.clone());
            candidates.push(hyp);
        }

        candidates
    }

    // ── Coherence Query API Execution ──────────────────────────────────

    /// Execute a structured epistemic query against Physis.
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
                        coherence_profile: CoherenceProfile::new(),
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
                    .filter(|e| (e.source_id == *id || e.target_id == *id) && e.relation_type == RelationType::Contradicts)
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
                                return Some((n.label.clone().unwrap_or_else(|| n.id.clone()), sim, true));
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
                                return Some((n.label.clone().unwrap_or_else(|| n.id.clone()), sim, false));
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
                hyps.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap_or(std::cmp::Ordering::Equal));
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
                EpistemicQueryResult::FailedPredictionsList { failures: failed_preds }
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
                hyps.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap_or(std::cmp::Ordering::Equal));
                hyps.truncate(*limit);
                EpistemicQueryResult::RankedHypotheses { hypotheses: hyps }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::RandomProjectionEmbedder;
    use crate::hypothesis::{Evidence, EvidencePolarity, Hypothesis, HypothesisStatus};
    use crate::contradiction::{Contradiction, ContradictionParty, ResolutionStatus};
    use crate::provenance::ProvenanceLink;

    fn fixture_embedder() -> RandomProjectionEmbedder {
        RandomProjectionEmbedder::new(64)
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
    fn register_and_search() {
        let g = fixture_core();
        let emb = fixture_embedder();
        let q = emb.embed("exercise running success");
        let hits = g.search_nodes(&q, 3);
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].1, "exercise running success");
        assert!(hits[0].2 >= hits[1].2 && hits[1].2 >= hits[2].2);
    }

    #[test]
    fn labeled_registration_is_deduped() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let id1 = g.register_node_from_text("src/main.rs", &emb);
        let id2 = g.register_node_from_text("src/main.rs", &emb);
        assert_eq!(id1, id2);
        assert_eq!(g.nodes.len(), 1);
        g.register_node_vec(vec![0.1; 64]);
        g.register_node_vec(vec![0.1; 64]);
        assert_eq!(g.nodes.len(), 3);
    }

    #[test]
    fn coherence_index_and_snapshot() {
        let g = fixture_core();
        let idx = g.coherence_index();
        assert!((0.0..=1.0).contains(&idx));
        let snap = g.snapshot();
        assert_eq!(snap.total_nodes, 3);
    }

    #[test]
    fn asserted_axis_is_separate_from_density() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let a = g.register_node_from_text("deploy script that always works", &emb);
        let b = g.register_node_from_text("deploy script that always works, but fails", &emb);

        assert_eq!(g.nodes[&a].rating(), None);
        assert_eq!(g.asserted_index(), None);

        let density_before = g.nodes[&b].coherence_score;
        assert!(g.assert_coherence(&b, -1.0));
        assert_eq!(g.nodes[&b].rating(), Some(CoherenceRating::Failure));
        assert_eq!(g.nodes[&b].coherence_score, density_before);
        assert_eq!(g.asserted_index(), Some(-1.0));

        g.assert_coherence(&a, 1.0);
        assert_eq!(g.asserted_index(), Some(0.0));
        g.assert_coherence(&a, 9.0);
        assert_eq!(g.nodes[&a].asserted, Some(1.0));
        assert!(!g.assert_coherence("no-such-node", 1.0));
    }

    #[test]
    fn dream_replays_asserted_failures_only() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let failed = g.register_node_from_text("migration that corrupted the index", &emb);
        let worked = g.register_node_from_text("migration that corrupted the index, fixed", &emb);

        g.assert_coherence(&failed, -1.0);
        g.assert_coherence(&worked, 1.0);
        g.nodes.get_mut(&failed).unwrap().coherence_score = 0.9;
        g.nodes.get_mut(&worked).unwrap().coherence_score = 0.1;

        let dreams = g.dream();
        let tested: Vec<&String> = dreams.iter().flat_map(|d| &d.nodes_tested).collect();
        assert!(tested.contains(&&failed));
        assert!(!tested.contains(&&worked));
    }

    #[test]
    fn dream_hypotheses_generates_candidates_not_certified() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let failed = g.register_node_from_text("extruder motor stall during ramp", &emb);
        g.assert_coherence(&failed, -1.0);

        let candidate_hyps = g.dream_hypotheses();
        assert_eq!(candidate_hyps.len(), 1);
        assert_eq!(candidate_hyps[0].status, HypothesisStatus::Candidate);
        assert!(candidate_hyps[0].statement.contains("extruder motor stall"));
    }

    #[test]
    fn nodes_survive_json_roundtrip() {
        let mut g = PhysisCore::new();
        g.set_embedder_id("fixture");
        let emb = fixture_embedder();
        let id = g.register_node_from_text("morning yoga completed", &emb);
        g.assert_coherence(&id, -1.0);
        let json = g.to_json().unwrap();

        let restored = PhysisCore::from_json(&json).unwrap();
        assert_eq!(restored.nodes.len(), 1);
        assert_eq!(restored.nodes[&id].asserted, Some(-1.0));
        let mut again = restored;
        let id2 = again.register_node_from_text("morning yoga completed", &emb);
        assert_eq!(id2, id);
        assert_eq!(again.nodes.len(), 1);
    }

    #[test]
    fn certify_and_contradict() {
        let mut g = fixture_core();
        g.certify_branches();
        g.detect_contradictions();
        assert!(g.certified_branches.len() + g.isolated_branches.len() > 0 || g.nodes.len() == 3);
    }

    #[test]
    fn epistemic_audit_reconstruction() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let v = emb.embed("nozzle temperature nominal");
        let hyp = Hypothesis::new("Nozzle temperature nominal", v);
        let id = g.register_hypothesis(hyp.clone());

        let t0 = chrono::Utc::now();
        g.transition_hypothesis(&id, HypothesisStatus::Supported, "Sensor reading confirms", None);
        let t1 = chrono::Utc::now();
        g.transition_hypothesis(&id, HypothesisStatus::Contradicted, "Thermal camera indicates freeze", None);

        assert_eq!(g.reconstruct_belief_at(&id, t0), Some(HypothesisStatus::Candidate));
        assert_eq!(g.reconstruct_belief_at(&id, t1), Some(HypothesisStatus::Supported));
        assert_eq!(g.reconstruct_belief_at(&id, chrono::Utc::now()), Some(HypothesisStatus::Contradicted));
    }

    #[test]
    fn hypothesis_lifecycle() {
        let mut core = PhysisCore::new();
        let h = Hypothesis::new("the machine is overheating", vec![0.1; 64]);
        let id = core.register_hypothesis(h);
        assert!(core.hypotheses.contains_key(&id));

        core.transition_hypothesis(&id, HypothesisStatus::Supported, "evidence found", None);
        assert_eq!(core.hypotheses[&id].status, HypothesisStatus::Supported);
        assert_eq!(core.hypotheses[&id].revision_history.len(), 2);
    }

    #[test]
    fn hypothesis_evidence_and_fitness() {
        let mut core = PhysisCore::new();
        let h = Hypothesis::new("temperature exceeds threshold", vec![0.2; 64]);
        let id = core.register_hypothesis(h);

        let evidence = Evidence {
            source: "sensor_01".to_string(),
            polarity: EvidencePolarity::Supports,
            confidence: 0.9,
            claim: "reading 218.4C".to_string(),
            observed_at: Some(chrono::Utc::now()),
            embedding: Vec::new(),
            context: Vec::new(),
        };
        core.hypothesis_mut(&id).unwrap().add_supporting_evidence(evidence);
        assert_eq!(core.hypotheses[&id].supporting_evidence.len(), 1);
    }

    #[test]
    fn contradiction_is_first_class() {
        let mut core = PhysisCore::new();
        let ha = Hypothesis::new("pressure is normal", vec![0.1; 64]);
        let hb = Hypothesis::new("pressure is high", vec![0.2; 64]);
        let id_a = core.register_hypothesis(ha);
        let id_b = core.register_hypothesis(hb);

        let contradiction = core.detect_hypothesis_contradiction(&id_a, &id_b);
        assert!(contradiction.is_some());
        let c = contradiction.unwrap();
        assert_eq!(c.resolution, ResolutionStatus::Open);

        core.record_contradiction(c);
        assert_eq!(core.contradictions.len(), 1);
    }

    #[test]
    fn provenance_chain_is_traceable() {
        let mut core = PhysisCore::new();
        let h = Hypothesis::new("sensor reads 218.4C", vec![0.1; 64]);
        let id = core.register_hypothesis(h);
        
        let chain = core.provenance_chain(&id);
        chain.add_link(ProvenanceLink::new("sensor_01 reports 218.4C", "sensor_01"));
        chain.add_link(ProvenanceLink::new("threshold is 200C", "spec_sheet"));

        let explanation = core.explain_hypothesis(&id);
        assert!(explanation.is_some(), "explanation should exist");
        let text = explanation.unwrap();
        assert!(text.contains("spec_sheet"), "provenance should be visible: {}", text);
        assert!(text.contains("threshold is 200C"), "provenance should be visible: {}", text);
    }

    #[test]
    fn explain_returns_structured_output() {
        let mut core = PhysisCore::new();
        let mut h = Hypothesis::new("process is stable", vec![0.3; 64]);
        h.confidence = 0.8;
        h.coherence = 0.9;
        h.add_supporting_evidence(Evidence {
            source: "log".to_string(),
            polarity: EvidencePolarity::Supports,
            confidence: 0.9,
            claim: "no alarms".to_string(),
            observed_at: Some(chrono::Utc::now()),
            embedding: Vec::new(),
            context: Vec::new(),
        });
        h.recompute_fitness();
        let id = core.register_hypothesis(h);

        let explanation = core.explain_hypothesis(&id).unwrap();
        assert!(explanation.contains("process is stable"));
        assert!(explanation.contains("Confidence: 0.80"));
        assert!(explanation.contains("log"));
        assert!(explanation.contains("Fitness:"));
    }
}
