//! First-class hypothesis model.
//!
//! A Hypothesis is a structured interpretation of an observation. It carries:
//! - statement and semantic representation (embedding)
//! - ontology references
//! - supporting and contradicting evidence
//! - assumptions
//! - predictions, expected outcomes, and actual outcomes
//! - confidence, multidimensional coherence, and decomposed fitness scores
//! - provenance
//! - revision history
//! - status in an extensible closed state machine
//!
//! Physis distinguishes:
//!   similarity ≠ consistency ≠ ontological validity ≠ empirical support ≠ predictive success ≠ causal support ≠ operational fitness ≠ truth.

use serde::{Deserialize, Serialize};

use crate::coherence_dimensions::CoherenceProfile;
use crate::models::Score;
use crate::temporal::TemporalValidity;

/// Closed status machine for a hypothesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HypothesisStatus {
    /// Under evaluation; not yet supported or refuted.
    Candidate,
    /// Supported by evidence but not yet certified.
    Supported,
    /// Contradicted by at least one piece of evidence.
    Contradicted,
    /// Fully confirmed by multiple independent sources.
    Confirmed,
    /// Neither supported nor refuted; inactive.
    Inert,
    /// Refuted by observation or experiment.
    Failed,
    /// Replaced by a newer hypothesis.
    Superseded,
    /// No current relationship to observations.
    Isolated,
    /// Certified by an authority (human or process).
    Certified,
}

impl HypothesisStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            HypothesisStatus::Candidate => "candidate",
            HypothesisStatus::Supported => "supported",
            HypothesisStatus::Contradicted => "contradicted",
            HypothesisStatus::Confirmed => "confirmed",
            HypothesisStatus::Inert => "inert",
            HypothesisStatus::Failed => "failed",
            HypothesisStatus::Superseded => "superseded",
            HypothesisStatus::Isolated => "isolated",
            HypothesisStatus::Certified => "certified",
        }
    }
}

/// One piece of evidence for or against a hypothesis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Short label, e.g. "maintenance_report_2026_08_12.txt"
    pub source: String,
    /// Supports or contradicts.
    pub polarity: EvidencePolarity,
    /// How confident we are in this evidence (0.0–1.0).
    pub confidence: Score,
    /// Free-text summary or extracted claim.
    pub claim: String,
    /// When this evidence was observed or recorded.
    pub observed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Optional embedding of the claim for similarity search.
    #[serde(default)]
    pub embedding: Vec<f32>,
    /// Optional contextual tags (e.g. machine, operator, environment).
    #[serde(default)]
    pub context: Vec<String>,
}

/// Whether evidence supports or contradicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidencePolarity {
    Supports,
    Contradicts,
}

/// One prediction made by a hypothesis, with its expected and actual outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    /// What was predicted.
    pub statement: String,
    /// Expected outcome if hypothesis holds.
    #[serde(default)]
    pub expected_outcome: Option<String>,
    /// When the prediction was made.
    pub made_at: chrono::DateTime<chrono::Utc>,
    /// When the outcome was observed (None = pending).
    pub observed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// What actually happened.
    pub actual_outcome: Option<String>,
    /// Did the prediction match reality? None = pending.
    pub correct: Option<bool>,
    /// Confidence in this prediction.
    #[serde(default = "default_one")]
    pub confidence: Score,
}

fn default_one() -> Score {
    1.0
}

/// Decomposed fitness score representing empirical and operational survival.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FitnessBreakdown {
    /// Semantic fit with domain vocabulary.
    pub semantic_fit: Score,
    /// Fit within the ontology schema and constraints.
    pub ontological_fit: Score,
    /// Internal non-contradiction with beliefs.
    pub logical_consistency: Score,
    /// Ratio and quality of supporting empirical observations.
    pub empirical_support: Score,
    /// Historical accuracy of predictions made by this hypothesis.
    pub predictive_success: Score,
    /// Operational outcomes in real workflows (PDCA / tasks).
    pub outcome_success: Score,
    /// Penalty deducted from active contradictions.
    pub contradiction_penalty: Score,
    /// Penalty deducted from failed predictions.
    pub failed_prediction_penalty: Score,
    /// Inspectable composite fitness in [0.0, 1.0].
    pub composite_fitness: Score,
}

