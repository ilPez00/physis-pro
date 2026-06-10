use serde::{Serialize, Deserialize};
use std::collections::{HashMap, VecDeque};

use crate::graph::RawNodeKey;

/// Pluggable scheduling strategy — domain-specific duration estimation,
/// dependency semantics, and priority computation.
pub trait ScheduleDomain: Send + Sync {
    fn estimate_duration(&self, activation_energy: f32, decay_metrics: f32) -> u64;
    fn resolve_dependencies(&self, edges: &[crate::graph::Edge]) -> Vec<TaskDependency>;
    fn compute_priority(&self, task: &ScheduledTask) -> f32;
    fn domain_name(&self) -> &'static str;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub id: String,
    pub name: String,
    pub capacity: f32,
    pub allocated_to: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDependency {
    pub predecessor: String,
    pub successor: String,
    pub lag: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: String,
    pub node_id: RawNodeKey,
    pub name: String,
    pub planned_duration_secs: u64,
    pub actual_duration_secs: u64,
    pub early_start: u64,
    pub early_finish: u64,
    pub late_start: u64,
    pub late_finish: u64,
    pub float: i64,
    pub progress: f32,
    pub dependencies: Vec<RawNodeKey>,
    pub deadline_unix: u64,
    pub resource_allocations: Vec<String>,
    pub on_critical_path: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalPathInfo {
    pub tasks: Vec<String>,
    pub total_duration: u64,
    pub total_float: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GanttDreamProposal {
    pub description: String,
    pub task_id: String,
    pub alternative_start: u64,
    pub new_float: i64,
    pub expected_impact: f32,
    pub kind: GanttDreamKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GanttDreamKind {
    Reorder { swap_with: String },
    ResourceBoost { additional_capacity: f32 },
    Descope { target_progress: f32 },
    Parallelize { concurrent_tasks: Vec<String> },
}

pub struct GanttScheduler {
    pub tasks: Vec<ScheduledTask>,
    pub resources: Vec<Resource>,
    pub domain: Box<dyn ScheduleDomain>,
    pub critical_path: Option<CriticalPathInfo>,
    dependencies: Vec<TaskDependency>,
}

pub struct AbstractDomain;
impl ScheduleDomain for AbstractDomain {
    fn estimate_duration(&self, activation_energy: f32, decay_metrics: f32) -> u64 {
        let base = (1.0 - activation_energy) * 3600.0;
        let decay_penalty = decay_metrics * 1800.0;
        (base + decay_penalty).max(60.0) as u64
    }

    fn resolve_dependencies(&self, edges: &[crate::graph::Edge]) -> Vec<TaskDependency> {
        edges.iter().map(|e| TaskDependency {
            predecessor: format!("task_{}", e.source.data),
            successor: format!("task_{}", e.target.data),
            lag: 0,
        }).collect()
    }

    fn compute_priority(&self, task: &ScheduledTask) -> f32 {
        1.0 - (task.float as f32 / task.early_start.max(1) as f32).clamp(0.0, 1.0)
    }

    fn domain_name(&self) -> &'static str { "abstract" }
}

pub struct OeeDomain;
impl ScheduleDomain for OeeDomain {
    fn estimate_duration(&self, activation_energy: f32, _decay_metrics: f32) -> u64 {
        let availability = activation_energy;
        let cycle_time_secs = 60.0;
        let units = 100.0;
        let ideal = cycle_time_secs * units;
        (ideal / availability.max(0.01)) as u64
    }

    fn resolve_dependencies(&self, edges: &[crate::graph::Edge]) -> Vec<TaskDependency> {
        edges.iter().filter_map(|e| {
            let predecessor = format!("task_{}", e.source.data);
            let successor = format!("task_{}", e.target.data);
            if predecessor != successor {
                Some(TaskDependency { predecessor, successor, lag: 0 })
            } else {
                None
            }
        }).collect()
    }

    fn compute_priority(&self, task: &ScheduledTask) -> f32 {
        let duration_ratio = task.actual_duration_secs as f32 / task.planned_duration_secs.max(1) as f32;
        let float_ratio = task.float as f32 / task.early_start.max(1) as f32;
        (duration_ratio + (1.0 - float_ratio.clamp(0.0, 1.0))) / 2.0
    }

    fn domain_name(&self) -> &'static str { "oee" }
}

impl GanttScheduler {
    pub fn new(domain: Box<dyn ScheduleDomain>) -> Self {
        Self {
            tasks: Vec::new(),
            resources: Vec::new(),
            domain,
            critical_path: None,
            dependencies: Vec::new(),
        }
    }

    pub fn add_task(
        &mut self,
        node_id: RawNodeKey,
        name: String,
        activation_energy: f32,
        decay_metrics: f32,
        deadline_unix: u64,
        dependencies: Vec<RawNodeKey>,
    ) -> String {
        let id = format!("task_{}", node_id.data);
        let duration = self.domain.estimate_duration(activation_energy, decay_metrics);

        self.tasks.push(ScheduledTask {
            id: id.clone(),
            node_id,
            name,
            planned_duration_secs: duration,
            actual_duration_secs: duration,
            early_start: 0,
            early_finish: duration,
            late_start: 0,
            late_finish: duration,
            float: 0,
            progress: 0.0,
            dependencies,
            deadline_unix,
            resource_allocations: Vec::new(),
            on_critical_path: false,
        });

        id
    }

    pub fn has_task(&self, task_id: &str) -> bool {
        self.tasks.iter().any(|t| t.id == task_id)
    }

    pub fn add_resource(&mut self, id: String, name: String, capacity: f32) {
        self.resources.push(Resource {
            id,
            name,
            capacity,
            allocated_to: Vec::new(),
        });
    }

    pub fn allocate_resource(&mut self, task_id: &str, resource_id: &str) -> bool {
        let resource = match self.resources.iter_mut().find(|r| r.id == resource_id) {
            Some(r) => r,
            None => return false,
        };
        if resource.allocated_to.len() as f32 >= resource.capacity {
            return false;
        }
        resource.allocated_to.push(task_id.to_string());
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            task.resource_allocations.push(resource_id.to_string());
            true
        } else {
            false
        }
    }

    pub fn update_task_progress(&mut self, task_id: &str, progress: f32) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            task.progress = progress.clamp(0.0, 1.0);
            task.actual_duration_secs = (task.planned_duration_secs as f32 * progress) as u64;
        }
    }

    fn recompute_cpm(&mut self) {
        if self.tasks.is_empty() {
            self.critical_path = None;
            return;
        }

        let task_map: HashMap<String, usize> = self.tasks.iter()
            .enumerate()
            .map(|(i, t)| (t.id.clone(), i))
            .collect();

        let mut adj: HashMap<usize, Vec<(usize, u64)>> = HashMap::new();
        let mut in_degree = vec![0usize; self.tasks.len()];

        for dep in &self.dependencies {
            if let (Some(&pred), Some(&succ)) = (task_map.get(&dep.predecessor), task_map.get(&dep.successor)) {
                adj.entry(pred).or_default().push((succ, dep.lag));
                in_degree[succ] += 1;
            }
        }

        let mut topo = Vec::new();
        let mut queue: VecDeque<usize> = in_degree.iter()
            .enumerate()
            .filter(|(_, &deg)| deg == 0)
            .map(|(i, _)| i)
            .collect();

        while let Some(idx) = queue.pop_front() {
            topo.push(idx);
            if let Some(neighbors) = adj.get(&idx) {
                for &(next, _) in neighbors {
                    in_degree[next] = in_degree[next].saturating_sub(1);
                    if in_degree[next] == 0 {
                        queue.push_back(next);
                    }
                }
            }
        }

        if topo.len() != self.tasks.len() {
            log::warn!("Gantt: cycle detected in task dependencies");
            return;
        }

        for &idx in &topo {
            let mut es = 0u64;
            let deps: Vec<usize> = self.dependencies.iter()
                .filter(|d| task_map.get(&d.successor) == Some(&idx))
                .filter_map(|d| task_map.get(&d.predecessor).copied())
                .collect();
            for &pred in &deps {
                es = es.max(self.tasks[pred].early_finish);
            }
            self.tasks[idx].early_start = es;
            self.tasks[idx].early_finish = es + self.tasks[idx].planned_duration_secs;
        }

        let project_end = self.tasks.iter().map(|t| t.early_finish).max().unwrap_or(0);

        for &idx in topo.iter().rev() {
            let successors: Vec<usize> = self.dependencies.iter()
                .filter(|d| task_map.get(&d.predecessor) == Some(&idx))
                .filter_map(|d| task_map.get(&d.successor).copied())
                .collect();

            let lf = if successors.is_empty() {
                project_end
            } else {
                successors.iter().map(|&s| self.tasks[s].late_start).min().unwrap_or(project_end)
            };
            let ls = lf.saturating_sub(self.tasks[idx].planned_duration_secs);
            self.tasks[idx].late_start = ls;
            self.tasks[idx].late_finish = lf;
            self.tasks[idx].float = ls as i64 - self.tasks[idx].early_start as i64;
            self.tasks[idx].on_critical_path = self.tasks[idx].float == 0;
        }

        let critical_tasks: Vec<String> = self.tasks.iter()
            .filter(|t| t.on_critical_path)
            .map(|t| t.id.clone())
            .collect();

        self.critical_path = Some(CriticalPathInfo {
            tasks: critical_tasks,
            total_duration: project_end,
            total_float: 0,
        });
    }

    pub fn recompute(&mut self) {
        let mut deps = Vec::new();

        for i in 0..self.tasks.len() {
            let task_id = self.tasks[i].id.clone();
            let dep_nodes: Vec<RawNodeKey> = self.tasks[i].dependencies.clone();
            for dep_node in &dep_nodes {
                let dep_id = format!("task_{}", dep_node.data);
                deps.push(TaskDependency {
                    predecessor: dep_id,
                    successor: task_id.clone(),
                    lag: 0,
                });
            }
            self.tasks[i].planned_duration_secs = self.domain.estimate_duration(
                self.tasks[i].progress,
                (self.tasks[i].actual_duration_secs as f32 / self.tasks[i].planned_duration_secs.max(1) as f32).clamp(0.0, 1.0),
            );
        }

        let domain_deps = self.domain.resolve_dependencies(&[]);
        deps.extend(domain_deps);
        self.dependencies = deps;
        self.recompute_cpm();
    }

    pub fn get_critical_path(&self) -> Option<&CriticalPathInfo> {
        self.critical_path.as_ref()
    }

    pub fn get_task(&self, task_id: &str) -> Option<&ScheduledTask> {
        self.tasks.iter().find(|t| t.id == task_id)
    }

    pub fn get_task_mut(&mut self, task_id: &str) -> Option<&mut ScheduledTask> {
        self.tasks.iter_mut().find(|t| t.id == task_id)
    }

    pub fn tasks_approaching_late_finish(&self, threshold_float: i64) -> Vec<&ScheduledTask> {
        self.tasks.iter()
            .filter(|t| t.float <= threshold_float && t.progress < 1.0)
            .collect()
    }

    pub fn schedule_health(&self) -> f32 {
        let at_risk = self.tasks_approaching_late_finish(0);
        if self.tasks.is_empty() {
            return 1.0;
        }
        1.0 - (at_risk.len() as f32 / self.tasks.len() as f32)
    }

    pub fn dream_proposals(&self) -> Vec<GanttDreamProposal> {
        let mut proposals = Vec::new();
        for task in self.tasks_approaching_late_finish(0) {
            if let Some(swap) = self.tasks.iter()
                .filter(|t| t.id != task.id && t.float > 0 && t.progress < 0.5)
                .max_by_key(|t| t.float)
            {
                proposals.push(GanttDreamProposal {
                    description: format!("Reorder {} (float={}) with {} (float={})",
                        task.name, task.float, swap.name, swap.float),
                    task_id: task.id.clone(),
                    alternative_start: swap.early_start,
                    new_float: task.float + swap.float / 2,
                    expected_impact: 0.3,
                    kind: GanttDreamKind::Reorder { swap_with: swap.id.clone() },
                });
            }

            if task.progress < 0.3 {
                proposals.push(GanttDreamProposal {
                    description: format!("Boost resources for {} to halve remaining duration", task.name),
                    task_id: task.id.clone(),
                    alternative_start: task.early_start,
                    new_float: task.float + (task.planned_duration_secs as i64 / 2),
                    expected_impact: 0.5,
                    kind: GanttDreamKind::ResourceBoost { additional_capacity: 1.0 },
                });
            }

            let concurrent: Vec<String> = self.tasks.iter()
                .filter(|t| t.id != task.id && t.float > task.float && !t.on_critical_path)
                .map(|t| t.id.clone())
                .take(2)
                .collect();

            if !concurrent.is_empty() {
                proposals.push(GanttDreamProposal {
                    description: format!("Parallelize {} with {} concurrent tasks", task.name, concurrent.len()),
                    task_id: task.id.clone(),
                    alternative_start: task.early_start,
                    new_float: task.float + 3600,
                    expected_impact: 0.4,
                    kind: GanttDreamKind::Parallelize { concurrent_tasks: concurrent },
                });
            }
        }
        proposals
    }

    pub fn cross_correlate(&self, goal_progress: &[(String, f32)]) -> f32 {
        if goal_progress.is_empty() || self.tasks.is_empty() {
            return 0.0;
        }

        let task_map: HashMap<String, f32> = self.tasks.iter()
            .map(|t| (t.id.clone(), t.progress))
            .collect();

        let mut paired = Vec::new();
        for (goal_id, gp) in goal_progress {
            if let Some(&tp) = task_map.get(goal_id) {
                paired.push((*gp, tp));
            }
        }

        if paired.len() < 2 {
            return paired.first().map(|(a, b)| {
                if a + b > 0.0 { 1.0 - (a - b).abs() / (a + b) } else { 0.0 }
            }).unwrap_or(0.0);
        }

        let n = paired.len() as f32;
        let sum_x: f32 = paired.iter().map(|(x, _)| x).sum();
        let sum_y: f32 = paired.iter().map(|(_, y)| y).sum();
        let sum_xy: f32 = paired.iter().map(|(x, y)| x * y).sum();
        let sum_x2: f32 = paired.iter().map(|(x, _)| x * x).sum();
        let sum_y2: f32 = paired.iter().map(|(_, y)| y * y).sum();

        let num = n * sum_xy - sum_x * sum_y;
        let denom = ((n * sum_x2 - sum_x * sum_x) * (n * sum_y2 - sum_y * sum_y)).sqrt();
        if denom == 0.0 { 0.0 } else { (num / denom).clamp(-1.0, 1.0) }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GanttTask {
    pub id: String,
    pub node_id: RawNodeKey,
    pub name: String,
    pub start_time: u64,
    pub end_time: u64,
    pub progress: f32,
    pub dependencies: Vec<RawNodeKey>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Trajectory {
    pub tasks: Vec<GanttTask>,
    pub coherence_index: f32,
    pub critical_path: Option<CriticalPathInfo>,
    pub schedule_health: f32,
    pub cross_correlation: f32,
}

pub fn compute_trajectory(
    scheduler: &GanttScheduler,
    goal_progress: &[(String, f32)],
) -> Trajectory {
    let tasks: Vec<GanttTask> = scheduler.tasks.iter().map(|st| {
        let deps = st.dependencies.clone();
        GanttTask {
            id: st.id.clone(),
            node_id: st.node_id,
            name: st.name.clone(),
            start_time: st.early_start,
            end_time: st.early_finish,
            progress: st.progress,
            dependencies: deps,
        }
    }).collect();

    let coherence = if tasks.is_empty() { 1.0 } else { scheduler.schedule_health() };

    Trajectory {
        tasks,
        coherence_index: coherence,
        critical_path: scheduler.critical_path.clone(),
        schedule_health: scheduler.schedule_health(),
        cross_correlation: scheduler.cross_correlate(goal_progress),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_scheduler() -> GanttScheduler {
        let mut s = GanttScheduler::new(Box::new(AbstractDomain));
        s.add_task(RawNodeKey { data: 1 }, "Task A".into(), 0.5, 0.0, 10000, vec![]);
        s.add_task(RawNodeKey { data: 2 }, "Task B".into(), 0.3, 0.0, 10000,
            vec![RawNodeKey { data: 1 }]);
        s.add_task(RawNodeKey { data: 3 }, "Task C".into(), 0.7, 0.0, 10000,
            vec![RawNodeKey { data: 2 }]);
        s.recompute();
        s
    }

    #[test]
    fn test_cpm_forward_pass() {
        let s = make_scheduler();
        let a = s.get_task("task_1").unwrap();
        let b = s.get_task("task_2").unwrap();
        let c = s.get_task("task_3").unwrap();

        assert_eq!(a.early_start, 0);
        assert!(b.early_start >= a.early_finish);
        assert!(c.early_start >= b.early_finish);
        assert!(c.early_finish > 0);
    }

    #[test]
    fn test_critical_path_detection() {
        let s = make_scheduler();
        let cp = s.get_critical_path().unwrap();
        assert!(!cp.tasks.is_empty());
    }

    #[test]
    fn test_add_and_allocate_resource() {
        let mut s = GanttScheduler::new(Box::new(AbstractDomain));
        let id = s.add_task(RawNodeKey { data: 1 }, "Test".into(), 0.5, 0.0, 10000, vec![]);
        s.add_resource("r1".into(), "Worker".into(), 2.0);
        assert!(s.allocate_resource(&id, "r1"));
    }

    #[test]
    fn test_update_progress() {
        let mut s = make_scheduler();
        s.update_task_progress("task_1", 0.5);
        assert!((s.get_task("task_1").unwrap().progress - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_schedule_health() {
        let s = make_scheduler();
        let health = s.schedule_health();
        assert!((0.0..=1.0).contains(&health));
    }

    #[test]
    fn test_dream_proposals() {
        let mut s = make_scheduler();
        s.update_task_progress("task_1", 0.1);
        s.recompute();
        let proposals = s.dream_proposals();
        assert!(!proposals.is_empty());
    }

    #[test]
    fn test_cross_correlation() {
        let s = make_scheduler();
        let goals = vec![
            ("task_1".into(), 0.5),
            ("task_2".into(), 0.3),
            ("task_3".into(), 0.8),
        ];
        let corr = s.cross_correlate(&goals);
        assert!((-1.0..=1.0).contains(&corr));
    }

    #[test]
    fn test_empty_scheduler() {
        let s = GanttScheduler::new(Box::new(AbstractDomain));
        assert!(s.get_critical_path().is_none());
        assert!((s.schedule_health() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_trajectory_backward_compat() {
        let s = make_scheduler();
        let traj = compute_trajectory(&s, &[]);
        assert!(!traj.tasks.is_empty());
        assert!(traj.critical_path.is_some());
    }

    #[test]
    fn test_oee_domain() {
        let domain = OeeDomain;
        let dur = domain.estimate_duration(0.8, 0.0);
        assert!(dur > 0);
        let low = domain.estimate_duration(0.2, 0.0);
        assert!(low > dur);
    }
}
