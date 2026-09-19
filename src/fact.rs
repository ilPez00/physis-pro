//! Specific facts, projected from what the engine already stores.
//!
//! A scalar reading, a JSON field, a table cell, a key/value pair inside text
//! and a sensor sample are all the same thing: a specific fact about a
//! `WorldObject`/event, with a value, a unit, a time and provenance. This
//! module is the read-only projection that says so — it is **not** a store,
//! a telemetry database or a second event log. A [`FactView`] borrows nothing
//! and owns only strings; it is derived on demand from an existing
//! [`crate::operational::OperationalEvent`] or a span of source text.
//!
//! # Why this exists
//!
//! The textbox must answer "what is CPU temperature?" with `72.4 °C` — not a
//! paragraph, and not a model call. A typed field already answers the question;
//! the only work left is projecting it into one shape and querying that shape
//! deterministically. `propose` is not `judge`, and a lookup is not retrieval.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::operational::OperationalEvent;

/// Where one fact came from. The pointer is into the event or the text — the
/// value is not copied out of the store that owns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "snake_case")]
pub enum FactOrigin {
    /// An authoritative `OperationalEvent.properties` entry.
    Property { key: String },
    /// A primitive leaf of `OperationalEvent.raw` not already a property.
    Raw { path: String },
    /// A deterministic span of source text, kept verbatim.
    Text { evidence: String },
}

impl FactOrigin {
    /// Rank for dedupe: a property beats a raw leaf beats a text span.
    fn rank(&self) -> u8 {
        match self {
            FactOrigin::Property { .. } => 0,
            FactOrigin::Raw { .. } => 1,
            FactOrigin::Text { .. } => 2,
        }
    }

    /// Short label for the provenance route.
    pub fn route(&self) -> &'static str {
        match self {
            FactOrigin::Property { .. } => "property",
            FactOrigin::Raw { .. } => "raw",
            FactOrigin::Text { .. } => "text",
        }
    }
}

/// One specific fact, read-only and derived.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactView {
    /// Canonical key, normalized (`cpu temperature` → `cpu_temperature`).
    pub key: String,
    /// The key as written by the source.
    pub display_key: String,
    /// The value as written.
    pub value: String,
    /// The parsed number, when the value is numeric.
    pub number: Option<f64>,
    /// The unit, when the value carried one (`°C`, `rpm`, `%`).
    pub unit: Option<String>,
    /// When the fact was observed, when known.
    pub observed_at: Option<DateTime<Utc>>,
    /// The object the fact is about, when known.
    pub subject: Option<String>,
    /// The native source (sysfs path, file path, URI) when the event has one.
    pub source: Option<String>,
    pub origin: FactOrigin,
}

/// Keys that are event identity or time, already carried by the view's own
/// `subject`/`observed_at`, so projecting them again as raw facts is noise.
const RESERVED_RAW_KEYS: [&str; 5] = [
    "machine_id",
    "timestamp",
    "event_key",
    "subject",
    "observed_at",
];

/// Normalize a key for matching: lowercase, runs of non-alphanumerics become
/// one `_`, leading/trailing `_` dropped.
pub fn normalize_key(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_us = true;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_us = false;
        } else if !last_us {
            out.push('_');
            last_us = true;
        }
    }
    out.trim_matches('_').to_string()
}

/// Split a scalar string into a number and an optional trailing unit.
///
/// Deliberately strict: `1.2.3` is not `1.2` with unit `.3`, so a version
/// number never becomes a reading. The unit must not begin with a digit or a
/// dot for the same reason.
pub fn parse_scalar(raw: &str) -> (Option<f64>, Option<String>) {
    let s = raw.trim();
    if s.is_empty() {
        return (None, None);
    }
    let bytes = s.as_bytes();
    let mut end = 0;
    if bytes[0] == b'+' || bytes[0] == b'-' {
        end = 1;
    }
    let mut seen_digit = false;
    while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
        if bytes[end].is_ascii_digit() {
            seen_digit = true;
        }
        end += 1;
    }
    if end < bytes.len() && (bytes[end] == b'e' || bytes[end] == b'E') {
        let mut e = end + 1;
        if e < bytes.len() && (bytes[e] == b'+' || bytes[e] == b'-') {
            e += 1;
        }
        let mut exp_digit = false;
        while e < bytes.len() && bytes[e].is_ascii_digit() {
            exp_digit = true;
            e += 1;
        }
        if exp_digit {
            end = e;
        }
    }
    if !seen_digit {
        return (None, None);
    }
    let number = match s[..end].parse::<f64>() {
        Ok(n) => n,
        Err(_) => return (None, None),
    };
    let rest = s[end..].trim();
    if rest.is_empty() {
        return (Some(number), None);
    }
    if rest.starts_with(|c: char| c.is_ascii_digit() || c == '.') {
        return (None, None);
    }
    (Some(number), Some(rest.to_string()))
}

