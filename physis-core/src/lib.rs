//! physis-core — embed, classify, cohere, and learn from quality feedback.
//!
//! The engine is a lean re-implementation of the physis-pro core:
//! deterministic embedding → ontology lookup → semiotic-grid classification →
//! coherence nodes with an asserted-failure dream loop → quality tracker.

pub mod classify;
pub mod core;
pub mod discovery;
pub mod embed;
pub mod models;
pub mod ontology;
pub mod quality;
pub mod store;

#[cfg(feature = "studio")]
pub mod studio;

#[cfg(feature = "embed-onnx")]
pub mod embed_onnx;

pub use classify::{CellClassifier, CellScore};
pub use core::PhysisCore;
pub use discovery::{discover, DiscoveryConfig, DiscoveryReport, ProposedDomain};
pub use embed::{RandomProjectionEmbedder, VectorEmbed};
pub use models::*;
pub use ontology::OntologyLoader;
pub use quality::{QualityFailure, QualityTracker};

#[cfg(feature = "studio")]
pub use studio::{run, StudioState};

#[cfg(feature = "embed-onnx")]
pub use embed_onnx::{OnnxConfig, OnnxEmbedder, PoolingStrategy};
