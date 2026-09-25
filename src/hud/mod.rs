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
pub mod lens_semantic;
pub mod palette;
pub mod row;
pub mod world;

/// The lens contract shared by every projection in the world surface.
///
/// A lens is a read-only **projection** over one focus + its neighbourhood
/// (`NavHit`s ranked by the same scorer the world projection uses — never
/// re-ranked here, never re-modelled). The trait gives the bin a uniform
/// seam for `--view <lens>` / `--snapshot <lens>`: one `render_rows` for the
/// list pane, one `snapshot` bounded to a terminal height for CI frames.
/// See H7 (docs/plans/2026-09-19-impossible-computer.md §H7).
pub trait Lens {
    /// Canonical name — the `--view`/`--snapshot` token.
    fn name(&self) -> &str;

    /// Plain-text rows in the graph grammar for the list pane. State
    /// markers (selected/pinned/changed) stay in the bin; this must never
    /// reach for the `App`.
    fn render_rows(&self) -> Vec<String>;

    /// Render bounded to `height` lines (last line truncated with `…` when
    /// over). `width` is accepted for future column shaping; current lenses
    /// stay single-column for terminal parity.
    fn snapshot(&self, _width: u16, height: u16) -> String {
        let rows = self.render_rows();
        let h = height as usize;
        let out: String = rows.iter().take(h.saturating_sub(1)).cloned().collect::<Vec<_>>().join("\n");
        let mut out = out;
        if rows.len() > h.saturating_sub(1) { out.push_str("\n…"); }
        out
    }
}

/// Registry of lenses by name, so `--snapshot <lens>` dispatches without an
/// `if/else` chain in the bin. Grows as lenses land; the trait object bound
/// keeps the bin decoupled from each lens struct's type.
pub fn lens_by_name(name: &str) -> Option<Box<dyn Lens>> {
    // Filled in by the bin via `register_lenses`; lenses are not statically
    // known here because they may depend on world state at projection time.
    // (Static list kept empty on purpose — see H7.)
    let _ = name;
    None
}


