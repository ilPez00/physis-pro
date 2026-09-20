//! Outcome evidence: every guided step leaves a record.
//!
//! The log is the one the action surface already writes
//! (`registry::default_obs_path()`, via `capability::Observations`) — guided
//! execution adds evidence *into* the existing record rather than opening a
//! second ledger. That is what makes the later consolidation step possible:
//! repeated successful choices are already countable in one place.

use serde::{Deserialize, Serialize};

use crate::capability::{ObservationRecord, Observations};

use super::GuidedStep;
use super::model::Route;

/// One guided step, as evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuidedActionObservation {
    pub intent: String,
    pub world_version: String,
    pub constraint_version: u32,
    /// How many transitions the world offered when the choice was made.
    pub considered_actions: usize,
    /// How the choice was reached.
    pub route: Route,
    pub verb: String,
    pub target: String,
    pub capability: String,
    /// `None` when the caller asked only for a plan (no execution).
    pub executed: bool,
    pub success: Option<bool>,
    pub detail: String,
}

impl GuidedActionObservation {
    /// Build from a step. `outcome` is the rendered executor output when the
    /// step was executed; `executed` distinguishes "not run" from "failed".
    pub fn from_step(
        step: &GuidedStep,
        intent: &str,
        outcome: Option<&str>,
        executed: bool,
    ) -> Self {
        let capability = step
            .context
            .capability_by_verb
            .get(&step.action.verb)
            .cloned()
            .unwrap_or_else(|| "unafforded".to_string());
        let success = outcome.map(|text| !text.contains("FAILED") && !text.contains("refused"));
        let detail = match (executed, outcome) {
            (true, Some(text)) => format!(
                "route={:?} considered={} target={} outcome={}",
                step.route,
                step.context.targets(&step.action.verb).len(),
                step.action.target,
                text.lines().next().unwrap_or("")
            ),
            _ => format!(
                "route={:?} considered={} target={} planned-not-executed",
                step.route,
                step.context.targets(&step.action.verb).len(),
                step.action.target
            ),
        };
        Self {
            intent: intent.to_string(),
            world_version: step.context.provenance.world_version.clone(),
            constraint_version: step.context.provenance.version,
            considered_actions: step
                .context
                .targets_by_verb
                .values()
                .map(|v| v.len())
                .sum(),
            route: step.route,
            verb: step.action.verb.clone(),
            target: step.action.target.to_string(),
            capability,
            executed,
            success,
            detail,
        }
    }

    /// Append to the registry's observation log. The `actor` is `guided`, so a
    /// recorded outcome can be told apart from a direct `action call`.
    pub fn record(&self, path: &std::path::Path) {
        let observations = Observations::new(path);
        observations.append(ObservationRecord {
            t: 0.0,
            actor: "guided".to_string(),
            object: self.target.clone(),
            verb: self.verb.clone(),
            capability: Some(self.capability.clone()),
            ok: self.success.unwrap_or(true),
            detail: self.detail.clone(),
            reliability: None,
        });
    }
}
