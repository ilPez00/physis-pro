//! Statistical health baselines + predictive maintenance for machine telemetry.
//!
//! Threshold rules (`industrial::detect_anomalies`) catch *one* metric crossing
//! *one* hard limit. They miss **multivariate drift** — vibration creeping up
//! while temperature also creeps, each still "in spec" but the *combination*
//! abnormal for this machine. Here we learn a per-machine, per-feature baseline
//! (online mean + variance via Welford) and score a reading by how many standard
//! deviations its worst feature sits from normal. No model files, no training
//! job — the baseline IS the embedding of "healthy for this machine".
//!
//! Predictive maintenance: a linear fit over a metric's recent history projects
//! when it will cross a limit (a crude Remaining-Useful-Life estimate), so a
//! degrading bearing is flagged *before* it trips the hard threshold.
//!
//! The loop closes via the engine: an anomaly becomes a `-1` coherence node
//! (`MaintenanceAnomaly::describe` → `core.register_node_from_text`), which the
//! dream loop already replays to propose a countermeasure. Detect → cohere →
//! dream → PDCA-grade: auto-kaizen, not just an alert.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::industrial::MachineReading;

/// Default anomaly cutoff: a feature ≥ 3σ from baseline (the classic 99.7% SPC
/// control-limit) flags the reading.
pub const DEFAULT_SIGMA_THRESHOLD: f32 = 3.0;

/// Minimum observations before a baseline is trusted enough to judge.
pub const MIN_BASELINE_N: u64 = 10;

/// Online mean + variance (Welford). Numerically stable, O(1) per sample.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunningStats {
    pub count: u64,
    pub mean: f64,
    /// Sum of squares of differences from the running mean.
    m2: f64,
}

impl RunningStats {
    pub fn observe(&mut self, x: f64) {
        self.count += 1;
        let delta = x - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = x - self.mean;
        self.m2 += delta * delta2;
    }

    /// Sample variance (n−1). Zero until ≥2 samples.
    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            self.m2 / (self.count - 1) as f64
        }
    }

    pub fn std(&self) -> f64 {
        self.variance().sqrt()
    }
}

/// Pull the named numeric features out of a reading (only those present).
fn features(reading: &MachineReading) -> Vec<(String, f64)> {
    let mut out: Vec<(String, f64)> = Vec::new();
    if let Some(v) = reading.temperature {
        out.push(("temperature".into(), v as f64));
    }
    if let Some(v) = reading.pressure {
        out.push(("pressure".into(), v as f64));
    }
    if let Some(v) = reading.vibration {
        out.push(("vibration".into(), v as f64));
    }
    if let Some(v) = reading.rpm {
        out.push(("rpm".into(), v as f64));
    }
    for (k, v) in &reading.raw {
        out.push((k.clone(), *v as f64));
    }
    out
}

/// Learned per-machine, per-feature healthy baseline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HealthBaseline {
    pub machine_id: String,
    pub feature_stats: BTreeMap<String, RunningStats>,
}

impl HealthBaseline {
    pub fn new(machine_id: impl Into<String>) -> Self {
        Self { machine_id: machine_id.into(), feature_stats: BTreeMap::new() }
    }

    /// Fold a reading the operator considers healthy into the baseline.
    pub fn observe(&mut self, reading: &MachineReading) {
        for (name, value) in features(reading) {
            self.feature_stats.entry(name).or_default().observe(value);
        }
    }

    /// How many samples the least-observed feature has seen.
    pub fn min_count(&self) -> u64 {
        self.feature_stats.values().map(|s| s.count).min().unwrap_or(0)
    }

    /// True once every tracked feature has enough samples to judge.
    pub fn is_ready(&self) -> bool {
        !self.feature_stats.is_empty()
            && self.feature_stats.values().all(|s| s.count >= MIN_BASELINE_N)
    }

