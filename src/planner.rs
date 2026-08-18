//! Counterfactual decision planning.
//!
//! A planner run compares the live classifier with a candidate ontology over a
//! bounded corpus. It returns an impact diff and never mutates live state.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellOutcome {
    pub domain: String,
    pub mode: String,
    pub score: f32,
}

impl CellOutcome {
    pub fn key(&self) -> String {
        format!("{}×{}", self.domain, self.mode)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerItem {
    pub id: String,
    pub source: String,
    pub before: CellOutcome,
    pub after: CellOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    pub from: String,
    pub to: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerSimulation {
    pub scenario_id: String,
    pub records_scanned: usize,
    pub changed: usize,
    pub unchanged: usize,
    pub low_confidence: usize,
    pub transitions: Vec<Transition>,
    pub items: Vec<PlannerItem>,
}

/// Summarize before/after classifications. `low_confidence` is intentionally
/// explicit: a changed result with a weak score belongs in human review.
pub fn simulate(
    scenario_id: impl Into<String>,
    items: Vec<PlannerItem>,
    low_confidence_threshold: f32,
) -> PlannerSimulation {
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut changed = 0;
    let mut low_confidence = 0;

    for item in &items {
        if item.before.key() != item.after.key() {
            changed += 1;
            *counts
                .entry((item.before.key(), item.after.key()))
                .or_default() += 1;
        }
        if item.before.score < low_confidence_threshold
            || item.after.score < low_confidence_threshold
        {
            low_confidence += 1;
        }
    }

    PlannerSimulation {
        scenario_id: scenario_id.into(),
        records_scanned: items.len(),
        changed,
        unchanged: items.len().saturating_sub(changed),
        low_confidence,
        transitions: counts
            .into_iter()
            .map(|((from, to), count)| Transition { from, to, count })
            .collect(),
        items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, before: (&str, &str, f32), after: (&str, &str, f32)) -> PlannerItem {
        PlannerItem {
            id: id.into(),
            source: format!("source/{id}.txt"),
            before: CellOutcome {
                domain: before.0.into(),
                mode: before.1.into(),
                score: before.2,
            },
            after: CellOutcome {
                domain: after.0.into(),
                mode: after.1.into(),
                score: after.2,
            },
        }
    }

    #[test]
    fn simulation_reports_transitions_and_review_load() {
        let result = simulate(
            "procurement-v2",
            vec![
                item(
                    "1",
                    ("OPERATE", "MONITOR", 0.91),
                    ("OPERATE", "CORRECT", 0.84),
                ),
                item(
                    "2",
                    ("OPERATE", "MONITOR", 0.88),
                    ("OPERATE", "CORRECT", 0.41),
                ),
                item(
                    "3",
                    ("OPERATE", "MONITOR", 0.92),
                    ("OPERATE", "MONITOR", 0.90),
                ),
            ],
            0.60,
        );

        assert_eq!(result.records_scanned, 3);
        assert_eq!(result.changed, 2);
        assert_eq!(result.unchanged, 1);
        assert_eq!(result.low_confidence, 1);
        assert_eq!(result.transitions[0].count, 2);
    }
}
