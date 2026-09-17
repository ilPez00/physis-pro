//! MVP: repository as world — thin wiring over existing slices.
//!
//! Reuses scanner + nav + pack + act + core map. No new stores.

pub mod deps;
pub mod log;
pub mod model;
pub mod repl;
pub mod slice;

pub use deps::{
    affected_dependents, bench_text, changed_symbols, classify_intent, contradictions_text,
    dependents_text, generator_state, history_text,
};
pub use log::{since_last_session_notice, TransitionLog};
pub use model::{artifacts_of, Artifact, SemanticDelta};
pub use repl::map_counts;
pub use slice::{diff_snapshots, snapshot_hashes, WorldSlice};
