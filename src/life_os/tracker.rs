//! LifeOS — native goal/habit tracking layer.
//!
//! Self-contained: physis OWNS the goal state rather than syncing it from an
//! external Praxis/Agora service over HTTP every tick. Callers push goals in
//! (`POST /api/v1/lifeos/goal`) and poll status/drift out
//! (`GET /api/v1/lifeos/status`). No outbound HTTP, no service URLs to configure.

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use chrono::Timelike;
use serde::{Deserialize, Serialize};

use crate::gantt::GanttScheduler;
use crate::graph::RawNodeKey;

/// Bioenergetic charge — the operational distillation of the Reich/Peat model
/// (charge = metabolic energy = capacity to do ordered work). Praxis derives it
/// from check-ins and pushes it in; the engine uses it to gate what work it asks
/// of the user. Ordered: Depleted < Steady < Charged.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum Charge {
    Depleted,
    #[default]
    Steady,
    Charged,
}

impl Charge {
    pub fn rank(self) -> u8 {
        match self {
            Charge::Depleted => 0,
            Charge::Steady => 1,
            Charge::Charged => 2,
        }
    }

    /// Availability factor (0–1) for the human OEE/schedule domain.
    pub fn availability(self) -> f32 {
        match self {
            Charge::Depleted => 0.3,
            Charge::Steady => 0.6,
            Charge::Charged => 1.0,
        }
    }

    /// Can a goal demanding `cost` be sustained at this charge?
    pub fn can_sustain(self, cost: Charge) -> bool {
        self.rank() >= cost.rank()
    }
}

/// A tracked life goal/habit. `coherence_target` is the desired steady-state
/// coherence (informational for now); `progress` is 0.0–1.0. `charge_cost` is the
/// minimum bioenergetic charge the goal demands (deep work = Charged, admin =
/// Steady, gentle/recovery = Depleted-ok).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LifeOSGoal {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub coherence_target: f32,
    #[serde(default)]
    pub progress: f32,
    #[serde(default)]
    pub charge_cost: Charge,
}

/// Schedule-drift snapshot clients poll (replaces the old outbound push).
#[derive(Serialize, Clone, Debug)]
pub struct DriftReport {
    pub schedule_health: f32,
    pub at_risk_tasks: Vec<String>,
    /// True when the schedule is unhealthy enough to warrant a nudge.
    pub drifting: bool,
}

/// Where a goal sits in the alchemical opus — read from its progress trajectory.
/// Effort/recovery is the *conservative* loop within a fixed form (Working);
/// solve-et-coagula is the *transmutative* loop on the form itself: Solutio (the
/// definition is failing — dissolve + re-form) vs Coagulating (a form is taking).
/// The keystone for phase-aware suggestions + the seeker's-protocol matching.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AlchemicalPhase {
    /// Too new to tell.
    Nascent,
    /// Progress rising — the form is crystallising.
    Coagulating,
    /// Oscillating mid-progress — effort/recovery within the form.
    Working,
    /// Stalled-low or declining — the definition isn't taking; dissolve + re-form.
    Solutio,
}

/// A concrete bioenergetic practice the engine proposes when charge is low or the
/// schedule drifts. `tool` is a bioenergetics-hub tool key (e.g. `5r`, `posture`,
/// `coldheat`) the UI opens directly — the countermeasure becomes actionable, not
/// just prose.
#[derive(Serialize, Clone, Debug)]
pub struct SuggestedPractice {
    pub tool: String,
    pub title: String,
    pub why: String,
}

/// A charge-governed "today" plan: what to do given the user's current charge.
#[derive(Serialize, Clone, Debug)]
pub struct TodayPlan {
    pub charge: Charge,
    /// Incomplete goals the current charge can sustain, lowest-progress first.
    pub doable: Vec<LifeOSGoal>,
    /// Incomplete goals that need more charge than is available right now.
    pub deferred: Vec<LifeOSGoal>,
    /// A bioenergetic countermeasure (prose) when charge is low or drifting.
    pub countermeasure: Option<String>,
    /// The primary openable practice (tool key + rationale), rotated through the day.
    pub suggested_practice: Option<SuggestedPractice>,
    /// Other recovery methods for the same state (the rest of the pool).
    pub alternatives: Vec<SuggestedPractice>,
    /// Alchemical phase per doable/deferred goal id (effort vs solve/coagula).
    pub phases: BTreeMap<String, AlchemicalPhase>,
    pub drift: DriftReport,
}

