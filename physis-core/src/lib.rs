//! physis-core — embed, classify, cohere, and learn from quality feedback.
//!
//! The engine maintains competing interpretations of reality, evaluates their
//! multidimensional coherence with observations, processes, evidence, and outcomes,
//! and preferentially retains interpretations that continue to work.

pub mod classify;
pub mod coherence_dimensions;
pub mod coherence_query;
pub mod contradiction;
pub mod core;
pub mod discovery;
pub mod embed;
pub mod epistemic;
pub mod explanation;
pub mod hypothesis;
pub mod models;
pub mod ontology;
pub mod process;
pub mod provenance;
pub mod quality;
pub mod relation;
pub mod store;
pub mod temporal;

#[cfg(feature = "studio")]
pub mod studio;

#[cfg(feature = "embed-onnx")]
pub mod embed_onnx;

pub use classify::{CellClassifier, CellScore};
pub use coherence_dimensions::{CoherenceDimension, CoherenceProfile};
pub use coherence_query::{
    EpistemicQuery, EpistemicQueryResult, FailedPredictionSummary, HypothesisSummary,
};
pub use contradiction::{Contradiction, ContradictionParty, ResolutionStatus};
pub use core::PhysisCore;
pub use discovery::{discover, DiscoveryConfig, DiscoveryReport, ProposedDomain};
pub use embed::{RandomProjectionEmbedder, VectorEmbed};
pub use epistemic::{EpistemicAuditTrail, EpistemicEvent, EpistemicEventType};
pub use explanation::{ExplanationReport, HistoricalPrecedent};
pub use hypothesis::{
    Evidence, EvidencePolarity, FitnessBreakdown, Hypothesis, HypothesisStatus, Prediction,
    Revision,
};
pub use models::*;
pub use ontology::OntologyLoader;
pub use process::{
    ProcessConstraint, ProcessCycle, ProcessDeviation, ProcessGoal, ProcessIntervention,
    ProcessMeasurement, ProcessOutcome, ProcessPlan, ProcessResource, ProcessState, ProcessTask,
    StateTransition, TaskState,
};
pub use provenance::{ProvenanceChain, ProvenanceLink};
pub use quality::{
    ContextualQualityTracker, FitnessContext, FitnessRecord, QualityFailure, QualityTracker,
};
pub use relation::{RelationType, TypedEdge};
pub use temporal::TemporalValidity;

#[cfg(feature = "studio")]
pub use studio::{run, run_with_model, StudioState};

#[cfg(feature = "embed-onnx")]
pub use embed_onnx::{OnnxConfig, OnnxEmbedder, PoolingStrategy};
