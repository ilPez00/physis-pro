//! Normalized operational events.
//!
//! This module is the common shape for machine telemetry, workflow steps,
//! email threads, system activity, and human verdicts. The goal is to keep
//! future connectors thin: they translate their native payloads into one
//! typed, timestamped record with provenance.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::industrial::{MachineReading, MachineStatus};
use crate::ingest::{DocKind, IngestedDoc};

/// Source family for an operational event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationalKind {
    Machine,
    Document,
    Email,
    Workflow,
    System,
    Verdict,
}

impl OperationalKind {
    pub fn as_str(self) -> &'static str {
        match self {
            OperationalKind::Machine => "machine",
            OperationalKind::Document => "document",
            OperationalKind::Email => "email",
            OperationalKind::Workflow => "workflow",
            OperationalKind::System => "system",
            OperationalKind::Verdict => "verdict",
        }
    }
}

/// Outcome or state associated with an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationalStatus {
    Success,
    Warning,
    Failure,
    Inert,
    Info,
}

impl OperationalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            OperationalStatus::Success => "success",
            OperationalStatus::Warning => "warning",
            OperationalStatus::Failure => "failure",
            OperationalStatus::Inert => "inert",
            OperationalStatus::Info => "info",
        }
    }
}

/// Where an event came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationalProvenance {
    /// Connector family, e.g. `industrial`, `ingest`, `email`, `moonraker`.
    pub connector: String,
    /// Normalized source family, e.g. `machine`, `document`, `workflow`.
    pub source: String,
    /// Optional native identifier like a machine id or message id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// Optional file path for watched files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// Optional URI or endpoint for network-originated events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

impl OperationalProvenance {
    pub fn new(connector: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            connector: connector.into(),
            source: source.into(),
            source_id: None,
            path: None,
            uri: None,
        }
    }
}

/// A normalized, timestamped operational record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationalEvent {
    /// Stable content hash for replay/dedupe.
    pub event_key: String,
    /// When the event occurred or was observed.
    pub observed_at: DateTime<Utc>,
    /// Optional duration in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<i64>,
    pub kind: OperationalKind,
    pub status: OperationalStatus,
    /// Human-readable subject, e.g. printer name, file title, or email subject.
    pub subject: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub properties: HashMap<String, String>,
    pub provenance: OperationalProvenance,
    #[serde(default)]
    pub raw: serde_json::Value,
}

impl OperationalEvent {
    pub fn new(
        kind: OperationalKind,
        status: OperationalStatus,
        observed_at: DateTime<Utc>,
        subject: impl Into<String>,
        provenance: OperationalProvenance,
    ) -> Self {
        let subject = subject.into();
        let mut event = Self {
            event_key: String::new(),
            observed_at,
            duration_secs: None,
            kind,
            status,
            subject,
            actor: None,
            target: None,
            machine_id: None,
            thread_id: None,
            project: None,
            evidence: Vec::new(),
            properties: HashMap::new(),
            provenance,
            raw: serde_json::Value::Null,
        };
        event.rekey();
        event
    }

    /// The causal-ledger mechanism this event corresponds to: external
    /// telemetry and documents are *received* from outside; workflow, system
    /// and verdict events are *generated* by the engine itself. Feeds the
    /// why-made chain so provenance survives into lineage and purposes.
    pub fn ledger_mechanism(&self) -> crate::address::Mechanism {
        match self.kind {
            OperationalKind::Machine
            | OperationalKind::Email
            | OperationalKind::Document => crate::address::Mechanism::Received,
            OperationalKind::Workflow
            | OperationalKind::System
            | OperationalKind::Verdict => crate::address::Mechanism::Generated,
        }
    }

    pub fn rekey(&mut self) {
        let mut hasher = Sha256::new();
        hasher.update(self.kind.as_str().as_bytes());
        hasher.update([0]);
        hasher.update(self.status.as_str().as_bytes());
        hasher.update([0]);
        hasher.update(self.observed_at.to_rfc3339().as_bytes());
        hasher.update([0]);
        hasher.update(self.subject.as_bytes());
        hasher.update([0]);
        hasher.update(self.provenance.connector.as_bytes());
        hasher.update([0]);
        hasher.update(self.provenance.source.as_bytes());
        hasher.update([0]);
        if let Some(id) = &self.provenance.source_id {
            hasher.update(id.as_bytes());
        }
        hasher.update([0]);
        if let Some(path) = &self.provenance.path {
            hasher.update(path.to_string_lossy().as_bytes());
        }
        hasher.update([0]);
        if let Some(uri) = &self.provenance.uri {
            hasher.update(uri.as_bytes());
        }
        self.event_key = format!("{:x}", hasher.finalize());
    }
}

