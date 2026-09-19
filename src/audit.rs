//! Append-only mutation log (Phase 16 I9/I10) — the audit/trust layer.
//!
//! Every graph mutation — register, edit, delete, assert, edge add — appends a
//! sealed row. The log is append-only and never rewritten, so "what changed,
//! when, and by whom" is replayable and auditable — the property an industrial
//! buyer requires before the "audit trail" claim is honest.
//!
//! It is a *log*, not a source of truth: the nodes/ledger remain authoritative,
//! and the log is what lets you answer "what did the graph look like when OEE
//! dropped". Combined with `/coherence/nodes/all?as_of=` (point-in-time node
//! existence) it becomes a time-travel audit view.
//!
//! Per-tenant, persisted to sled as a whole-list row (`audit:{uid}`), capped to
//! [`MAX_AUDIT`] rows — recent truth, not unbounded history.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Ceiling on retained audit rows per tenant. Audit answers "what changed
/// recently, and was it this?" — unbounded growth would be its own liability.
pub const MAX_AUDIT: usize = 10_000;

/// One sealed mutation record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Monotonic per-tenant sequence — stable across replays.
    pub seq: u64,
    /// When the mutation happened.
    pub at: DateTime<Utc>,
    /// Mutation op: `register`, `update`, `edit`, `delete`, `assert`, `edge:add`.
    pub op: String,
    /// The node (or edge) id this mutation touched.
    pub subject_id: String,
    /// Object kind (I5) of the subject, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_kind: Option<String>,
    /// Who caused it — the tenant header user, or `api`.
    pub actor: String,
    /// Short human-readable note (label change, verdict, relation).
    pub detail: String,
}

#[derive(Debug, Clone, Default)]
pub struct MutationLog {
    entries: Vec<AuditEntry>,
    next_seq: u64,
}

impl MutationLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a sealed row. Never rewrites history; drops the oldest beyond
    /// [`MAX_AUDIT`].
    pub fn record(
        &mut self,
        op: &str,
        subject_id: &str,
        subject_kind: Option<String>,
        actor: &str,
        detail: impl Into<String>,
    ) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.entries.push(AuditEntry {
            seq,
            at: Utc::now(),
            op: op.to_string(),
            subject_id: subject_id.to_string(),
            subject_kind,
            actor: actor.to_string(),
            detail: detail.into(),
        });
        if self.entries.len() > MAX_AUDIT {
            let drop = self.entries.len() - MAX_AUDIT;
            self.entries.drain(0..drop);
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Newest-first, capped to `n`.
    pub fn recent(&self, n: usize) -> Vec<&AuditEntry> {
        self.entries.iter().rev().take(n).collect()
    }

    /// Everything touching one subject, oldest first (the change history of a
    /// single node — the "what happened to this work order" view).
    pub fn for_subject(&self, subject_id: &str) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.subject_id == subject_id)
            .collect()
    }

    /// Everything at or before `cut` — the time-travel slice (I10).
    pub fn as_of(&self, cut: DateTime<Utc>) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| e.at <= cut).collect()
    }

    pub fn audit_key(uid: &str) -> String {
        format!("audit:{uid}")
    }

    pub fn save(&self, db: &sled::Db, uid: &str) -> anyhow::Result<()> {
        db.insert(Self::audit_key(uid), serde_json::to_vec(&self.entries)?)?;
        Ok(())
    }

    pub fn load(db: &sled::Db, uid: &str) -> Self {
        let mut log = Self::new();
        if let Ok(Some(raw)) = db.get(Self::audit_key(uid)) {
            if let Ok(entries) = serde_json::from_slice::<Vec<AuditEntry>>(&raw) {
                log.next_seq = entries
                    .iter()
                    .map(|e| e.seq.saturating_add(1))
                    .max()
                    .unwrap_or(0);
                log.entries = entries;
            }
        }
        log
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_appends_and_is_append_only() {
        let mut log = MutationLog::new();
        log.record("register", "n1", Some("machine".into()), "a", "registered");
        log.record("assert", "n1", Some("machine".into()), "a", "verdict -0.8");
        assert_eq!(log.len(), 2);
        assert_eq!(log.entries[0].seq, 0);
        assert_eq!(log.entries[1].seq, 1);
        // recent() is newest-first.
        assert_eq!(log.recent(1)[0].op, "assert");
    }

    #[test]
    fn for_subject_returns_full_history_of_a_node() {
        let mut log = MutationLog::new();
        log.record("register", "n1", None, "a", "r");
        log.record("edit", "n2", None, "a", "e");
        log.record("delete", "n1", None, "a", "d");
        let hist = log.for_subject("n1");
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0].op, "register");
        assert_eq!(hist[1].op, "delete");
    }

    #[test]
    fn as_of_filters_by_time() {
        let mut log = MutationLog::new();
        log.record("register", "n1", None, "a", "r");
        let cut = Utc::now();
        log.record("assert", "n1", None, "a", "v");
        assert_eq!(log.as_of(cut).len(), 1);
        assert_eq!(log.as_of(cut)[0].op, "register");
    }

    #[test]
    fn capped_at_max_audit() {
        let mut log = MutationLog::new();
        for i in 0..(MAX_AUDIT + 5) {
            log.record("register", &format!("n{i}"), None, "a", "r");
        }
        assert_eq!(log.len(), MAX_AUDIT);
        assert_eq!(log.entries.first().unwrap().subject_id, "n5");
    }

    #[test]
    fn save_then_load_round_trips_and_resumes_seq() {
        let tag: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("physis_aud_{}_{}", std::process::id(), tag));
        std::fs::create_dir_all(&dir).unwrap();
        let db = sled::open(dir.join("a.sled")).unwrap();

        let mut log = MutationLog::new();
        log.record("register", "n1", None, "a", "r");
        log.save(&db, "t").unwrap();

        let loaded = MutationLog::load(&db, "t");
        assert_eq!(loaded.len(), 1);
        // Seq resumes, so appends stay monotonic across restarts.
        let mut loaded = loaded;
        loaded.record("assert", "n1", None, "a", "v");
        assert_eq!(loaded.entries[1].seq, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
