//! OEE (Overall Equipment Effectiveness) — the canonical lean / Industry-4.0 KPI.
//!
//! `OEE = Availability × Performance × Quality`. Each factor maps to two of the
//! classic **six big losses**:
//!
//! | Factor       | Losses                                   |
//! |--------------|------------------------------------------|
//! | Availability | Breakdowns, Setup & Adjustments          |
//! | Performance  | Minor Stops (idling), Reduced Speed      |
//! | Quality      | Defects/Rework, Startup Rejects          |
//!
//! The Quality factor is the same fact physis already tracks as coherence:
//! a good part is a `+1.0` node, a scrap part a `-1.0` node. So OEE Quality and
//! the global coherence index agree by construction, and the dream loop already
//! sees every defect.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Why a machine stopped or under-produced. Each variant rolls up into exactly
/// one OEE factor (one of the six big losses).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Unplanned failure. Availability loss.
    Breakdown,
    /// Changeover / tooling / adjustment. Availability loss.
    SetupAdjustment,
    /// Short stoppage, jam, blocked/starved. Performance loss.
    MinorStop,
    /// Running below ideal cycle time. Performance loss.
    ReducedSpeed,
    /// Break, no demand, no scheduled production. NOT counted against the
    /// machine — excluded from Planned Production Time.
    PlannedStop,
}

/// Which OEE factor a loss is charged against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LossFactor {
    Availability,
    Performance,
    Quality,
    /// Planned — not a loss against OEE.
    None,
}

impl StopReason {
    /// The OEE factor this stop reason is charged against.
    pub fn factor(self) -> LossFactor {
        match self {
            StopReason::Breakdown | StopReason::SetupAdjustment => LossFactor::Availability,
            StopReason::MinorStop | StopReason::ReducedSpeed => LossFactor::Performance,
            StopReason::PlannedStop => LossFactor::None,
        }
    }

    /// Human label for the six-big-losses Pareto.
    pub fn label(self) -> &'static str {
        match self {
            StopReason::Breakdown => "Breakdown",
            StopReason::SetupAdjustment => "Setup & Adjustment",
            StopReason::MinorStop => "Minor Stop / Idling",
            StopReason::ReducedSpeed => "Reduced Speed",
            StopReason::PlannedStop => "Planned Stop",
        }
    }
}

/// A single recorded stoppage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopEvent {
    pub reason: StopReason,
    pub duration_secs: f32,
    #[serde(default)]
    pub note: String,
}

/// One machine's shift accumulator. Push stops and production counts as the
/// shift runs, then `compute()` for the report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OeeShift {
    pub machine_id: String,
    /// Total scheduled time for the shift, in seconds (e.g. 8h = 28800).
    pub shift_length_secs: f32,
    /// Theoretical fastest time to make one good part, in seconds.
    pub ideal_cycle_time_secs: f32,
    /// All stoppages recorded this shift.
    pub stops: Vec<StopEvent>,
    /// Total parts produced (good + scrap).
    pub total_count: u64,
    /// Good parts (pass quality). `total_count - good_count` = scrap/rework.
    pub good_count: u64,
}

impl OeeShift {
    pub fn new(
        machine_id: impl Into<String>,
        shift_length_secs: f32,
        ideal_cycle_time_secs: f32,
    ) -> Self {
        Self {
            machine_id: machine_id.into(),
            shift_length_secs: shift_length_secs.max(0.0),
            ideal_cycle_time_secs: ideal_cycle_time_secs.max(0.0),
            stops: Vec::new(),
            total_count: 0,
            good_count: 0,
        }
    }

    /// Record a stoppage.
    pub fn record_stop(&mut self, reason: StopReason, duration_secs: f32, note: impl Into<String>) {
        self.stops.push(StopEvent {
            reason,
            duration_secs: duration_secs.max(0.0),
            note: note.into(),
        });
    }

    /// Record produced parts. `good` pass quality, `scrap` are defects/rework.
    pub fn record_production(&mut self, good: u64, scrap: u64) {
        self.good_count += good;
        self.total_count += good + scrap;
    }

