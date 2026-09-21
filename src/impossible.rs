//! Impossible-machine run report: graded machines, propositions,
//! disagreements, verdict — read from the run artifact, never estimated.
//!
//! The artifact is `results.json` under
//! `physis-core/research/impossible_machine/experiments/` (or
//! `research/impossible_machine/experiments/` when the HUD opens on the core
//! checkout directly). Claim and grade travel together through
//! `physis_core::machines` views; a missing artifact is a display gap, and
//! the text says so instead of inventing a report.

use std::path::{Path, PathBuf};

/// Candidate artifact directories for a workspace root.
fn experiment_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![
        root.join("physis-core/research/impossible_machine/experiments"),
        root.join("research/impossible_machine/experiments"),
    ];
    dirs.sort();
    dirs.dedup();
    dirs
}

/// Shorten a claim for one HUD line: characters, never bytes, so a Σ or ψ
/// is not cut in half.
fn short(s: &str, max: usize) -> String {
    let mut out: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        out.push('…');
    }
    out
}

/// Read-only report text for the HUD machine view. Baked-in nothing: without
/// the artifact there is no report.
pub fn report_text(root: &Path) -> String {
    let mut o = String::new();
    o.push_str("── IMPOSSIBLE MACHINE (read-only — not run) ──\n");
    o.push_str("question: does the small-gap prime deviation carry 2C2 = 1.32032?\n");
    o.push_str("layers: EXACT certifies · NUMERICAL certifies (error-bounded) · HEURISTIC never certifies · CONJECTURAL never certifies\n");
    let mut live = false;
    for dir in experiment_dirs(root) {
        let r = dir.join("results.json");
        let Ok(rep) = physis_core::machines::ImpossibleReport::load(&r) else {
            continue;
        };
        live = true;
        let names: Vec<&str> = rep.machines.iter().map(|m| m.name.as_str()).collect();
        o.push_str(&format!(
            "machines ({} reports): {}\n",
            rep.machines.len(),
            names.join(", ")
        ));
        for m in &rep.machines {
            o.push_str(&format!("  {} — {}\n", m.name, short(&m.operation, 90)));
            for ob in &m.observations {
                o.push_str(&format!(
                    "    [{:?}] {} — {}\n",
                    ob.status,
                    ob.target,
                    short(&ob.claim, 110)
                ));
            }
            if !m.failure_mode.is_empty() {
                o.push_str(&format!("    fails as: {}\n", short(&m.failure_mode, 110)));
            }
        }
        if !rep.propositions.is_empty() {
            o.push_str(&format!("propositions ({}):\n", rep.propositions.len()));
            for p in &rep.propositions {
                o.push_str(&format!(
                    "  [{:?}] {} — {}\n",
                    p.status,
                    p.id,
                    short(&p.claim, 100)
                ));
            }
        }
        for d in &rep.disagreements {
            o.push_str(&format!(
                "disagreement: {} vs {} on {}: {} / {}\n",
                d.a,
                d.b,
                d.subject,
                short(&d.a_claim, 60),
                short(&d.b_claim, 60)
            ));
        }
        if !rep.verdict.is_empty() {
            o.push_str(&format!("verdict: {}\n", short(&rep.verdict, 160)));
        }
    }
    if !live {
        o.push_str("(no results.json under this workspace — open the physis checkout to see the run)\n");
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Without the artifact there is no report — and the text says so.
    #[test]
    fn without_artifact_names_the_gap() {
        let dir = tempfile::tempdir().unwrap();
        let o = report_text(dir.path());
        assert!(o.contains("2C2 = 1.32032"), "{o}");
        assert!(o.contains("HEURISTIC never certifies"), "{o}");
        assert!(o.contains("no results.json"), "{o}");
    }

    /// A run artifact renders graded machines, propositions, disagreements
    /// and the verdict — grades attached, nothing invented.
    #[test]
    fn with_artifact_renders_grades() {
        let dir = tempfile::tempdir().unwrap();
        let exp = dir.path().join("research/impossible_machine/experiments");
        std::fs::create_dir_all(&exp).unwrap();
        std::fs::write(
            exp.join("results.json"),
            r#"{
                "machine_reports": [{
                    "name": "euclid", "operation": "sieve",
                    "failure_mode": "RH invisible", "physis_equivalent": "EXACT",
                    "observations": [
                        {"machine": "euclid", "target": "psi-exact",
                         "claim": "ψ constructed exactly", "quantity": 0.0,
                         "status": "EstablishedFinite"}
                    ]
                }],
                "propositions": [
                    {"id": "P9", "machines": ["empedocles"],
                     "claim": "|M(x)| < √x", "status": "Contradicted",
                     "witness": "Odlyzko–te Riele", "quarantined": false}
                ],
                "disagreements": [
                    {"a": "euclid", "b": "nous", "subject": "visibility",
                     "a_claim": "invisible", "b_claim": "visible"}
                ],
                "verdict": "RH REDUCED TO A NEW EXPLICIT PROBLEM",
                "verdict_witness": "seven stances"
            }"#,
        )
        .unwrap();
        let o = report_text(dir.path());
        assert!(o.contains("machines (1 reports): euclid"), "{o}");
        assert!(o.contains("[EstablishedFinite] psi-exact"), "{o}");
        assert!(o.contains("[Contradicted] P9"), "{o}");
        assert!(o.contains("euclid vs nous on visibility"), "{o}");
        assert!(o.contains("RH REDUCED"), "{o}");
    }
}