impl Default for FitnessBreakdown {
    fn default() -> Self {
        Self {
            semantic_fit: 0.5,
            ontological_fit: 0.5,
            logical_consistency: 0.5,
            empirical_support: 0.5,
            predictive_success: 0.5,
            outcome_success: 0.5,
            contradiction_penalty: 0.0,
            failed_prediction_penalty: 0.0,
            composite_fitness: 0.5,
        }
    }
}

/// One revision step in a hypothesis's history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Revision {
    /// ISO timestamp of the revision.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// What changed (free text for audit).
    pub description: String,
    /// What the status was before.
    pub previous_status: HypothesisStatus,
    /// What the status is now.
    pub new_status: HypothesisStatus,
    /// Provenance reference (e.g. feedback id, observation id).
    pub trigger: Option<String>,
}

/// A first-class interpretation of reality.
///
/// Hypotheses are the unit of evaluation, revision, and explanation in Physis.
/// They replace implicit "current classification" with an explicit, revisable,
/// evidence-tracked interpretation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    /// Unique identifier.
    pub id: String,
    /// Human-readable statement.
    pub statement: String,
    /// Semantic embedding of the statement.
    pub embedding: Vec<f32>,
    /// Ontology cells this hypothesis maps to (DOMAIN×MODE keys).
    pub ontology_refs: Vec<String>,
    /// Explicit assumptions underlying this hypothesis.
    #[serde(default)]
    pub assumptions: Vec<String>,
    /// Supporting evidence.
    pub supporting_evidence: Vec<Evidence>,
    /// Contradicting evidence.
    pub contradicting_evidence: Vec<Evidence>,
    /// Predictions made and their outcomes.
    pub predictions: Vec<Prediction>,
    /// Expected outcomes listed explicitly.
    #[serde(default)]
    pub expected_outcomes: Vec<String>,
    /// Actual outcomes observed.
    #[serde(default)]
    pub actual_outcomes: Vec<String>,
    /// Revision history (append-only).
    pub revision_history: Vec<Revision>,
    /// Current status in the closed state machine.
    pub status: HypothesisStatus,
    /// Confidence from semantic similarity / source authority (0.0–1.0).
    pub confidence: Score,
    /// Coherence profile with the current belief graph.
    pub coherence_profile: CoherenceProfile,
    /// Coherence score (composite 0.0–1.0).
    pub coherence: Score,
    /// Decomposed fitness score.
    pub fitness_breakdown: FitnessBreakdown,
    /// Fitness: how well this hypothesis has survived interaction with outcomes (0.0–1.0).
    pub fitness: Score,
    /// When this hypothesis was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// When this hypothesis was last revised.
    pub revised_at: chrono::DateTime<chrono::Utc>,
    /// Temporal validity window.
    pub temporal: TemporalValidity,
    /// Optional provenance chain entry.
    pub provenance: Option<String>,
}

