//! Epistemic Audit Trail — replayable timeline of beliefs, observations, and revisions.
//!
//! Answers:
//!   - "What did Physis believe at time T?"
//!   - "What evidence caused the belief to change?"
//!   - "Which hypotheses evolved, were contradicted, or were superseded?"

use serde::{Deserialize, Serialize};

use crate::hypothesis::HypothesisStatus;
use crate::models::Score;

/// Types of epistemic events recorded on the timeline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EpistemicEventType {
    /// Initial assertion or baseline belief.
    BeliefFormed,
    /// New empirical observation ingested.
    ObservationIngested,
    /// Conflict/contradiction detected between two claims.
    ContradictionDetected,
    /// New candidate hypothesis generated.
    HypothesisGenerated,
    /// Prediction formulated by a hypothesis.
    PredictionFormulated,
    /// Outcome observed for a prior prediction.
    OutcomeObserved,
    /// Fitness score adjusted based on evidence or outcomes.
    FitnessShifted,
    /// Status transition of a hypothesis (e.g. Supported -> Contradicted).
    StatusTransition,
    /// Discovery of an ontology gap requiring conceptual extension.
    OntologyGapDiscovered,
    /// Promotion of a candidate ontology domain to active status.
    OntologyDomainPromoted,
}

/// A single step in the epistemic audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpistemicEvent {
    /// Unique event ID.
    pub id: String,
    /// When this event occurred.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Type of epistemic change.
    pub event_type: EpistemicEventType,
    /// ID of the subject entity (hypothesis, node, observation, contradiction).
    pub subject_id: String,
    /// Human-readable explanation of what occurred.
    pub description: String,
    /// Prior state or status (if applicable).
    #[serde(default)]
    pub prior_state: Option<String>,
    /// Posterior state or status (if applicable).
    #[serde(default)]
    pub posterior_state: Option<String>,
    /// Source or trigger of this change (file, operator, sensor, feedback ID).
    #[serde(default)]
    pub source: Option<String>,
    /// Confidence or fitness value at this instant.
    #[serde(default)]
    pub metric_value: Option<Score>,
}

impl EpistemicEvent {
    pub fn new(
        event_type: EpistemicEventType,
        subject_id: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            event_type,
            subject_id: subject_id.into(),
            description: description.into(),
            prior_state: None,
            posterior_state: None,
            source: None,
            metric_value: None,
        }
    }

    pub fn with_transition(
        mut self,
        prior: impl Into<String>,
        posterior: impl Into<String>,
    ) -> Self {
        self.prior_state = Some(prior.into());
        self.posterior_state = Some(posterior.into());
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_metric(mut self, metric: Score) -> Self {
        self.metric_value = Some(metric);
        self
    }
}

/// The full epistemic audit trail of the engine.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EpistemicAuditTrail {
    pub events: Vec<EpistemicEvent>,
}

impl EpistemicAuditTrail {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, event: EpistemicEvent) {
        self.events.push(event);
    }

    /// Retrieve all events relevant to a specific entity ID in chronological order.
    pub fn history_for(&self, subject_id: &str) -> Vec<&EpistemicEvent> {
        self.events.iter().filter(|e| e.subject_id == subject_id).collect()
    }

    /// Reconstruct what was believed about a subject at time `when`.
    pub fn reconstruct_status_at(
        &self,
        subject_id: &str,
        when: chrono::DateTime<chrono::Utc>,
    ) -> Option<HypothesisStatus> {
        let mut last_status: Option<HypothesisStatus> = None;
        for ev in &self.events {
            if ev.subject_id == subject_id && ev.timestamp <= when {
                if let Some(ref post) = ev.posterior_state {
                    match post.as_str() {
                        "candidate" => last_status = Some(HypothesisStatus::Candidate),
                        "supported" => last_status = Some(HypothesisStatus::Supported),
                        "contradicted" => last_status = Some(HypothesisStatus::Contradicted),
                        "confirmed" => last_status = Some(HypothesisStatus::Confirmed),
                        "inert" => last_status = Some(HypothesisStatus::Inert),
                        "failed" => last_status = Some(HypothesisStatus::Failed),
                        "superseded" => last_status = Some(HypothesisStatus::Superseded),
                        "isolated" => last_status = Some(HypothesisStatus::Isolated),
                        "certified" => last_status = Some(HypothesisStatus::Certified),
                        _ => {}
                    }
                }
            }
        }
        last_status
    }

    /// Format chronological summary for audit reporting.
    pub fn summary(&self) -> String {
        let mut lines = Vec::new();
        for (i, ev) in self.events.iter().enumerate() {
            let t = ev.timestamp.format("%Y-%m-%d %H:%M:%S UTC");
            lines.push(format!(
                "t{i} [{t}] [{:?}]: {} (subject: {})",
                ev.event_type, ev.description, ev.subject_id
            ));
        }
        lines.join("\n")
    }
}
