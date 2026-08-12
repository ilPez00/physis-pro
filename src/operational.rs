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
        event.properties.insert("status".to_string(), reading.status.as_str().to_lowercase());
        if let Some(temp) = reading.temperature {
            event.properties.insert("temperature".to_string(), temp.to_string());
        }
        if let Some(pressure) = reading.pressure {
            event.properties.insert("pressure".to_string(), pressure.to_string());
        }
        if let Some(vibration) = reading.vibration {
            event.properties.insert("vibration".to_string(), vibration.to_string());
        }
        if let Some(rpm) = reading.rpm {
            event.properties.insert("rpm".to_string(), rpm.to_string());
        }
        event.properties.extend(reading.raw.iter().map(|(k, v)| (k.clone(), v.to_string())));
        event.raw = serde_json::to_value(reading).unwrap_or(serde_json::Value::Null);
        event.rekey();
        event
    }
}

impl From<&IngestedDoc> for OperationalEvent {
    fn from(doc: &IngestedDoc) -> Self {
        let (kind, status) = match doc.kind {
            DocKind::Email => (OperationalKind::Email, OperationalStatus::Info),
            DocKind::VoiceTranscript | DocKind::Spreadsheet | DocKind::Report | DocKind::Invoice | DocKind::Unknown => {
                (OperationalKind::Document, OperationalStatus::Info)
            }
        };
        let mut provenance = OperationalProvenance::new("ingest", kind.as_str());
        provenance.path = Some(doc.path.clone());
        provenance.source_id = doc.meta.thread_id.clone().or_else(|| {
            doc.path.file_name().and_then(|s| s.to_str()).map(|s| s.to_string())
        });

        let mut event = OperationalEvent::new(kind, status, doc.timestamp, doc.title.clone(), provenance);
        event.thread_id = doc.meta.thread_id.clone();
        event.actor = doc.meta.from.clone();
        event.target = doc.meta.vendor.clone();
        event.project = doc.ontology_tags.first().cloned();
        event.evidence.push(doc.body.clone());
        if let Some(from) = &doc.meta.from {
            event.properties.insert("from".to_string(), from.clone());
        }
        if let Some(subject) = &doc.meta.subject {
            event.properties.insert("subject".to_string(), subject.clone());
        }
        if let Some(thread_id) = &doc.meta.thread_id {
            event.properties.insert("thread_id".to_string(), thread_id.clone());
        }
        event.properties.insert("doc_kind".to_string(), format!("{:?}", doc.kind));
        event.raw = serde_json::to_value(doc).unwrap_or(serde_json::Value::Null);
        event.rekey();
        event
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
            timestamp: DateTime::parse_from_rfc3339("2026-08-12T08:55:00Z").unwrap().with_timezone(&Utc),
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
        assert_eq!(event.properties.get("bed_target").map(String::as_str), Some("60"));
        assert!(!event.event_key.is_empty());
    }

    #[test]
    fn email_doc_normalizes_to_event() {
        let doc = IngestedDoc {
            path: PathBuf::from("/tmp/mail/eml/chain.eml"),
            kind: DocKind::Email,
            title: "Supplier delay".to_string(),
            body: "Replacement nozzle ships tomorrow.".to_string(),
            timestamp: DateTime::parse_from_rfc3339("2026-08-12T09:30:00Z").unwrap().with_timezone(&Utc),
            ontology_tags: vec!["workshop".to_string()],
            meta: DocMeta {
                vendor: None,
                from: Some("supplier@example.com".to_string()),
                date: None,
                doc_number: None,
                subject: Some("Delay update".to_string()),
                thread_id: Some("msg-123".to_string()),
            },
        };

        let event = OperationalEvent::from(&doc);
        assert_eq!(event.kind, OperationalKind::Email);
        assert_eq!(event.status, OperationalStatus::Info);
        assert_eq!(event.thread_id.as_deref(), Some("msg-123"));
        assert_eq!(event.actor.as_deref(), Some("supplier@example.com"));
        assert_eq!(event.properties.get("thread_id").map(String::as_str), Some("msg-123"));
        assert!(!event.event_key.is_empty());
    }
}
