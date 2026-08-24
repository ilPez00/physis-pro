//! study — the learning / teaching layer. A Study Log records what a student
//! works on with full context metadata — **date, time, place, file type,
//! directory, and order** — then reduces it through the engine's machines:
//! the operational timeline (order + incidents), chronos (study rhythms),
//! elenchus (argument training), and discovery (concept gaps).
//!
//! Every study action is a `StudyEvent`; the recorder (`physis-pro study`) turns
//! filesystem activity into events via `diff_snapshots` (pure + testable); the
//! web server persists them per tenant and `session()` aggregates them into a
//! `StudySession` the `/study` dashboard renders.
//!
//! Study events are deliberately `OperationalEvent`-shaped on the way out
//! (provenance connector `study`), so they flow through the same timeline
//! incident detection the shop floor uses — but never mix with it.

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::machines::chronos::{detect_rhythms, TimedEvent};
use crate::operational::{
    OperationalEvent, OperationalKind, OperationalProvenance, OperationalStatus,
};
use crate::operational_report::OperationalTimeline;

/// What the student did to a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StudyAction {
    /// A new file the student created.
    Generated,
    /// An existing file the student changed.
    Edited,
    /// A file that was already open at the start of the session.
    Opened,
    /// A file the student deleted.
    Deleted,
}

impl StudyAction {
    pub fn as_str(self) -> &'static str {
        match self {
            StudyAction::Generated => "generated",
            StudyAction::Edited => "edited",
            StudyAction::Opened => "opened",
            StudyAction::Deleted => "deleted",
        }
    }
}

/// One study event with its full context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyEvent {
    /// When the student worked on the file (RFC3339).
    pub observed_at: DateTime<Utc>,
    pub action: StudyAction,
    /// Absolute/relative path of the file.
    pub path: String,
    /// Context metadata: where the study happened (room, library, lab).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub place: String,
    /// File type, e.g. "md", "tex", "py". Derived from the path when absent.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub file_type: String,
}

impl StudyEvent {
    pub fn new(
        observed_at: DateTime<Utc>,
        action: StudyAction,
        path: impl Into<String>,
        place: impl Into<String>,
    ) -> Self {
        let path = path.into();
        let file_type = Path::new(&path)
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();
        Self {
            observed_at,
            action,
            path,
            place: place.into(),
            file_type,
        }
    }

    fn name(&self) -> String {
        Path::new(&self.path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.path.clone())
    }

    /// The file type as reported on the wire, or derived from the path when it
    /// was omitted (events built by `StudyEvent::new` already carry it).
    fn file_type_of(&self) -> String {
        if !self.file_type.is_empty() {
            return self.file_type.clone();
        }
        Path::new(&self.path)
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    fn dir(&self) -> String {
        Path::new(&self.path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| ".".to_string())
    }

    /// Project this study event onto the shared `OperationalEvent` model so the
    /// timeline pipeline (sort + incident) can be reused untouched.
    pub fn to_op_event(&self) -> OperationalEvent {
        let status = match self.action {
            StudyAction::Generated => OperationalStatus::Success,
            StudyAction::Edited => OperationalStatus::Info,
            StudyAction::Opened => OperationalStatus::Inert,
            StudyAction::Deleted => OperationalStatus::Warning,
        };
        let mut props = HashMap::new();
        props.insert("action".to_string(), self.action.as_str().to_string());
        props.insert("place".to_string(), self.place.clone());
        props.insert("dir".to_string(), self.dir());
        props.insert("file_type".to_string(), self.file_type_of());
        props.insert("path".to_string(), self.path.clone());
        let mut ev = OperationalEvent::new(
            OperationalKind::Document,
            status,
            self.observed_at,
            self.name(),
            OperationalProvenance::new("study", "file"),
        );
        ev.properties = props;
        ev
    }
}

/// A filesystem truth for the recorder: path + mtime + size. Two snapshots of
/// the same directory diff into study actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSnapshot {
    pub path: String,
    pub mtime_unix: u64,
    pub size: u64,
}