impl From<&MachineReading> for OperationalEvent {
    fn from(reading: &MachineReading) -> Self {
        let status = match reading.status {
            MachineStatus::Online => OperationalStatus::Success,
            MachineStatus::Offline => OperationalStatus::Failure,
            MachineStatus::Warning => OperationalStatus::Warning,
            MachineStatus::Critical => OperationalStatus::Failure,
        };
        let mut provenance = OperationalProvenance::new("industrial", "machine");
        provenance.source_id = Some(reading.machine_id.clone());

        let mut event = OperationalEvent::new(
            OperationalKind::Machine,
            status,
            reading.timestamp,
            reading.machine_id.clone(),
            provenance,
        );
        event.machine_id = Some(reading.machine_id.clone());
        event
            .properties
            .insert("status".to_string(), reading.status.as_str().to_lowercase());
        if let Some(temp) = reading.temperature {
            event
                .properties
                .insert("temperature".to_string(), temp.to_string());
        }
        if let Some(pressure) = reading.pressure {
            event
                .properties
                .insert("pressure".to_string(), pressure.to_string());
        }
        if let Some(vibration) = reading.vibration {
            event
                .properties
                .insert("vibration".to_string(), vibration.to_string());
        }
        if let Some(rpm) = reading.rpm {
            event.properties.insert("rpm".to_string(), rpm.to_string());
        }
        event
            .properties
            .extend(reading.raw.iter().map(|(k, v)| (k.clone(), v.to_string())));
        event.raw = serde_json::to_value(reading).unwrap_or(serde_json::Value::Null);
        event.rekey();
        event
    }
}

impl From<&IngestedDoc> for OperationalEvent {    fn from(doc: &IngestedDoc) -> Self {
        let (kind, status) = match doc.kind {
            DocKind::Email => (OperationalKind::Email, OperationalStatus::Info),
            DocKind::VoiceTranscript
            | DocKind::Spreadsheet
            | DocKind::Report
            | DocKind::Invoice
            | DocKind::OcrDocument
            | DocKind::Image
            | DocKind::Video
            | DocKind::Unknown => (OperationalKind::Document, OperationalStatus::Info),
        };
        let mut provenance = OperationalProvenance::new("ingest", kind.as_str());
        provenance.path = Some(doc.path.clone());
        provenance.source_id = doc.meta.thread_id.clone().or_else(|| {
            doc.path
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        });

        let mut event =
            OperationalEvent::new(kind, status, doc.timestamp, doc.title.clone(), provenance);
        event.thread_id = doc.meta.thread_id.clone();
        event.actor = doc.meta.from.clone();
        event.target = doc.meta.vendor.clone();
        event.project = doc.ontology_tags.first().cloned();
        event.evidence.push(doc.body.clone());
        if let Some(from) = &doc.meta.from {
            event.properties.insert("from".to_string(), from.clone());
        }
        if let Some(subject) = &doc.meta.subject {
            event
                .properties
                .insert("subject".to_string(), subject.clone());
        }
        if let Some(thread_id) = &doc.meta.thread_id {
            event
                .properties
                .insert("thread_id".to_string(), thread_id.clone());
        }
        event
            .properties
            .insert("doc_kind".to_string(), format!("{:?}", doc.kind));
        event.raw = serde_json::to_value(doc).unwrap_or(serde_json::Value::Null);
        event.rekey();
        event
    }
}

/// Ceiling on stored events per tenant. Events are cheap and lossy by design —
/// the timeline and ledger are what matter — so the store is a bounded ring,
/// oldest dropped first, rather than unbounded history.
pub const MAX_STORED_EVENTS: usize = 10_000;

/// Sled key for one tenant's event ring. Whole-list write, exactly like goals —
/// the ring is append-mostly and small (≤ `MAX_STORED_EVENTS`), so rewriting
/// the row on each ingest is correct and simple.
pub fn events_key(uid: &str) -> String {
    format!("op_events:{uid}")
}

pub fn save_events(db: &sled::Db, uid: &str, events: &[OperationalEvent]) -> anyhow::Result<()> {
    db.insert(events_key(uid), serde_json::to_vec(events)?)?;
    Ok(())
}

pub fn load_events(db: &sled::Db, uid: &str) -> Vec<OperationalEvent> {
    db.get(events_key(uid))
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_slice::<Vec<OperationalEvent>>(&v).ok())
        .unwrap_or_default()
}

