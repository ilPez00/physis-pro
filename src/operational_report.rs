//! Timeline and incident views over normalized operational events.
//!
//! This is deliberately small: one sorted event stream, one incident summary,
//! and a markdown renderer that can later back a CLI command or web view.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::operational::{OperationalEvent, OperationalKind, OperationalStatus};

/// One row in the operational timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub observed_at: DateTime<Utc>,
    pub kind: OperationalKind,
    pub status: OperationalStatus,
    pub subject: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub properties: HashMap<String, String>,
}

impl From<&OperationalEvent> for TimelineEntry {
    fn from(event: &OperationalEvent) -> Self {
        Self {
            observed_at: event.observed_at,
            kind: event.kind,
            status: event.status,
            subject: event.subject.clone(),
            actor: event.actor.clone(),
            machine_id: event.machine_id.clone(),
            thread_id: event.thread_id.clone(),
            evidence: event.evidence.clone(),
            properties: event.properties.clone(),
        }
    }
}

/// One incident window extracted from the timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationalIncident {
    pub headline: String,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub trigger: TimelineEntry,
    #[serde(default)]
    pub related: Vec<TimelineEntry>,
    pub note: String,
    pub next_action: String,
}

/// A full timeline report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationalTimeline {
    #[serde(default)]
    pub entries: Vec<TimelineEntry>,
    #[serde(default)]
    pub counts_by_kind: HashMap<OperationalKind, usize>,
    #[serde(default)]
    pub counts_by_status: HashMap<OperationalStatus, usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incident: Option<OperationalIncident>,
}

impl OperationalTimeline {
    pub fn from_events(events: &[OperationalEvent]) -> Self {
        let mut entries: Vec<TimelineEntry> = events.iter().map(TimelineEntry::from).collect();
        entries.sort_by(|a, b| {
            a.observed_at
                .cmp(&b.observed_at)
                .then_with(|| a.subject.cmp(&b.subject))
        });

        let mut counts_by_kind = HashMap::new();
        let mut counts_by_status = HashMap::new();
        for e in &entries {
            *counts_by_kind.entry(e.kind).or_insert(0) += 1;
            *counts_by_status.entry(e.status).or_insert(0) += 1;
        }

        let incident = entries
            .iter()
            .find(|e| {
                matches!(
                    e.status,
                    OperationalStatus::Warning | OperationalStatus::Failure
                )
            })
            .map(|trigger| build_incident(trigger, &entries));

        Self {
            entries,
            counts_by_kind,
            counts_by_status,
            incident,
        }
    }

    pub fn render_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Operational Timeline\n\n");

        if self.entries.is_empty() {
            out.push_str("No events.\n");
            return out;
        }

        out.push_str("## Summary\n\n");
        out.push_str(&format!("- Events: {}\n", self.entries.len()));
        out.push_str(&format!(
            "- Machine: {}\n",
            self.counts_by_kind
                .get(&OperationalKind::Machine)
                .copied()
                .unwrap_or(0)
        ));
        out.push_str(&format!(
            "- Document: {}\n",
            self.counts_by_kind
                .get(&OperationalKind::Document)
                .copied()
                .unwrap_or(0)
        ));
        out.push_str(&format!(
            "- Email: {}\n",
            self.counts_by_kind
                .get(&OperationalKind::Email)
                .copied()
                .unwrap_or(0)
        ));
        out.push_str(&format!(
            "- Warning: {}\n",
            self.counts_by_status
                .get(&OperationalStatus::Warning)
                .copied()
                .unwrap_or(0)
        ));
        out.push_str(&format!(
            "- Failure: {}\n",
            self.counts_by_status
                .get(&OperationalStatus::Failure)
                .copied()
                .unwrap_or(0)
        ));

        if let Some(incident) = &self.incident {
            out.push_str("\n## Incident\n\n");
            out.push_str(&format!("- Headline: {}\n", incident.headline));
            out.push_str(&format!(
                "- Window: {} to {}\n",
                incident.window_start.to_rfc3339(),
                incident.window_end.to_rfc3339()
            ));
            out.push_str(&format!("- Note: {}\n", incident.note));
            out.push_str(&format!("- Next action: {}\n", incident.next_action));
        }

        out.push_str("\n## Events\n\n");
        for entry in &self.entries {
            out.push_str(&format!(
                "- {} [{} / {}] {}\n",
                entry.observed_at.to_rfc3339(),
                entry.kind.as_str(),
                entry.status.as_str(),
                entry.subject
            ));
            if let Some(actor) = &entry.actor {
                out.push_str(&format!("  - actor: {}\n", actor));
            }
            if let Some(machine_id) = &entry.machine_id {
                out.push_str(&format!("  - machine: {}\n", machine_id));
            }
            if let Some(thread_id) = &entry.thread_id {
                out.push_str(&format!("  - thread: {}\n", thread_id));
            }
            for ev in &entry.evidence {
                out.push_str(&format!("  - evidence: {}\n", ev));
            }
        }

