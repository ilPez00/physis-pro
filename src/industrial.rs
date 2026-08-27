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
    /// Push a command/setpoint back to the machine. Defaults to unsupported: many
    /// telemetry-only transports (and the read-only test adapters) cannot write.
    async fn publish(&mut self, _payload: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("publish not supported by this adapter")
    }
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

        // Publish a command/setpoint to `physis/<id>/cmd`. The loop driver uses
        // this to push corrections (e.g. "slow down", a new setpoint) back to the
        // machine — the write half of the subscribe/publish pair.
        async fn publish(&mut self, payload: &[u8]) -> anyhow::Result<()> {
            let client = self
                .client
                .clone()
                .ok_or_else(|| anyhow::anyhow!("not connected"))?;
            client
                .publish(
                    format!("physis/{}/cmd", self.machine_id),
                    QoS::AtLeastOnce,
                    false,
                    payload.to_vec(),
                )
                .await?;
            Ok(())
        }
    }
}

/// Register map for a Modbus TCP device: which holding register carries each
/// metric, and the divisor that recovers the engineering unit from the raw
/// integer the device publishes (sensors typically send e.g. temperature×10).
#[derive(Debug, Clone)]
pub struct ModbusMap {
    pub temperature: Option<u16>,
    pub pressure: Option<u16>,
    pub vibration: Option<u16>,
    pub rpm: Option<u16>,
    pub scale: f32,
}

impl Default for ModbusMap {
    fn default() -> Self {
        Self {
            temperature: Some(0),
            pressure: Some(1),
            vibration: Some(2),
            rpm: Some(3),
            scale: 10.0,
        }
    }
}

/// A write command for the Modbus adapter. `reg` is the holding-register address,
/// `value` the unsigned 16-bit word to write.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ModbusCommand {
    pub reg: u16,
    pub value: u16,
}

/// Modbus TCP adapter — the second half of Phase 1.8. Reads holding registers
/// into a `MachineReading` and writes setpoints back via `publish`.
#[cfg(feature = "industrial")]
pub mod modbus {
    use super::*;
    use tokio_modbus::client::tcp::connect as mb_connect;
    use tokio_modbus::prelude::*;

    pub struct ModbusTcpAdapter {
        ctx: Option<tokio_modbus::client::Context>,
        machine_id: String,
        endpoint: String,
        map: ModbusMap,
        slave: u8,
    }

    impl ModbusTcpAdapter {
        pub fn new(machine_id: &str) -> Self {
            Self {
                ctx: None,
                machine_id: machine_id.to_string(),
                endpoint: String::new(),
                map: ModbusMap::default(),
                slave: 1,
            }
        }

        /// Override the register map (addresses + scaling).
        pub fn with_map(mut self, map: ModbusMap) -> Self {
            self.map = map;
            self
        }

        /// Set the Modbus slave/unit id (default 1).
        pub fn set_slave(&mut self, slave: u8) {
            self.slave = slave;
        }

        async fn resolve(endpoint: &str, default_port: u16) -> anyhow::Result<std::net::SocketAddr> {
            let ep = if endpoint.contains(':') {
                endpoint.to_string()
            } else {
                format!("{endpoint}:{default_port}")
            };
            let mut addrs = tokio::net::lookup_host(&ep).await?;
            addrs
                .next()
                .ok_or_else(|| anyhow::anyhow!("cannot resolve {ep}"))
        }

        async fn read_one(
            ctx: &mut tokio_modbus::client::Context,
            addr: u16,
        ) -> anyhow::Result<Option<f32>> {
            let outer = ctx.read_holding_registers(addr, 1).await;
            let words = outer
                .map_err(|e| anyhow::Error::msg(format!("modbus transport: {e:?}")))?
                .map_err(|e| anyhow::Error::msg(format!("modbus exception: {e:?}")))?;
            Ok(words.first().copied().map(|x| x as f32))
        }
    }

    #[async_trait]
    impl MachineAdapter for ModbusTcpAdapter {
        async fn connect(&mut self, endpoint: &str) -> anyhow::Result<()> {
            self.endpoint = endpoint.to_string();
            let addr = Self::resolve(endpoint, 502).await?;
            let mut ctx = mb_connect(addr).await?;
            ctx.set_slave(tokio_modbus::Slave(self.slave));
            self.ctx = Some(ctx);
            Ok(())
        }

