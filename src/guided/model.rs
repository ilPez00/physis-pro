//! The model side, kept deliberately thin.
//!
//! Phase A ships one backend: `MockGuidedModel`, which is deterministic. That
//! is not a placeholder for nothing — it is the *ceiling* of the design, because
//! the constraint already contains the answer most of the time, and it makes the
//! `GuidedModel` seam testable without a network, a GPU or a licence.
//!
//! The trait is the only place a provider enters. Adapters compile the same
//! `GenerationContext` to llama.cpp grammar / Ollama structured output / a
//! provider tool schema (Phase B) and implement this one method.

use serde::{Deserialize, Serialize};

use super::action::{GuidedAction, GuidedArgs};
use super::constraint::GenerationContext;

/// How the step was reached. Reported, never inferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    /// Physis answered; the model was never asked.
    Exact,
    /// The model chose within the constraint.
    Model,
}

/// Counters for the whole point of the exercise: how much inference was made
/// unnecessary, and how often the language refused something anyway.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    pub model_calls: u64,
    pub deterministic_bypasses: u64,
    pub rejections: u64,
}

impl Metrics {
    /// Share of steps that never reached a model. `None` before any step, so a
    /// caller cannot print `0 %` as if it were a measurement.
    pub fn bypass_rate(&self) -> Option<f64> {
        let total = self.model_calls + self.deterministic_bypasses;
        if total == 0 {
            return None;
        }
        Some(self.deterministic_bypasses as f64 / total as f64)
    }
}

/// The one method a provider backend must implement.
pub trait GuidedModel {
    fn choose(&self, ctx: &GenerationContext, intent: &str) -> Result<GuidedAction, String>;
}

/// Deterministic backend: scores every legal (verb, target) pair by token
/// overlap with the intent and takes the best. Ties break on the constraint's
/// own order, so the same world and intent always choose the same action.
#[derive(Debug, Default, Clone)]
pub struct MockGuidedModel;

impl GuidedModel for MockGuidedModel {
    fn choose(&self, ctx: &GenerationContext, intent: &str) -> Result<GuidedAction, String> {
        let tokens = tokens_of(intent);
        let mut best: Option<(usize, String, super::affordance::ObjectHandle)> = None;
        for (verb, targets) in &ctx.targets_by_verb {
            let verb_score = overlap(verb, &tokens);
            for target in targets {
                let score = verb_score
                    + overlap(&target.label, &tokens)
                    + overlap(&target.path, &tokens)
                    + overlap(&target.kind, &tokens);
                let candidate = (score, verb.clone(), target.handle.clone());
                if best.as_ref().map(|b| score > b.0).unwrap_or(true) {
                    best = Some(candidate);
                }
            }
        }
        let Some((_, verb, target)) = best else {
            return Err("no legal action in this world slice".into());
        };
        Ok(GuidedAction::new(verb, target, GuidedArgs::None))
    }
}

/// Deterministic resolution: when the intent names exactly one *visible* object
/// and the language has exactly one read-shaped verb for its kind, Physis
/// answers without a model. This is the "CPU temperature" path — exact facts do
/// not go through generation.
pub fn resolve_exact(ctx: &GenerationContext, intent: &str) -> Option<GuidedAction> {
    let tokens = tokens_of(intent);
    if tokens.is_empty() {
        return None;
    }
    // Exactly one visible object whose own words intersect the intent, and no
    // second one that does: two candidates is a choice, and a choice is the
    // model's.
    let matching: Vec<&super::affordance::ObjectRef> = ctx
        .visible_objects
        .iter()
        .filter(|o| {
            overlap(&o.label, &tokens) > 0
                || overlap(&o.path, &tokens) > 0
                || overlap(&o.kind, &tokens) > 0
        })
        .collect();
    if matching.len() != 1 {
        return None;
    }
    let object = matching[0];
    // Exactly one legal verb for it, and it must be read-shaped: a bypass that
    // could write would be a model-free mutation path, which this is not.
    let mut verbs: Vec<&String> = ctx
        .targets_by_verb
        .iter()
        .filter(|(_, targets)| targets.iter().any(|t| t.handle == object.handle))
        .map(|(verb, _)| verb)
        .collect();
    verbs.sort();
    if verbs.len() != 1 || !super::affordance::READ_VERBS.contains(&verbs[0].as_str()) {
        return None;
    }
    Some(GuidedAction::new(
        verbs[0].clone(),
        object.handle.clone(),
        GuidedArgs::None,
    ))
}

fn tokens_of(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() > 2)
        .map(|t| t.to_string())
        .collect()
}

fn overlap(text: &str, tokens: &[String]) -> usize {
    let hay = text.to_lowercase();
    tokens.iter().filter(|t| hay.contains(t.as_str())).count()
}
