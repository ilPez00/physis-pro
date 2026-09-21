//! `physis-hud` as a world renderer: one world, many projections.
//!
//! This crate layer owns the object contract the HUD navigates. `src/bin/hud.rs`
//! keeps the terminal event loop and the existing `App`; this module grows the
//! architecture underneath it, one packet at a time:
//!
//! ```text
//! WORLD STATE (nav index, registry, relations, observations)
//!     ↓
//! FOCUS + INTENT (what object/scope the projection is built around)
//!     ↓
//! PROJECTION / LENS (how the same world is currently being shown)
//!     ↓
//! CANVAS + INSPECTOR (what is drawn)
//!     ↓
//! ACTION (capability-derived, never a bare command string)
//!     ↓
//! OBSERVATION / DELTA (what changed)
//!     ↓
//! WORLD STATE
//! ```
//!
//! The extraction rule: `src/bin/hud.rs` shrinks by moving code here only when
//! the moved piece is covered by the bin's tests *before* the move — the
//! renderer and its gates stay green at every intermediate commit.
//!
//! [`NavWorkspace`](crate::nav::workspace::NavWorkspace) stays the live world
//! state; [`crate::registry::Registry`] stays the authoritative action source;
//! [`crate::world_object::WorldObject`] is the unified read-only projection
//! over both. Nothing here is a second store.

pub mod world;
pub mod palette;
pub mod config;
pub mod intent;
