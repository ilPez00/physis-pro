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
//!
//! | module | owns |
//! |---|---|
//! | [`world`] | `Cursor`, `Selection`, `Focus`, `Trail`, `ProjectionId`, `FocusState` — the lens state that travels with the operator, including the invariant that a projection switch never moves the focus |
//! | [`config`] | four-layer configuration merge (builtin < user < workspace < flags) with per-key layer provenance, so a save writes each key back to the layer it came from |
//! | [`intent`] | the deterministic bare-text chain and the `IntentIR` compile; every resolution names its route, so no routing is hidden |
//! | [`lens_object`] | the object projection: recorded relations (with their origin) plus retrieval neighbours, which are always `Inferred` |
//! | [`palette`] | action rows derived from the guided world view — never a hand-written menu |
//! | [`event`] | `HudEvent` and the pure `reduce`: the one place an event becomes a change, and the reason model output can only annotate |
//! | [`bridge`] | the pure adapters between the bin's `App` and the contract above |

pub mod bridge;
pub mod config;
pub mod event;
pub mod intent;
pub mod lens_object;
pub mod palette;
pub mod world;