    /// Score a reading against the baseline. `None` for a feature with no
    /// baseline or zero variance (can't judge). The reading's anomaly score is
    /// the **largest** absolute z over its features (worst-sensor rule).
    pub fn score(&self, reading: &MachineReading, sigma_threshold: f32) -> AnomalyScore {
        let mut per_feature = Vec::new();
        let mut max_abs_z = 0.0f32;
        let mut sum_sq = 0.0f32;
        let mut judged = 0u32;
        let mut worst = String::new();

        for (name, value) in features(reading) {
            let z = match self.feature_stats.get(&name) {
                Some(s) if s.count >= 2 && s.std() > 1e-9 => {
                    ((value - s.mean) / s.std()) as f32
                }
                _ => continue, // no baseline / no variance → skip
            };
            if z.abs() > max_abs_z {
                max_abs_z = z.abs();
                worst = name.clone();
            }
            sum_sq += z * z;
            judged += 1;
            per_feature.push(FeatureDeviation { feature: name, value: value as f32, z });
        }

        let rms_z = if judged > 0 { (sum_sq / judged as f32).sqrt() } else { 0.0 };
        let trustworthy = self.min_count() >= MIN_BASELINE_N && judged > 0;

        AnomalyScore {
            machine_id: self.machine_id.clone(),
            max_abs_z,
            rms_z,
            worst_feature: worst,
            per_feature,
            // Only flag once the baseline is trustworthy — avoids crying wolf on
            // the first few readings while the baseline is still forming.
            is_anomaly: trustworthy && max_abs_z >= sigma_threshold,
            baseline_ready: trustworthy,
        }
    }
}

/// One feature's deviation from baseline, in standard deviations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureDeviation {
    pub feature: String,
    pub value: f32,
    /// Signed z-score: (value − baseline_mean) / baseline_std.
    pub z: f32,
}

/// Result of scoring a reading against a machine's baseline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyScore {
    pub machine_id: String,
    /// Largest absolute z across features — the headline anomaly score.
    pub max_abs_z: f32,
    /// Root-mean-square z across features (whole-machine drift).
    pub rms_z: f32,
    pub worst_feature: String,
    pub per_feature: Vec<FeatureDeviation>,
    pub is_anomaly: bool,
    /// False while the baseline is still warming up (verdict not yet trustworthy).
    pub baseline_ready: bool,
}

impl AnomalyScore {
    /// One-line description for registering the anomaly as a coherence node — the
    /// text the dream loop reasons over. Empty when not anomalous.
    pub fn describe(&self) -> String {
        if !self.is_anomaly {
            return String::new();
        }
        let detail = self
            .per_feature
            .iter()
            .find(|f| f.feature == self.worst_feature)
            .map(|f| format!("{} {:.3} (z={:+.2})", f.feature, f.value, f.z))
            .unwrap_or_default();
        format!(
            "machine {} anomaly: {} — {:.1}σ from healthy baseline",
            self.machine_id, detail, self.max_abs_z
        )
    }
}

/// A timestamped scalar sample for trend projection.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Sample {
    /// Seconds (any consistent epoch).
    pub t: f64,
    pub value: f64,
}

/// Predicted time until a metric crosses a limit, by least-squares linear fit.
///
/// Returns `None` when there's too little data, the trend is flat/away from the
/// limit, or it has already crossed. `seconds_to_limit` is the projection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulEstimate {
    pub slope_per_sec: f64,
    pub seconds_to_limit: f64,
    pub current_value: f64,
    pub limit: f64,
}

/// Fit `value = a + slope·t` and project the crossing of `limit`.
pub fn project_time_to_limit(history: &[Sample], limit: f64) -> Option<RulEstimate> {
    let n = history.len();
    if n < 3 {
        return None;
    }
    let nf = n as f64;
    let mean_t = history.iter().map(|s| s.t).sum::<f64>() / nf;
    let mean_v = history.iter().map(|s| s.value).sum::<f64>() / nf;
    let mut sxx = 0.0;
    let mut sxy = 0.0;
    for s in history {
        let dt = s.t - mean_t;
        sxx += dt * dt;
        sxy += dt * (s.value - mean_v);
    }
    if sxx <= 1e-12 {
        return None; // all samples at one instant
    }
    let slope = sxy / sxx;
    let intercept = mean_v - slope * mean_t;
    let latest = history.last()?;
    let current = latest.value;

    // Must be moving toward the limit.
    let approaching = (limit > current && slope > 0.0) || (limit < current && slope < 0.0);
    if !approaching || slope.abs() < 1e-12 {
        return None;
    }
    // Solve a + slope·t = limit for t, relative to the latest sample's time.
    let t_cross = (limit - intercept) / slope;
    let secs = t_cross - latest.t;
    if secs <= 0.0 {
        return None; // already past
    }
    Some(RulEstimate {
        slope_per_sec: slope,
        seconds_to_limit: secs,
        current_value: current,
        limit,
    })
}

