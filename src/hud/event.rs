//! The event boundary: everything expensive reports here, and `reduce` is the
//! only function that turns an event into a change.
//!
//! ```text
//! agent worker · source diff · keyboard input · fleet probe
//!         |  (HudEvent, from the worker thread or the loop)
//!         v
//!       reduce(&mut FocusState, &HudEvent) -> Vec<HudEffect>
//!         |
//!         v
//!   the bin applies the effects (status line, reading pane, focus, cursor)
//! ```
//!
//! Three rules this module makes checkable rather than aspirational:
//!
//! 1. **A model's output is never labelled as fact.** `AgentReply` fills a
//!    reading pane whose title carries the route, so the pane always shows
//!    what produced it.
//! 2. **No model output can acquire shell authority.** The effect set has no
//!    executable variant at all: a reply can set a status line and a reading
//!    pane, and that is the whole of its reach.
//! 3. **Navigation has one mutation path.** Every cursor move, selection, focus
//!    change, descent and ascent goes through `reduce`. The bin's key loop
//!    emits a `HudEvent`; it never writes cursor/selection/focus fields
//!    directly. This makes the class of bug that PH-027 found structurally
//!    harder: there is one function to audit instead of twenty scattered arms.
//!
//! ## Navigation events
//!
//! Each navigation event has exactly one producer in the bin's key loop and
//! one reducer branch here. No event exists without a producer; no navigation
//! mutation exists without an event.
//!
//! | event        | producer           | observable change                |
//! |---|---|---|
//! | `MoveCursor` | Up/Down keys       | `FocusState::cursor.row`         |
//! | `Select`     | Space              | `FocusState::selection` toggled  |
//! | `ClearSelection` | `F` key        | `FocusState::selection` emptied  |
//! | `FocusObject`| Enter (leaf)       | `FocusState::focus` refocused    |
//! | `Descend`    | Enter (container)  | trail pushed, focus set          |
//! | `Ascend`     | Esc                | trail popped, focus restored     |

use crate::hud::world::{FocusState, ProjectionId};
use crate::nav::ObjectId;

/// Something that happened. Carried into `reduce`; never handled inline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HudEvent {
    /// An agent worker finished (or failed). The one event a thread sends.
    AgentReply { agent: String, ok: bool, text: String },
    /// An agent call started on a worker.
    AgentStarted { agent: String },
    /// A rescan changed the workspace: what a delta badge is computed from.
    SourceDiff {
        changed: usize,
        affected: usize,
        summary: String,
    },
    /// The operator switched how the world is shown. Focus and selection
    /// survive; only the cursor resets (rows belong to the projection).
    ProjectionChanged { projection: ProjectionId },

    // ── Navigation events (PH-029) ────────────────────────────────────────
    // Each has exactly one producer in the bin's key loop. No speculative
    // events: if a variant has no caller, it does not exist.

    /// Move the cursor by `delta` rows (positive = down, negative = up),
    /// skipping header rows. The bin enforces bounds before emitting this.
    MoveCursor { delta: i32 },

    /// Toggle the selection for `id`. Producer: Space key on a hit row.
    Select { id: ObjectId },

    /// Clear the explicit selection. Producer: `F` key.
    ClearSelection,

    /// Refocus on `id` without descending (leaf objects). Producer: Enter on
    /// an object that has no children (passage, note, leaf file).
    FocusObject { id: ObjectId },

    /// Descend into a container: push the current scope onto the trail and
    /// move focus to `id`. Producer: Enter on an object with children.
    Descend {
        /// Scope label pushed onto the trail stack (for breadcrumbs).
        scope: String,
        /// Human-readable breadcrumb label shown in the header.
        crumb: String,
        /// The object being descended into.
        id: ObjectId,
    },

    /// Climb one scope out: pop the trail and restore the prior scope.
    /// Producer: Esc key when the trail is non-empty.
    Ascend,
}

