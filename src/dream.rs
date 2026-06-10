use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::gantt::GanttScheduler;
use crate::models::{cosine_sim, Dream, Experience, Goal};

#[derive(Debug, Clone)]
pub enum DreamContext {
    Random,
    ScheduleDrift {
        task_id: String,
        float_before: i64,
        alternative_order: Vec<String>,
    },
    Countermeasure {
        steps: Vec<String>,
        expected_impact: f32,
        triggered_by_goal: String,
    },
    Interpolation {
        source_goal: String,
        target_goal: String,
    },
}

pub struct DreamEngine {
    pub dreams: Vec<Dream>,
    pub dream_contexts: Vec<DreamContext>,
    rng: StdRng,
}

impl DreamEngine {
    pub fn new() -> Self {
        Self {
            dreams: Vec::new(),
            dream_contexts: Vec::new(),
            rng: StdRng::from_entropy(),
        }
    }

    pub fn generate_dreams(&mut self, goals: &[Goal], batch_size: usize) -> Vec<Dream> {
        self.generate_dreams_with_context(goals, batch_size, None, None)
    }

    pub fn generate_dreams_with_context(
        &mut self,
        goals: &[Goal],
        batch_size: usize,
        scheduler: Option<&GanttScheduler>,
        experiences: Option<&[Experience]>,
    ) -> Vec<Dream> {
        if goals.is_empty() || batch_size == 0 {
            return vec![];
        }

        let mut dreams = Vec::new();
        let mut contexts = Vec::new();

        let schedule_proposals = scheduler.map(|s| s.dream_proposals()).unwrap_or_default();

        for i in 0..batch_size {
            let roll: f64 = self.rng.gen();

            let (dream, context) = if !schedule_proposals.is_empty() && roll < 0.3 {
                let prop = &schedule_proposals[i % schedule_proposals.len()];
                let embedding = self.dream_from_proposal(goals, prop);
                let desc = prop.description.clone();
                let ctx = DreamContext::ScheduleDrift {
                    task_id: prop.task_id.clone(),
                    float_before: prop.new_float,
                    alternative_order: match &prop.kind {
                        crate::gantt::GanttDreamKind::Reorder { swap_with } => {
                            vec![prop.task_id.clone(), swap_with.clone()]
                        }
                        crate::gantt::GanttDreamKind::Parallelize { concurrent_tasks } => {
                            concurrent_tasks.clone()
                        }
                        _ => vec![],
                    },
                };
                (Dream::new(
                    goals[0].embedding.clone(),
                    embedding,
                    "schedule_dream".to_string(),
                    desc,
                ), ctx)
            } else if let Some(exps) = experiences {
                if !exps.is_empty() && roll < 0.6 {
                    self.generate_countermeasure_dream(goals, exps)
                } else if roll < 0.8 {
                    self.generate_interpolation_dream(goals)
                } else {
                    self.generate_mutation_dream(goals)
                }
            } else if roll < 0.4 {
                self.generate_interpolation_dream(goals)
            } else if roll < 0.7 {
                let extrapolated = self.extrapolate(goals);
                let ctx = DreamContext::Random;
                (extrapolated, ctx)
            } else {
                self.generate_mutation_dream(goals)
            };

            dreams.push(dream);
            contexts.push(context);
        }

        self.dreams.extend(dreams.clone());
        self.dream_contexts.extend(contexts);
        dreams
    }

    fn dream_from_proposal(&mut self, goals: &[Goal], proposal: &crate::gantt::GanttDreamProposal) -> Vec<f32> {
        if goals.is_empty() {
            return vec![];
        }
        let g = &goals[self.rng.gen_range(0..goals.len())];
        let boost = proposal.expected_impact;
        g.embedding.iter().map(|x| x * (1.0 + boost * 0.1)).collect()
    }

