//! App-to-library bridge: the narrow seam where the bin meets `src/hud/`.
//!
//! The bin's `App` keeps owning rows, hits and the selection list. This
//! module holds the three pure adapters that translate between that ownership
//! and the projection contract (`FocusState`, `Selection`, `Cursor`), so the
//! wiring commit moves logic without moving state.

use crate::hud::world::{Cursor, FocusState, ProjectionId, Selection};
use crate::nav::NavHit;

/// Wire the bin's current hit into the durable `Selection` form: the hit's
/// canonical id becomes the single selection, so a later projection switch
/// keeps pointing at the same world object.
pub fn selection_of(hit: Option<&NavHit>) -> Selection {
    match hit {
        Some(h) => Selection::single(h.object.id.clone()),
        None => Selection::empty(),
    }
}

/// The cursor row a lens renders from: the App's list index, clamped to the
/// rows the projection actually produced. Cursor rows belong to the
/// projection, so a switch resets them (see `FocusState::switch_projection`).
pub fn cursor_for(sel: usize, rows: usize) -> Cursor {
    Cursor::new(sel.min(rows.saturating_sub(1)))
}

/// Translate the bin's view name into the library `ProjectionId` through the
/// one name table (`ProjectionId::from_name`). Exhaustive names match; anything
/// else is the world, which is where `Esc` already returns.
pub fn projection_of(view: &str) -> ProjectionId {
    ProjectionId::from_name(view).unwrap_or(ProjectionId::World)
}

/// Apply a bin view change to a `FocusState` through the invariant-preserving
/// switch: focus and selection survive, cursor resets.
pub fn switch_to(state: &mut FocusState, view: &str) {
    state.switch_projection(projection_of(view));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::{ObjectId, ObjectKind, PhysisObject, RetrievalScore, SourceRef};

    fn hit(label: &str) -> NavHit {
        NavHit {
            object: PhysisObject {
                id: ObjectId::new("fs", "abc123"),
                kind: ObjectKind::File,
                label: label.to_string(),
                source: Some(SourceRef {
                    path: label.to_string(),
                    start_line: Some(1),
                    end_line: Some(1),
                }),
                mtime: 0.0,
                hash: "abc123".to_string(),
                text: label.to_string(),
                embedding: Vec::new(),
            },
            score: RetrievalScore { total: 1.0, components: Default::default() },
        }
    }

    #[test]
    fn selection_names_the_hit_id_so_projections_share_it() {
        let h = hit("src/a.rs");
        let sel = selection_of(Some(&h));
        assert!(sel.contains(&h.object.id));
        assert_eq!(sel.ids.len(), 1);
        assert!(selection_of(None).ids.is_empty());
    }

    #[test]
    fn view_switch_through_the_bridge_keeps_focus() {
        let h = hit("src/a.rs");
        let mut state = FocusState::new();
        state.focus.refocus(Some(h.object.id.clone()));
        state.selection = selection_of(Some(&h));
        switch_to(&mut state, "graph");
        assert_eq!(state.projection, ProjectionId::ObjectGraph);
        assert_eq!(state.focus.object, Some(h.object.id.clone()));
        assert!(state.selection.contains(&h.object.id));
        switch_to(&mut state, "world");
        assert_eq!(state.projection, ProjectionId::World);
        assert_eq!(state.focus.object, Some(h.object.id));
    }

    #[test]
    fn cursor_clamps_to_the_rendered_rows() {
        assert_eq!(cursor_for(9, 3).row, 2);
        assert_eq!(cursor_for(1, 3).row, 1);
        assert_eq!(cursor_for(0, 0).row, 0);
    }
}