fn practice(tool: &str, title: &str, why: &str) -> SuggestedPractice {
    SuggestedPractice { tool: tool.into(), title: title.into(), why: why.into() }
}

/// Peat-metabolic / parasympathetic restoration methods for a depleted charge.
fn depleted_pool() -> Vec<SuggestedPractice> {
    vec![
        practice("5r", "5R regenerative breathing", "Rebuild metabolic charge (Peat) with regenerative breathing before pushing."),
        practice("coldheat", "Warmth protocol", "Raise metabolic rate with heat/light (Peat) — cold drains, warmth restores."),
        practice("hydration", "Rehydrate + salt", "Cellular energy needs water + sodium (Peat) — depletion often hides as thirst."),
        practice("circadian", "Get bright light", "Light restores rhythm + thyroid (Peat) — the cheapest charge."),
        practice("gratitude", "Gratitude pause", "Shift to parasympathetic; lower the stress load so energy returns."),
        practice("color", "Color meditation", "Calm the nervous system before re-engaging."),
    ]
}

/// Reich-release / focus-reset methods for a steady-but-drifting state.
fn drifting_pool() -> Vec<SuggestedPractice> {
    vec![
        practice("posture", "Release the armor", "Release held tension (Reich: jaw / chest / diaphragm) to clear the block."),
        practice("5r", "Emotional-control breath", "5R controllo emozionale — steady the system, then one focused block."),
        practice("guided", "Focused reset", "A short guided session to re-center on the critical path."),
        practice("cbt", "Reframe the block", "Defuse the stuck thought (CBT) so the schedule can move."),
        practice("bolt", "Breath-hold reset", "CO2-tolerance breathing calms arousal and sharpens focus."),
    ]
}

/// Native in-memory goal store. No HTTP client, no external dependency.
#[derive(Default)]
pub struct LifeOSTracker {
    pub goals: Vec<LifeOSGoal>,
    /// Current bioenergetic charge (pushed in by Praxis from check-ins).
    pub charge: Charge,
    /// Per-goal progress trajectory (unix ts, progress) — drives the phase read.
    /// Only real changes are recorded, so the slope reflects actual movement.
    history: BTreeMap<String, Vec<(i64, f32)>>,
}