    fn generate_countermeasure_dream(
        &mut self,
        goals: &[Goal],
        experiences: &[Experience],
    ) -> (Dream, DreamContext) {
        let stagnant_goals: Vec<&Goal> = goals.iter()
            .filter(|g| {
                let recent: Vec<&Experience> = experiences.iter()
                    .filter(|e| e.goal_id == g.id)
                    .collect();
                recent.last().map_or(false, |e| e.grade < 0.3)
            })
            .collect();

        if stagnant_goals.is_empty() {
            let g = &goals[self.rng.gen_range(0..goals.len())];
            let embedding: Vec<f32> = g.embedding.iter()
                .map(|x| x + self.rng.gen::<f32>() * 0.2 - 0.1)
                .collect();
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
            let embedding: Vec<f32> = embedding.iter().map(|x| x / norm).collect();
            let desc = format!("Explore near {} after stagnation", g.domain_name);
            return (Dream::new(g.embedding.clone(), embedding, "explore".to_string(), desc), DreamContext::Random);
        }

        let source = stagnant_goals[0];
        let breakthrough = experiences.iter()
            .filter(|e| e.grade > 0.7)
            .max_by_key(|e| (e.grade * 100.0) as i64);

        let (embedding, steps) = if let Some(best) = breakthrough {
            let vec = best.after.clone();
            let steps = vec![
                format!("Apply breakthrough pattern from experience {}", &best.id[..8]),
                format!("Increase grade target from {:.2} to {:.2}", source.progress, best.grade),
                format!("Shift vector toward {:.2?}", &best.after[..2.min(best.after.len())]),
            ];
            (vec, steps)
        } else {
            let vec: Vec<f32> = source.embedding.iter()
                .map(|x| x + self.rng.gen::<f32>() * 0.3 - 0.15)
                .collect();
            let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
            let vec: Vec<f32> = vec.iter().map(|x| x / norm).collect();
            let steps = vec![
                format!("Investigate stagnation in goal {}", &source.id[..8]),
                "Run diagnostic on recent low-grade experiences".to_string(),
                "Propose course correction vector".to_string(),
            ];
            (vec, steps)
        };

        let desc = format!("Countermeasure for {} — {} steps", source.domain_name, steps.len());
        (Dream::new(source.embedding.clone(), embedding, "countermeasure".to_string(), desc),
         DreamContext::Countermeasure {
             steps,
             expected_impact: 0.3,
             triggered_by_goal: source.id.clone(),
         })
    }

    fn generate_interpolation_dream(&mut self, goals: &[Goal]) -> (Dream, DreamContext) {
        if goals.len() < 2 {
            return self.generate_mutation_dream(goals);
        }
        let g1 = &goals[self.rng.gen_range(0..goals.len())];
        let g2 = &goals[self.rng.gen_range(0..goals.len())];
        let t: f32 = self.rng.gen();
        let embedding: Vec<f32> = g1.embedding.iter()
            .zip(g2.embedding.iter())
            .map(|(a, b)| a * (1.0 - t) + b * t)
            .collect();
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
        let embedding: Vec<f32> = embedding.iter().map(|x| x / norm).collect();

        let desc = format!("Interpolate {:.2} {} ↔ {}", t, g1.domain_name, g2.domain_name);
        (Dream::new(g1.embedding.clone(), embedding, "interpolate".to_string(), desc),
         DreamContext::Interpolation {
             source_goal: g1.id.clone(),
             target_goal: g2.id.clone(),
         })
    }

    fn generate_mutation_dream(&mut self, goals: &[Goal]) -> (Dream, DreamContext) {
        let g = &goals[self.rng.gen_range(0..goals.len())];
        let noise_std: f32 = 0.1 + self.rng.gen::<f32>() * 0.3;
        let embedding: Vec<f32> = g.embedding.iter()
            .map(|x| x + self.rng.gen::<f32>() * noise_std - noise_std * 0.5)
            .collect();
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
        let embedding: Vec<f32> = embedding.iter().map(|x| x / norm).collect();

        let desc = format!("Mutate {} with noise_std={:.3}", g.domain_name, noise_std);
        (Dream::new(g.embedding.clone(), embedding, "mutate".to_string(), desc),
         DreamContext::Random)
    }

    fn interpolate(&mut self, goals: &[Goal]) -> Dream {
        let (dream, _) = self.generate_interpolation_dream(goals);
        dream
    }

    fn extrapolate(&mut self, goals: &[Goal]) -> Dream {
        let g = &goals[self.rng.gen_range(0..goals.len())];

        let dim = g.embedding.len();
        let centroid: Vec<f32> = (0..dim)
            .map(|i| goals.iter().map(|g| g.embedding[i]).sum::<f32>() / goals.len() as f32)
            .collect();

        let factor: f32 = 0.3 + self.rng.gen::<f32>() * 0.5;
        let embedding: Vec<f32> = g.embedding.iter()
            .zip(centroid.iter())
            .map(|(a, c)| a + (a - c) * factor)
            .collect();
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
        let embedding: Vec<f32> = embedding.iter().map(|x| x / norm).collect();

        let desc = format!("Extrapolate {} factor={:.2}", g.domain_name, factor);
        Dream::new(g.embedding.clone(), embedding, "extrapolate".to_string(), desc)
    }

