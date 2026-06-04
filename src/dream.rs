//! Stochastic dream generation — interpolate, extrapolate, and mutate goal
//! vectors to produce novel candidate embeddings for evaluation.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::models::{cosine_sim, Dream, Goal};

/// Stochastic dream engine — generates novel candidate embeddings by interpolating,
/// extrapolating, and mutating existing goal vectors.
pub struct DreamEngine {
    /// All generated dreams (kept for grading and novelty scoring).
    pub dreams: Vec<Dream>,
    rng: StdRng,
}

impl DreamEngine {
    /// Create a new dream engine with an entropy-seeded RNG.
    pub fn new() -> Self {
        Self {
            dreams: Vec::new(),
            rng: StdRng::from_entropy(),
        }
    }

    /// Generate `batch_size` dreams from the given goal set using random interpolation,
    /// extrapolation, and mutation. Returns only the newly generated dreams.
    pub fn generate_dreams(&mut self, goals: &[Goal], batch_size: usize) -> Vec<Dream> {
        if goals.is_empty() || batch_size == 0 {
            return vec![];
        }

        let mut dreams = Vec::new();
        for _ in 0..batch_size {
            let roll: f64 = self.rng.gen();
            let dream = if roll < 0.4 {
                self.interpolate(goals)
            } else if roll < 0.7 {
                self.extrapolate(goals)
            } else {
                self.mutate(goals)
            };
            dreams.push(dream);
        }

        self.dreams.extend(dreams.clone());
        dreams
    }

    /// Interpolate between two random goal vectors.
    fn interpolate(&mut self, goals: &[Goal]) -> Dream {
        let g1 = &goals[self.rng.gen_range(0..goals.len())];
        let g2 = &goals[self.rng.gen_range(0..goals.len())];
        let t: f32 = self.rng.gen();
        let embedding: Vec<f32> = g1.embedding.iter()
            .zip(g2.embedding.iter())
            .map(|(a, b)| a * (1.0 - t) + b * t)
            .collect();
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
        let embedding: Vec<f32> = embedding.iter().map(|x| x / norm).collect();

        Dream::new(g1.embedding.clone(), embedding, "dream".to_string(), "description".to_string())
    }

    /// Extrapolate from a goal vector away from the centroid.
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

        Dream::new(g.embedding.clone(), embedding, "mutate".to_string(), "mutated dream".to_string())
    }


    /// Mutate a goal vector with gaussian noise.
    fn mutate(&mut self, goals: &[Goal]) -> Dream {
        let g = &goals[self.rng.gen_range(0..goals.len())];
        let noise_std: f32 = 0.1 + self.rng.gen::<f32>() * 0.2;
        let embedding: Vec<f32> = g.embedding.iter()
            .map(|x| x + self.rng.gen::<f32>() * noise_std - noise_std * 0.5)
            .collect();
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
        let embedding: Vec<f32> = embedding.iter().map(|x| x / norm).collect();

        Dream::new(g.embedding.clone(), embedding, "mutate".to_string(), "mutated dream".to_string())
    }
    pub fn evaluate_dream(&mut self, dream_id: &str, grade: f32) -> bool {
        if let Some(dream) = self.dreams.iter_mut().find(|d| d.id == dream_id) {
            dream.grade = Some(grade);
            true
        } else {
            false
        }
    }

    /// Compute novelty score: 1.0 - cosine similarity between source and embedding.
    pub fn novelty(&self, dream: &Dream) -> f32 {
        1.0 - cosine_sim(&dream.source, &dream.embedding)
    }

    /// Return all dreams that have not yet been graded.
    pub fn ungraded_dreams(&self) -> Vec<&Dream> {
        self.dreams.iter().filter(|d| d.grade.is_none()).collect()
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
    fn test_evaluate_dream_accept() {
        let mut engine = DreamEngine::new();
        let goals = test_goals(32);
        let dreams = engine.generate_dreams(&goals, 1);
        let dream_id = dreams[0].id.clone();
        assert!(engine.evaluate_dream(&dream_id, 0.8));
        assert_eq!(engine.dreams[0].grade, Some(0.8));
    }

    #[test]
    fn test_evaluate_dream_reject() {
        let mut engine = DreamEngine::new();
        let goals = test_goals(32);
        let dreams = engine.generate_dreams(&goals, 1);
        let dream_id = dreams[0].id.clone();
        assert!(engine.evaluate_dream(&dream_id, 0.3));
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