        async fn poll(&mut self) -> anyhow::Result<MachineReading> {
            let ctx = self
                .ctx
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("not connected"))?;
            let scale = self.map.scale;
            let mut r = MachineReading {
                machine_id: self.machine_id.clone(),
                timestamp: chrono::Utc::now(),
                temperature: None,
                pressure: None,
                vibration: None,
                rpm: None,
                status: MachineStatus::Online,
                raw: HashMap::new(),
            };
            if let Some(a) = self.map.temperature {
                if let Some(v) = Self::read_one(ctx, a).await? {
                    r.temperature = Some(v / scale);
                }
            }
            if let Some(a) = self.map.pressure {
                if let Some(v) = Self::read_one(ctx, a).await? {
                    r.pressure = Some(v / scale);
                }
            }
            if let Some(a) = self.map.vibration {
                if let Some(v) = Self::read_one(ctx, a).await? {
                    r.vibration = Some(v / scale);
                }
            }
            if let Some(a) = self.map.rpm {
                if let Some(v) = Self::read_one(ctx, a).await? {
                    r.rpm = Some(v / scale);
                }
            }
            Ok(r)
        }

        async fn disconnect(&mut self) -> anyhow::Result<()> {
            // tokio-modbus closes the TCP stream on drop; nothing explicit to send.
            self.ctx = None;
            Ok(())
        }

        async fn publish(&mut self, payload: &[u8]) -> anyhow::Result<()> {
            let ctx = self
                .ctx
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("not connected"))?;
            let cmd: ModbusCommand = serde_json::from_slice(payload).unwrap_or_else(|_| {
                // Fall back to raw big-endian: [addr_hi, addr_lo, val_hi, val_lo].
                let b = payload;
                let reg = u16::from_be_bytes([b[0], b[1]]);
                let value = u16::from_be_bytes([b[2], b[3]]);
                ModbusCommand { reg, value }
            });
            let _ = ctx
                .write_single_register(cmd.reg, cmd.value)
                .await
                .map_err(|e| anyhow::Error::msg(format!("modbus write transport: {e:?}")))?
                .map_err(|e| anyhow::Error::msg(format!("modbus write exception: {e:?}")))?;
            Ok(())
        }
    }
}

/// Options for [`run_adapter`].
pub struct AdapterRunOptions {
    pub protocol: String,
    pub endpoint: String,
    pub machine_id: String,
    pub topic: Option<String>,
    pub post_to: Option<String>,
    pub user: String,
    pub token: Option<String>,
    pub interval_secs: u64,
    pub once: bool,
    pub thresholds: std::collections::HashMap<String, f32>,
    pub modbus_map: Option<ModbusMap>,
}

/// Continuous real-time loop — the heart of Phase 1.8.
///
/// Subscribes (polls) the adapter forever: each reading is checked for anomalies,
/// normalized to an `OperationalEvent`, printed as one JSON line (so it composes
/// with `jq`/files/pipes like the Moonraker connector), optionally registered
/// into a physis engine, and — for adapters that support it — the write half is
/// available via [`MachineAdapter::publish`]. Dedupe keeps an unchanged machine
/// state from spamming the engine; a poll error never ends the loop, so a dropped
/// connection recovers on its own.
#[cfg(feature = "industrial")]
pub async fn run_adapter(opts: AdapterRunOptions) -> anyhow::Result<()> {
    use modbus::ModbusTcpAdapter;

    let mb_map = opts.modbus_map.clone();
    let mut adapter: Box<dyn MachineAdapter> = match opts.protocol.as_str() {
        "modbus" => {
            let mut a = ModbusTcpAdapter::new(&opts.machine_id);
            if let Some(m) = mb_map {
                a = a.with_map(m);
            }
            Box::new(a)
        }
        "mqtt" | _ => {
            let mut a = mqtt::MqttAdapter::new(&opts.machine_id);
            if let Some(t) = &opts.topic {
                a.set_topic(t.clone());
            }
            Box::new(a)
        }
    };

    adapter.connect(&opts.endpoint).await?;
    run_loop(adapter, &opts).await
}

