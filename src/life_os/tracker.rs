//! LifeOS — native goal/habit tracking layer.
//!
//! Self-contained: physis OWNS the goal state rather than syncing it from an
//! external Praxis/Agora service over HTTP every tick. Callers push goals in
//! (`POST /api/v1/lifeos/goal`) and poll status/drift out
//! (`GET /api/v1/lifeos/status`). No outbound HTTP, no service URLs to configure.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

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

/// A charge-governed "today" plan: what to do given the user's current charge.
#[derive(Serialize, Clone, Debug)]
pub struct TodayPlan {
    pub charge: Charge,
    /// Incomplete goals the current charge can sustain, lowest-progress first.
    pub doable: Vec<LifeOSGoal>,
    /// Incomplete goals that need more charge than is available right now.
    pub deferred: Vec<LifeOSGoal>,
    /// A bioenergetic countermeasure when charge is low or the schedule is drifting.
    pub countermeasure: Option<String>,
    pub drift: DriftReport,
}

/// Native in-memory goal store. No HTTP client, no external dependency.
#[derive(Default)]
pub struct LifeOSTracker {
    pub goals: Vec<LifeOSGoal>,
    /// Current bioenergetic charge (pushed in by Praxis from check-ins).
    pub charge: Charge,
}

impl LifeOSTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a goal by `id`. Returns true if it replaced an existing one.
    pub fn upsert_goal(&mut self, goal: LifeOSGoal) -> bool {
        if let Some(existing) = self.goals.iter_mut().find(|g| g.id == goal.id) {
            *existing = goal;
            true
        } else {
            self.goals.push(goal);
            false
        }
    }

    /// Set a goal's progress (0.0–1.0). Returns false if unknown.
    pub fn update_progress(&mut self, id: &str, progress: f32) -> bool {
        match self.goals.iter_mut().find(|g| g.id == id) {
            Some(g) => {
                g.progress = progress.clamp(0.0, 1.0);
                true
            }
            None => false,
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
        let countermeasure = match self.charge {
            Charge::Depleted => Some(
                "Charge depleted — restore before pushing. Diaphragmatic breathing + warmth/light \
                 (Peat), release jaw & chest armoring (Reich). Take the gentle wins; don't force the \
                 critical path.".to_string(),
            ),
            Charge::Steady if drift.drifting => Some(
                "Steady but the schedule is slipping — one focused 25-min block on the critical-path \
                 goal, then a short reset.".to_string(),
            ),
            _ => None,
        };

        TodayPlan { charge: self.charge, doable, deferred, countermeasure, drift }
    }

    /// Persist this tenant's goals + charge to sled under `lifeos:<uid>`.
    pub fn save(&self, db: &sled::Db, uid: &str) -> anyhow::Result<()> {
        let snapshot = (&self.goals, self.charge);
        db.insert(format!("lifeos:{uid}"), serde_json::to_vec(&snapshot)?)?;
        Ok(())
    }

    /// Load a tenant's state from sled (empty tracker if absent/unreadable).
    /// Tolerates the old goals-only format (no charge).
    pub fn load(db: &sled::Db, uid: &str) -> Self {
        let raw = db.get(format!("lifeos:{uid}")).ok().flatten();
        let Some(v) = raw else { return Self::default() };
        if let Ok((goals, charge)) = serde_json::from_slice::<(Vec<LifeOSGoal>, Charge)>(&v) {
            Self { goals, charge }
        } else if let Ok(goals) = serde_json::from_slice::<Vec<LifeOSGoal>>(&v) {
            Self { goals, charge: Charge::default() }
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
}
