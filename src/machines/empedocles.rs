//! empedocles — the physical-machine machine.
//!
//! Perceive a machine's telemetry; reduce each reading to one of the four
//! **elements** (Earth/mechanical, Air/pneumatic, Fire/thermal, Water/hydraulic);
//! elaborate by detecting *anomaly* — a deviation from the running baseline past
//! threshold (Empedocles' Strife: the reading pulling away from its element's
//! steady mixture). Coherence maps directly: a reading is Stable only while it
//! sits near baseline; a sustained departure is Anomaly.
//!
//! Pure + testable; `register_*` is the only core touch. The camera/psutil
//! perception layer lives in the standalone `physis-empedocles` repo — this is
//! the thin reducer over the shared core.

use crate::core::PhysisCore;
use crate::embed::VectorEmbed;

/// Empedocles' four roots, mapped to physical-machine domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Element {
    /// Mechanical: vibration, torque, position, RPM.
    Earth,
    /// Pneumatic: air pressure, flow rate, valve state.
    Air,
    /// Thermal: temperature, heat flux, flame signal.
    Fire,
    /// Hydraulic/cooling: fluid pressure, flow, level, coolant temp.
    Water,
}

impl Element {
    /// Best-effort classification of a metric name into an element.
    pub fn of_metric(metric: &str) -> Element {
        let m = metric.to_ascii_lowercase();
        // Hydraulic/cooling wins over thermal: a coolant temp is Water, not Fire.
        if m.contains("coolant") || m.contains("fluid") || m.contains("hydraul") {
            Element::Water
        } else if m.contains("temp") || m.contains("thermal") || m.contains("heat") || m.contains("flame") {
            Element::Fire
        } else if m.contains("valve")
            || m.contains("pneum")
            || (m.contains("air") && (m.contains("pressure") || m.contains("flow")))
        {
            Element::Air
        } else if m.contains("level") || m.contains("flow") {
            Element::Water
        } else {
            // vibration / torque / rpm / position / cpu / load default to the solid root.
            Element::Earth
        }
    }
}

/// One telemetry sample: a metric reading against its running baseline.
#[derive(Debug, Clone)]
pub struct Reading {
    pub metric: String,
    pub value: f32,
    pub baseline: f32,
}