impl LifeOSTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a goal by `id`. Returns true if it replaced an existing one.
    pub fn upsert_goal(&mut self, goal: LifeOSGoal) -> bool {
        let (id, progress) = (goal.id.clone(), goal.progress);
        let replaced = if let Some(existing) = self.goals.iter_mut().find(|g| g.id == goal.id) {
            *existing = goal;
            true
        } else {
            self.goals.push(goal);
            false
        };
        self.record_progress(&id, progress);
        replaced
    }

    /// Set a goal's progress (0.0–1.0). Returns false if unknown.
    pub fn update_progress(&mut self, id: &str, progress: f32) -> bool {
        match self.goals.iter_mut().find(|g| g.id == id) {
            Some(g) => {
                g.progress = progress.clamp(0.0, 1.0);
                let p = g.progress;
                self.record_progress(id, p);
                true
            }
            None => false,
        }
    }

    /// Append a progress sample for a goal, deduping no-change repeats so the
    /// trajectory reflects real movement. Capped to the last 64 samples.
    fn record_progress(&mut self, id: &str, progress: f32) {
        let p = progress.clamp(0.0, 1.0);
        let h = self.history.entry(id.to_string()).or_default();
        if h.last().is_none_or(|(_, last)| (last - p).abs() > 0.001) {
            h.push((chrono::Utc::now().timestamp(), p));
            if h.len() > 64 {
                let overflow = h.len() - 64;
                h.drain(0..overflow);
            }
        }
    }

    /// Alchemical phase of a goal, read from its progress trajectory (slope +
    /// stuck-low detection). `Nascent` until there's enough signal.
    pub fn phase_of(&self, id: &str) -> AlchemicalPhase {
        let h = match self.history.get(id) {
            Some(h) if h.len() >= 3 => h,
            _ => return AlchemicalPhase::Nascent,
        };
        // Least-squares slope of progress over time → progress/day.
        let n = h.len() as f64;
        let mean_t = h.iter().map(|(t, _)| *t as f64).sum::<f64>() / n;
        let mean_p = h.iter().map(|(_, p)| *p as f64).sum::<f64>() / n;
        let (mut sxx, mut sxy) = (0.0f64, 0.0f64);
        for (t, p) in h {
            let dt = *t as f64 - mean_t;
            sxx += dt * dt;
            sxy += dt * (*p as f64 - mean_p);
        }
        let slope_per_day = if sxx > 0.0 { (sxy / sxx) * 86_400.0 } else { 0.0 };
        let last = h.last().map(|(_, p)| *p).unwrap_or(0.0);
        let span_days = (h.last().unwrap().0 - h.first().unwrap().0) as f64 / 86_400.0;

        const RISE: f64 = 0.02; // >2%/day is meaningful movement
        if slope_per_day > RISE {
            AlchemicalPhase::Coagulating
        } else if slope_per_day < -RISE {
            AlchemicalPhase::Solutio // declining — the form is dissolving
        } else if last >= 0.95 {
            AlchemicalPhase::Coagulating // basically formed
        } else if last < 0.3 && span_days >= 7.0 {
            AlchemicalPhase::Solutio // stuck low for a week+ → re-form, don't grind
        } else {
            AlchemicalPhase::Working
        }
    }

    /// Remove a goal by id. Returns false if unknown.
    pub fn remove_goal(&mut self, id: &str) -> bool {
        let before = self.goals.len();
        self.goals.retain(|g| g.id != id);
        self.goals.len() != before
    }

    /// Set the current bioenergetic charge (Praxis derives this from check-ins).
    pub fn set_charge(&mut self, charge: Charge) {
        self.charge = charge;
    }

    /// Charge-governed plan: split incomplete goals into what the current charge
    /// can sustain vs what needs more, and propose a bioenergetic countermeasure
    /// when depleted or drifting. This is where the Reich/Peat philosophy operates
    /// — energy gates the work the system asks for, rather than nagging regardless.
    pub fn today(&self, scheduler: &GanttScheduler) -> TodayPlan {
        let mut doable = Vec::new();
        let mut deferred = Vec::new();
        for g in &self.goals {
            if g.progress >= 1.0 {
                continue;
            }
            if self.charge.can_sustain(g.charge_cost) {
                doable.push(g.clone());
            } else {
                deferred.push(g.clone());
            }
        }
        doable.sort_by(|a, b| a.progress.partial_cmp(&b.progress).unwrap_or(std::cmp::Ordering::Equal));

        let drift = self.drift_report(scheduler);
        let (countermeasure, pool) = match self.charge {
            Charge::Depleted => (
                Some(
                    "Charge depleted — restore before pushing. Diaphragmatic breathing + warmth/light \
                     (Peat), release jaw & chest armoring (Reich). Take the gentle wins; don't force the \
                     critical path.".to_string(),
                ),
                depleted_pool(),
            ),
            Charge::Steady if drift.drifting => (
                Some(
                    "Steady but the schedule is slipping — release the held tension, then one focused \
                     25-min block on the critical-path goal.".to_string(),
                ),
                drifting_pool(),
            ),
            _ => (None, Vec::new()),
        };

        // Rotate the primary practice through the day (deterministic, no RNG) so the
        // same method isn't proposed every time; the rest become alternatives.
        let (suggested_practice, alternatives) = if pool.is_empty() {
            (None, Vec::new())
        } else {
            let i = chrono::Utc::now().hour() as usize % pool.len();
            let primary = pool[i].clone();
            let alts = pool
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, p)| p.clone())
                .collect();
            (Some(primary), alts)
        };

        // Alchemical phase per surfaced goal (effort/recovery vs solve/coagula).
        let phases: BTreeMap<String, AlchemicalPhase> = doable
            .iter()
            .chain(deferred.iter())
            .map(|g| (g.id.clone(), self.phase_of(&g.id)))
            .collect();

        TodayPlan { charge: self.charge, doable, deferred, countermeasure, suggested_practice, alternatives, phases, drift }
    }

    /// Persist this tenant's goals + charge + trajectory to sled under `lifeos:<uid>`.
    pub fn save(&self, db: &sled::Db, uid: &str) -> anyhow::Result<()> {
        let snapshot = (&self.goals, self.charge, &self.history);
        db.insert(format!("lifeos:{uid}"), serde_json::to_vec(&snapshot)?)?;
        Ok(())
    }

    /// Load a tenant's state from sled (empty tracker if absent/unreadable).
    /// Tolerates older formats (goals+charge, or goals-only).
    pub fn load(db: &sled::Db, uid: &str) -> Self {
        let raw = db.get(format!("lifeos:{uid}")).ok().flatten();
        let Some(v) = raw else { return Self::default() };
        type Full = (Vec<LifeOSGoal>, Charge, BTreeMap<String, Vec<(i64, f32)>>);
        if let Ok((goals, charge, history)) = serde_json::from_slice::<Full>(&v) {
            Self { goals, charge, history }
        } else if let Ok((goals, charge)) = serde_json::from_slice::<(Vec<LifeOSGoal>, Charge)>(&v) {
            Self { goals, charge, history: BTreeMap::new() }
        } else if let Ok(goals) = serde_json::from_slice::<Vec<LifeOSGoal>>(&v) {
            Self { goals, charge: Charge::default(), history: BTreeMap::new() }
        } else {
            Self::default()
        }
    }

    fn task_id_for(goal_id: &str) -> (RawNodeKey, String) {
        let mut hasher = DefaultHasher::new();
        goal_id.hash(&mut hasher);
        let node = RawNodeKey { data: hasher.finish() };
        let task_id = format!("task_{}", node.data);
        (node, task_id)
    }

    /// Mirror goals into the scheduler: add any missing task (idempotent — tasks
    /// are keyed `task_<hash>`, not the goal id) and keep each task's progress in
    /// sync with the goal. Call this each tick before `recompute`.
    pub fn apply_to_scheduler(&self, scheduler: &mut GanttScheduler) {
        for goal in &self.goals {
            let (node, task_id) = Self::task_id_for(&goal.id);
            if !scheduler.has_task(&task_id) {
                // Default 1-hour planned duration for a synced goal.
                scheduler.add_task_with_duration(node, goal.title.clone(), 3600, 0, vec![]);
            }
            scheduler.update_task_progress(&task_id, goal.progress);
        }
    }

    /// Current schedule-drift snapshot (what the old `push_schedule_drift` sent,
    /// now pulled by clients instead of pushed over HTTP).
    pub fn drift_report(&self, scheduler: &GanttScheduler) -> DriftReport {
        let health = scheduler.schedule_health();
        let at_risk_tasks: Vec<String> = scheduler
            .tasks_approaching_late_finish(3600)
            .iter()
            .map(|t| t.id.clone())
            .collect();
        DriftReport {
            schedule_health: health,
            at_risk_tasks,
            drifting: health < 0.7 || health.is_nan(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gantt::AbstractDomain;

    fn goal(id: &str, progress: f32) -> LifeOSGoal {
        LifeOSGoal { id: id.into(), title: format!("Goal {id}"), coherence_target: 1.0, progress, charge_cost: Charge::Steady }
    }

    #[test]
    fn test_upsert_and_update() {
        let mut t = LifeOSTracker::new();
        assert!(!t.upsert_goal(goal("a", 0.0))); // new
        assert!(t.upsert_goal(goal("a", 0.5)));   // replaced
        assert_eq!(t.goals.len(), 1);
        assert!(t.update_progress("a", 0.8));
        assert!(!t.update_progress("ghost", 0.1));
        assert!((t.goals[0].progress - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_apply_to_scheduler_idempotent_and_syncs_progress() {
        let mut t = LifeOSTracker::new();
        t.upsert_goal(goal("a", 0.4));
        let mut s = GanttScheduler::new(Box::new(AbstractDomain));
        t.apply_to_scheduler(&mut s);
        t.apply_to_scheduler(&mut s); // second call must not duplicate
        assert_eq!(s.tasks.len(), 1);
        assert!((s.tasks[0].progress - 0.4).abs() < 1e-6);
        // progress change propagates on next apply
        t.update_progress("a", 0.9);
        t.apply_to_scheduler(&mut s);
        assert_eq!(s.tasks.len(), 1);
        assert!((s.tasks[0].progress - 0.9).abs() < 1e-6);
    }

    #[test]
    fn test_drift_report() {
        let t = LifeOSTracker::new();
        let s = GanttScheduler::new(Box::new(AbstractDomain));
        let d = t.drift_report(&s);
        // empty schedule = healthy
        assert!((d.schedule_health - 1.0).abs() < 1e-6);
        assert!(!d.drifting);
    }

    #[test]
    fn test_remove() {
        let mut t = LifeOSTracker::new();
        t.upsert_goal(goal("a", 0.0));
        assert!(t.remove_goal("a"));
        assert!(!t.remove_goal("a"));
        assert!(t.goals.is_empty());
    }

    #[test]
    fn test_charge_gates_today() {
        let mut t = LifeOSTracker::new();
        t.upsert_goal(LifeOSGoal { id: "deep".into(), title: "deep work".into(), coherence_target: 1.0, progress: 0.2, charge_cost: Charge::Charged });
        t.upsert_goal(LifeOSGoal { id: "admin".into(), title: "admin".into(), coherence_target: 1.0, progress: 0.1, charge_cost: Charge::Steady });
        t.upsert_goal(LifeOSGoal { id: "rest".into(), title: "stretch".into(), coherence_target: 1.0, progress: 0.0, charge_cost: Charge::Depleted });
        let s = GanttScheduler::new(Box::new(AbstractDomain));

        // Depleted: only the depleted-ok goal is doable; a countermeasure appears.
        t.set_charge(Charge::Depleted);
        let plan = t.today(&s);
        assert_eq!(plan.doable.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(), ["rest"]);
        assert_eq!(plan.deferred.len(), 2);
        assert!(plan.countermeasure.is_some());
        // A practice is proposed (rotated through the depleted pool) + alternatives.
        let pool = depleted_pool();
        let primary = plan.suggested_practice.as_ref().unwrap();
        assert!(pool.iter().any(|p| p.tool == primary.tool));
        assert_eq!(plan.alternatives.len(), pool.len() - 1);
        assert!(!plan.alternatives.iter().any(|a| a.tool == primary.tool));

        // Charged: everything doable, sorted by progress asc, no countermeasure.
        t.set_charge(Charge::Charged);
        let plan = t.today(&s);
        assert_eq!(plan.doable.len(), 3);
        assert_eq!(plan.doable[0].id, "rest"); // progress 0.0 first
        assert!(plan.deferred.is_empty());
        assert!(plan.countermeasure.is_none());
    }

    #[test]
    fn test_charge_ordering() {
        assert!(Charge::Charged.can_sustain(Charge::Steady));
        assert!(!Charge::Depleted.can_sustain(Charge::Steady));
        assert!(Charge::Steady.can_sustain(Charge::Steady));
        assert!(Charge::Charged.availability() > Charge::Depleted.availability());
    }

    #[test]
    fn test_alchemical_phase() {
        let mut t = LifeOSTracker::new();
        let day = 86_400i64;
        let now = chrono::Utc::now().timestamp();
        t.history.insert("rise".into(), vec![(now - 3 * day, 0.1), (now - 2 * day, 0.3), (now - day, 0.5), (now, 0.7)]);
        assert_eq!(t.phase_of("rise"), AlchemicalPhase::Coagulating);
        t.history.insert("fall".into(), vec![(now - 3 * day, 0.7), (now - 2 * day, 0.5), (now - day, 0.4), (now, 0.2)]);
        assert_eq!(t.phase_of("fall"), AlchemicalPhase::Solutio);
        t.history.insert("stuck".into(), vec![(now - 10 * day, 0.15), (now - 5 * day, 0.16), (now, 0.15)]);
        assert_eq!(t.phase_of("stuck"), AlchemicalPhase::Solutio);
        t.history.insert("work".into(), vec![(now - 2 * day, 0.50), (now - day, 0.52), (now, 0.50)]);
        assert_eq!(t.phase_of("work"), AlchemicalPhase::Working);
        assert_eq!(t.phase_of("new"), AlchemicalPhase::Nascent);
    }

    #[test]
    fn test_record_progress_dedup() {
        let mut t = LifeOSTracker::new();
        t.record_progress("g", 0.2);
        t.record_progress("g", 0.2); // no-change → not recorded
        t.record_progress("g", 0.5);
        assert_eq!(t.history.get("g").unwrap().len(), 2);
    }
}
