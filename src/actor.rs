use std::collections::HashMap;

use serde::Serialize;

use crate::gantt::GanttScheduler;
use crate::models::{cosine_dist, cosine_sim, Experience, Goal, Score};

#[derive(Debug, Clone, Serialize)]
pub struct PDCAStats {
    pub total_actions: usize,
    pub total_goals: usize,
    pub avg_grade: f64,
    pub mean_progress: f64,
    pub stagnant_count: usize,
    pub schedule_correlation: f32,
    pub at_risk_tasks: Vec<String>,
}

#[derive(Debug)]
pub struct PDCActor {
    pub experiences: Vec<Experience>,
    pub config_stagnant_threshold: f32,
    pub config_stagnant_window: usize,
}

impl PDCActor {
    pub fn new(stagnant_threshold: f32, stagnant_window: usize) -> Self {
        Self {
            experiences: Vec::new(),
            config_stagnant_threshold: stagnant_threshold,
            config_stagnant_window: stagnant_window,
        }
    }

    pub fn plan<'a>(&self, goals: &'a [Goal]) -> Vec<&'a Goal> {
        let mut sorted: Vec<&Goal> = goals.iter().collect();
        sorted.sort_by(|a, b| {
            a.progress.partial_cmp(&b.progress).unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.truncate(3);
        sorted
    }

    pub fn plan_with_schedule<'a>(
        &self,
        goals: &'a [Goal],
        schedule_priorities: Option<&HashMap<String, f32>>,
    ) -> Vec<&'a Goal> {
        let priorities = match schedule_priorities {
            Some(p) => p,
            None => return self.plan(goals),
        };

        let mut scored: Vec<(&Goal, f32)> = goals.iter().map(|g| {
            let schedule_priority = priorities.get(&g.id).copied().unwrap_or(0.0);
            let progress_score = 1.0 - g.progress;
            (g, progress_score + schedule_priority)
        }).collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(3);
        scored.into_iter().map(|(g, _)| g).collect()
    }

    pub fn do_action(&mut self, goal_id: &str, before: Vec<f32>, after: Vec<f32>) -> Experience {
        let exp = Experience::new(
            goal_id,
            before,
            after,
            "action".to_string(),
            "rationale".to_string(),
            std::collections::HashMap::new(),
        );
        self.experiences.push(exp.clone());
        exp
    }

    pub fn check(&mut self, experience_id: &str, grade: Score) -> bool {
        if let Some(exp) = self.experiences.iter_mut().find(|e| e.id == experience_id) {
            exp.grade = grade;
            true
        } else {
            false
        }
    }

    pub fn act(&self, experiences: &[Experience], goals: &mut [Goal]) -> Vec<String> {
        let mut adjustments = Vec::new();

        for goal in goals.iter_mut() {
            let relevant: Vec<&Experience> = experiences
                .iter()
                .filter(|e| e.goal_id == goal.id)
                .collect();

            if relevant.is_empty() {
                continue;
            }

            let avg_grade: Score = relevant.iter().map(|e| e.grade).sum::<Score>() / relevant.len() as Score;
            goal.progress = (goal.progress + avg_grade * 0.1).clamp(0.0, 1.0);

            if let Some(latest) = relevant.last() {
                let sim = cosine_sim(&goal.embedding, &latest.after);
                let vec_progress = 1.0 - cosine_dist(&goal.embedding, &latest.after);
                goal.progress = goal.progress.max(vec_progress.clamp(0.0, 1.0));
                adjustments.push(format!("{:.4}", sim));
            }
        }

        adjustments
    }

    pub fn act_with_schedule(
        &self,
        experiences: &[Experience],
        goals: &mut [Goal],
        scheduler_updates: Option<&mut GanttScheduler>,
    ) -> Vec<String> {
        let adjustments = self.act(experiences, goals);

        if let Some(sched) = scheduler_updates {
            for goal in goals.iter() {
                sched.update_task_progress(&goal.id, goal.progress);
            }
            sched.recompute();
        }

        adjustments
    }

    pub fn stats(&self, goals: &[Goal]) -> PDCAStats {
        self.stats_with_schedule(goals, None)
    }

    pub fn stats_with_schedule(
        &self,
        goals: &[Goal],
        scheduler: Option<&GanttScheduler>,
    ) -> PDCAStats {
        let grades: Vec<Score> = self.experiences.iter().map(|e| e.grade).collect();
        let avg_grade = if grades.is_empty() {
            0.0
        } else {
            grades.iter().sum::<Score>() / grades.len() as Score
        };

        let mean_progress = if goals.is_empty() {
            0.0
        } else {
            goals.iter().map(|g| g.progress).sum::<Score>() / goals.len() as Score
        };

        let stagnant = self.detect_stagnant(goals);
        let schedule_correlation = scheduler.map_or(0.0, |s| {
            let goal_progress: Vec<(String, f32)> = goals.iter()
                .map(|g| (g.id.clone(), g.progress))
                .collect();
            s.cross_correlate(&goal_progress)
        });
        let at_risk_tasks = scheduler.map_or(Vec::new(), |s| {
            s.tasks_approaching_late_finish(0)
                .iter()
                .map(|t| t.id.clone())
                .collect()
        });

        PDCAStats {
            total_actions: self.experiences.len(),
            total_goals: goals.len(),
            avg_grade: avg_grade as f64,
            mean_progress: mean_progress as f64,
            stagnant_count: stagnant.len(),
            schedule_correlation,
            at_risk_tasks,
        }
    }

    pub fn detect_stagnant(&self, goals: &[Goal]) -> Vec<String> {
        let mut stagnant = Vec::new();
        let window = self.config_stagnant_window;
        let threshold = self.config_stagnant_threshold;

        for goal in goals {
            let goal_exps: Vec<&Experience> = self.experiences
                .iter()
                .filter(|e| e.goal_id == goal.id)
                .collect();

            if goal_exps.len() < window {
                continue;
            }

            let recent: Vec<Score> = goal_exps.iter()
                .rev()
                .take(window)
                .map(|e| e.grade)
                .collect();

            if recent.iter().all(|g| *g < threshold) {
                stagnant.push(goal.id.clone());
            }
        }

        stagnant
    }

    pub fn detect_stagnant_with_schedule(
        &self,
        goals: &[Goal],
        scheduler: Option<&GanttScheduler>,
    ) -> Vec<String> {
        let mut stagnant = self.detect_stagnant(goals);

        if let Some(sched) = scheduler {
            for task in sched.tasks_approaching_late_finish(0) {
                stagnant.push(task.id.clone());
            }
        }

        stagnant
    }

    pub fn is_working(&self, goals: &[Goal]) -> bool {
        let stagnant = self.detect_stagnant(goals);
        goals.iter().any(|g| g.progress < 1.0 && !stagnant.contains(&g.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gantt::{AbstractDomain, GanttScheduler};

    fn test_goal(progress: f32) -> Goal {
        let embedding = vec![0.1, 0.2, 0.3, 0.4];
        let mut g = Goal::new_vec(embedding);
        g.progress = progress;
        g
    }

    #[test]
    fn test_pdca_cycle() {
        let mut actor = PDCActor::new(0.2, 3);
        let mut goals = vec![test_goal(0.0), test_goal(0.0)];

        let planned = actor.plan(&goals);
        assert_eq!(planned.len(), 2);

        let exp = actor.do_action(&goals[0].id, vec![0.0, 0.0, 0.0, 0.0], vec![0.2, 0.3, 0.4, 0.5]);
        assert!(actor.check(&exp.id, 0.8));

        let adjustments = actor.act(&actor.experiences, &mut goals);
        assert!(!adjustments.is_empty());
    }

    #[test]
    fn test_plan_with_schedule_no_scheduler() {
        let actor = PDCActor::new(0.2, 3);
        let goals = vec![test_goal(0.0), test_goal(0.5)];
        let planned = actor.plan_with_schedule(&goals, None);
        assert_eq!(planned.len(), 2);
    }

    #[test]
    fn test_act_with_schedule_updates_scheduler() {
        let mut actor = PDCActor::new(0.2, 3);
        let mut goals = vec![test_goal(0.0)];
        let mut scheduler = GanttScheduler::new(Box::new(AbstractDomain));

        let exp = actor.do_action(&goals[0].id, vec![0.0; 4], vec![0.2, 0.3, 0.4, 0.5]);
        actor.check(&exp.id, 0.8);

        let _ = actor.act_with_schedule(&actor.experiences, &mut goals, Some(&mut scheduler));
    }

    #[test]
    fn test_stats_with_schedule() {
        let actor = PDCActor::new(0.2, 3);
        let goals = vec![test_goal(0.3)];
        let scheduler = GanttScheduler::new(Box::new(AbstractDomain));

        let stats = actor.stats_with_schedule(&goals, Some(&scheduler));
        assert_eq!(stats.total_goals, 1);
    }

    #[test]
    fn test_is_working_with_incomplete_goal() {
        let actor = PDCActor::new(0.3, 3);
        let goals = vec![test_goal(0.0)];
        assert!(actor.is_working(&goals));
    }

    #[test]
    fn test_is_working_all_complete() {
        let actor = PDCActor::new(0.3, 3);
        let goal = test_goal(1.0);
        assert!(!actor.is_working(&[goal]));
    }

    #[test]
    fn test_is_working_stagnant_goal() {
        let mut actor = PDCActor::new(0.3, 3);
        let goal = test_goal(0.5);
        for _ in 0..5 {
            let exp = actor.do_action(&goal.id, vec![0.0, 0.0, 0.0, 0.0], vec![0.1, 0.1, 0.1, 0.1]);
            actor.check(&exp.id, 0.1);
        }
        assert!(!actor.is_working(&[goal]));
    }

    #[test]
    fn test_is_working_no_goals() {
        let actor = PDCActor::new(0.3, 3);
        assert!(!actor.is_working(&[]));
    }

    #[test]
    fn test_is_working_mixed() {
        let actor = PDCActor::new(0.3, 3);
        let done = test_goal(1.0);
        let active = test_goal(0.5);
        assert!(actor.is_working(&[done, active]));
    }

    #[test]
    fn test_stagnant_detection() {
        let mut actor = PDCActor::new(0.3, 3);
        let goal = test_goal(0.5);

        for _ in 0..5 {
            let exp = actor.do_action(&goal.id, vec![0.0, 0.0, 0.0, 0.0], vec![0.1, 0.1, 0.1, 0.1]);
            actor.check(&exp.id, 0.1);
        }

        let goals = vec![goal];
        let stagnant = actor.detect_stagnant(&goals);
        assert_eq!(stagnant.len(), 1);
    }
}