/// What the bin should do about an event. Deliberately inert: no effect runs a
/// command, opens a socket or touches a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HudEffect {
    /// Replace the status line.
    Status(String),
    /// Replace the reading pane's (title, body).
    Reading { title: String, body: String },
    /// No agent call is in flight any more.
    ClearPending,
    /// Move the cursor to the given row. The bin must update `self.sel` and
    /// sync `ListState` from this — never by writing `sel` directly.
    SetCursor { row: usize },
}

impl HudEvent {
    /// Is this event's content produced by a model? Used by the renderer to
    /// keep the route visible; never used to grant authority.
    pub fn is_model_produced(&self) -> bool {
        matches!(self, HudEvent::AgentReply { .. })
    }
}

/// The one place an event becomes a change. Pure: same state + event gives
/// the same effects, so it is testable without a terminal.
pub fn reduce(state: &mut FocusState, event: &HudEvent) -> Vec<HudEffect> {
    match event {
        HudEvent::AgentStarted { agent } => vec![HudEffect::Status(format!(
            "@{agent}: asking in the background — the TUI stays responsive"
        ))],
        HudEvent::AgentReply { agent, ok, text } => {
            let mut out = vec![HudEffect::ClearPending];
            if *ok {
                out.push(HudEffect::Status(format!(
                    "@{agent} replied ({} chars) — reading pane shows the reply",
                    text.len()
                )));
            } else {
                let first = text.lines().next().unwrap_or("no detail");
                out.push(HudEffect::Status(format!("@{agent} failed: {first}")));
            }
            // The title carries the route: a model's answer is presented as a
            // reply, never as a fact the world asserts.
            out.push(HudEffect::Reading {
                title: format!("@{agent} reply (route: model)"),
                body: text.clone(),
            });
            out
        }
        HudEvent::SourceDiff {
            changed,
            affected,
            summary,
        } => vec![HudEffect::Status(format!(
            "rescanned: {changed} changed, {affected} affected — {summary}"
        ))],
        HudEvent::ProjectionChanged { projection } => {
            // The invariant lives in one function; the reducer calls it rather
            // than re-implementing the rule.
            state.switch_projection(*projection);
            // Cursor reset is encoded in switch_projection.
            vec![HudEffect::SetCursor { row: 0 }]
        }

        // ── Navigation ────────────────────────────────────────────────────

        HudEvent::MoveCursor { delta } => {
            // The new row is clamped by the bin (which knows the row count);
            // we write it into FocusState.cursor so the reducer owns it.
            let new_row = if *delta < 0 {
                state.cursor.row.saturating_sub((-delta) as usize)
            } else {
                state.cursor.row.saturating_add(*delta as usize)
                // Clamping to rows.len() - 1 is done in the bin where the
                // row count is known; the reducer only does the arithmetic.
            };
            state.cursor.row = new_row;
            vec![HudEffect::SetCursor { row: new_row }]
        }

        HudEvent::Select { id } => {
            state.selection.toggle(id.clone());
            vec![]
        }

        HudEvent::ClearSelection => {
            state.selection.clear();
            vec![]
        }

        HudEvent::FocusObject { id } => {
            state.focus.refocus(Some(id.clone()));
            vec![]
        }

        HudEvent::Descend { scope, crumb, id } => {
            state.descend(scope.clone(), crumb.clone(), id.clone());
            vec![HudEffect::SetCursor { row: 0 }]
        }

        HudEvent::Ascend => {
            state.ascend();
            // The bin restores the prior rows and resyncs the cursor from the
            // stack; we reset to 0 here as the safe default.
            vec![HudEffect::SetCursor { row: 0 }]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud::world::Cursor;
    use crate::nav::ObjectId;

    fn oid(key: &str) -> ObjectId {
        ObjectId::new("fs", key)
    }

    fn status_of(effects: &[HudEffect]) -> String {
        effects
            .iter()
            .find_map(|e| match e {
                HudEffect::Status(s) => Some(s.clone()),
                _ => None,
            })
            .expect("event sets a status line")
    }

    fn cursor_of(effects: &[HudEffect]) -> Option<usize> {
        effects.iter().find_map(|e| match e {
            HudEffect::SetCursor { row } => Some(*row),
            _ => None,
        })
    }

    // ── Existing tests (preserved) ──────────────────────────────────────

    #[test]
    fn a_model_reply_never_reaches_the_world_without_its_route() {
        let mut state = FocusState::new();
        let effects = reduce(
            &mut state,
            &HudEvent::AgentReply {
                agent: "jev".into(),
                ok: true,
                text: "the answer".into(),
            },
        );
        let reading = effects
            .iter()
            .find_map(|e| match e {
                HudEffect::Reading { title, .. } => Some(title.clone()),
                _ => None,
            })
            .expect("a reply fills the reading pane");
        assert!(reading.contains("route: model"), "{reading}");
    }

    #[test]
    fn a_reply_can_only_annotate_not_execute() {
        // The authority test: the effect set has no executable variant, so a
        // model reply cannot become a command whatever it contains.
        let mut state = FocusState::new();
        let effects = reduce(
            &mut state,
            &HudEvent::AgentReply {
                agent: "jev".into(),
                ok: true,
                text: "$ rm -rf / --no-preserve-root".into(),
            },
        );
        assert!(effects.iter().all(|e| matches!(
            e,
            HudEffect::Status(_)
                | HudEffect::Reading { .. }
                | HudEffect::ClearPending
                | HudEffect::SetCursor { .. }
        )));
    }

    #[test]
    fn a_failed_reply_says_so_instead_of_showing_an_empty_pane() {
        let mut state = FocusState::new();
        let effects = reduce(
            &mut state,
            &HudEvent::AgentReply {
                agent: "hermes".into(),
                ok: false,
                text: "no provider configured\nmore".into(),
            },
        );
        let status = status_of(&effects);
        assert!(status.contains("failed"), "{status}");
        assert!(status.contains("no provider configured"), "{status}");
        assert!(effects.contains(&HudEffect::ClearPending));
    }

    #[test]
    fn projection_events_go_through_the_focus_preserving_switch() {
        let mut state = FocusState::new();
        let file = ObjectId::new("fs", "abc123");
        state.focus.refocus(Some(file.clone()));
        state.selection.toggle(file.clone());
        let effects = reduce(
            &mut state,
            &HudEvent::ProjectionChanged {
                projection: ProjectionId::ObjectGraph,
            },
        );
        // Focus and selection survive; cursor resets.
        assert_eq!(state.projection, ProjectionId::ObjectGraph);
        assert_eq!(state.focus.object, Some(file.clone()));
        assert!(state.selection.contains(&file));
        assert_eq!(cursor_of(&effects), Some(0), "projection resets cursor");
    }

    #[test]
    fn a_source_diff_is_reported_with_both_counts() {
        let mut state = FocusState::new();
        let effects = reduce(
            &mut state,
            &HudEvent::SourceDiff {
                changed: 3,
                affected: 7,
                summary: "nav index: 12 objects".into(),
            },
        );
        let status = status_of(&effects);
        assert!(status.contains("3 changed"), "{status}");
        assert!(status.contains("7 affected"), "{status}");
    }

    #[test]
    fn only_agent_replies_claim_a_model_route() {
        assert!(HudEvent::AgentReply {
            agent: "x".into(),
            ok: true,
            text: String::new()
        }
        .is_model_produced());
        assert!(!HudEvent::SourceDiff {
            changed: 1,
            affected: 0,
            summary: String::new()
        }
        .is_model_produced());
        assert!(!HudEvent::ProjectionChanged {
            projection: ProjectionId::World
        }
        .is_model_produced());
    }

    // ── Navigation event tests (PH-029) ──────────────────────────────────

    #[test]
    fn move_cursor_changes_only_cursor() {
        let mut state = FocusState::new();
        let file = oid("abc");
        state.focus.refocus(Some(file.clone()));
        state.selection.toggle(file.clone());

        let effects = reduce(&mut state, &HudEvent::MoveCursor { delta: 3 });
        assert_eq!(state.cursor.row, 3, "cursor advances by delta");
        assert_eq!(cursor_of(&effects), Some(3));
        // Focus and selection are untouched.
        assert_eq!(state.focus.object, Some(file.clone()));
        assert!(state.selection.contains(&file));
    }

    #[test]
    fn move_cursor_saturates_at_zero() {
        let mut state = FocusState::new();
        state.cursor = Cursor::new(1);
        let effects = reduce(&mut state, &HudEvent::MoveCursor { delta: -5 });
        assert_eq!(state.cursor.row, 0, "cursor clamps at 0");
        assert_eq!(cursor_of(&effects), Some(0));
    }

    #[test]
    fn select_toggles_canonical_object_id_in_focus_state() {
        let mut state = FocusState::new();
        let a = oid("a");
        let b = oid("b");

        reduce(&mut state, &HudEvent::Select { id: a.clone() });
        assert!(state.selection.contains(&a), "a selected");
        assert!(!state.selection.contains(&b), "b not selected");

        reduce(&mut state, &HudEvent::Select { id: b.clone() });
        assert!(state.selection.contains(&a));
        assert!(state.selection.contains(&b), "b now selected");

        // Toggle off.
        reduce(&mut state, &HudEvent::Select { id: a.clone() });
        assert!(!state.selection.contains(&a), "a deselected");
    }

    #[test]
    fn clear_selection_empties_canonical_selection() {
        let mut state = FocusState::new();
        state.selection.toggle(oid("a"));
        state.selection.toggle(oid("b"));

        let effects = reduce(&mut state, &HudEvent::ClearSelection);
        assert!(state.selection.ids.is_empty(), "selection cleared");
        assert!(effects.is_empty(), "clear needs no annotation");
    }

    #[test]
    fn focus_object_changes_canonical_focus() {
        let mut state = FocusState::new();
        let file = oid("file1");

        let effects = reduce(&mut state, &HudEvent::FocusObject { id: file.clone() });
        assert_eq!(state.focus.object, Some(file), "focus updated");
        assert!(effects.is_empty(), "focus change needs no annotation");
    }

    #[test]
    fn descend_records_trail_and_sets_focus() {
        let mut state = FocusState::new();
        let dir = oid("dir1");
        let file = oid("file1");
        state.focus.refocus(Some(dir.clone()));

        let effects = reduce(
            &mut state,
            &HudEvent::Descend {
                scope: "src".into(),
                crumb: "src/".into(),
                id: file.clone(),
            },
        );
        assert_eq!(state.focus.object, Some(file), "focus = descended object");
        assert_eq!(state.trail.stack, vec!["src".to_string()], "trail pushed");
        assert_eq!(
            state.trail.crumbs,
            vec!["src/".to_string()],
            "crumb pushed"
        );
        assert_eq!(cursor_of(&effects), Some(0), "descend resets cursor");
    }

    #[test]
    fn ascend_pops_trail_and_resets_cursor() {
        let mut state = FocusState::new();
        let dir = oid("dir1");
        let file = oid("file1");
        state.focus.refocus(Some(dir));
        state.descend("src".into(), "src/".into(), file);

        let effects = reduce(&mut state, &HudEvent::Ascend);
        assert!(state.trail.stack.is_empty(), "trail popped");
        assert_eq!(cursor_of(&effects), Some(0), "ascend resets cursor");
    }

    #[test]
    fn projection_changed_preserves_symbol_focus_too() {
        // Once a symbol is focused, switching projection must not jump back to
        // the file. Focus belongs to the world; rows belong to projections.
        let mut state = FocusState::new();
        let symbol = oid("sym_run");
        state.focus.refocus(Some(symbol.clone()));
        state.descend("engine.rs".into(), "engine.rs".into(), symbol.clone());
        // Now switch projection.
        reduce(
            &mut state,
            &HudEvent::ProjectionChanged {
                projection: ProjectionId::ObjectGraph,
            },
        );
        assert_eq!(
            state.focus.object,
            Some(symbol),
            "symbol focus survives projection switch"
        );
    }
}
