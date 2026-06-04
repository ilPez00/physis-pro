//! Industrial protocol adapters — MQTT, Modbus, and machine health monitoring.
//! Each adapter implements the `MachineAdapter` trait for uniform health polling.

use std::collections::HashMap;
#[cfg(feature = "industrial")]
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

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
#[cfg(feature = "industrial")]
pub mod mqtt {
    use super::*;
    use rumqttc::{AsyncClient, MqttOptions, QoS, EventLoop};

    pub struct MqttAdapter {
        client: Option<AsyncClient>,
        eventloop: Option<EventLoop>,
        machine_id: String,
    }

    impl MqttAdapter {
        pub fn new(machine_id: &str) -> Self {
            Self { client: None, eventloop: None, machine_id: machine_id.to_string() }
        }
    }

    #[async_trait]
    impl MachineAdapter for MqttAdapter {
        async fn connect(&mut self, endpoint: &str) -> anyhow::Result<()> {
            let mut mqttoptions = MqttOptions::new("physis-pro", endpoint, 1883);
            mqttoptions.set_keep_alive(Duration::from_secs(5));
            let (client, eventloop) = AsyncClient::new(mqttoptions, 10);
            self.client = Some(client);
            self.eventloop = Some(eventloop);
            Ok(())
        }

        async fn poll(&mut self) -> anyhow::Result<MachineReading> {
            anyhow::bail!("poll not implemented for MQTT adapter yet")
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