/// Diff two snapshots into per-file actions. Pure — the recorder feeds it one
/// snapshot per poll tick.
///
/// - path present in `cur` but not `prev` → `Generated`
/// - path present in both but mtime or size changed → `Edited`
/// - path present in `prev` but not `cur` → `Deleted`
/// - callers may reinterpret the *first* tick (empty `prev`) as `Opened`
pub fn diff_snapshots(prev: &[FileSnapshot], cur: &[FileSnapshot]) -> Vec<(String, StudyAction)> {
    let prev_map: HashMap<&str, &FileSnapshot> =
        prev.iter().map(|f| (f.path.as_str(), f)).collect();
    let cur_map: HashMap<&str, &FileSnapshot> = cur.iter().map(|f| (f.path.as_str(), f)).collect();
    let mut out = Vec::new();
    for f in cur {
        match prev_map.get(f.path.as_str()) {
            None => out.push((f.path.clone(), StudyAction::Generated)),
            Some(p) if p.mtime_unix != f.mtime_unix || p.size != f.size => {
                out.push((f.path.clone(), StudyAction::Edited))
            }
            Some(_) => {}
        }
    }
    for f in prev {
        if !cur_map.contains_key(f.path.as_str()) {
            out.push((f.path.clone(), StudyAction::Deleted));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Walk a directory tree (min_depth 1) into a sorted snapshot. Skips hidden
/// dirs (.git, target, node_modules) so working dirs stay clean.
pub fn snapshot_dir(root: &Path) -> std::io::Result<Vec<FileSnapshot>> {
    let mut snap = Vec::new();
    let mut queue = vec![root.to_path_buf()];
    while let Some(dir) = queue.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let p = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if p.is_dir() {
                queue.push(p);
            } else {
                if let Ok(meta) = entry.metadata() {
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    snap.push(FileSnapshot {
                        path: p.to_string_lossy().to_string(),
                        mtime_unix: mtime,
                        size: meta.len(),
                    });
                }
            }
        }
    }
    snap.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(snap)
}

/// A rhythm summary (chronos's `Rhythm` isn't Serialize; this is).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyRhythm {
    pub label: String,
    pub period_days: f64,
    pub count: usize,
    pub last: DateTime<Utc>,
    pub next_due: DateTime<Utc>,
    pub confidence: f32,
}

/// A file-type / directory tally, sorted by count descending.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tally {
    pub key: String,
    pub count: usize,
}

/// Aggregated view of a student's session — everything the `/study`
/// dashboard renders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudySession {
    #[serde(default)]
    pub events: Vec<StudyEvent>,
    pub timeline: OperationalTimeline,
    /// Files by type, e.g. "md" → 14, desc.
    #[serde(default)]
    pub by_type: Vec<Tally>,
    /// Files by directory, desc.
    #[serde(default)]
    pub by_dir: Vec<Tally>,
    /// Activity split: generated / edited / opened / deleted.
    #[serde(default)]
    pub by_action: Vec<Tally>,
    /// chronos study rhythms (a label needs ≥ 3 occurrences).
    #[serde(default)]
    pub rhythms: Vec<StudyRhythm>,
}

fn tally<I: IntoIterator<Item = String>>(items: I) -> Vec<Tally> {
    let mut map: HashMap<String, usize> = HashMap::new();
    for k in items {
        *map.entry(k).or_insert(0) += 1;
    }
    let mut v: Vec<Tally> = map
        .into_iter()
        .map(|(key, count)| Tally { key, count })
        .collect();
    v.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
    v
}

