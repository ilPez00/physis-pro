//! PDCA (Plan-Do-Check-Act) cycle orchestrator for driving goals through
//! iterative state transitions. Tracks experiences, detects stagnation,
//! and produces plans with measurable progress.

use serde::Serialize;

use crate::models::{cosine_dist, cosine_sim, Experience, Goal, Score};

/// Aggregate statistics from the PDCA cycle.
#[derive(Debug, Clone, Serialize)]
pub struct PDCAStats {
    /// Number of actions executed across all goals.
    pub total_actions: usize,
    /// Number of goals being tracked.
    pub total_goals: usize,
    /// Average grade across all experiences.
    pub avg_grade: f64,
    /// Mean progress across all goals (0.0–1.0).
    pub mean_progress: f64,
    /// Number of goals that have stalled below the progress threshold.
    pub stagnant_count: usize,
}

/// Drives the Plan-Do-Check-Act cycle over a set of vector-space goals.
///
/// Each action produces an Experience (before/after + delta), stagnation is
/// detected when recent progress stays below `config_stagnant_threshold`.
#[derive(Debug)]
pub struct PDCActor {
    /// History of all experiences across all goals.
    pub experiences: Vec<Experience>,
    /// Progress threshold below which a goal is considered stagnant.
    pub config_stagnant_threshold: f32,
    /// Number of recent actions to evaluate for stagnation.
    pub config_stagnant_window: usize,
}

impl PDCActor {
    /// Create a new PDCActor with stagnation detection parameters.
    ///
    /// `stagnant_threshold` — progress must stay under this fraction to flag.
    /// `stagnant_window` — number of recent actions to inspect.
    pub fn new(stagnant_threshold: f32, stagnant_window: usize) -> Self {
        Self {
            experiences: Vec::new(),
            config_stagnant_threshold: stagnant_threshold,
            config_stagnant_window: stagnant_window,
        }
    }

    /// Plan — return goals sorted by priority (lowest progress first, then stagnant).
    pub fn plan<'a>(&self, goals: &'a [Goal]) -> Vec<&'a Goal> {
        let mut sorted: Vec<&Goal> = goals.iter().collect();
        sorted.sort_by(|a, b| {
            a.progress.partial_cmp(&b.progress).unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.truncate(3);
        sorted
    }

    /// Do — record a state transition for a goal, returning the new Experience.
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

    /// Check — assign a grade to an experience by ID. Returns false if not found.
    pub fn check(&mut self, experience_id: &str, grade: Score) -> bool {
        if let Some(exp) = self.experiences.iter_mut().find(|e| e.id == experience_id) {
            exp.grade = grade;
            true
        } else {
            false
        }
    }

    /// Act — update goal progress based on recent experience grades and vector deltas.
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

            // Progress also measured by cosine distance to latest experience
            if let Some(latest) = relevant.last() {
                let sim = cosine_sim(&goal.embedding, &latest.after);
                let vec_progress = 1.0 - cosine_dist(&goal.embedding, &latest.after);
                goal.progress = goal.progress.max(vec_progress.clamp(0.0, 1.0));
                adjustments.push(format!("{:.4}", sim));
            }
        }

        adjustments
    }

    /// Aggregate PDCA statistics: action count, average grade, progress, stagnation.
    pub fn stats(&self, goals: &[Goal]) -> PDCAStats {
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

        PDCAStats {
            total_actions: self.experiences.len(),
            total_goals: goals.len(),
            avg_grade: avg_grade as f64,
            mean_progress: mean_progress as f64,
            stagnant_count: stagnant.len(),
        }
    }

    fn detect_stagnant(&self, goals: &[Goal]) -> Vec<String> {
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

    /// Returns true when at least one goal is both incomplete and not stagnant.
    pub fn is_working(&self, goals: &[Goal]) -> bool {
        let stagnant = self.detect_stagnant(goals);
        goals.iter().any(|g| g.progress < 1.0 && !stagnant.contains(&g.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(!actor.is_working(&[goal])); // stagnant = not working
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
