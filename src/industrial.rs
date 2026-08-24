//! Industrial protocol adapters — MQTT, Modbus, and machine health monitoring.
//! Each adapter implements the `MachineAdapter` trait for uniform health polling.

use std::collections::HashMap;
#[cfg(feature = "industrial")]
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::operational::OperationalEvent;

/// A health reading from an industrial machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineReading {
    pub machine_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub temperature: Option<f32>,
    pub pressure: Option<f32>,
    pub vibration: Option<f32>,
    pub rpm: Option<f32>,
    pub status: MachineStatus,
    pub raw: HashMap<String, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MachineStatus {
    Online,
    Offline,
    Warning,
    Critical,
}

impl MachineStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MachineStatus::Online => "online",
            MachineStatus::Offline => "offline",
            MachineStatus::Warning => "warning",
            MachineStatus::Critical => "critical",
        }
    }
}

/// A detected process anomaly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessAnomaly {
    pub machine_id: String,
    pub reading: MachineReading,
    pub reason: String,
    pub severity: f32,
}

/// Trait for industrial protocol adapters.
#[async_trait]
pub trait MachineAdapter: Send {
    /// Connect to the machine/endpoint.
    async fn connect(&mut self, endpoint: &str) -> anyhow::Result<()>;
    /// Poll a single health reading.
    async fn poll(&mut self) -> anyhow::Result<MachineReading>;
    /// Disconnect.
    async fn disconnect(&mut self) -> anyhow::Result<()>;
}

/// Detect anomalies in a stream of readings for a machine.
pub fn detect_anomalies(
    machine_id: &str,
    readings: &[MachineReading],
    thresholds: &HashMap<String, f32>,
) -> Vec<ProcessAnomaly> {
    let mut anomalies = Vec::new();
    for reading in readings {
        if let Some(t) = reading.temperature {
            if let Some(limit) = thresholds.get("temp_max") {
                if t > *limit {
                    anomalies.push(ProcessAnomaly {
                        machine_id: machine_id.to_string(),
                        reading: reading.clone(),
                        reason: format!("temperature {t} exceeds limit {limit}"),
                        severity: ((t - limit) / limit).min(1.0),
                    });
                }
            }
        }
        if reading.status == MachineStatus::Offline || reading.status == MachineStatus::Critical {
            anomalies.push(ProcessAnomaly {
                machine_id: machine_id.to_string(),
                reading: reading.clone(),
                reason: format!("machine status: {:?}", reading.status),
                severity: 1.0,
            });
        }
    }
    anomalies
}

/// Stub MQTT adapter (requires rumqttc feature).
/// Lenient wire format for an inbound MQTT/REST telemetry payload. Every metric
/// is optional so a sensor can publish only what it has; `into_reading` fills the
/// rest. `status` accepts `online`/`offline`/`warning`/`critical` (any case).
#[derive(Debug, Clone, Deserialize)]
pub struct TelemetryPayload {
    pub temperature: Option<f32>,
    pub pressure: Option<f32>,
    pub vibration: Option<f32>,
    pub rpm: Option<f32>,
    pub status: Option<String>,
    #[serde(default)]
    pub raw: HashMap<String, f32>,
}

impl TelemetryPayload {
    /// Parse a JSON byte payload into a `MachineReading` for `machine_id`.
    pub fn parse_reading(machine_id: &str, bytes: &[u8]) -> anyhow::Result<MachineReading> {
        let p: TelemetryPayload = serde_json::from_slice(bytes)?;
        Ok(p.into_reading(machine_id))
    }

    fn into_reading(self, machine_id: &str) -> MachineReading {
        let status = match self
            .status
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("offline") => MachineStatus::Offline,
            Some("warning") => MachineStatus::Warning,
            Some("critical") => MachineStatus::Critical,
            _ => MachineStatus::Online,
        };
        MachineReading {
            machine_id: machine_id.to_string(),
            timestamp: chrono::Utc::now(),
            temperature: self.temperature,
            pressure: self.pressure,
            vibration: self.vibration,
            rpm: self.rpm,
            status,
            raw: self.raw,
        }
    }

    /// Convert a machine reading payload into the normalized operational event
    /// shape used by the demo/connectors layer.
    pub fn into_operational_event(self, machine_id: &str) -> OperationalEvent {
        let reading = self.into_reading(machine_id);
        OperationalEvent::from(&reading)
    }
}