/// Aggregate study events into a session report.
pub fn session(events: &[StudyEvent]) -> StudySession {
    let ops: Vec<OperationalEvent> = events.iter().map(StudyEvent::to_op_event).collect();
    let timeline = OperationalTimeline::from_events(&ops);

    let rhythm_events: Vec<TimedEvent> = events
        .iter()
        .map(|e| TimedEvent {
            label: format!("{} / {}", e.dir(), e.name()),
            at: e.observed_at,
        })
        .collect();
    let rhythms = detect_rhythms(&rhythm_events)
        .into_iter()
        .map(|r| StudyRhythm {
            label: r.label,
            period_days: r.period_days,
            count: r.count,
            last: r.last,
            next_due: r.next_due,
            confidence: r.confidence,
        })
        .collect();

    StudySession {
        events: events.to_vec(),
        timeline,
        by_type: tally(events.iter().map(|e| {
            let t = e.file_type_of();
            if t.is_empty() {
                "none".to_string()
            } else {
                t
            }
        })),
        by_dir: tally(events.iter().map(|e| e.dir())),
        by_action: tally(events.iter().map(|e| e.action.as_str().to_string())),
        rhythms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(path: &str, mtime: u64, size: u64) -> FileSnapshot {
        FileSnapshot {
            path: path.into(),
            mtime_unix: mtime,
            size,
        }
    }

    #[test]
    fn diff_snapshots_detects_generate_edit_delete() {
        let prev = vec![
            snap("a.md", 100, 5),
            snap("b.md", 200, 5),
            snap("gone.md", 300, 5),
        ];
        let cur = vec![
            snap("a.md", 101, 5),   // changed mtime → edited
            snap("b.md", 200, 9),   // changed size → edited
            snap("new.md", 400, 5), // new → generated
        ];
        let mut got = diff_snapshots(&prev, &cur);
        got.sort_by(|a, b| a.0.cmp(&b.0));
        let got: Vec<(String, String)> = got
            .into_iter()
            .map(|(p, a)| (p, a.as_str().to_string()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("a.md".into(), "edited".into()),
                ("b.md".into(), "edited".into()),
                ("gone.md".into(), "deleted".into()),
                ("new.md".into(), "generated".into()),
            ]
        );
    }

    #[test]
    fn empty_prev_means_everything_is_new() {
        let cur = vec![snap("a.md", 1, 2), snap("b.tex", 3, 4)];
        let got: Vec<(String, String)> = diff_snapshots(&[], &cur)
            .into_iter()
            .map(|(p, a)| (p, a.as_str().to_string()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("a.md".into(), "generated".into()),
                ("b.tex".into(), "generated".into())
            ]
        );
    }

    #[test]
    fn event_carries_full_context_metadata() {
        let t = DateTime::parse_from_rfc3339("2026-08-13T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let e = StudyEvent::new(
            t,
            StudyAction::Generated,
            "/home/u/topo/notes/lecture-03.md",
            "Library",
        );
        assert_eq!(e.file_type, "md");
        assert_eq!(e.dir(), "/home/u/topo/notes");
        assert_eq!(e.name(), "lecture-03.md");
        let op = e.to_op_event();
        assert_eq!(op.provenance.connector, "study");
        assert_eq!(op.status, OperationalStatus::Success);
        assert_eq!(op.properties.get("place").unwrap(), "Library");
        assert_eq!(op.properties.get("action").unwrap(), "generated");
    }

    #[test]
    fn session_orders_timeline_and_tallies_by_type_dir_action() {
        let t0 = DateTime::parse_from_rfc3339("2026-08-13T09:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let t1 = DateTime::parse_from_rfc3339("2026-08-13T09:10:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let t2 = DateTime::parse_from_rfc3339("2026-08-13T09:20:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let evs = vec![
            StudyEvent::new(t2, StudyAction::Edited, "d/notes/b.md", "home"),
            StudyEvent::new(t0, StudyAction::Generated, "d/notes/a.md", "home"),
            StudyEvent::new(t1, StudyAction::Generated, "d/ex/f.py", "home"),
        ];
        let s = session(&evs);
        assert_eq!(s.timeline.entries.len(), 3);
        assert!(
            s.timeline.entries[0].observed_at <= s.timeline.entries[2].observed_at,
            "timeline must be oldest-first"
        );
        assert_eq!(
            s.by_action
                .iter()
                .map(|t| (t.key.as_str(), t.count))
                .collect::<Vec<_>>(),
            vec![("generated", 2), ("edited", 1)]
        );
        let types: Vec<(String, usize)> =
            s.by_type.iter().map(|t| (t.key.clone(), t.count)).collect();
        assert_eq!(types, vec![("md".into(), 2), ("py".into(), 1)]);
        assert_eq!(s.by_dir.len(), 2);
        assert!(
            s.rhythms.is_empty(),
            "fewer than 3 occurrences per label → no rhythm"
        );
    }

    #[test]
    fn session_detects_a_daily_rhythm() {
        let base = DateTime::parse_from_rfc3339("2026-08-10T08:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let days: Vec<i64> = vec![0, 1, 2, 3, 4, 5];
        let evs: Vec<StudyEvent> = days
            .iter()
            .map(|d| {
                StudyEvent::new(
                    base + chrono::Duration::days(*d),
                    StudyAction::Edited,
                    "notes/inventory.md",
                    "home",
                )
            })
            .collect();
        let s = session(&evs);
        let r = &s.rhythms[0];
        assert_eq!(r.label, "notes / inventory.md");
        assert!(
            (r.period_days - 1.0).abs() < 0.01,
            "daily rhythm, got {}",
            r.period_days
        );
        assert!(r.confidence > 0.5);
    }
}