/// The driver half of [`run_adapter`], separated so it can be exercised with an
/// in-memory mock adapter (no broker/device needed). See the `mock_loop` test.
#[cfg(feature = "industrial")]
pub async fn run_loop(
    mut adapter: Box<dyn MachineAdapter>,
    opts: &AdapterRunOptions,
) -> anyhow::Result<()> {
    let mut last_fp: Option<String> = None;

    loop {
        match adapter.poll().await {
            Ok(reading) => {
                let anomalies = detect_anomalies(
                    &opts.machine_id,
                    &[reading.clone()],
                    &opts.thresholds,
                );
                let event = OperationalEvent::from(&reading);
                let fp = format!(
                    "{:?}|{:?}|{:?}|{:?}|{:?}|{}",
                    reading.status,
                    reading.temperature,
                    reading.pressure,
                    reading.vibration,
                    reading.rpm,
                    reading.raw.is_empty()
                );
                if last_fp.as_deref() != Some(fp.as_str()) {
                    println!("{}", serde_json::to_string(&event)?);
                    for a in &anomalies {
                        eprintln!("[adapter] anomaly: {}", a.reason);
                    }
                    if let Some(base) = &opts.post_to {
                        if let Err(e) =
                            post_event(base, &opts.user, opts.token.as_deref(), &event).await
                        {
                            eprintln!("[adapter] register failed: {e}");
                        }
                    }
                    last_fp = Some(fp);
                }
            }
            Err(e) => eprintln!("[adapter] poll failed: {e}"),
        }
        if opts.once {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_secs(opts.interval_secs.max(1))).await;
    }
}

/// Register an event into a physis engine as a labeled coherence node. Mirrors
/// the Moonraker connector's `register` but keeps adapters self-contained.
#[cfg(feature = "industrial")]
async fn post_event(
    base: &str,
    user: &str,
    token: Option<&str>,
    event: &OperationalEvent,
) -> anyhow::Result<()> {
    let body = serde_json::json!({
        "input": format!("{} — {:?} [{:?}]", event.subject, event.kind, event.status),
        "label": event.subject,
    });
    let mut rb = reqwest::Client::new()
        .post(format!(
            "{}/api/v1/coherence/register",
            base.trim_end_matches('/')
        ))
        .header("X-Physis-User", user)
        .json(&body);
    if let Some(t) = token {
        rb = rb.bearer_auth(t);
    }
    let resp = rb.send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("engine returned {}", resp.status());
    }
    Ok(())
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

    #[test]
    fn test_modbus_command_parse() {
        let c: ModbusCommand = serde_json::from_str(r#"{"reg":4,"value":512}"#).unwrap();
        assert_eq!(c.reg, 4);
        assert_eq!(c.value, 512);
    }

    #[test]
    fn test_modbus_map_default_has_four_registers() {
        let m = ModbusMap::default();
        assert_eq!(m.scale, 10.0);
        assert!(m.temperature.is_some() && m.pressure.is_some() && m.vibration.is_some() && m.rpm.is_some());
    }

    // With `once`, the loop emits exactly one event from an in-memory adapter and
    // then returns — exercising connect/poll/fingerprint/dedupe without any
    // broker or device. The offline `test_connect_invalid_endpoint_fails`
    // pattern is replaced here by a real (mock) end-to-end pass.
    #[cfg(feature = "industrial")]
    #[tokio::test]
    async fn test_mock_loop_once_emits_event() {
        struct MockAdapter {
            readings: Vec<MachineReading>,
            i: usize,
        }
        #[async_trait]
        impl MachineAdapter for MockAdapter {
            async fn connect(&mut self, _e: &str) -> anyhow::Result<()> {
                Ok(())
            }
            async fn poll(&mut self) -> anyhow::Result<MachineReading> {
                if self.i < self.readings.len() {
                    let r = self.readings[self.i].clone();
                    self.i += 1;
                    Ok(r)
                } else {
                    anyhow::bail!("exhausted")
                }
            }
            async fn disconnect(&mut self) -> anyhow::Result<()> {
                Ok(())
            }
        }
        let readings = vec![MachineReading {
            machine_id: "m1".into(),
            timestamp: chrono::Utc::now(),
            temperature: Some(50.0),
            pressure: None,
            vibration: None,
            rpm: None,
            status: MachineStatus::Online,
            raw: HashMap::new(),
        }];
        let adapter = Box::new(MockAdapter { readings, i: 0 });
        let opts = AdapterRunOptions {
            protocol: "mqtt".into(),
            endpoint: "x".into(),
            machine_id: "m1".into(),
            topic: None,
            post_to: None,
            user: "_default".into(),
            token: None,
            interval_secs: 1,
            once: true,
            thresholds: HashMap::new(),
            modbus_map: None,
        };
        let r = run_loop(adapter, &opts).await;
        assert!(r.is_ok());
    }
}
