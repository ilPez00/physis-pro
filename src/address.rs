//! File addressing (PLAN Phase 10) — where a file lives in space, when it
//! matters in time, and why it exists (cause), filed alongside the meaning
//! grid. The theme graph answers *what a file means*; an address answers
//! *where / when / why-made*. See docs/plans/2026-08-26-space-time-cause-datawork.md.
//!
//! Framing: Aristotle's four causes as filing dimensions. Formal (the
//! DOMAIN×MODE cell) and material (DocKind) already exist elsewhere; this
//! module carries efficient (what produced it) and final (what it serves),
//! plus the two physical axes.

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Where a file lives: a named place (space ontology id) plus the canonical
/// root it was scanned under. Unknown places stay raw roots until onboarding
/// names them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpaceRef {
    /// Place label — resolved against the space ontology when loaded,
    /// otherwise the canonical scan root itself.
    pub place: String,
    /// Canonical directory root the file was ingested under.
    pub root: String,
}

/// When a file matters: filesystem reality plus the period bucket it lands in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeRef {
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    /// When this engine observed/registered the file.
    pub ingested: DateTime<Utc>,
    /// Rhythm bucket from the time ontology (`YYYY-MM` month form by default).
    pub period: String,
}

/// How a file came to be (efficient cause). One file's producing event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CauseRef {
    pub mechanism: Mechanism,
    /// Who/what ran the mechanism ("watch", "vault-import", "agent", …).
    pub actor: String,
    /// When the producing event happened (may precede `TimeRef::ingested`).
    pub at: DateTime<Utc>,
}

/// Vocabulary of production mechanisms — the causal ledger (D3) reuses these
/// as event types so addresses and lineage never drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mechanism {
    /// Brought into being by a tool/actor running here (scan, watch, agent).
    Generated,
    /// Taken in from outside unchanged (import, download, mail).
    Received,
    /// Produced from another artifact of ours (export, report, backfill).
    Derived,
    /// A later event changed an existing artifact (edit, correction).
    Corrected,
    /// Referenced by a node/report without changing the file.
    Cited,
    /// Attached to a goal/workflow — final cause recorded as history.
    Serves,
}

impl Mechanism {
    pub fn as_str(&self) -> &'static str {
        match self {
            Mechanism::Generated => "generated",
            Mechanism::Received => "received",
            Mechanism::Derived => "derived",
            Mechanism::Corrected => "corrected",
            Mechanism::Cited => "cited",
            Mechanism::Serves => "serves",
        }
    }
}

/// The four-coordinate address of one file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileAddress {
    pub space: SpaceRef,
    pub time: TimeRef,
    pub cause: CauseRef,
}

impl FileAddress {
    /// Derive an address at ingestion time. `root` is the canonical scan/import
    /// root; place defaults to it until the space ontology names it. Filesystem
    /// metadata failures degrade to `now` rather than failing ingestion — an
    /// approximate address beats no registration.
    pub fn derive(
        path: &Path,
        root: &Path,
        mechanism: Mechanism,
        actor: &str,
        ingested: DateTime<Utc>,
    ) -> Self {
        let (created, modified) = fs_times(path, ingested);
        Self {
            space: SpaceRef {
                place: root.display().to_string(),
                root: root.display().to_string(),
            },
            time: TimeRef {
                created,
                modified,
                ingested,
                period: period_of(modified),
            },
            cause: CauseRef {
                mechanism,
                actor: actor.to_string(),
                at: ingested,
            },
        }
    }
}

/// Read (birth, mtime); birth often unavailable on Linux, so mtime stands in
/// for creation when the filesystem does not expose it.
fn fs_times(path: &Path, fallback: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    let to_utc = |t: std::io::Result<std::time::SystemTime>| {
        t.ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| {
                Utc.timestamp_opt(d.as_secs() as i64, d.subsec_nanos())
                    .single()
            })
            .unwrap_or(fallback)
    };
    match std::fs::metadata(path) {
        Ok(meta) => {
            let modified = to_utc(meta.modified());
            let created = to_utc(meta.created().or_else(|_| meta.modified()));
            (created, modified)
        }
        Err(_) => (fallback, fallback),
    }
}

/// Month bucket (`YYYY-MM`) — the default rhythm kind until the time ontology
/// assigns finer periods (shift/sprint) during onboarding.
pub fn period_of(at: DateTime<Utc>) -> String {
    use chrono::Datelike;
    format!("{:04}-{:02}", at.year(), at.month())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).single().unwrap()
    }

    #[test]
    fn derive_reads_fs_metadata_and_buckets_the_period() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("note.md");
        std::fs::write(&f, "hello").unwrap();
        let now = ts(1_756_000_000); // 2025-08-24-ish; any stable value works

        let a = FileAddress::derive(&f, dir.path(), Mechanism::Generated, "watch", now);
        assert_eq!(a.space.root, dir.path().display().to_string());
        assert_eq!(a.space.place, a.space.root);
        assert_eq!(a.cause.mechanism, Mechanism::Generated);
        assert_eq!(a.cause.actor, "watch");
        // Real file: mtime ≈ creation moment of the temp file, not our fallback.
        assert!(a.time.created > ts(1_600_000_000));
        assert_eq!(a.time.period.len(), 7, "YYYY-MM");
        assert_eq!(&a.time.period[4..5], "-");
    }

    #[test]
    fn missing_file_degrades_to_fallback_not_panic() {
        let now = ts(1_700_000_000);
        let a = FileAddress::derive(
            Path::new("/nonexistent/x.md"),
            Path::new("/"),
            Mechanism::Received,
            "import",
            now,
        );
        assert_eq!(a.time.created, now);
        assert_eq!(a.time.modified, now);
        assert_eq!(a.time.ingested, now);
        assert_eq!(a.time.period, period_of(now));
    }

    #[test]
    fn mechanism_vocabulary_is_stable() {
        // The ledger (D3) persists these strings; renaming would orphan rows.
        for (m, s) in [
            (Mechanism::Generated, "generated"),
            (Mechanism::Received, "received"),
            (Mechanism::Derived, "derived"),
            (Mechanism::Corrected, "corrected"),
            (Mechanism::Cited, "cited"),
            (Mechanism::Serves, "serves"),
        ] {
            assert_eq!(m.as_str(), s);
            assert_eq!(serde_json::to_value(m).unwrap(), serde_json::json!(s));
        }
    }

    #[test]
    fn address_round_trips_through_json() {
        let now = ts(1_750_000_000);
        let a = FileAddress::derive(
            Path::new("/tmp/a.txt"),
            Path::new("/tmp"),
            Mechanism::Derived,
            "praxis",
            now,
        );
        let v = serde_json::to_string(&a).unwrap();
        let b: FileAddress = serde_json::from_str(&v).unwrap();
        assert_eq!(a, b);
    }
}
