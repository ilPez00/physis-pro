use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::classify::CellClassifier;
use crate::gantt::GanttScheduler;
use crate::models::{cosine_sim, Dream, DreamReview, Experience, Goal};

/// Keep a proposal on one line. A goal's label is whatever prompt created it,
/// which can be a paragraph; a dream description is a headline.
fn truncate_name(name: &str) -> String {
    const MAX: usize = 48;
    if name.chars().count() <= MAX {
        name.to_string()
    } else {
        name.chars().take(MAX - 1).collect::<String>() + "…"
    }
}

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
            rng: StdRng::from_os_rng(),
        }
    }

    /// Engine with a fixed seed: which generator fires and which goal it
    /// draws is RNG-driven, so tests (and reproducible demos) pin it.
    pub fn new_seeded(seed: u64) -> Self {
        Self {
            dreams: Vec::new(),
            dream_contexts: Vec::new(),
            rng: StdRng::seed_from_u64(seed),
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
            let roll: f64 = self.rng.random();

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
                (
                    Dream::new(
                        goals[0].embedding.clone(),
                        embedding,
                        "schedule_dream".to_string(),
                        desc,
                    ),
                    ctx,
                )
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

    fn dream_from_proposal(
        &mut self,
        goals: &[Goal],
        proposal: &crate::gantt::GanttDreamProposal,
    ) -> Vec<f32> {
        if goals.is_empty() {
            return vec![];
        }
        let g = &goals[self.rng.random_range(0..goals.len())];
        let boost = proposal.expected_impact;
        // Impact scales the DIRECTION, not the magnitude: embeddings are
        // L2-normalized (VectorEmbed's documented invariant), so renormalize
        // after scaling — cosine against an unnormalized vector is distorted.
        let embedding: Vec<f32> = g
            .embedding
            .iter()
            .map(|x| x * (1.0 + boost * 0.1))
            .collect();
        let norm: f32 = embedding
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt()
            .max(1e-8);
        embedding.iter().map(|x| x / norm).collect()
    }

    fn generate_countermeasure_dream(
        &mut self,
        goals: &[Goal],
        experiences: &[Experience],
    ) -> (Dream, DreamContext) {
        let stagnant_goals: Vec<&Goal> = goals
            .iter()
            .filter(|g| {
                let recent: Vec<&Experience> =
                    experiences.iter().filter(|e| e.goal_id == g.id).collect();
                recent.last().is_some_and(|e| e.grade < 0.3)
            })
            .collect();

        if stagnant_goals.is_empty() {
            let g = &goals[self.rng.random_range(0..goals.len())];
            let embedding: Vec<f32> = g
                .embedding
                .iter()
                .map(|x| x + self.rng.random::<f32>() * 0.2 - 0.1)
                .collect();
            let norm: f32 = embedding
                .iter()
                .map(|x| x * x)
                .sum::<f32>()
                .sqrt()
                .max(1e-8);
            let embedding: Vec<f32> = embedding.iter().map(|x| x / norm).collect();
            let desc = format!("Explore near {} after stagnation", truncate_name(g.name()));
            return (
                Dream::new(g.embedding.clone(), embedding, "explore".to_string(), desc),
                DreamContext::Random,
            );
        }

        let source = stagnant_goals[0];
        let breakthrough = experiences
            .iter()
            .filter(|e| e.grade > 0.7)
            .max_by_key(|e| (e.grade * 100.0) as i64);

        let (embedding, steps) = if let Some(best) = breakthrough {
            let vec = best.after.clone();
            let steps = vec![
                format!(
                    "Apply breakthrough pattern from experience {}",
                    &best.id[..8]
                ),
                format!(
                    "Increase grade target from {:.2} to {:.2}",
                    source.progress, best.grade
                ),
                format!(
                    "Shift vector toward {:.2?}",
                    &best.after[..2.min(best.after.len())]
                ),
            ];
            (vec, steps)
        } else {
            let vec: Vec<f32> = source
                .embedding
                .iter()
                .map(|x| x + self.rng.random::<f32>() * 0.3 - 0.15)
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

        let desc = format!(
            "Countermeasure for {} — {} steps",
            truncate_name(source.name()),
            steps.len()
        );
        (
            Dream::new(
                source.embedding.clone(),
                embedding,
                "countermeasure".to_string(),
                desc,
            ),
            DreamContext::Countermeasure {
                steps,
                expected_impact: 0.3,
                triggered_by_goal: source.id.clone(),
            },
        )
    }

    fn generate_interpolation_dream(&mut self, goals: &[Goal]) -> (Dream, DreamContext) {
        if goals.len() < 2 {
            return self.generate_mutation_dream(goals);
        }
        let g1 = &goals[self.rng.random_range(0..goals.len())];
        let g2 = &goals[self.rng.random_range(0..goals.len())];
        let t: f32 = self.rng.random();
        let embedding: Vec<f32> = g1
            .embedding
            .iter()
            .zip(g2.embedding.iter())
            .map(|(a, b)| a * (1.0 - t) + b * t)
            .collect();
        let norm: f32 = embedding
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt()
            .max(1e-8);
        let embedding: Vec<f32> = embedding.iter().map(|x| x / norm).collect();

        let desc = format!(
            "Interpolate {:.2} {} ↔ {}",
            t,
            truncate_name(g1.name()),
            truncate_name(g2.name())
        );
        (
            Dream::new(
                g1.embedding.clone(),
                embedding,
                "interpolate".to_string(),
                desc,
            ),
            DreamContext::Interpolation {
                source_goal: g1.id.clone(),
                target_goal: g2.id.clone(),
            },
        )
    }

    fn generate_mutation_dream(&mut self, goals: &[Goal]) -> (Dream, DreamContext) {
        let g = &goals[self.rng.random_range(0..goals.len())];
        let noise_std: f32 = 0.1 + self.rng.random::<f32>() * 0.3;
        let embedding: Vec<f32> = g
            .embedding
            .iter()
            .map(|x| x + self.rng.random::<f32>() * noise_std - noise_std * 0.5)
            .collect();
        let norm: f32 = embedding
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt()
            .max(1e-8);
        let embedding: Vec<f32> = embedding.iter().map(|x| x / norm).collect();

        let desc = format!(
            "Mutate {} with noise_std={:.3}",
            truncate_name(g.name()),
            noise_std
        );
        (
            Dream::new(g.embedding.clone(), embedding, "mutate".to_string(), desc),
            DreamContext::Random,
        )
    }

    fn extrapolate(&mut self, goals: &[Goal]) -> Dream {
        let g = &goals[self.rng.random_range(0..goals.len())];

        let dim = g.embedding.len();
        let centroid: Vec<f32> = (0..dim)
            .map(|i| goals.iter().map(|g| g.embedding[i]).sum::<f32>() / goals.len() as f32)
            .collect();

        let factor: f32 = 0.3 + self.rng.random::<f32>() * 0.5;
        let embedding: Vec<f32> = g
            .embedding
            .iter()
            .zip(centroid.iter())
            .map(|(a, c)| a + (a - c) * factor)
            .collect();
        let norm: f32 = embedding
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt()
            .max(1e-8);
        let embedding: Vec<f32> = embedding.iter().map(|x| x / norm).collect();

        let desc = format!(
            "Extrapolate {} factor={:.2}",
            truncate_name(g.name()),
            factor
        );
        Dream::new(
            g.embedding.clone(),
            embedding,
            "extrapolate".to_string(),
            desc,
        )
    }
}

/// The ontological validation contract for generated dreams.
///
/// Applies the Generative Ontology method (`research_new`): the ontology is a
/// validation contract, not only a classification vocabulary — an output that
/// cannot be named by the grid is not a plan, it is structure-shaped noise.
/// The dream generators interpolate and mutate goal embeddings freely, so a
/// dream between a machine fault and an invoice can land where classification
/// finds nothing. [`DreamContract::review`] names the closest ontology entry;
/// [`DreamEngine::generate_dreams_contracted`] accepts only nameable dreams
/// and retries the rest up to a bounded number of times. Rejections are
/// counted in the result — a 100% rejection rate is the honest signal that
/// the goals themselves sit outside the ontology, not something to hide.
pub struct DreamContract<'a> {
    classifier: &'a CellClassifier,
    min_cosine: f32,
    max_retries: usize,
}

/// What the contracted generator hands back: the accepted dreams (each with
/// its review verdict attached), their parallel contexts, and the accounting
/// that makes rejection visible.
#[derive(Debug, Clone)]
pub struct ContractedDreams {
    pub dreams: Vec<Dream>,
    pub contexts: Vec<DreamContext>,
    /// How many generated dreams failed the review across all attempts.
    pub rejected: usize,
    /// Lowest accepted cosine — how much contract slack the batch used.
    pub weakest_accepted: f32,
}

impl<'a> DreamContract<'a> {
    /// `min_cosine` is the naming threshold (0.5 is a reasonable default:
    /// clearly-related entries score 0.6+ under the production embedders,
    /// unrelated text scores below 0.4). `max_retries` bounds regeneration.
    pub fn new(classifier: &'a CellClassifier, min_cosine: f32, max_retries: usize) -> Self {
        Self {
            classifier,
            min_cosine,
            max_retries,
        }
    }

    /// Name a generated embedding against the grid: closest entry, its cell,
    /// and whether the pair clears the threshold. One pass over the cells —
    /// the classifier's own entry walk, without the blended cell scoring,
    /// because the contract is about *nameability*, not rank.
    pub fn review(&self, embedding: &[f32]) -> DreamReview {
        let mut best: Option<(String, String, f32)> = None;
        for cell in &self.classifier.0.cells {
            for (i, e) in cell.embeddings.iter().enumerate() {
                let s = cosine_sim(embedding, e);
                if best.as_ref().map(|(_, _, b)| s > *b).unwrap_or(true) {
                    if let Some(name) = cell.entries.get(i) {
                        best = Some((
                            name.clone(),
                            format!("{}\u{d7}{}", cell.domain, cell.mode),
                            s,
                        ));
                    }
                }
            }
        }
        let (best_entry, best_cell, cosine) = best
            .map(|(n, c, s)| (Some(n), Some(c), s))
            .unwrap_or((None, None, 0.0));
        DreamReview {
            accepted: cosine >= self.min_cosine,
            best_entry,
            best_cell,
            cosine,
            min_cosine: self.min_cosine,
        }
    }
}

impl DreamEngine {
    /// Generate dreams under the contract: generate a batch, review every
    /// dream against the ontology, keep the accepted ones, and regenerate the
    /// shortfall up to `max_retries` times.
    ///
    /// The engine's raw pool (`self.dreams`) still records every generated
    /// dream including rejected ones — the pool is the subconscious, the
    /// contract governs what gets *proposed*. Callers serialize the returned
    /// batch, not the pool.
    pub fn generate_dreams_contracted(
        &mut self,
        goals: &[Goal],
        batch_size: usize,
        scheduler: Option<&GanttScheduler>,
        experiences: Option<&[Experience]>,
        contract: &DreamContract,
    ) -> ContractedDreams {
        let mut accepted: Vec<Dream> = Vec::new();
        let mut contexts: Vec<DreamContext> = Vec::new();
        let mut rejected = 0usize;
        let mut weakest_accepted = f32::INFINITY;

        for _attempt in 0..=contract.max_retries {
            let need = batch_size.saturating_sub(accepted.len());
            if need == 0 {
                break;
            }
            let batch = self.generate_dreams_with_context(goals, need, scheduler, experiences);
            let batch_len = batch.len();
            // The engine extended its parallel pool with this batch; copy the
            // fresh contexts out (never split them off — the pool's dream and
            // context vectors must stay aligned, rejected dreams included).
            let fresh: Vec<DreamContext> =
                self.dream_contexts[self.dream_contexts.len().saturating_sub(batch_len)..].to_vec();
            for (mut dream, context) in batch.into_iter().zip(fresh) {
                let verdict = contract.review(&dream.embedding);
                if verdict.accepted {
                    weakest_accepted = weakest_accepted.min(verdict.cosine);
                    dream.review = Some(verdict);
                    accepted.push(dream);
                    contexts.push(context);
                } else {
                    rejected += 1;
                }
            }
        }

        ContractedDreams {
            dreams: accepted,
            contexts,
            rejected,
            weakest_accepted: if weakest_accepted.is_finite() {
                weakest_accepted
            } else {
                0.0
            },
        }
    }
}

impl DreamEngine {
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
        self.dreams
            .iter()
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
        (0..5)
            .map(|_| {
                let v: Vec<f32> = (0..dim).map(|_| rng.random::<f32>() * 2.0 - 1.0).collect();
                let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
                let embedding: Vec<f32> = v.iter().map(|x| x / norm).collect();
                Goal::new(embedding, "test".to_string())
            })
            .collect()
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
            &goals[0].id,
            vec![0.0; 32],
            vec![0.1; 32],
            "action".to_string(),
            "test".to_string(),
            std::collections::HashMap::new(),
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

    // ── Ontological validation contract ─────────────────────────────────
    //
    // Fixture: two tight cells on orthogonal axes of a 4-d space. Cell A sits
    // on +x, cell B on +y. A goal on +z is nameable by neither — everything
    // generated from it must fail the contract, everything generated from
    // cell-A goals must pass it.

    use crate::models::Facets;

    fn unit(v: [f32; 4]) -> Vec<f32> {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
        v.iter().map(|x| x / norm).collect()
    }

    fn audit_fixture() -> CellClassifier {
        let cell = |domain: &str, mode: &str, entries: Vec<Vec<f32>>| {
            physis_core::classify::Cell::new(
                domain.to_string(),
                mode.to_string(),
                entries.clone(),
                (0..entries.len())
                    .map(|i| format!("{domain}-{mode}-{i}"))
                    .collect(),
                vec![Facets::default(); entries.len()],
            )
        };
        crate::classify::from_cells(vec![
            cell(
                "MACHINE",
                "WORK",
                vec![unit([1.0, 0.0, 0.0, 0.0]), unit([0.95, 0.05, 0.0, 0.0])],
            ),
            cell(
                "OFFICE",
                "PLAN",
                vec![unit([0.0, 1.0, 0.0, 0.0]), unit([0.05, 0.95, 0.0, 0.0])],
            ),
        ])
    }

    fn goals_along(v: [f32; 4], n: usize) -> Vec<Goal> {
        let e = unit(v);
        (0..n)
            .map(|_| Goal::new(e.clone(), "test".to_string()))
            .collect()
    }

    #[test]
    fn contract_accepts_dreams_the_grid_can_name() {
        let classifier = audit_fixture();
        // +x goals: every generator (mutate/jitter/extrapolate/interpolate)
        // stays within ~0.2 of the goal direction, and cell A's nearest
        // entries sit at cosine ≈ 0.95+ from there — comfortably above 0.5.
        let mut engine = DreamEngine::new();
        let goals = goals_along([1.0, 0.0, 0.0, 0.0], 5);
        let contract = DreamContract::new(&classifier, 0.5, 2);
        let result = engine.generate_dreams_contracted(&goals, 6, None, None, &contract);
        assert_eq!(result.dreams.len(), 6, "all should clear: {:?}", result);
        assert_eq!(result.rejected, 0);
        assert!(result.weakest_accepted >= 0.5);
        for d in &result.dreams {
            let review = d.review.as_ref().expect("accepted dreams carry a verdict");
            assert!(review.accepted);
            assert_eq!(
                review.best_cell.as_deref(),
                Some("MACHINE\u{d7}WORK"),
                "cell-A dreams must name cell A"
            );
        }
        // Parallel invariants: contexts and dreams returned in lockstep, and
        // the engine's raw pool still holds every dream (accepted AND the
        // ones the first attempt produced), with its parallel contexts
        // unbroken.
        assert_eq!(result.contexts.len(), result.dreams.len());
        assert_eq!(engine.dreams.len(), engine.dream_contexts.len());
    }

    #[test]
    fn contract_rejects_dreams_the_grid_cannot_name() {
        let classifier = audit_fixture();
        let mut engine = DreamEngine::new();
        // +z: orthogonal to both cells; no entry within 0.5 of anything
        // generated from here.
        let goals = goals_along([0.0, 0.0, 1.0, 0.0], 3);
        let contract = DreamContract::new(&classifier, 0.5, 2);
        let result = engine.generate_dreams_contracted(&goals, 4, None, None, &contract);
        assert!(result.dreams.is_empty(), "nothing nameable: {:?}", result);
        assert!(result.rejected >= 4, "every attempt rejected: {:?}", result);
        assert_eq!(result.weakest_accepted, 0.0);
        // The engine's raw pool still recorded the dreams (the subconscious
        // keeps what the contract refuses to propose).
        assert!(!engine.dreams.is_empty());
    }

    #[test]
    fn contract_mixed_goals_split_accurately() {
        let classifier = audit_fixture();
        // Seeded: which goal each generator draws is RNG-driven, and an
        // OS-seeded engine can (CI proved it, seed-space ~2%) draw only
        // on-grid goals for the whole batch, making `rejected > 0` flaky.
        // Seed 3 draws both kinds deterministically (6 accepted / 9 rejected).
        let mut engine = DreamEngine::new_seeded(3);
        let mut goals = goals_along([1.0, 0.0, 0.0, 0.0], 3);
        goals.extend(goals_along([0.0, 0.0, 1.0, 0.0], 2));
        let contract = DreamContract::new(&classifier, 0.5, 2);
        let result = engine.generate_dreams_contracted(&goals, 8, None, None, &contract);
        // Interpolation between an on-grid and an off-grid goal can go either
        // way; the contract's job is to SORT them, not to accept everything.
        assert!(
            result.rejected > 0,
            "off-grid goals must produce some rejections: {:?}",
            result
        );
        for d in &result.dreams {
            let review = d.review.as_ref().expect("accepted dreams carry a verdict");
            assert!(review.accepted);
            assert!(review.cosine >= 0.5);
        }
    }

    #[test]
    fn review_names_the_closest_entry_and_cell() {
        let classifier = audit_fixture();
        let contract = DreamContract::new(&classifier, 0.5, 2);
        let v = contract.review(&unit([1.0, 0.0, 0.0, 0.0]));
        assert!(v.accepted, "{v:?}");
        assert_eq!(v.best_entry.as_deref(), Some("MACHINE-WORK-0"));
        assert_eq!(v.best_cell.as_deref(), Some("MACHINE\u{d7}WORK"));
        let orthogonal = contract.review(&unit([0.0, 0.0, 1.0, 0.0]));
        assert!(!orthogonal.accepted, "{orthogonal:?}");
        assert!(orthogonal.cosine < 0.5);
        // An empty grid names nothing and rejects everything, honestly.
        let empty_classifier = crate::classify::from_cells(vec![]);
        let empty = DreamContract::new(&empty_classifier, 0.5, 2);
        let none = empty.review(&unit([1.0, 0.0, 0.0, 0.0]));
        assert!(!none.accepted);
        assert_eq!(none.best_entry, None);
    }

    #[test]
    fn legacy_dreams_load_without_a_review() {
        // Dreams persisted before the contract existed must deserialize with
        // review = None (the field is serde-defaulted), and a reviewed dream
        // round-trips its verdict.
        let legacy = r#"{
            "id": "d1", "source": [], "embedding": [],
            "grade": null, "dream_type": "mutate", "description": "old"
        }"#;
        let d: Dream = serde_json::from_str(legacy).unwrap();
        assert!(d.review.is_none());
        let mut d2 = Dream::new(vec![], vec![], "mutate".into(), "new".into());
        d2.review = Some(DreamReview {
            accepted: true,
            best_entry: Some("MACHINE-WORK-0".into()),
            best_cell: Some("MACHINE\u{d7}WORK".into()),
            cosine: 0.9,
            min_cosine: 0.5,
        });
        let json = serde_json::to_string(&d2).unwrap();
        let back: Dream = serde_json::from_str(&json).unwrap();
        let review = back.review.expect("review survives the round trip");
        assert_eq!(review.cosine, 0.9);
    }
}
