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

/// A tracked life goal/habit. `coherence_target` is the desired steady-state
/// coherence (informational for now); `progress` is 0.0–1.0.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LifeOSGoal {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub coherence_target: f32,
    #[serde(default)]
    pub progress: f32,
}

/// Schedule-drift snapshot clients poll (replaces the old outbound push).
#[derive(Serialize, Clone, Debug)]
pub struct DriftReport {
    pub schedule_health: f32,
    pub at_risk_tasks: Vec<String>,
    /// True when the schedule is unhealthy enough to warrant a nudge.
    pub drifting: bool,
}

/// Native in-memory goal store. No HTTP client, no external dependency.
#[derive(Default)]
pub struct LifeOSTracker {
    pub goals: Vec<LifeOSGoal>,
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
        LifeOSGoal { id: id.into(), title: format!("Goal {id}"), coherence_target: 1.0, progress }
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
}