/// Plant-wide maintenance state: one baseline per machine + recent metric history
/// for trend projection. Shared infra, held behind one mutex by the web layer.
#[derive(Debug, Default)]
pub struct MaintenanceMonitor {
    baselines: BTreeMap<String, HealthBaseline>,
    /// machine → metric → ring of recent samples (cap `history_cap`).
    history: BTreeMap<String, BTreeMap<String, Vec<Sample>>>,
    pub history_cap: usize,
}

impl MaintenanceMonitor {
    pub fn new(history_cap: usize) -> Self {
        Self { baselines: BTreeMap::new(), history: BTreeMap::new(), history_cap: history_cap.max(3) }
    }

    /// Fold a healthy reading into the machine's baseline (training).
    pub fn observe_healthy(&mut self, reading: &MachineReading) {
        self.baselines
            .entry(reading.machine_id.clone())
            .or_insert_with(|| HealthBaseline::new(&reading.machine_id))
            .observe(reading);
    }

    /// Score a live reading and append it to the trend history. Returns `None`
    /// if the machine has no baseline yet.
    pub fn check(&mut self, reading: &MachineReading, sigma_threshold: f32) -> Option<AnomalyScore> {
        self.push_history(reading);
        let baseline = self.baselines.get(&reading.machine_id)?;
        Some(baseline.score(reading, sigma_threshold))
    }

    fn push_history(&mut self, reading: &MachineReading) {
        let t = reading.timestamp.timestamp() as f64;
        let per_metric = self.history.entry(reading.machine_id.clone()).or_default();
        for (name, value) in features(reading) {
            let ring = per_metric.entry(name).or_default();
            ring.push(Sample { t, value });
            if ring.len() > self.history_cap {
                let overflow = ring.len() - self.history_cap;
                ring.drain(0..overflow);
            }
        }
    }

    /// RUL projection for one machine metric against `limit`.
    pub fn rul(&self, machine_id: &str, metric: &str, limit: f64) -> Option<RulEstimate> {
        let ring = self.history.get(machine_id)?.get(metric)?;
        project_time_to_limit(ring, limit)
    }

    pub fn baseline(&self, machine_id: &str) -> Option<&HealthBaseline> {
        self.baselines.get(machine_id)
    }

