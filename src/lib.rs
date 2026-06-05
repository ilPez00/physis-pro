//! Physis — an ontological mapper and PDCA dream engine.

pub mod ai;
pub mod linguistic;
pub mod actor;
pub mod config;
pub mod core;
pub mod dream;
pub mod embed;
pub mod mapper;
pub mod models;
pub mod graph;
pub mod rachmaninov;
pub mod sensory;
pub mod video;
pub mod storage;
pub mod vfs;
pub mod trie;
pub mod quantize;
pub mod life_os;
pub mod social;
pub mod output;
pub mod reconstruct;
pub mod scanner;
pub mod network;
pub mod mcp;
pub mod quality;
pub mod wiki;
pub mod industrial;
pub mod ingest;
pub mod inventory;
pub mod ontology;
pub mod ontology_nonhuman;
pub mod gantt;
pub mod classify;

#[cfg(feature = "cli")]
pub mod cli;

/// The PDCA cycle orchestrator — tracks progress, detects stagnation.
pub use actor::PDCActor;
/// Configuration and ontology loading.
pub use config::{OntologyLoader, PhysisConfig};
/// Core engine state and coherence snapshots.
pub use core::{CoherenceSnapshot, PhysisCore};
/// Stochastic dream generation engine.
pub use dream::DreamEngine;
/// Vector embedding trait and random-projection implementation.
pub use embed::VectorEmbed;
/// Maps filesystem trees into ontological trie structures.
pub use mapper::OntologyMapper;
/// Core data models: Goal, Dream, Entity, Score, etc.
pub use models::*;
/// Output formatters: wiki, JSON graph, Mermaid mindmap, semiotic renderers.
pub use output::*;
/// Product quantizer for compressed vector storage.
pub use quantize::ProductQuantizer;
/// Vector-space search, reconstruction, and nearest-neighbor lookup.
pub use reconstruct::{Reconstruction, Neighbor, reconstruct, reconstruct_with_llm, find_neighbors, find_nearest_goals};
/// Filesystem scanner: FileInfo, scan_project, extension lists.
pub use scanner::*;
/// Hybrid trie data structure with TF-IDF and vector similarity search.
pub use trie::DynamicVectorTrie;

#[cfg(feature = "cli")]
/// CLI application orchestrator.
pub use cli::PhysisApp;

pub use linguistic::*;