/// The object a fact is about. `machine_id` wins over `subject` because a
/// telemetry event's subject may be a display name while the machine id is the
/// stable identity a query constrains on ("for pansa").
fn subject_of(ev: &OperationalEvent) -> Option<String> {
    ev.machine_id.clone().or_else(|| {
        if ev.subject.is_empty() {
            None
        } else {
            Some(ev.subject.clone())
        }
    })
}

/// The native source string, when the event carries one.
fn source_of(ev: &OperationalEvent) -> Option<String> {
    ev.provenance
        .path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .or_else(|| ev.provenance.uri.clone())
        .or_else(|| ev.provenance.source_id.clone())
}

/// Project an event's `properties` into facts.
fn facts_from_properties(ev: &OperationalEvent) -> Vec<FactView> {
    let subject = subject_of(ev);
    let source = source_of(ev);
    let mut out = Vec::with_capacity(ev.properties.len());
    for (k, v) in &ev.properties {
        let (number, unit) = parse_scalar(v);
        out.push(FactView {
            key: normalize_key(k),
            display_key: k.clone(),
            value: v.clone(),
            number,
            unit,
            observed_at: Some(ev.observed_at),
            subject: subject.clone(),
            source: source.clone(),
            origin: FactOrigin::Property { key: k.clone() },
        });
    }
    out
}

/// Project primitive leaves of `raw` that are not already a property. The walk
/// keeps the JSON path so a nested leaf still says exactly where it lives.
fn facts_from_raw(ev: &OperationalEvent, property_keys: &HashSet<String>) -> Vec<FactView> {
    let subject = subject_of(ev);
    let source = source_of(ev);
    let mut out = Vec::new();
    let mut stack: Vec<(String, &serde_json::Value)> = vec![(String::new(), &ev.raw)];
    while let Some((path, value)) = stack.pop() {
        match value {
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    let child = if path.is_empty() {
                        k.clone()
                    } else {
                        format!("{path}.{k}")
                    };
                    stack.push((child, v));
                }
            }
            serde_json::Value::Array(items) => {
                for (i, v) in items.iter().enumerate() {
                    stack.push((format!("{path}[{i}]"), v));
                }
            }
            other => {
                if let Some(s) = primitive_str(other) {
                    push_raw(&mut out, ev, property_keys, &path, &s, &subject, &source);
                }
            }
        }
    }
    out
}