    /// Seconds of stops charged against availability (breakdowns + setup).
    fn unplanned_stop_secs(&self) -> f32 {
        self.stops
            .iter()
            .filter(|s| s.reason.factor() == LossFactor::Availability)
            .map(|s| s.duration_secs)
            .sum()
    }

    /// Seconds of planned stops (breaks, no demand) — excluded from planned time.
    fn planned_stop_secs(&self) -> f32 {
        self.stops
            .iter()
            .filter(|s| s.reason == StopReason::PlannedStop)
            .map(|s| s.duration_secs)
            .sum()
    }

    /// Compute the OEE report for this shift.
    pub fn compute(&self) -> OeeReport {
        // Planned Production Time = shift − planned (non-charged) stops.
        let planned_production_time = (self.shift_length_secs - self.planned_stop_secs()).max(0.0);
        // Run Time = planned production time − unplanned (availability) stops.
        let run_time = (planned_production_time - self.unplanned_stop_secs()).max(0.0);

        let availability = safe_ratio(run_time, planned_production_time);

        // Performance = (ideal cycle × total count) / run time, capped at 1.0
        // (a machine can't truly exceed ideal; over-1.0 means cycle time is wrong).
        let performance = if run_time > 0.0 {
            ((self.ideal_cycle_time_secs * self.total_count as f32) / run_time).min(1.0)
        } else {
            0.0
        };

        let quality = safe_ratio(self.good_count as f32, self.total_count as f32);

        OeeReport {
            machine_id: self.machine_id.clone(),
            availability,
            performance,
            quality,
            oee: availability * performance * quality,
            planned_production_time_secs: planned_production_time,
            run_time_secs: run_time,
            total_count: self.total_count,
            good_count: self.good_count,
            losses: self.loss_breakdown(),
            world_class: availability * performance * quality >= 0.85,
        }
    }

    /// Six-big-losses breakdown (seconds + part counts), descending — feeds a
    /// Pareto chart for targeting the biggest loss first (lean priority rule).
    fn loss_breakdown(&self) -> Vec<LossBucket> {
        use std::collections::BTreeMap;
        let mut secs: BTreeMap<&'static str, (StopReason, f32)> = BTreeMap::new();
        for s in &self.stops {
            if s.reason == StopReason::PlannedStop {
                continue;
            }
            let e = secs.entry(s.reason.label()).or_insert((s.reason, 0.0));
            e.1 += s.duration_secs;
        }
        let mut buckets: Vec<LossBucket> = secs
            .into_values()
            .map(|(reason, seconds)| LossBucket {
                reason,
                label: reason.label().to_string(),
                factor: reason.factor(),
                lost_secs: seconds,
                lost_parts: 0,
            })
            .collect();

        // Quality loss as a virtual bucket (scrap parts → equivalent lost time).
        let scrap = self.total_count.saturating_sub(self.good_count);
        if scrap > 0 {
            buckets.push(LossBucket {
                reason: StopReason::Breakdown, // placeholder; factor disambiguates
                label: "Defects / Rework".to_string(),
                factor: LossFactor::Quality,
                lost_secs: self.ideal_cycle_time_secs * scrap as f32,
                lost_parts: scrap,
            });
        }

        buckets.sort_by(|a, b| {
            b.lost_secs
                .partial_cmp(&a.lost_secs)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        buckets
    }
}

/// One loss line for the six-big-losses Pareto.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LossBucket {
    pub reason: StopReason,
    pub label: String,
    pub factor: LossFactor,
    pub lost_secs: f32,
    pub lost_parts: u64,
}

/// Computed OEE for one machine over one shift.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OeeReport {
    pub machine_id: String,
    pub availability: f32,
    pub performance: f32,
    pub quality: f32,
    pub oee: f32,
    pub planned_production_time_secs: f32,
    pub run_time_secs: f32,
    pub total_count: u64,
    pub good_count: u64,
    pub losses: Vec<LossBucket>,
    /// True when OEE ≥ 0.85 (the textbook "world-class" threshold).
    pub world_class: bool,
}