/// Append an event, dropping the oldest beyond [`MAX_STORED_EVENTS`].
pub fn push_capped(events: &mut Vec<OperationalEvent>, event: OperationalEvent) {
    events.push(event);
    if events.len() > MAX_STORED_EVENTS {
        let overflow = events.len() - MAX_STORED_EVENTS;
        events.drain(0..overflow);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::industrial::{MachineReading, MachineStatus};
    use crate::ingest::{DocKind, DocMeta, IngestedDoc};

    #[test]
    fn machine_reading_normalizes_to_event() {
        let reading = MachineReading {
            machine_id: "voron-01".to_string(),
            timestamp: DateTime::parse_from_rfc3339("2026-08-12T08:55:00Z")
                .unwrap()
                .with_timezone(&Utc),
            temperature: Some(78.5),
            pressure: None,
            vibration: Some(0.12),
            rpm: Some(1200.0),
            status: MachineStatus::Warning,
            raw: HashMap::from([(String::from("bed_target"), 60.0)]),
        };

        let event = OperationalEvent::from(&reading);
        assert_eq!(event.kind, OperationalKind::Machine);
        assert_eq!(event.status, OperationalStatus::Warning);
        assert_eq!(event.machine_id.as_deref(), Some("voron-01"));
        assert_eq!(
            event.properties.get("bed_target").map(String::as_str),
            Some("60")
        );
        assert!(!event.event_key.is_empty());
    }

    #[test]
    fn email_doc_normalizes_to_event() {
        let doc = IngestedDoc {
            path: PathBuf::from("/tmp/mail/eml/chain.eml"),
            kind: DocKind::Email,
            title: "Supplier delay".to_string(),
            body: "Replacement nozzle ships tomorrow.".to_string(),
            timestamp: DateTime::parse_from_rfc3339("2026-08-12T09:30:00Z")
                .unwrap()
                .with_timezone(&Utc),
             ontology_tags: vec!["workshop".to_string()],
            meta: DocMeta {
                vendor: None,
                from: Some("supplier@example.com".to_string()),
                date: None,
                doc_number: None,
                subject: Some("Delay update".to_string()),
                thread_id: Some("msg-123".to_string()),
            },
            template: None,
        };

        let event = OperationalEvent::from(&doc);
        assert_eq!(event.kind, OperationalKind::Email);
        assert_eq!(event.status, OperationalStatus::Info);
        assert_eq!(event.thread_id.as_deref(), Some("msg-123"));
        assert_eq!(event.actor.as_deref(), Some("supplier@example.com"));
        assert_eq!(
            event.properties.get("thread_id").map(String::as_str),
            Some("msg-123")
        );
        assert!(!event.event_key.is_empty());
    }

    #[test]
    fn push_capped_drops_oldest_beyond_ceiling() {
        let mut events = Vec::new();
        for i in 0..(MAX_STORED_EVENTS + 5) {
            let p = OperationalProvenance::new("industrial", "machine");
            let mut ev = OperationalEvent::new(
                OperationalKind::Machine,
                OperationalStatus::Info,
                Utc::now(),
                format!("machine-{i}"),
                p,
            );
            // Unique event_key per iteration so the ring holds exactly the tail.
            ev.properties.insert("i".into(), i.to_string());
            ev.rekey();
            push_capped(&mut events, ev);
        }
        assert_eq!(events.len(), MAX_STORED_EVENTS);
        assert_eq!(
            events.first().unwrap().properties.get("i").map(String::as_str),
            Some("5")
        );
        assert_eq!(
            events.last().unwrap().properties.get("i").map(String::as_str),
            Some((MAX_STORED_EVENTS + 4).to_string()).as_deref()
        );
    }

    #[test]
    fn save_then_load_events_round_trips() {
        let tag: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("physis_ev_{}_{}", std::process::id(), tag));
        std::fs::create_dir_all(&dir).unwrap();
        let db = sled::open(dir.join("t.sled")).unwrap();

        let p = OperationalProvenance::new("industrial", "machine");
        let ev = OperationalEvent::new(
            OperationalKind::Machine,
            OperationalStatus::Warning,
            Utc::now(),
            "voron-01",
            p,
        );
        save_events(&db, "tenant-a", std::slice::from_ref(&ev)).unwrap();
        let loaded = load_events(&db, "tenant-a");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].event_key, ev.event_key);
        assert!(load_events(&db, "tenant-b").is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ledger_mechanism_groups_by_kind() {
        let machine = |kind: OperationalKind| {
            OperationalEvent::new(
                kind,
                OperationalStatus::Info,
                Utc::now(),
                "x",
                OperationalProvenance::new("c", "s"),
            )
        };
        assert_eq!(
            machine(OperationalKind::Machine).ledger_mechanism(),
            crate::address::Mechanism::Received
        );
        assert_eq!(
            machine(OperationalKind::System).ledger_mechanism(),
            crate::address::Mechanism::Generated
        );
    }
}
