//! Task graph (plan S4).
//!
//! A task graph is a **fold over observations**, not a second store. The
//! append-only log is the source of truth; this module projects it. Folding the
//! same log twice yields the same graph, and folding it in `seq` order is what
//! makes that true — observations are addressed by sequence, not by arrival.
//!
//! The rules the exit gate tests:
//! - The fold is replay-stable: same observations, same graph.
//! - Expiring a task creates a child that names its parent and the reason.
//! - A task may be `reported` (an agent says it is done) but never reaches
//!   `Completed` without a verifier record. Reported is a claim; verified is
//!   the gate.

use std::collections::BTreeMap;

use physis_core::observe::Observation;
use physis_core::process::TaskState;
use serde::{Deserialize, Serialize};

/// Observations about tasks carry this `source`.
pub const TASK_SOURCE: &str = "task";

/// One thing that happened to a task. Encoded in an observation's `body` as
/// JSON, so the log stays flat and the schema lives here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TaskEvent {
    Created {
        title: String,
        #[serde(default)]
        parent: Option<String>,
        #[serde(default)]
        depends_on: Vec<String>,
    },
    State {
        to: TaskState,
    },
    /// An agent or person claims the task is done. A claim, not an acceptance.
    Reported {
        detail: String,
    },
    /// A verifier looked. `ok` is the verifier's finding, recorded as such.
    Verified {
        verifier: String,
        #[serde(default)]
        ok: bool,
    },
    /// The task ran out of time or was abandoned, with a reason.
    Expired {
        reason: String,
    },
}

impl TaskEvent {
    pub fn parse(body: &str) -> Result<Self, String> {
        serde_json::from_str(body).map_err(|e| format!("task event: {e}"))
    }

    pub fn render(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// The observation that records this event on a task.
    pub fn observation(task_id: &str, event: &TaskEvent) -> Observation {
        Observation::new(TASK_SOURCE, task_id).with_body(event.render())
    }

    /// Recover `(task id, event)` from an observation, or `None` when the
    /// observation is not about a task or its body does not parse.
    pub fn from_observation(obs: &Observation) -> Option<(String, TaskEvent)> {
        if obs.source != TASK_SOURCE {
            return None;
        }
        Self::parse(&obs.body)
            .ok()
            .map(|e| (obs.subject.clone(), e))
    }
}

/// A verifier's record. Kept as a list: a task can be verified more than once,
/// and the log keeps every look.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verification {
    pub verifier: String,
    pub ok: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub state: TaskState,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub children: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// The latest claim that the task is done. Present does not mean accepted.
    #[serde(default)]
    pub reported: Option<String>,
    #[serde(default)]
    pub verifications: Vec<Verification>,
    #[serde(default)]
    pub expired_reason: Option<String>,
}

impl Task {
    /// True when at least one verifier recorded a passing look.
    pub fn verified(&self) -> bool {
        self.verifications.iter().any(|v| v.ok)
    }
}