/// A raw leaf as text, or `None` for containers and null.
fn primitive_str(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn push_raw(
    out: &mut Vec<FactView>,
    ev: &OperationalEvent,
    property_keys: &HashSet<String>,
    path: &str,
    value: &str,
    subject: &Option<String>,
    source: &Option<String>,
) {
    let leaf = path.rsplit('.').next().unwrap_or(path);
    let normalized = normalize_key(leaf);
    if property_keys.contains(&normalized)
        || RESERVED_RAW_KEYS.contains(&normalized.as_str())
        || path.is_empty()
    {
        return;
    }
    let (number, unit) = parse_scalar(value);
    out.push(FactView {
        key: normalized,
        display_key: path.to_string(),
        value: value.to_string(),
        number,
        unit,
        observed_at: Some(ev.observed_at),
        subject: subject.clone(),
        source: source.clone(),
        origin: FactOrigin::Raw {
            path: path.to_string(),
        },
    });
}

/// Project every specific fact an event carries: authoritative properties first,
/// then raw leaves not already represented.
pub fn facts_from_event(ev: &OperationalEvent) -> Vec<FactView> {
    let mut property_keys = HashSet::with_capacity(ev.properties.len());
    for k in ev.properties.keys() {
        property_keys.insert(normalize_key(k));
    }
    let mut facts = facts_from_properties(ev);
    facts.extend(facts_from_raw(ev, &property_keys));
    dedupe_facts(facts)
}

/// Project facts from a batch of events.
pub fn facts_from_events(events: &[OperationalEvent]) -> Vec<FactView> {
    let mut facts = Vec::new();
    for ev in events {
        facts.extend(facts_from_event(ev));
    }
    dedupe_facts(facts)
}

/// Collapse facts that are duplicate representations of the **same
/// observation**: same subject, same key and same `observed_at`. Two readings
/// of one metric at different times are two facts and both survive — the
/// projection preserves history; choosing the newest is the query's job, not
/// the projection's.
///
/// Among duplicates the most authoritative origin wins: property beats raw
/// beats text. First occurrence wins on a tie, so the projection is
/// deterministic.
pub fn dedupe_facts(facts: Vec<FactView>) -> Vec<FactView> {
    let mut best: HashMap<(String, String, Option<DateTime<Utc>>), usize> = HashMap::new();
    let mut out: Vec<FactView> = Vec::with_capacity(facts.len());
    for fact in facts {
        let id = (
            fact.subject.clone().unwrap_or_default(),
            fact.key.clone(),
            fact.observed_at,
        );
        match best.get(&id) {
            Some(&i) if out[i].origin.rank() <= fact.origin.rank() => {}
            Some(&i) => {
                out[i] = fact;
            }
            None => {
                best.insert(id, out.len());
                out.push(fact);
            }
        }
    }
    out
}

/// Project deterministic fact spans out of source text, grounded verbatim.
///
/// Recognized syntax, one fact per line, numeric only:
/// `key: value [unit]`, `key = value [unit]`, and `<key> is <number> [unit]`.
/// The evidence span is the whole line, kept exactly as written.
pub fn facts_from_text(
    text: &str,
    at: Option<DateTime<Utc>>,
    subject: Option<&str>,
) -> Vec<FactView> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = parse_fact_line(line) else {
            continue;
        };
        let (number, unit) = parse_scalar(&value);
        if number.is_none() {
            continue;
        }
        out.push(FactView {
            key: normalize_key(&key),
            display_key: key,
            value,
            number,
            unit,
            observed_at: at,
            subject: subject.map(str::to_string),
            source: None,
            origin: FactOrigin::Text {
                evidence: line.to_string(),
            },
        });
    }
    out
}

/// Find the `key`/`value` pair of one obvious fact line, if it is one.
fn parse_fact_line(line: &str) -> Option<(String, String)> {
    for sep in [':', '='] {
        if let Some((k, v)) = line.split_once(sep) {
            let key = k.trim();
            let value = v.trim();
            if valid_text_key(key) && !value.is_empty() && parse_scalar(value).0.is_some() {
                return Some((key.to_string(), value.to_string()));
            }
        }
    }
    let lower = line.to_ascii_lowercase();
    if let Some(idx) = lower.find(" is ") {
        let key = line[..idx].trim();
        let value = line[idx + 4..].trim();
        if valid_text_key(key) && !value.is_empty() && parse_scalar(value).0.is_some() {
            return Some((key.to_string(), value.to_string()));
        }
    }
    None
}

