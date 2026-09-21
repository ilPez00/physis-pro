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
//!   the bin applies the effects (status line, reading pane, focus)
//! ```
//!
//! Two rules this module makes checkable rather than aspirational:
//!
//! 1. **A model's output is never labelled as fact.** `AgentReply` fills a
//!    reading pane whose title carries the route, so the pane always shows
//!    what produced it.
//! 2. **No model output can acquire shell authority.** The effect set has no
//!    executable variant at all: a reply can set a status line and a reading
//!    pane, and that is the whole of its reach.

use crate::hud::world::{FocusState, ProjectionId};

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
    /// The operator switched how the world is shown.
    ProjectionChanged { projection: ProjectionId },
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
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::ObjectId;

    fn status_of(effects: &[HudEffect]) -> String {
        effects
            .iter()
            .find_map(|e| match e {
                HudEffect::Status(s) => Some(s.clone()),
                _ => None,
            })
            .expect("event sets a status line")
    }

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
            HudEffect::Status(_) | HudEffect::Reading { .. } | HudEffect::ClearPending
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
        assert!(effects.is_empty(), "a projection switch needs no annotation");
        assert_eq!(state.projection, ProjectionId::ObjectGraph);
        assert_eq!(state.focus.object, Some(file.clone()));
        assert!(state.selection.contains(&file));
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
}