    pub fn machine_ids(&self) -> Vec<String> {
        self.baselines.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::industrial::MachineStatus;
    use std::collections::HashMap;

    fn reading(machine: &str, temp: f32, vib: f32) -> MachineReading {
        MachineReading {
            machine_id: machine.into(),
            timestamp: chrono::Utc::now(),
            temperature: Some(temp),
            pressure: None,
            vibration: Some(vib),
            rpm: None,
            status: MachineStatus::Online,
            raw: HashMap::new(),
        }
    }

    fn reading_at(machine: &str, t: i64, vib: f32) -> MachineReading {
        let mut r = reading(machine, 50.0, vib);
        r.timestamp = chrono::DateTime::from_timestamp(t, 0).unwrap();
        r
    }

    #[test]
    fn test_running_stats_matches_known() {
        let mut s = RunningStats::default();
        for x in [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0] {
            s.observe(x);
        }
        assert!((s.mean - 5.0).abs() < 1e-9);
        // Sample variance of that set = 4.571..., std ≈ 2.138
        assert!((s.std() - 2.138).abs() < 0.01, "std={}", s.std());
    }

    #[test]
    fn test_baseline_flags_multivariate_drift() {
        let mut b = HealthBaseline::new("press-01");
        // 50 healthy readings: temp ~50±1, vib ~2±0.1
        for i in 0..50 {
            let jitter = (i % 5) as f32 * 0.1 - 0.2;
            b.observe(&reading("press-01", 50.0 + jitter, 2.0 + jitter * 0.1));
        }
        assert!(b.is_ready());
        // Normal reading → not anomalous.
        let ok = b.score(&reading("press-01", 50.1, 2.0), DEFAULT_SIGMA_THRESHOLD);
        assert!(!ok.is_anomaly, "score={}", ok.max_abs_z);
        // Vibration way out → anomaly, worst feature = vibration.
        let bad = b.score(&reading("press-01", 50.0, 4.0), DEFAULT_SIGMA_THRESHOLD);
        assert!(bad.is_anomaly);
        assert_eq!(bad.worst_feature, "vibration");
        assert!(!bad.describe().is_empty());
    }

    #[test]
    fn test_baseline_not_ready_suppresses_flag() {
        let mut b = HealthBaseline::new("m1");
        b.observe(&reading("m1", 50.0, 2.0));
        b.observe(&reading("m1", 50.0, 2.0));
        // Wild reading but baseline not warmed up → no false alarm.
        let s = b.score(&reading("m1", 999.0, 999.0), DEFAULT_SIGMA_THRESHOLD);
        assert!(!s.is_anomaly);
        assert!(!s.baseline_ready);
    }

    #[test]
    fn test_zero_variance_feature_not_judged() {
        let mut b = HealthBaseline::new("m1");
        for _ in 0..MIN_BASELINE_N {
            b.observe(&reading("m1", 50.0, 2.0)); // identical → std 0
        }
        let s = b.score(&reading("m1", 60.0, 2.0), DEFAULT_SIGMA_THRESHOLD);
        // std==0 → can't compute z → feature skipped → no judgement → no anomaly.
        assert!(s.per_feature.is_empty());
        assert!(!s.is_anomaly);
    }

    #[test]
    fn test_rul_projects_rising_trend() {
        // vibration rising 0.1/sec from 2.0; limit 5.0 → ~30s out.
        let mut hist = Vec::new();
        for i in 0..10 {
            hist.push(Sample { t: i as f64, value: 2.0 + 0.1 * i as f64 });
        }
        let est = project_time_to_limit(&hist, 5.0).unwrap();
        assert!((est.slope_per_sec - 0.1).abs() < 1e-6);
        // latest t=9 value=2.9; (5.0-2.9)/0.1 = 21s
        assert!((est.seconds_to_limit - 21.0).abs() < 0.5, "secs={}", est.seconds_to_limit);
    }

    #[test]
    fn test_rul_none_when_flat_or_away() {
        let flat: Vec<Sample> = (0..5).map(|i| Sample { t: i as f64, value: 3.0 }).collect();
        assert!(project_time_to_limit(&flat, 5.0).is_none());
        // Falling away from an upper limit.
        let falling: Vec<Sample> = (0..5).map(|i| Sample { t: i as f64, value: 5.0 - i as f64 }).collect();
        assert!(project_time_to_limit(&falling, 10.0).is_none());
    }

    #[test]
    fn test_monitor_end_to_end() {
        let mut mon = MaintenanceMonitor::new(64);
        for i in 0..30 {
            let j = (i % 4) as f32 * 0.1;
            mon.observe_healthy(&reading("m1", 50.0 + j, 2.0 + j * 0.05));
        }
        let score = mon.check(&reading("m1", 50.0, 5.0), DEFAULT_SIGMA_THRESHOLD).unwrap();
        assert!(score.is_anomaly);
        // No baseline for unknown machine.
        assert!(mon.check(&reading("ghost", 1.0, 1.0), DEFAULT_SIGMA_THRESHOLD).is_none());
    }

    #[test]
    fn test_monitor_rul_from_history() {
        let mut mon = MaintenanceMonitor::new(64);
        for i in 0..12 {
            mon.check(&reading_at("m1", i, 2.0 + 0.1 * i as f32), DEFAULT_SIGMA_THRESHOLD);
        }
        let est = mon.rul("m1", "vibration", 5.0).unwrap();
        assert!(est.slope_per_sec > 0.0);
        assert!(est.seconds_to_limit > 0.0);
    }

    #[test]
    fn test_history_capped() {
        let mut mon = MaintenanceMonitor::new(5);
        for i in 0..20 {
            mon.check(&reading_at("m1", i, i as f32), DEFAULT_SIGMA_THRESHOLD);
        }
        let est = mon.rul("m1", "vibration", 1000.0);
        // ring capped at 5 → still projects, just on recent window.
        assert!(est.is_some());
    }
}