impl Hypothesis {
    pub fn new(statement: impl Into<String>, embedding: Vec<f32>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            statement: statement.into(),
            embedding,
            ontology_refs: Vec::new(),
            assumptions: Vec::new(),
            supporting_evidence: Vec::new(),
            contradicting_evidence: Vec::new(),
            predictions: Vec::new(),
            expected_outcomes: Vec::new(),
            actual_outcomes: Vec::new(),
            revision_history: vec![Revision {
                timestamp: now,
                description: "Created".to_string(),
                previous_status: HypothesisStatus::Candidate,
                new_status: HypothesisStatus::Candidate,
                trigger: None,
            }],
            status: HypothesisStatus::Candidate,
            confidence: 0.5,
            coherence_profile: CoherenceProfile::new(),
            coherence: 0.5,
            fitness_breakdown: FitnessBreakdown::default(),
            fitness: 0.5,
            created_at: now,
            revised_at: now,
            temporal: TemporalValidity::permanent(),
            provenance: None,
        }
    }

    /// Add an assumption.
    pub fn add_assumption(&mut self, assumption: impl Into<String>) {
        self.assumptions.push(assumption.into());
    }

    /// Add supporting evidence and update status and fitness.
    pub fn add_supporting_evidence(&mut self, evidence: Evidence) {
        self.supporting_evidence.push(evidence);
        if self.status == HypothesisStatus::Candidate && !self.supporting_evidence.is_empty() {
            self.status = HypothesisStatus::Supported;
        }
        self.recompute_fitness();
        self.revise("Added supporting evidence", None);
    }

    /// Add contradicting evidence and update status and fitness.
    pub fn add_contradicting_evidence(&mut self, evidence: Evidence) {
        self.contradicting_evidence.push(evidence);
        self.status = HypothesisStatus::Contradicted;
        self.recompute_fitness();
        self.revise("Added contradicting evidence", None);
    }

    /// Record a prediction and its outcome.
    pub fn record_prediction(&mut self, prediction: Prediction) {
        if let Some(ref outcome) = prediction.actual_outcome {
            self.actual_outcomes.push(outcome.clone());
        }
        self.predictions.push(prediction);
        self.recompute_fitness();
        self.revise("Recorded prediction outcome", None);
    }

    /// Transition to a new status, recording the revision.
    pub fn transition_to(
        &mut self,
        new_status: HypothesisStatus,
        description: impl Into<String>,
        trigger: Option<String>,
    ) {
        let previous = self.status;
        self.status = new_status;
        self.revised_at = chrono::Utc::now();
        self.revision_history.push(Revision {
            timestamp: self.revised_at,
            description: description.into(),
            previous_status: previous,
            new_status,
            trigger,
        });
    }

    /// Recompute the decomposed fitness score.
    pub fn recompute_fitness(&mut self) {
        let support_len = self.supporting_evidence.len() as Score;
        let contradict_len = self.contradicting_evidence.len() as Score;
        let total_ev = support_len + contradict_len;

        let empirical = if total_ev == 0.0 {
            0.5
        } else {
            let support_conf: Score = self.supporting_evidence.iter().map(|e| e.confidence).sum();
            let contradict_conf: Score = self.contradicting_evidence.iter().map(|e| e.confidence).sum();
            (0.5 + 0.5 * ((support_conf - contradict_conf) / total_ev)).clamp(0.0, 1.0)
        };

        let resolved_preds: Vec<&Prediction> = self.predictions.iter().filter(|p| p.correct.is_some()).collect();
        let (pred_success, failed_preds_penalty) = if resolved_preds.is_empty() {
            (0.5, 0.0)
        } else {
            let succ = resolved_preds.iter().filter(|p| p.correct == Some(true)).count() as Score;
            let fail = resolved_preds.iter().filter(|p| p.correct == Some(false)).count() as Score;
            let s_ratio = succ / resolved_preds.len() as Score;
            let f_penalty = (fail * 0.15).min(0.5);
            (s_ratio, f_penalty)
        };

        let contra_penalty = (contradict_len * 0.1).min(0.4);

        let composite = (0.20 * self.fitness_breakdown.semantic_fit
            + 0.15 * self.fitness_breakdown.ontological_fit
            + 0.15 * self.fitness_breakdown.logical_consistency
            + 0.25 * empirical
            + 0.25 * pred_success
            - contra_penalty
            - failed_preds_penalty)
            .clamp(0.0, 1.0);

        self.fitness_breakdown = FitnessBreakdown {
            semantic_fit: self.fitness_breakdown.semantic_fit,
            ontological_fit: self.fitness_breakdown.ontological_fit,
            logical_consistency: self.fitness_breakdown.logical_consistency,
            empirical_support: empirical,
            predictive_success: pred_success,
            outcome_success: self.fitness_breakdown.outcome_success,
            contradiction_penalty: contra_penalty,
            failed_prediction_penalty: failed_preds_penalty,
            composite_fitness: composite,
        };

        self.fitness = composite;
    }

    /// Convenience: total evidence count.
    pub fn evidence_count(&self) -> usize {
        self.supporting_evidence.len() + self.contradicting_evidence.len()
    }

    /// Convenience: unresolved predictions.
    pub fn pending_predictions(&self) -> usize {
        self.predictions.iter().filter(|p| p.correct.is_none()).count()
    }

    fn revise(&mut self, description: impl Into<String>, trigger: Option<String>) {
        let previous = self.status;
        self.revised_at = chrono::Utc::now();
        self.revision_history.push(Revision {
            timestamp: self.revised_at,
            description: description.into(),
            previous_status: previous,
            new_status: self.status,
            trigger,
        });
    }
}
