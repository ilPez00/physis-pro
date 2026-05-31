//! Physis — an ontological mapper and PDCA dream engine.
//!
//! Physis builds structured ontological maps from filesystem contents and drives
//! a Plan-Do-Check-Act (PDCA) loop over the resulting vector-space goals.
//! Everything lives in pure vector space — goals, experiences, dreams, coherence.
//!
//! # Core concepts
//!
//! - **Ontological mapping** — scan directories, extract entities & relationships,
//!   store them in a `DynamicVectorTrie` with TF-IDF scoring and random-projection
//!   vector embeddings.
//! - **PDCA loop** — track goals through Plan → Do → Check → Act cycles, detect
//!   stagnation, and drive iterative improvement.
//! - **Dream engine** — generate stochastic "dreams" (hypotheses) from the goal set,
//!   then grade and curate them.
//! - **Semantic search** — hybrid trie + vector search via the `DynamicVectorTrie`
//!   and `RandomProjectionEmbedder`.
//! - **AI agents** — `ProviderCascade` (OpenAI / Anthropic) powers deep-scan,
//!   reconstruction, and agent-driven tool use.
//! - **Network scanning** — watch filesystem changes with hash caching.
//! - **Output formats** — Wiki-style, JSON graph, Mermaid mindmap.
//!
//! # Feature flags
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `cli`   | CLI subcommands via `clap` |
//! | `web`   | Axum HTTP server on `:19876` |
//! | `mcp`   | MCP protocol server (stdio) |
//! | `network` | Filesystem watching via `notify` |
//! | `voice` | Voice input via `reqwest` |
//! | `tui`   | Terminal UI via `ratatui` |
//! | `embed-onnx` | ONNX MiniLM embeddings (upgrade from RP) |
//!
//! # Example
//!
//! ```ignore
//! use physis::config::PhysisConfig;
//! use physis::OntologyMapper;
//!
//! let config = PhysisConfig::default();
//! let ontology = physis::OntologyLoader::load_all(&config);
//! let mut mapper = OntologyMapper::new(ontology, 384);
//! let goals = mapper.map_filesystem(std::path::Path::new("/some/dir"), None);
//! println!("Found {} goals", goals.len());
//! ```

pub mod ai;
pub mod linguistic;

pub mod actor;
pub mod config;
pub mod core;
pub mod dream;
pub mod embed;
pub mod graph;
pub mod mapper;
pub mod rachmaninov;
pub mod storage;
pub mod gantt;
pub mod sensory;
pub mod models;
pub mod ontology;
pub mod ontology_nonhuman;
pub mod output;
pub mod quantize;
pub mod reconstruct;
pub mod scanner;
pub mod trie;

pub mod network;
pub mod mcp;
pub mod quality;
pub mod wiki;
pub mod industrial;
pub mod ingest;
pub mod inventory;
pub mod vfs;

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