/// A text key must be a short label with at least one letter, so a bare time
/// like `14:30` is not read as `14 = 30`.
fn valid_text_key(key: &str) -> bool {
    !key.is_empty()
        && key.split_whitespace().count() <= 6
        && key.chars().any(|c| c.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operational::{OperationalKind, OperationalProvenance, OperationalStatus};
    use std::path::PathBuf;

    fn at(hour: u32, min: u32) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(&format!("2026-09-19T{hour:02}:{min:02}:00Z"))
            .unwrap()
            .with_timezone(&Utc)
    }

    fn event(subject: &str, props: &[(&str, &str)], raw: serde_json::Value) -> OperationalEvent {
        event_at(subject, props, raw, at(12, 0))
    }

    fn event_at(
        subject: &str,
        props: &[(&str, &str)],
        raw: serde_json::Value,
        observed_at: DateTime<Utc>,
    ) -> OperationalEvent {
        let mut prov = OperationalProvenance::new("test", "machine");
        prov.source_id = Some(subject.to_string());
        prov.path = Some(PathBuf::from("/sys/fake/temp1_input"));
        let mut ev = OperationalEvent::new(
            OperationalKind::Machine,
            OperationalStatus::Info,
            observed_at,
            subject,
            prov,
        );
        ev.machine_id = Some(subject.to_string());
        for (k, v) in props {
            ev.properties.insert(k.to_string(), v.to_string());
        }
        ev.raw = raw;
        ev.rekey();
        ev
    }

    #[test]
    fn machine_property_yields_a_fact_pointing_at_the_property() {
        let ev = event("pansa", &[("temperature", "78.5")], serde_json::Value::Null);
        let facts = facts_from_event(&ev);
        assert_eq!(facts.len(), 1, "{facts:?}");
        let f = &facts[0];
        assert_eq!(f.key, "temperature");
        assert_eq!(f.number, Some(78.5));
        assert_eq!(f.subject.as_deref(), Some("pansa"));
        assert_eq!(
            f.origin,
            FactOrigin::Property {
                key: "temperature".into()
            }
        );
        assert_eq!(f.source.as_deref(), Some("/sys/fake/temp1_input"));
    }

    #[test]
    fn raw_duplicate_of_a_property_collapses_to_the_property() {
        let raw = serde_json::json!({"temperature": 78.5, "bed_target": 60.0});
        let ev = event("pansa", &[("temperature", "78.5")], raw);
        let facts = facts_from_event(&ev);
        let temps: Vec<_> = facts.iter().filter(|f| f.key == "temperature").collect();
        assert_eq!(temps.len(), 1, "{facts:?}");
        assert!(matches!(temps[0].origin, FactOrigin::Property { .. }));
        let bed = facts.iter().find(|f| f.key == "bed_target").unwrap();
        assert!(matches!(bed.origin, FactOrigin::Raw { .. }));
    }

    #[test]
    fn raw_identity_and_time_leaves_are_not_facts() {
        let raw = serde_json::json!({"machine_id": "pansa", "timestamp": "2026-09-19T12:00:00Z"});
        let ev = event("pansa", &[("temperature", "78.5")], raw);
        let facts = facts_from_event(&ev);
        assert_eq!(facts.len(), 1, "{facts:?}");
    }

    #[test]
    fn text_span_yields_a_fact_with_verbatim_evidence() {
        let facts = facts_from_text("CPU temperature: 72.4 °C", Some(at(12, 0)), Some("pansa"));
        assert_eq!(facts.len(), 1, "{facts:?}");
        let f = &facts[0];
        assert_eq!(f.key, "cpu_temperature");
        assert_eq!(f.display_key, "CPU temperature");
        assert_eq!(f.number, Some(72.4));
        assert_eq!(f.unit.as_deref(), Some("°C"));
        assert_eq!(
            f.origin,
            FactOrigin::Text {
                evidence: "CPU temperature: 72.4 °C".into()
            }
        );
    }

    #[test]
    fn text_equals_and_is_forms_parse_but_prose_does_not() {
        let facts = facts_from_text(
            "rpm = 1200\npressure is 2.5 bar\nthe sky is blue\n14:30\nversion: 1.2.3",
            None,
            None,
        );
        let keys: Vec<&str> = facts.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(keys, vec!["rpm", "pressure"], "{facts:?}");
        assert_eq!(facts[0].number, Some(1200.0));
        assert_eq!(facts[1].unit.as_deref(), Some("bar"));
    }

    #[test]
    fn two_readings_at_different_times_both_survive_projection() {
        // Dedupe collapses duplicate representations of one observation, never
        // history. Two temperature readings an hour apart are two facts.
        let morning = event_at(
            "pansa",
            &[("temperature", "60.0")],
            serde_json::Value::Null,
            at(9, 0),
        );
        let noon = event_at(
            "pansa",
            &[("temperature", "78.5")],
            serde_json::Value::Null,
            at(12, 0),
        );
        let facts = facts_from_events(&[morning, noon]);
        let temps: Vec<_> = facts.iter().filter(|f| f.key == "temperature").collect();
        assert_eq!(temps.len(), 2, "{facts:?}");
        assert_eq!(temps[0].number, Some(60.0));
        assert_eq!(temps[1].number, Some(78.5));
        assert_ne!(temps[0].observed_at, temps[1].observed_at);
    }

    #[test]
    fn projection_is_deterministic_across_runs() {
        let ev = event(
            "pansa",
            &[("temperature", "78.5")],
            serde_json::json!({"fan": 3}),
        );
        assert_eq!(facts_from_event(&ev), facts_from_event(&ev));
    }

    #[test]
    fn normalize_key_folds_spaces_and_punctuation() {
        assert_eq!(normalize_key("CPU temperature"), "cpu_temperature");
        assert_eq!(normalize_key("temp (°C)"), "temp_c");
        assert_eq!(normalize_key("  cpu_temp  "), "cpu_temp");
    }
}