        out
    }
}

fn build_incident(trigger: &TimelineEntry, entries: &[TimelineEntry]) -> OperationalIncident {
    let window_start = trigger.observed_at - Duration::minutes(30);
    let window_end = trigger.observed_at + Duration::minutes(30);
    let related: Vec<TimelineEntry> = entries
        .iter()
        .filter(|entry| {
            let within_window =
                entry.observed_at >= window_start && entry.observed_at <= window_end;
            let same_machine =
                trigger.machine_id.is_some() && trigger.machine_id == entry.machine_id;
            let same_thread = trigger.thread_id.is_some() && trigger.thread_id == entry.thread_id;
            within_window || same_machine || same_thread
        })
        .cloned()
        .collect();

    let note = match trigger.kind {
        OperationalKind::Machine => {
            if related.iter().any(|e| e.kind == OperationalKind::Email) && related.iter().any(|e| e.kind == OperationalKind::Document) {
                "Machine evidence aligns with local document and email context, so this looks like a workflow friction window.".to_string()
            } else if related.iter().any(|e| e.kind == OperationalKind::Email) {
                "Machine evidence lines up with an email thread, so the event is probably part of a customer or supplier handoff.".to_string()
            } else {
                "Machine evidence suggests an operational interruption that should be reviewed before the next run.".to_string()
            }
        }
        OperationalKind::Email => "Email thread contains a process delay or approval step that should be reconciled with the operational record.".to_string(),
        OperationalKind::Document => "Document evidence indicates a workflow step that should be normalized into the event stream.".to_string(),
        _ => "Operational interruption detected.".to_string(),
    };

    let next_action = match trigger.kind {
        OperationalKind::Machine => "Inspect the machine, attach the related note/email to the incident, and confirm a before-next-job action.".to_string(),
        OperationalKind::Email => "Clear the pending reply or approval and link the thread to the affected workflow.".to_string(),
        OperationalKind::Document => "Promote the document into a structured workflow event and attach the owning task." .to_string(),
        _ => "Review the related events and record a human verdict.".to_string(),
    };

    OperationalIncident {
        headline: format!("{} event on {}", trigger.status.as_str(), trigger.subject),
        window_start,
        window_end,
        trigger: trigger.clone(),
        related,
        note,
        next_action,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::industrial::{MachineReading, MachineStatus};
    use crate::ingest::{DocKind, DocMeta, IngestedDoc};

    fn machine_event() -> OperationalEvent {
        let reading = MachineReading {
            machine_id: "voron-01".to_string(),
            timestamp: DateTime::parse_from_rfc3339("2026-08-12T09:12:00Z")
                .unwrap()
                .with_timezone(&Utc),
            temperature: Some(78.5),
            pressure: None,
            vibration: Some(0.12),
            rpm: Some(1200.0),
            status: MachineStatus::Warning,
            raw: HashMap::from([(String::from("progress"), 0.42)]),
        };
        OperationalEvent::from(&reading)
    }

    fn email_event() -> OperationalEvent {
        let doc = IngestedDoc {
            path: std::path::PathBuf::from("/tmp/mail/eml/chain.eml"),
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
        };
        OperationalEvent::from(&doc)
    }

    fn document_event() -> OperationalEvent {
        let doc = IngestedDoc {
            path: std::path::PathBuf::from("/tmp/workshop/notes.md"),
            kind: DocKind::Report,
            title: "Resumed after nozzle clean".to_string(),
            body: "cleaned nozzle, resumed".to_string(),
            timestamp: DateTime::parse_from_rfc3339("2026-08-12T09:16:00Z")
                .unwrap()
                .with_timezone(&Utc),
            ontology_tags: vec!["workshop".to_string()],
            meta: DocMeta {
                vendor: None,
                from: None,
                date: None,
                doc_number: None,
                subject: None,
                thread_id: None,
            },
        };
        OperationalEvent::from(&doc)
    }

    #[test]
    fn builds_sorted_timeline_and_incident() {
        let events = vec![email_event(), machine_event(), document_event()];
        let timeline = OperationalTimeline::from_events(&events);

        assert_eq!(timeline.entries.len(), 3);
        assert_eq!(timeline.entries[0].subject, "voron-01");
        assert_eq!(timeline.entries[1].subject, "Resumed after nozzle clean");
        assert_eq!(timeline.entries[2].subject, "Supplier delay");
        let incident = timeline.incident.expect("incident should exist");
        assert!(incident.headline.contains("warning"));
        assert!(incident.note.contains("workflow friction"));
        assert!(incident.next_action.contains("Inspect"));
    }

    #[test]
    fn renders_markdown() {
        let timeline =
            OperationalTimeline::from_events(&[machine_event(), document_event(), email_event()]);
        let md = timeline.render_markdown();
        assert!(md.contains("# Operational Timeline"));
        assert!(md.contains("Incident"));
        assert!(md.contains("voron-01"));
        assert!(md.contains("Supplier delay"));
    }
}
