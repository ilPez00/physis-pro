//! Moonraker adapter for Klipper printers.
//!
//! This is intentionally thin: it polls Moonraker's JSON-RPC API, converts the
//! returned printer state into a normalized [`OperationalEvent`], and keeps the
//! Moonraker-specific field mapping out of the rest of the engine.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::operational::{OperationalEvent, OperationalKind, OperationalProvenance, OperationalStatus};

#[derive(Debug, Clone, Deserialize)]
struct JsonRpcResponse<T> {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    #[allow(dead_code)]
    id: Option<Value>,
    result: T,
}

#[derive(Debug, Clone, Deserialize)]
struct MoonrakerQueryResult {
    #[allow(dead_code)]
    eventtime: Option<f64>,
    status: HashMap<String, Value>,
}

/// Replayable snapshot of Moonraker state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoonrakerSnapshot {
    pub observed_at: DateTime<Utc>,
    pub status: HashMap<String, Value>,
}

impl MoonrakerSnapshot {
    pub fn from_json(bytes: &[u8]) -> anyhow::Result<Self> {
        let parsed: MoonrakerSnapshot = serde_json::from_slice(bytes)?;
        Ok(parsed)
    }
}

/// Thin HTTP JSON-RPC adapter for Moonraker.
#[derive(Debug, Clone)]
pub struct MoonrakerAdapter {
    client: reqwest::Client,
    base_url: String,
    machine_id: String,
    objects: Vec<String>,
}