/// Legal task-state transitions. `Completed` and `Failed` are terminal; a
/// blocked task can resume; an intervened task can resume or fail.
pub fn can_transition(from: TaskState, to: TaskState) -> bool {
    use TaskState::*;
    if from == to {
        return true;
    }
    match from {
        Pending => matches!(to, InProgress | Blocked | Failed | Intervened),
        InProgress => matches!(to, Completed | Blocked | Failed | Intervened),
        Blocked => matches!(to, InProgress | Failed | Intervened),
        Intervened => matches!(to, InProgress | Failed),
        Completed | Failed => false,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TaskGraph {
    pub tasks: BTreeMap<String, Task>,
}

impl TaskGraph {
    /// Fold observations into a graph. Ordered by `seq`, so the result depends
    /// on the log's sequence and not on the order the caller happened to hold
    /// the records in.
    pub fn fold(observations: &[Observation]) -> Self {
        let mut ordered: Vec<&Observation> = observations.iter().collect();
        ordered.sort_by_key(|o| o.seq);
        let mut g = TaskGraph::default();
        for obs in ordered {
            if let Some((id, ev)) = TaskEvent::from_observation(obs) {
                g.apply(&id, ev);
            }
        }
        g
    }

    /// Apply one event. A transition that is illegal or unverified is **not**
    /// applied: the fold refuses rather than inventing a state the log does not
    /// support.
    pub fn apply(&mut self, id: &str, ev: TaskEvent) {
        match ev {
            TaskEvent::Created {
                title,
                parent,
                depends_on,
            } => {
                self.tasks.entry(id.to_string()).or_insert_with(|| Task {
                    id: id.to_string(),
                    title: title.clone(),
                    state: TaskState::Pending,
                    parent: parent.clone(),
                    children: Vec::new(),
                    depends_on: depends_on.clone(),
                    reported: None,
                    verifications: Vec::new(),
                    expired_reason: None,
                });
                if let Some(p) = &parent {
                    if let Some(pt) = self.tasks.get_mut(p) {
                        if !pt.children.iter().any(|c| c == id) {
                            pt.children.push(id.to_string());
                        }
                    }
                }
            }
            TaskEvent::State { to } => {
                let Some(task) = self.tasks.get_mut(id) else {
                    return;
                };
                // The acceptance gate: reported is not accepted.
                if to == TaskState::Completed && !task.verified() {
                    return;
                }
                if can_transition(task.state, to) {
                    task.state = to;
                }
            }
            TaskEvent::Reported { detail } => {
                if let Some(task) = self.tasks.get_mut(id) {
                    task.reported = Some(detail);
                }
            }
            TaskEvent::Verified { verifier, ok } => {
                if let Some(task) = self.tasks.get_mut(id) {
                    task.verifications.push(Verification { verifier, ok });
                }
            }
            TaskEvent::Expired { reason } => {
                let child_index = self
                    .tasks
                    .get(id)
                    .map(|t| t.children.len() + 1)
                    .unwrap_or(1);
                let child_id = format!("{id}~expired{child_index}");
                if let Some(task) = self.tasks.get_mut(id) {
                    task.expired_reason = Some(reason.clone());
                    if can_transition(task.state, TaskState::Intervened) {
                        task.state = TaskState::Intervened;
                    }
                }
                // The child names its parent and carries the reason, so the
                // retry is traceable to why it exists.
                self.apply(
                    &child_id,
                    TaskEvent::Created {
                        title: format!("retry of {id}: {reason}"),
                        parent: Some(id.to_string()),
                        depends_on: Vec::new(),
                    },
                );
            }
        }
    }

    /// Move a task, refusing an illegal or unverified transition with a reason.
    /// This is the direct API; the fold uses the same rules.
    pub fn transition(&mut self, id: &str, to: TaskState) -> Result<(), String> {
        let task = self
            .tasks
            .get_mut(id)
            .ok_or_else(|| format!("no such task: {id}"))?;
        if to == TaskState::Completed && !task.verified() {
            return Err(format!(
                "task {id} is reported but not verified; a verifier record is required to complete"
            ));
        }
        if !can_transition(task.state, to) {
            return Err(format!(
                "illegal transition for {id}: {:?} -> {to:?}",
                task.state
            ));
        }
        task.state = to;
        Ok(())
    }

    /// Expire a task: it gets a reason and a child that names its parent.
    pub fn expire(&mut self, id: &str, reason: &str) -> Result<String, String> {
        if !self.tasks.contains_key(id) {
            return Err(format!("no such task: {id}"));
        }
        self.apply(
            id,
            TaskEvent::Expired {
                reason: reason.to_string(),
            },
        );
        let child = self
            .tasks
            .get(id)
            .and_then(|t| t.children.last().cloned())
            .ok_or_else(|| format!("expire produced no child for {id}"))?;
        Ok(child)
    }

    pub fn get(&self, id: &str) -> Option<&Task> {
        self.tasks.get(id)
    }

    /// A text view for the CLI: one line per task, children indented.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for (id, t) in &self.tasks {
            let flag = if t.verified() {
                "verified"
            } else if t.reported.is_some() {
                "reported"
            } else {
                "-"
            };
            out.push_str(&format!(
                "  {:<24} {:<12} {:<9} {}",
                id,
                format!("{:?}", t.state).to_lowercase(),
                flag,
                t.title
            ));
            if let Some(p) = &t.parent {
                out.push_str(&format!("  (parent {p})"));
            }
            if let Some(r) = &t.expired_reason {
                out.push_str(&format!("  (expired: {r})"));
            }
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(seq: u64, task: &str, ev: TaskEvent) -> Observation {
        let mut o = TaskEvent::observation(task, &ev);
        o.seq = seq;
        o
    }

    fn created(title: &str) -> TaskEvent {
        TaskEvent::Created {
            title: title.to_string(),
            parent: None,
            depends_on: vec![],
        }
    }

    #[test]
    fn fold_is_replay_stable_and_order_independent_by_seq() {
        let log = vec![
            obs(1, "t1", created("do the thing")),
            obs(
                2,
                "t1",
                TaskEvent::State {
                    to: TaskState::InProgress,
                },
            ),
            obs(
                3,
                "t1",
                TaskEvent::Reported {
                    detail: "looks done".into(),
                },
            ),
            obs(
                4,
                "t1",
                TaskEvent::Verified {
                    verifier: "verifier".into(),
                    ok: true,
                },
            ),
            obs(
                5,
                "t1",
                TaskEvent::State {
                    to: TaskState::Completed,
                },
            ),
        ];
        let a = TaskGraph::fold(&log);
        let b = TaskGraph::fold(&log);
        assert_eq!(a, b, "same observations, same graph");

        // Reversing the slice must not change the fold: order comes from seq.
        let mut shuffled = log.clone();
        shuffled.reverse();
        assert_eq!(a, TaskGraph::fold(&shuffled));

        assert_eq!(a.get("t1").unwrap().state, TaskState::Completed);
        assert!(a.get("t1").unwrap().verified());
    }

    #[test]
    fn reported_is_never_accepted_without_a_verifier_record() {
        let log = vec![
            obs(1, "t2", created("ship it")),
            obs(
                2,
                "t2",
                TaskEvent::State {
                    to: TaskState::InProgress,
                },
            ),
            obs(
                3,
                "t2",
                TaskEvent::Reported {
                    detail: "done, trust me".into(),
                },
            ),
            // No verification: this must not complete.
            obs(
                4,
                "t2",
                TaskEvent::State {
                    to: TaskState::Completed,
                },
            ),
        ];
        let g = TaskGraph::fold(&log);
        let t = g.get("t2").unwrap();
        assert_eq!(t.state, TaskState::InProgress, "reported is not accepted");
        assert_eq!(t.reported.as_deref(), Some("done, trust me"));

        // Direct API refuses too, with a reason.
        let mut g2 = TaskGraph::fold(&log);
        let err = g2.transition("t2", TaskState::Completed).unwrap_err();
        assert!(err.contains("not verified"), "{err}");

        // A verifier record opens the gate.
        let mut g3 = TaskGraph::fold(&log);
        g3.apply(
            "t2",
            TaskEvent::Verified {
                verifier: "verifier".into(),
                ok: true,
            },
        );
        assert!(g3.transition("t2", TaskState::Completed).is_ok());
    }

    #[test]
    fn a_failing_verification_does_not_open_the_gate() {
        let mut g = TaskGraph::fold(&[obs(1, "t3", created("x"))]);
        g.apply(
            "t3",
            TaskEvent::Verified {
                verifier: "verifier".into(),
                ok: false,
            },
        );
        assert!(!g.get("t3").unwrap().verified());
        assert!(g.transition("t3", TaskState::Completed).is_err());
    }

    #[test]
    fn expire_creates_a_child_naming_parent_and_reason() {
        let mut g = TaskGraph::fold(&[obs(1, "t4", created("long job"))]);
        let child = g.expire("t4", "deadline passed").unwrap();
        let c = g.get(&child).unwrap();
        assert_eq!(c.parent.as_deref(), Some("t4"));
        assert!(c.title.contains("deadline passed"), "{}", c.title);
        assert_eq!(
            g.get("t4").unwrap().expired_reason.as_deref(),
            Some("deadline passed")
        );
        assert_eq!(g.get("t4").unwrap().state, TaskState::Intervened);
        assert_eq!(g.get("t4").unwrap().children, vec![child.clone()]);

        // And it survives a round trip through the log.
        let mut log = vec![obs(1, "t4", created("long job"))];
        log.push(obs(
            2,
            "t4",
            TaskEvent::Expired {
                reason: "deadline passed".into(),
            },
        ));
        let folded = TaskGraph::fold(&log);
        assert!(folded.get("t4~expired1").is_some());
        assert_eq!(
            folded.get("t4~expired1").unwrap().parent.as_deref(),
            Some("t4")
        );
    }

    #[test]
    fn illegal_transitions_are_refused() {
        let mut g = TaskGraph::fold(&[obs(1, "t5", created("x"))]);
        // Pending -> Completed is not legal even verified.
        g.apply(
            "t5",
            TaskEvent::Verified {
                verifier: "v".into(),
                ok: true,
            },
        );
        assert!(g.transition("t5", TaskState::Completed).is_err());
        assert!(g.transition("t5", TaskState::InProgress).is_ok());
        assert!(g.transition("t5", TaskState::Completed).is_ok());
        // Terminal.
        assert!(g.transition("t5", TaskState::InProgress).is_err());
    }
}