    fn mutate(&mut self, goals: &[Goal]) -> Dream {
        let (dream, _) = self.generate_mutation_dream(goals);
        dream
    }

    pub fn evaluate_dream(&mut self, dream_id: &str, grade: f32) -> bool {
        if let Some(dream) = self.dreams.iter_mut().find(|d| d.id == dream_id) {
            dream.grade = Some(grade);
            true
        } else {
            false
        }
    }

    pub fn novelty(&self, dream: &Dream) -> f32 {
        1.0 - cosine_sim(&dream.source, &dream.embedding)
    }

    pub fn ungraded_dreams(&self) -> Vec<&Dream> {
        self.dreams.iter().filter(|d| d.grade.is_none()).collect()
    }

    pub fn get_context(&self, dream_id: &str) -> Option<&DreamContext> {
        self.dreams.iter()
            .position(|d| d.id == dream_id)
            .and_then(|idx| self.dream_contexts.get(idx))
    }
}

impl Default for DreamEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gantt::{AbstractDomain, GanttScheduler};

    fn test_goals(dim: usize) -> Vec<Goal> {
        let mut rng = StdRng::seed_from_u64(42);
        (0..5).map(|_| {
            let v: Vec<f32> = (0..dim).map(|_| rng.gen::<f32>() * 2.0 - 1.0).collect();
            let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
            let embedding: Vec<f32> = v.iter().map(|x| x / norm).collect();
            Goal::new(embedding, "test".to_string())
        }).collect()
    }

    #[test]
    fn test_generate_dreams() {
        let mut engine = DreamEngine::new();
        let goals = test_goals(32);
        let dreams = engine.generate_dreams(&goals, 10);
        assert_eq!(dreams.len(), 10);
        for d in &dreams {
            assert_eq!(d.source.len(), 32);
            assert_eq!(d.embedding.len(), 32);
        }
    }

    #[test]
    fn test_generate_with_context() {
        let mut engine = DreamEngine::new();
        let goals = test_goals(32);
        let scheduler = GanttScheduler::new(Box::new(AbstractDomain));
        let dreams = engine.generate_dreams_with_context(&goals, 5, Some(&scheduler), None);
        assert_eq!(dreams.len(), 5);
    }

    #[test]
    fn test_generate_countermeasure() {
        let mut engine = DreamEngine::new();
        let mut goals = test_goals(32);
        goals[0].progress = 0.1;

        let mut exps = Vec::new();
        let exp = Experience::new(
            &goals[0].id, vec![0.0; 32], vec![0.1; 32],
            "action".to_string(), "test".to_string(), std::collections::HashMap::new(),
        );
        exps.push(exp);

        let dreams = engine.generate_dreams_with_context(&goals, 3, None, Some(&exps));
        assert_eq!(dreams.len(), 3);
    }

    #[test]
    fn test_evaluate_dream() {
        let mut engine = DreamEngine::new();
        let goals = test_goals(32);
        let dreams = engine.generate_dreams(&goals, 1);
        let dream_id = dreams[0].id.clone();
        assert!(engine.evaluate_dream(&dream_id, 0.8));
        assert_eq!(engine.dreams[0].grade, Some(0.8));
    }

    #[test]
    fn test_dream_context_tracking() {
        let mut engine = DreamEngine::new();
        let goals = test_goals(32);
        let _ = engine.generate_dreams(&goals, 3);
        assert_eq!(engine.dream_contexts.len(), 3);
    }

    #[test]
    fn test_dream_nonexistent_id() {
        let mut engine = DreamEngine::new();
        assert!(!engine.evaluate_dream("nonexistent", 0.5));
    }

    #[test]
    fn test_generate_no_goals() {
        let mut engine = DreamEngine::new();
        let dreams = engine.generate_dreams(&[], 5);
        assert!(dreams.is_empty());
    }

    #[test]
    fn test_novelty() {
        let mut engine = DreamEngine::new();
        let goals = test_goals(32);
        let dreams = engine.generate_dreams(&goals, 1);
        let n = engine.novelty(&dreams[0]);
        assert!((0.0..=2.0).contains(&n));
    }

    #[test]
    fn test_ungraded_dreams() {
        let mut engine = DreamEngine::new();
        let goals = test_goals(32);
        engine.generate_dreams(&goals, 5);
        assert_eq!(engine.ungraded_dreams().len(), 5);
    }
}