impl MoonrakerAdapter {
    pub fn new(base_url: impl Into<String>, machine_id: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            machine_id: machine_id.into(),
            objects: vec![
                "webhooks".to_string(),
                "print_stats".to_string(),
                "virtual_sdcard".to_string(),
                "toolhead".to_string(),
                "heater_bed".to_string(),
                "extruder".to_string(),
            ],
        }
    }

    pub fn with_objects(mut self, objects: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.objects = objects.into_iter().map(Into::into).collect();
        self
    }

    pub async fn poll_snapshot(&self) -> anyhow::Result<MoonrakerSnapshot> {
        let query = self.query_status().await?;
        Ok(MoonrakerSnapshot {
            observed_at: Utc::now(),
            status: query.status,
        })
    }

    pub async fn poll_event(&self) -> anyhow::Result<OperationalEvent> {
        let snapshot = self.poll_snapshot().await?;
        Ok(self.event_from_snapshot(snapshot))
    }

    pub fn event_from_snapshot(&self, snapshot: MoonrakerSnapshot) -> OperationalEvent {
        let observed_at = snapshot.observed_at;
        let status_map = snapshot.status;

        let webhooks = status_map.get("webhooks");
        let print_stats = status_map.get("print_stats");
        let virtual_sdcard = status_map.get("virtual_sdcard");

        let printer_state = webhooks
            .and_then(|v| v.get("state"))
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let print_state = print_stats
            .and_then(|v| v.get("state"))
            .and_then(Value::as_str)
            .unwrap_or("standby");
        let filename = print_stats
            .and_then(|v| v.get("filename"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.machine_id);
        let message = print_stats
            .and_then(|v| v.get("message"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .or_else(|| webhooks.and_then(|v| v.get("state_message")).and_then(Value::as_str))
            .unwrap_or("");
        let progress = virtual_sdcard
            .and_then(|v| v.get("progress"))
            .and_then(Value::as_f64)
            .or_else(|| print_stats.and_then(|v| v.get("info")).and_then(|v| v.get("current_layer")).and_then(Value::as_f64));

        let (status, severity) = classify_status(printer_state, print_state);
        let subject = if filename == self.machine_id {
            self.machine_id.clone()
        } else {
            filename.to_string()
        };
        let mut event = OperationalEvent::new(
            OperationalKind::Machine,
            status,
            observed_at,
            subject,
            OperationalProvenance {
                connector: "moonraker".to_string(),
                source: "machine".to_string(),
                source_id: Some(self.machine_id.clone()),
                path: None,
                uri: Some(self.base_url.clone()),
            },
        );
        event.machine_id = Some(self.machine_id.clone());
        event.target = Some(filename.to_string());
        event.duration_secs = print_stats
            .and_then(|v| v.get("print_duration"))
            .and_then(Value::as_f64)
            .map(|secs| secs.max(0.0).round() as i64);
        event.actor = Some("moonraker".to_string());
        event.evidence.push(format!("webhooks.state={printer_state}"));
        event.evidence.push(format!("print_stats.state={print_state}"));
        if !message.is_empty() {
            event.evidence.push(message.to_string());
        }
        if let Some(progress) = progress {
            event.properties.insert("progress".to_string(), progress.to_string());
        }
        event.properties.insert("printer_state".to_string(), printer_state.to_string());
        event.properties.insert("print_state".to_string(), print_state.to_string());
        event.properties.insert("severity".to_string(), severity.to_string());
        if let Some(message) = message.strip_prefix("") {
            if !message.is_empty() {
                event.properties.insert("message".to_string(), message.to_string());
            }
        }
        if let Some(file_path) = virtual_sdcard
            .and_then(|v| v.get("file_path"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            event.properties.insert("file_path".to_string(), file_path.to_string());
        }
        if let Some(total_duration) = print_stats
            .and_then(|v| v.get("total_duration"))
            .and_then(Value::as_f64)
        {
            event.properties.insert("total_duration".to_string(), total_duration.to_string());
        }
        if let Some(print_duration) = print_stats
            .and_then(|v| v.get("print_duration"))
            .and_then(Value::as_f64)
        {
            event.properties.insert("print_duration".to_string(), print_duration.to_string());
        }
        if let Some(filament_used) = print_stats
            .and_then(|v| v.get("filament_used"))
            .and_then(Value::as_f64)
        {
            event.properties.insert("filament_used".to_string(), filament_used.to_string());
        }
        event.raw = serde_json::json!({
            "observed_at": observed_at,
            "status": status_map,
        });
        event.rekey();
        event
    }

    async fn query_status(&self) -> anyhow::Result<MoonrakerQueryResult> {
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "printer.objects.query",
            "params": {
                "objects": self.objects.iter().map(|k| (k.clone(), serde_json::Value::Null)).collect::<serde_json::Map<_, _>>()
            },
            "id": 1u64,
        });

        let url = format!("{}/server/jsonrpc", self.base_url);
        let response = self.client.post(url).json(&payload).send().await?.error_for_status()?;
        let parsed: JsonRpcResponse<MoonrakerQueryResult> = response.json().await?;
        Ok(parsed.result)
    }
}

fn classify_status(printer_state: &str, print_state: &str) -> (OperationalStatus, f32) {
    let printer_state = printer_state.to_ascii_lowercase();
    let print_state = print_state.to_ascii_lowercase();
    match (printer_state.as_str(), print_state.as_str()) {
        ("error", _) => (OperationalStatus::Failure, 1.0),
        ("shutdown", _) => (OperationalStatus::Failure, 1.0),
        ("startup", _) => (OperationalStatus::Info, 0.2),
        ("ready", "error") => (OperationalStatus::Failure, 1.0),
        ("ready", "paused") => (OperationalStatus::Warning, 0.7),
        ("ready", "printing") => (OperationalStatus::Info, 0.4),
        ("ready", "complete") => (OperationalStatus::Success, 0.1),
        ("ready", "cancelled") => (OperationalStatus::Warning, 0.6),
        ("ready", "standby") => (OperationalStatus::Success, 0.1),
        ("ready", _) => (OperationalStatus::Info, 0.3),
        _ if print_state == "error" => (OperationalStatus::Failure, 1.0),
        _ if print_state == "paused" => (OperationalStatus::Warning, 0.7),
        _ if print_state == "printing" => (OperationalStatus::Info, 0.4),
        _ if print_state == "complete" => (OperationalStatus::Success, 0.1),
        _ => (OperationalStatus::Info, 0.2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fixture_and_builds_event() {
        let fixture = include_str!("../tests/fixtures/moonraker_snapshot.json");
        let snapshot: MoonrakerSnapshot = serde_json::from_str(fixture).unwrap();
        let adapter = MoonrakerAdapter::new("http://printer.local:7125", "voron-01");
        let event = adapter.event_from_snapshot(snapshot);

        assert_eq!(event.kind, OperationalKind::Machine);
        assert_eq!(event.machine_id.as_deref(), Some("voron-01"));
        assert_eq!(event.status, OperationalStatus::Warning);
        assert_eq!(event.properties.get("print_state").map(String::as_str), Some("paused"));
        assert_eq!(event.properties.get("progress").map(String::as_str), Some("0.42"));
        assert!(event.evidence.iter().any(|e| e.contains("webhooks.state=ready")));
        assert!(!event.event_key.is_empty());
    }

    #[test]
    fn classifies_printer_states() {
        assert_eq!(classify_status("ready", "complete").0, OperationalStatus::Success);
        assert_eq!(classify_status("ready", "paused").0, OperationalStatus::Warning);
        assert_eq!(classify_status("error", "standby").0, OperationalStatus::Failure);
    }
}