impl Reading {
    /// Signed departure from baseline.
    pub fn deviation(&self) -> f32 {
        self.value - self.baseline
    }
    /// Which element this reading belongs to.
    pub fn element(&self) -> Element {
        Element::of_metric(&self.metric)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Near baseline → the mixture holds (coherence +1).
    Stable,
    /// Past threshold → Strife pulling the reading from its element (coherence −1).
    Anomaly,
}

impl Status {
    /// Map to the engine's coherence rating.
    pub fn coherence(self) -> f32 {
        match self {
            Status::Stable => 1.0,
            Status::Anomaly => -1.0,
        }
    }
}

/// Classify a reading: anomalous if `|deviation| > threshold`.
/// Mirrors `detect.py::Detector.check`.
pub fn classify(r: &Reading, threshold: f32) -> Status {
    if r.deviation().abs() > threshold {
        Status::Anomaly
    } else {
        Status::Stable
    }
}

/// Trend slope over a window of values (last − first) / len.
/// Mirrors `metrics.py::get_trend`. A `|trend|` past 0.5 is itself anomalous.
pub fn trend(values: &[f32]) -> f32 {
    if values.len() < 2 {
        return 0.0;
    }
    (values[values.len() - 1] - values[0]) / values.len() as f32
}

/// Adapter: register each reading as a labelled coherence node (label encodes
/// element + status) and bridge it into the shared graph. Returns node ids.
pub fn register_readings(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    readings: &[Reading],
    threshold: f32,
) -> Vec<String> {
    let mut ids = Vec::new();
    for r in readings {
        let status = classify(r, threshold);
        let text = format!(
            "{:?}/{}: {:?} (value {:.2} baseline {:.2} dev {:+.2})",
            r.element(),
            r.metric,
            status,
            r.value,
            r.baseline,
            r.deviation()
        );
        let id = core.register_node_from_text(&text, embedder);
        core.bridge_node(&id);
        ids.push(id);
    }
    ids
}

/// Engine-side reduce: register each reading and elaborate (score coherence,
/// detect anomalous trend) entirely in-Rust. Python Machine.reduce()
/// delegates here (Phase-8 read-back; Python stays pure perception).
pub fn reduce(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    readings: &[Reading],
    threshold: f32,
) -> f32 {
    // coherence = mean of per-reading Status (+1 stable, -1 anomaly)
    let mut score: f32 = 0.0;
    for r in readings {
        let status = classify(r, threshold);
        let text = format!("{:?}/{} value {:.2} baseline {:.2} dev {:+.2}",
            r.element(), r.metric, r.value, r.baseline, r.deviation());
        core.register_node_from_text(&text, embedder);
        core.bridge_node(&text);
        score += status.coherence();
    }
    if !readings.is_empty() {
        score /= readings.len() as f32;
    }
    // a sustained upward trend is itself a strife signal -> penalise
    let vals: Vec<f32> = readings.iter().map(|r| r.value).collect();
    if trend(&vals).abs() > 0.5 {
        score -= 0.5;
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::RandomProjectionEmbedder;

    #[test]
    fn metric_names_map_to_elements() {
        assert_eq!(Element::of_metric("temp_coretemp"), Element::Fire);
        assert_eq!(Element::of_metric("valve_state"), Element::Air);
        assert_eq!(Element::of_metric("coolant_temp"), Element::Water);
        assert_eq!(Element::of_metric("spindle_rpm"), Element::Earth);
    }

    #[test]
    fn classify_flags_deviation_past_threshold() {
        let r = Reading { metric: "cpu_percent".into(), value: 95.0, baseline: 40.0 };
        assert_eq!(classify(&r, 2.0), Status::Anomaly);
        assert_eq!(classify(&r, 2.0).coherence(), -1.0);
        let steady = Reading { metric: "cpu_percent".into(), value: 41.0, baseline: 40.0 };
        assert_eq!(classify(&steady, 2.0), Status::Stable);
    }

    #[test]
    fn reduce_scores_stable_readings_positive() {
        let mut core = PhysisCore::new();
        let embedder = RandomProjectionEmbedder::new(64);
        let steady = Reading { metric: "cpu_percent".into(), value: 41.0, baseline: 40.0 };
        let s = reduce(&mut core, &embedder, &[steady], 2.0);
        assert_eq!(s, 1.0); // one stable reading -> +1
    }

    #[test]
    fn reduce_penalises_anomaly_and_trend() {
        let mut core = PhysisCore::new();
        let embedder = RandomProjectionEmbedder::new(64);
        let hot = Reading { metric: "temp_core".into(), value: 95.0, baseline: 40.0 };
        let s = reduce(&mut core, &embedder, &[hot], 2.0);
        assert_eq!(s, -1.0); // anomalous -> -1
        // trend past 0.5 penalises further
        let rising = [
            Reading { metric: "temp_core".into(), value: 10.0, baseline: 10.0 },
            Reading { metric: "temp_core".into(), value: 40.0, baseline: 10.0 },
            Reading { metric: "temp_core".into(), value: 90.0, baseline: 10.0 },
        ];
        let s2 = reduce(&mut core, &embedder, &rising, 2.0);
        assert!(s2 < -0.5, "rising trend should penalise below mean");
    }

    #[test]
    fn trend_is_zero_without_movement() {
        assert_eq!(trend(&[]), 0.0);
        assert_eq!(trend(&[5.0]), 0.0);
        assert!(trend(&[10.0, 20.0, 30.0, 40.0]) > 0.0);
    }
}