/// Plant-level rollup — each machine weighted by its planned production time, so
/// a machine scheduled twice as long counts twice. Returns aggregate factors and
/// a merged six-big-losses Pareto across the plant.
pub fn plant_oee(reports: &[OeeReport]) -> PlantOee {
    let total_planned: f32 = reports.iter().map(|r| r.planned_production_time_secs).sum();

    let weighted = |f: fn(&OeeReport) -> f32| -> f32 {
        if total_planned <= 0.0 {
            return 0.0;
        }
        reports
            .iter()
            .map(|r| f(r) * r.planned_production_time_secs)
            .sum::<f32>()
            / total_planned
    };

    let availability = weighted(|r| r.availability);
    let performance = weighted(|r| r.performance);
    let quality = weighted(|r| r.quality);

    // Merge loss buckets across machines by label.
    use std::collections::BTreeMap;
    let mut merged: BTreeMap<String, LossBucket> = BTreeMap::new();
    for r in reports {
        for b in &r.losses {
            let e = merged.entry(b.label.clone()).or_insert_with(|| LossBucket {
                reason: b.reason,
                label: b.label.clone(),
                factor: b.factor,
                lost_secs: 0.0,
                lost_parts: 0,
            });
            e.lost_secs += b.lost_secs;
            e.lost_parts += b.lost_parts;
        }
    }
    let mut losses: Vec<LossBucket> = merged.into_values().collect();
    losses.sort_by(|a, b| {
        b.lost_secs
            .partial_cmp(&a.lost_secs)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    PlantOee {
        machine_count: reports.len(),
        availability,
        performance,
        quality,
        oee: availability * performance * quality,
        total_good: reports.iter().map(|r| r.good_count).sum(),
        total_parts: reports.iter().map(|r| r.total_count).sum(),
        losses,
    }
}

/// Plant-wide OEE rollup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantOee {
    pub machine_count: usize,
    pub availability: f32,
    pub performance: f32,
    pub quality: f32,
    pub oee: f32,
    pub total_good: u64,
    pub total_parts: u64,
    pub losses: Vec<LossBucket>,
}

/// Plant-wide, multi-machine OEE state. One live shift accumulator per machine.
/// Shop-floor infra is plant-global, not per-user, so the web layer holds this
/// behind a single `Mutex` rather than in per-tenant state.
#[derive(Debug, Default)]
pub struct OeeBoard {
    shifts: HashMap<String, OeeShift>,
}

impl OeeBoard {
    pub fn new() -> Self {
        Self {
            shifts: HashMap::new(),
        }
    }

    /// Register a machine or update its shift parameters. Existing accumulated
    /// counts/stops are preserved (use `start_shift` to zero them).
    pub fn upsert_machine(
        &mut self,
        machine_id: &str,
        shift_length_secs: f32,
        ideal_cycle_time_secs: f32,
    ) {
        match self.shifts.get_mut(machine_id) {
            Some(s) => {
                s.shift_length_secs = shift_length_secs.max(0.0);
                s.ideal_cycle_time_secs = ideal_cycle_time_secs.max(0.0);
            }
            None => {
                self.shifts.insert(
                    machine_id.to_string(),
                    OeeShift::new(machine_id, shift_length_secs, ideal_cycle_time_secs),
                );
            }
        }
    }

    /// Begin a fresh shift for a machine, clearing stops and counts but keeping
    /// its configured shift length and ideal cycle time. No-op if unknown.
    pub fn start_shift(&mut self, machine_id: &str) -> bool {
        if let Some(s) = self.shifts.get_mut(machine_id) {
            s.stops.clear();
            s.total_count = 0;
            s.good_count = 0;
            true
        } else {
            false
        }
    }

    /// Record a stoppage. Returns false if the machine is unknown.
    pub fn record_stop(
        &mut self,
        machine_id: &str,
        reason: StopReason,
        duration_secs: f32,
        note: impl Into<String>,
    ) -> bool {
        match self.shifts.get_mut(machine_id) {
            Some(s) => {
                s.record_stop(reason, duration_secs, note);
                true
            }
            None => false,
        }
    }

    /// Record produced parts. Returns false if the machine is unknown.
    pub fn record_production(&mut self, machine_id: &str, good: u64, scrap: u64) -> bool {
        match self.shifts.get_mut(machine_id) {
            Some(s) => {
                s.record_production(good, scrap);
                true
            }
            None => false,
        }
    }

    /// OEE report for one machine, or `None` if unknown.
    pub fn report(&self, machine_id: &str) -> Option<OeeReport> {
        self.shifts.get(machine_id).map(|s| s.compute())
    }

    /// All machine ids currently tracked.
    pub fn machine_ids(&self) -> Vec<String> {
        self.shifts.keys().cloned().collect()
    }

    /// Plant-wide rollup across every tracked machine.
    pub fn plant(&self) -> PlantOee {
        let reports: Vec<OeeReport> = self.shifts.values().map(|s| s.compute()).collect();
        plant_oee(&reports)
    }

    /// Persist the board (all machine shifts) to sled under `oee`.
    pub fn save(&self, db: &sled::Db) -> anyhow::Result<()> {
        db.insert("oee", serde_json::to_vec(&self.shifts)?)?;
        Ok(())
    }

    /// Load the board from sled (empty if absent/unreadable).
    pub fn load(db: &sled::Db) -> Self {
        let shifts = db
            .get("oee")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_slice::<HashMap<String, OeeShift>>(&v).ok())
            .unwrap_or_default();
        Self { shifts }
    }
}