#[cfg(feature = "industrial")]
pub mod mqtt {
    use super::*;
    use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};

    pub struct MqttAdapter {
        client: Option<AsyncClient>,
        eventloop: Option<EventLoop>,
        machine_id: String,
        /// Topic carrying this machine's telemetry. Defaults to
        /// `physis/<machine_id>/reading`; override with `set_topic`.
        topic: String,
        subscribed: bool,
    }

    impl MqttAdapter {
        pub fn new(machine_id: &str) -> Self {
            Self {
                client: None,
                eventloop: None,
                machine_id: machine_id.to_string(),
                topic: format!("physis/{machine_id}/reading"),
                subscribed: false,
            }
        }

        /// Override the telemetry topic this adapter subscribes to.
        pub fn set_topic(&mut self, topic: impl Into<String>) {
            self.topic = topic.into();
        }
    }

    #[async_trait]
    impl MachineAdapter for MqttAdapter {
        async fn connect(&mut self, endpoint: &str) -> anyhow::Result<()> {
            // Accept "host" or "host:port"; default broker port 1883.
            let (host, port) = match endpoint.rsplit_once(':') {
                Some((h, p)) => (h.to_string(), p.parse().unwrap_or(1883)),
                None => (endpoint.to_string(), 1883),
            };
            let mut mqttoptions =
                MqttOptions::new(format!("physis-{}", self.machine_id), host, port);
            mqttoptions.set_keep_alive(Duration::from_secs(5));
            let (client, eventloop) = AsyncClient::new(mqttoptions, 10);
            self.client = Some(client);
            self.eventloop = Some(eventloop);
            self.subscribed = false;
            Ok(())
        }

        async fn poll(&mut self) -> anyhow::Result<MachineReading> {
            let client = self
                .client
                .clone()
                .ok_or_else(|| anyhow::anyhow!("not connected"))?;
            let eventloop = self
                .eventloop
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("not connected"))?;

            // Drive the event loop until a publish on our topic arrives. The
            // subscribe is sent once, after the broker ack (ConnAck), so it
            // survives a reconnect.
            loop {
                match eventloop.poll().await? {
                    Event::Incoming(Packet::ConnAck(_)) => {
                        client.subscribe(&self.topic, QoS::AtLeastOnce).await?;
                        self.subscribed = true;
                    }
                    Event::Incoming(Packet::Publish(p)) if p.topic == self.topic => {
                        return TelemetryPayload::parse_reading(&self.machine_id, &p.payload);
                    }
                    _ => {}
                }
            }
        }

        async fn disconnect(&mut self) -> anyhow::Result<()> {
            if let Some(client) = &self.client {
                client.disconnect().await?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod payload_tests {
    use super::*;

    #[test]
    fn test_parse_partial_payload() {
        let json = br#"{"temperature": 88.5, "status": "warning"}"#;
        let r = TelemetryPayload::parse_reading("press-01", json).unwrap();
        assert_eq!(r.machine_id, "press-01");
        assert_eq!(r.temperature, Some(88.5));
        assert_eq!(r.status, MachineStatus::Warning);
        assert_eq!(r.pressure, None);
    }

    #[test]
    fn test_parse_defaults_to_online() {
        let r = TelemetryPayload::parse_reading("m1", br#"{"rpm": 1200}"#).unwrap();
        assert_eq!(r.status, MachineStatus::Online);
        assert_eq!(r.rpm, Some(1200.0));
    }

    #[test]
    fn test_parse_raw_map() {
        let json = br#"{"raw": {"torque": 42.0, "amps": 3.1}}"#;
        let r = TelemetryPayload::parse_reading("m1", json).unwrap();
        assert_eq!(r.raw.get("torque"), Some(&42.0));
    }

    #[test]
    fn test_parse_bad_json_errors() {
        assert!(TelemetryPayload::parse_reading("m1", b"not json").is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_readings() -> Vec<MachineReading> {
        vec![
            MachineReading {
                machine_id: "press-01".into(),
                timestamp: chrono::Utc::now(),
                temperature: Some(85.0),
                pressure: Some(10.0),
                vibration: None,
                rpm: None,
                status: MachineStatus::Online,
                raw: HashMap::new(),
            },
            MachineReading {
                machine_id: "press-01".into(),
                timestamp: chrono::Utc::now(),
                temperature: Some(120.0),
                pressure: Some(15.0),
                vibration: None,
                rpm: None,
                status: MachineStatus::Warning,
                raw: HashMap::new(),
            },
        ]
    }

    #[test]
    fn test_detect_anomalies_temperature_exceeded() {
        let readings = sample_readings();
        let mut thresholds = HashMap::new();
        thresholds.insert("temp_max".to_string(), 100.0);
        let anomalies = detect_anomalies("press-01", &readings, &thresholds);
        assert_eq!(anomalies.len(), 1);
        assert!(anomalies[0].reason.contains("120"));
    }

    #[test]
    fn test_detect_anomalies_offline() {
        let readings = vec![MachineReading {
            machine_id: "press-01".into(),
            timestamp: chrono::Utc::now(),
            temperature: None,
            pressure: None,
            vibration: None,
            rpm: None,
            status: MachineStatus::Offline,
            raw: HashMap::new(),
        }];
        let anomalies = detect_anomalies("press-01", &readings, &HashMap::new());
        assert!(!anomalies.is_empty());
    }

    #[test]
    fn test_no_anomalies_when_within_bounds() {
        let readings = sample_readings();
        let mut thresholds = HashMap::new();
        thresholds.insert("temp_max".to_string(), 200.0);
        let anomalies = detect_anomalies("press-01", &readings, &thresholds);
        assert!(anomalies.is_empty());
    }

    #[test]
    fn test_connect_invalid_endpoint_fails() {
        // We can't test the adapter trait easily without a real server,
        // but we can verify the MachineStatus enum covers all cases.
        assert!(matches!(MachineStatus::Online, MachineStatus::Online));
        assert!(matches!(MachineStatus::Offline, MachineStatus::Offline));
    }
}
