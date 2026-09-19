//! Append-only transition log for the MVP loop.
//!
//! Every meaningful interaction becomes one versioned record:
//! `(timestamp, S_t, intent/action A_t, observed Δ, validation/outcome,
//! S*_{t+1}, provenance, uncertainty)`.
//!
//! Content-addressed snapshots (sha256 of canonical JSON) + compact deltas,
//! never full-state duplication, never overwrite. Supports future Time Machine
//! replay and world-model training without redesign; neither is built here.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const LOG_VERSION: u32 = 1;

/// Stable state pointer: content hash + object/file counts + map hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateRef {
    pub content_hash: String,
    pub objects: usize,
    pub files: usize,
    pub map_hash: String,
}

/// One transition record. All fields present so future replay/training needs
/// no redesign; unknown future fields are ignored via serde defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    pub v: u32,
    pub seq: u64,
    pub timestamp: String,
    pub state_before: StateRef,
    pub intent: String,
    pub action: String,
    pub delta: CompactDelta,
    pub validation: Vec<String>,
    pub outcome: String,
    pub state_after: StateRef,
    pub provenance: Vec<String>,
    pub uncertainty: f32,
    /// The world slice this transition ran against. `None` for interactions
    /// that did not scope one (a plain inspect of an already-focused object).
    #[serde(default)]
    pub slice: Option<SliceCost>,
}

/// What Physis handed the model for this transition: the measured size of the
/// world slice, not an estimate.
///
/// Recorded per transition because it is the evidence for the MVP's central
/// claim (a model received the relevant portion, not the undifferentiated
/// world), and because a future world-model / transplant learner needs the
/// slice's *shape* — how much of the world the action ran against — alongside
/// the state and the delta.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SliceCost {
    /// Repository objects the retrieval ranked over.
    pub objects_considered: usize,
    /// Objects whose text entered the slice.
    pub objects_selected: usize,
    /// Tokens of every candidate window — the undifferentiated world.
    pub candidate_tokens: usize,
    /// Tokens actually handed over.
    pub final_tokens: usize,
    /// Retrieval backend that answered (never assumed).
    pub provider: String,
    /// Paths the slice was assembled from.
    pub paths: Vec<String>,
}

/// Compact delta: path lists + counts, not full text.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompactDelta {
    pub changed: Vec<String>,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub objects_remapped: usize,
    pub neighborhoods_changed: usize,
    pub relations_added: Vec<String>,
    pub relations_removed: Vec<String>,
    pub no_change: bool,
}

pub fn hash_canonical(v: &impl Serialize) -> String {
    let json = serde_json::to_string(v).unwrap_or_default();
    let mut h = Sha256::new();
    h.update(json.as_bytes());
    format!("{:x}", h.finalize())
}

/// Notice for edits that landed while the loop was closed: the opener absorbs
/// them into its snapshot, so a bare `rescan` afterwards would (correctly per
/// its code, wrongly per the demo) report nothing. Diff the last recorded
/// `state_after` content hash against the fresh scan and say so. Nothing is
/// re-recorded — the log stays append-only; the banner only reports what the
/// open already observed. Shared by the REPL banner and the HUD startup line.
pub fn since_last_session_notice(log: &TransitionLog, fresh_hash: &str) -> Option<String> {
    let all = log.read_all();
    let last = all.last()?;
    if last.state_after.content_hash.is_empty() || last.state_after.content_hash == fresh_hash {
        return None;
    }
    Some(format!(
        "since last session: workspace changed ({} → {})\nrun `rescan` only for in-session edits — this Δ was absorbed at open",
        &last.state_after.content_hash[..8.min(last.state_after.content_hash.len())],
        &fresh_hash[..8.min(fresh_hash.len())],
    ))
}

pub struct TransitionLog {
    path: PathBuf,
    next_seq: u64,
}