/// `n / d`, with `0.0` when the denominator is non-positive (no shift, no parts).
fn safe_ratio(n: f32, d: f32) -> f32 {
    if d > 0.0 {
        (n / d).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Textbook example: 8h shift, 30min planned break, 47min breakdown, ideal
    /// cycle 1.0s, 19271 total parts, 423 scrap.
    #[test]
    fn test_textbook_oee() {
        let mut shift = OeeShift::new("press-01", 8.0 * 3600.0, 1.0);
        shift.record_stop(StopReason::PlannedStop, 30.0 * 60.0, "lunch");
        shift.record_stop(StopReason::Breakdown, 47.0 * 60.0, "die jam");
        shift.record_production(19271 - 423, 423);

        let r = shift.compute();
        // Planned prod time = 28800 − 1800 = 27000s. Run = 27000 − 2820 = 24180s.
        assert!((r.planned_production_time_secs - 27000.0).abs() < 1.0);
        assert!((r.run_time_secs - 24180.0).abs() < 1.0);
        // Availability = 24180/27000 ≈ 0.8956
        assert!(
            (r.availability - 0.8956).abs() < 0.001,
            "A={}",
            r.availability
        );
        // Performance = (1.0 × 19271)/24180 ≈ 0.7970
        assert!(
            (r.performance - 0.7970).abs() < 0.001,
            "P={}",
            r.performance
        );
        // Quality = 18848/19271 ≈ 0.9780
        assert!((r.quality - 0.9780).abs() < 0.001, "Q={}", r.quality);
        // OEE ≈ 0.6977
        assert!((r.oee - 0.6977).abs() < 0.001, "OEE={}", r.oee);
        assert!(!r.world_class);
    }

    #[test]
    fn test_perfect_shift_is_world_class() {
        let mut shift = OeeShift::new("m1", 3600.0, 2.0);
        shift.record_production(1800, 0); // 1800 parts × 2s = 3600s = full run
        let r = shift.compute();
        assert!((r.availability - 1.0).abs() < 1e-6);
        assert!((r.performance - 1.0).abs() < 1e-6);
        assert!((r.quality - 1.0).abs() < 1e-6);
        assert!((r.oee - 1.0).abs() < 1e-6);
        assert!(r.world_class);
    }

    #[test]
    fn test_performance_capped_at_one() {
        // Claim more parts than ideal cycle allows → perf must cap at 1.0.
        let mut shift = OeeShift::new("m1", 100.0, 1.0);
        shift.record_production(500, 0); // 500s of ideal work in 100s run → 5.0 capped
        let r = shift.compute();
        assert!((r.performance - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_empty_shift_is_zero_not_nan() {
        let shift = OeeShift::new("m1", 0.0, 0.0);
        let r = shift.compute();
        assert_eq!(r.oee, 0.0);
        assert!(r.oee.is_finite());
    }

    #[test]
    fn test_loss_breakdown_sorted_desc() {
        let mut shift = OeeShift::new("m1", 3600.0, 1.0);
        shift.record_stop(StopReason::MinorStop, 60.0, "jam");
        shift.record_stop(StopReason::Breakdown, 600.0, "motor");
        shift.record_stop(StopReason::SetupAdjustment, 300.0, "changeover");
        shift.record_production(100, 50);
        let r = shift.compute();
        // Biggest loss first.
        assert_eq!(r.losses[0].label, "Breakdown");
        assert!(r
            .losses
            .iter()
            .any(|b| b.factor == LossFactor::Quality && b.lost_parts == 50));
    }

    #[test]
    fn test_planned_stop_not_charged() {
        let mut a = OeeShift::new("m1", 3600.0, 1.0);
        a.record_stop(StopReason::PlannedStop, 600.0, "break");
        a.record_production(3000, 0);
        let r = a.compute();
        // Planned prod time = 3000s, run = 3000s, 3000 parts × 1s → perf 1.0, A 1.0.
        assert!((r.availability - 1.0).abs() < 1e-6);
        assert!((r.performance - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_plant_rollup_weighted() {
        let mut a = OeeShift::new("a", 3600.0, 1.0);
        a.record_production(3600, 0); // OEE 1.0
        let mut b = OeeShift::new("b", 3600.0, 1.0);
        b.record_stop(StopReason::Breakdown, 1800.0, "down");
        b.record_production(1800, 0); // A=0.5, P=1.0, Q=1.0 → OEE 0.5
        let plant = plant_oee(&[a.compute(), b.compute()]);
        assert_eq!(plant.machine_count, 2);
        // Equal planned weight initially, but b's planned time = 3600 too.
        // availability weighted = (1.0*3600 + 0.5*3600)/7200 = 0.75
        assert!(
            (plant.availability - 0.75).abs() < 0.001,
            "A={}",
            plant.availability
        );
        assert_eq!(plant.total_good, 5400);
    }

    #[test]
    fn test_plant_empty_is_zero() {
        let plant = plant_oee(&[]);
        assert_eq!(plant.oee, 0.0);
        assert_eq!(plant.machine_count, 0);
    }

    #[test]
    fn test_board_lifecycle() {
        let mut board = OeeBoard::new();
        board.upsert_machine("press-01", 3600.0, 1.0);
        assert!(board.record_production("press-01", 1800, 0));
        assert!(board.record_stop("press-01", StopReason::Breakdown, 600.0, "motor"));
        let r = board.report("press-01").unwrap();
        assert_eq!(r.total_count, 1800);
        assert!(r.availability < 1.0); // breakdown ate into it
                                       // Unknown machine is a no-op, not a panic.
        assert!(!board.record_production("ghost", 1, 0));
        assert!(board.report("ghost").is_none());
    }

    #[test]
    fn test_board_start_shift_resets_counts() {
        let mut board = OeeBoard::new();
        board.upsert_machine("m1", 3600.0, 1.0);
        board.record_production("m1", 100, 5);
        assert!(board.start_shift("m1"));
        let r = board.report("m1").unwrap();
        assert_eq!(r.total_count, 0);
        assert_eq!(r.good_count, 0);
    }

    #[test]
    fn test_board_plant_rollup() {
        let mut board = OeeBoard::new();
        board.upsert_machine("a", 3600.0, 1.0);
        board.upsert_machine("b", 3600.0, 1.0);
        board.record_production("a", 3600, 0);
        board.record_stop("b", StopReason::Breakdown, 1800.0, "");
        board.record_production("b", 1800, 0);
        let plant = board.plant();
        assert_eq!(plant.machine_count, 2);
        assert!((plant.availability - 0.75).abs() < 0.001);
    }
}
