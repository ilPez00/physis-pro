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

/// How one record's coverage moved under the candidate ontology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageShift {
    /// Weak/absent before, confident after — the candidate *sees* this record.
    NewlyCovered,
    /// Confident before, weak/absent after — the candidate *blinds* us here.
    NewlyUncovered,
    /// Confidence verdict unchanged.
    Stable,
}

/// Per-cell count deltas: the reports and rollups whose numbers would move.
/// A cell with a non-zero delta is, transitively, an affected report line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellDelta {
    pub cell: String,
    pub before_count: usize,
    pub after_count: usize,
    pub delta: i64,
}

/// PLAN 7.2 impact diff: coverage gaps + changed aggregates, every row traced
/// back to the record ids that produced it (their `source` is their
/// provenance). Pure derivation from a saved simulation — nothing here
/// touches live state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAnalysis {
    /// Records the candidate newly classifies with acceptable confidence.
    pub newly_covered: Vec<String>,
    /// Records that lose acceptable-confidence classification — the gaps the
    /// candidate opens.
    pub newly_uncovered: Vec<String>,
    /// Per-cell population deltas, largest absolute movement first.
    pub cell_deltas: Vec<CellDelta>,
    /// Mean confidence delta across all records (+gained / −lost).
    pub mean_confidence_delta: f32,
}

impl ImpactAnalysis {
    /// Derive the impact of a simulation. A record counts as "covered" when
    /// both sides agree it landed somewhere real (`score >= threshold`);
    /// `unknown×unknown` never counts as covered regardless of score.
    pub fn derive(sim: &PlannerSimulation, threshold: f32) -> Self {
        let covered = |o: &CellOutcome| {
            o.domain != "unknown" && o.mode != "unknown" && o.score >= threshold
        };

        let mut newly_covered = Vec::new();
        let mut newly_uncovered = Vec::new();
        let mut before_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut after_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut total_delta = 0.0f32;

        for item in &sim.items {
            let was = covered(&item.before);
            let now = covered(&item.after);
            match (was, now) {
                (false, true) => newly_covered.push(item.id.clone()),
                (true, false) => newly_uncovered.push(item.id.clone()),
                _ => {}
            }
            *before_counts.entry(item.before.key()).or_default() += 1;
            *after_counts.entry(item.after.key()).or_default() += 1;
            total_delta += item.after.score - item.before.score;
        }

        let cells: std::collections::BTreeSet<&String> =
            before_counts.keys().chain(after_counts.keys()).collect();
        let mut cell_deltas: Vec<CellDelta> = cells
            .into_iter()
            .map(|cell| {
                let b = before_counts.get(cell).copied().unwrap_or(0);
                let a = after_counts.get(cell).copied().unwrap_or(0);
                CellDelta {
                    cell: cell.clone(),
                    before_count: b,
                    after_count: a,
                    delta: a as i64 - b as i64,
                }
            })
            .filter(|d| d.delta != 0)
            .collect();
        cell_deltas.sort_by_key(|d| std::cmp::Reverse(d.delta.abs()));

        Self {
            newly_covered,
            newly_uncovered,
            cell_deltas,
            mean_confidence_delta: if sim.items.is_empty() {
                0.0
            } else {
                total_delta / sim.items.len() as f32
            },
        }
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

    #[test]
    fn impact_derives_coverage_and_changed_aggregates() {
        let sim = simulate(
            "procurement-v2",
            vec![
                // Confident move: covered both sides, aggregate shifts.
                item("1", ("OPERATE", "MONITOR", 0.91), ("OPERATE", "CORRECT", 0.84)),
                // Falls out of coverage: the candidate opens a gap here.
                item("2", ("OPERATE", "MONITOR", 0.88), ("STUDY", "WORK", 0.20)),
                // Gains coverage the base did not have.
                item("3", ("BOND", "PLAY", 0.30), ("BOND", "PLAY", 0.77)),
                // Unchanged, confident.
                item("4", ("HEAL", "REST", 0.90), ("HEAL", "REST", 0.91)),
            ],
            0.60,
        );

        let impact = ImpactAnalysis::derive(&sim, 0.60);

        assert_eq!(impact.newly_covered, vec!["3".to_string()]);
        assert_eq!(impact.newly_uncovered, vec!["2".to_string()]);

        // Both items leave OPERATE×MONITOR (one to CORRECT, one out of
        // coverage), so the cell empties: that is a report line at zero.
        let monitor = impact
            .cell_deltas
            .iter()
            .find(|d| d.cell == "OPERATE×MONITOR")
            .expect("monitor cell delta present");
        assert_eq!((monitor.before_count, monitor.after_count), (2, 0));

        // Sorted by absolute movement, largest first.
        assert!(
            impact
                .cell_deltas
                .first()
                .map(|d| d.delta.abs())
                .unwrap_or(0)
                >= impact
                    .cell_deltas
                    .last()
                    .map(|d| d.delta.abs())
                    .unwrap_or(0)
        );
    }

    #[test]
    fn unknown_never_counts_as_covered() {
        let sim = simulate(
            "s",
            vec![item(
                "1",
                ("OPERATE", "WORK", 0.99),
                ("unknown", "unknown", 0.99),
            )],
            0.6,
        );
        let impact = ImpactAnalysis::derive(&sim, 0.6);
        assert_eq!(impact.newly_uncovered, vec!["1".to_string()]);
        assert!(impact.newly_covered.is_empty());
    }
}