impl TransitionLog {
    pub fn open(root: &Path) -> anyhow::Result<Self> {
        let dir = root.join(".physis");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("mvp_transitions.jsonl");
        let next_seq = std::fs::read_to_string(&path)
            .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count() as u64)
            .unwrap_or(0);
        Ok(Self { path, next_seq })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&mut self, mut t: Transition) -> anyhow::Result<u64> {
        t.v = LOG_VERSION;
        t.seq = self.next_seq;
        t.timestamp = chrono::Utc::now().to_rfc3339();
        let line = serde_json::to_string(&t)?;
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(f, "{line}")?;
        self.next_seq += 1;
        Ok(t.seq)
    }

    pub fn read_all(&self) -> Vec<Transition> {
        std::fs::read_to_string(&self.path)
            .map(|s| {
                s.lines()
                    .filter(|l| !l.trim().is_empty())
                    .filter_map(|l| serde_json::from_str(l).ok())
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(n: usize) -> StateRef {
        StateRef {
            content_hash: format!("h{n}"),
            objects: n,
            files: n,
            map_hash: format!("m{n}"),
        }
    }

    fn transition(intent: &str) -> Transition {
        Transition {
            v: 0,
            seq: 0,
            timestamp: String::new(),
            state_before: state(1),
            intent: intent.to_string(),
            action: "test".to_string(),
            delta: CompactDelta::default(),
            validation: vec![],
            outcome: "ok".to_string(),
            state_after: state(2),
            provenance: vec!["src/lib.rs".to_string()],
            uncertainty: 0.25,
            slice: None,
        }
    }

    /// The record carries everything a future Time Machine / world-model
    /// trainer needs, and the version is stamped at write time.
    #[test]
    fn records_are_versioned_and_complete() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = TransitionLog::open(dir.path()).unwrap();
        log.append(transition("a")).unwrap();
        let all = log.read_all();
        assert_eq!(all.len(), 1);
        let t = &all[0];
        assert_eq!(t.v, LOG_VERSION);
        assert_eq!(t.seq, 0);
        assert!(
            !t.timestamp.is_empty(),
            "timestamp must be stamped on write"
        );
        assert_eq!(t.state_before.objects, 1);
        assert_eq!(t.state_after.objects, 2);
        assert!(t.uncertainty >= 0.0);
    }

    /// Append-only: history is never rewritten, and sequence resumes across
    /// reopen so a replay can order the whole session.
    #[test]
    fn history_is_append_only_and_sequence_continues() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut log = TransitionLog::open(dir.path()).unwrap();
            log.append(transition("first")).unwrap();
            log.append(transition("second")).unwrap();
        }
        let mut log = TransitionLog::open(dir.path()).unwrap();
        let seq = log.append(transition("third")).unwrap();
        assert_eq!(seq, 2, "sequence resumes at the existing count");
        let all = log.read_all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].intent, "first");
        assert_eq!(all[2].intent, "third");
    }

    /// An action that changed nothing is still a transition: the absence of a
    /// delta is the observation.
    #[test]
    fn no_change_transitions_are_recorded() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = TransitionLog::open(dir.path()).unwrap();
        let mut t = transition("inspect");
        t.delta.no_change = true;
        log.append(t).unwrap();
        let all = log.read_all();
        assert!(all[0].delta.no_change);
        assert!(all[0].delta.changed.is_empty());
    }

    /// The world slice is part of the record: a learner needs to know what the
    /// action ran against, and a reviewer needs the compression it bought.
    #[test]
    fn slice_cost_round_trips_in_the_record() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = TransitionLog::open(dir.path()).unwrap();
        let mut t = transition("scope");
        t.slice = Some(SliceCost {
            objects_considered: 2992,
            objects_selected: 8,
            candidate_tokens: 1_204_946,
            final_tokens: 743,
            provider: "random-projection".into(),
            paths: vec!["src/rag.rs".into()],
        });
        log.append(t).unwrap();
        let s = log.read_all()[0].slice.clone().expect("slice must persist");
        assert_eq!(s.objects_considered, 2992);
        assert_eq!(s.final_tokens, 743);
        assert!(s.candidate_tokens > s.final_tokens);
        assert_eq!(s.paths, vec!["src/rag.rs".to_string()]);
    }

    #[test]
    fn content_hash_is_stable_and_content_addressed() {
        let a = hash_canonical(&vec!["x".to_string()]);
        let b = hash_canonical(&vec!["x".to_string()]);
        let c = hash_canonical(&vec!["y".to_string()]);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    /// The absorbed-at-open notice fires only when the workspace moved while
    /// the loop was closed; unchanged state and an empty log stay silent.
    #[test]
    fn since_last_session_notice_fires_only_on_change() {
        let dir = tempfile::tempdir().unwrap();
        let log = TransitionLog::open(dir.path()).unwrap();
        assert!(since_last_session_notice(&log, "h2").is_none());
        let mut log = TransitionLog::open(dir.path()).unwrap();
        log.append(transition("edit")).unwrap(); // state_after.content_hash = "h2"
        assert!(since_last_session_notice(&log, "h2").is_none());
        let note = since_last_session_notice(&log, "h9").expect("must notice drift");
        assert!(note.contains("h2") && note.contains("h9"), "{note}");
    }
}
