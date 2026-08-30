//! Physis — an ontological mapper and PDCA dream engine.

pub mod actor;
pub mod address;
pub mod admin;
pub mod ai;
pub mod atomic;
pub mod axes;
#[cfg(feature = "ingest")]
pub mod batch_csv;
pub mod classify;
pub mod communities;
pub mod config;
pub mod content;
pub mod context;
pub mod core;
pub mod corpus;
pub mod demo_corpus;
pub mod discovery;
pub mod doctree;
pub mod dream;
pub mod embed;
#[cfg(feature = "empedocles")]
pub mod empedocles;
pub mod extract;
pub mod gantt;
pub mod graph;
pub mod indexer;
pub mod industrial;
pub mod industrial_benchmark;
pub mod ingest;
pub mod interest;
pub mod inventory;
pub mod ledger;
pub mod life_os;
pub mod linguistic;
pub mod machines;
pub mod mails;
pub mod maintenance;
pub mod management_pack;
pub mod mapper;
pub mod mcp;
pub mod models;
pub mod monitor;
pub mod moonraker;
pub mod moonraker_connect;
pub mod network;
pub mod notebook;
pub mod oee;
pub mod ontology;
pub mod ontology_graph;
pub mod ontology_nonhuman;
pub mod operational;
pub mod operational_report;
pub mod output;
pub mod quality;
pub mod quantize;
pub mod rachmaninov;
pub mod rbac;
pub mod reconcile;
pub mod reconstruct;
pub mod report;
pub mod routes;
pub mod review_desk;
pub mod salience;
pub mod scanner;
pub mod semiotic_grid;
pub mod sensory;
pub mod sessions;
pub mod shifts;
pub mod social;
pub mod storage;
pub mod study;
pub mod trie;
pub mod vfs;
pub mod video;
#[cfg(feature = "web")]
pub mod web;
pub mod wiki;
pub mod wiki_gen;

#[cfg(feature = "spanner")]
pub mod spanner;

pub mod decisions;
pub mod grounding;
pub mod history;
pub mod license;
pub mod pack;
pub mod planner;
pub mod postgres;
pub mod praxis;
pub mod rag;
pub mod sql;
pub mod structsim;
pub mod vault;

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
/// Normalized operational event model.
pub use operational::*;
/// Output formatters: wiki, JSON graph, Mermaid mindmap, semiotic renderers.
pub use output::*;
/// Product quantizer for compressed vector storage.
pub use quantize::ProductQuantizer;
/// Vector-space search, reconstruction, and nearest-neighbor lookup.
pub use reconstruct::{
    find_nearest_goals, find_neighbors, reconstruct, reconstruct_with_llm, Neighbor, Reconstruction,
};
/// Filesystem scanner: FileInfo, scan_project, extension lists.
pub use scanner::*;
/// Hybrid trie data structure with TF-IDF and vector similarity search.
pub use trie::DynamicVectorTrie;

#[cfg(feature = "cli")]
/// CLI application orchestrator.
pub use cli::PhysisApp;

pub use linguistic::*;
