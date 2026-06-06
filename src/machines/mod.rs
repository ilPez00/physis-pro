//! Philosophical machines — thin reducers over the physis core.
//!
//! Each machine turns a modality into *primitives* (the smallest elaborable
//! unit), registers them as labeled coherence nodes, and lets the core handle
//! recall / coherence / dream / durability. See `MACHINES_BUILD_PLAN.md`.
//!
//! Shipped: `chronos` (temporal). Planned: techne, soma, elenchus, lex, harmony,
//! polis.

pub mod chronos;
