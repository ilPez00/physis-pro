//! Phase 6.4 — weekly management pack (PLAN 6.4).
//!
//! One deterministic Markdown document a manager actually reads: headline
//! counts, exceptions needing decisions, reconciliation status, and provenance
//! for every number. Composes the existing cores (review desk, batch CSV,
//! reconcile) — no new analysis, just aggregation + rendering.
//!
//! Deterministic: same inputs → byte-identical output, so the pack can be
//! diffed week over week and generated in tests without fixtures.

use crate::reconcile::{MatchStatus, ReconciledLine};
use crate::review_desk::ReviewDeskReport;

/// Everything the pack needs, already computed by the caller.
#[derive(Debug, Clone, Default)]
pub struct PackInput<'a> {
    pub week_label: &'a str,
    pub plant: &'a str,
    /// Review-desk pass over this week's documents.
    pub desk: Option<&'a ReviewDeskReport>,
    /// Reconciliation lines from 6.3.
    pub recon: &'a [ReconciledLine],
    /// Free-text operational notes, newest first.
    pub notes: &'a [String],
}

/// Build the weekly pack. Every section carries its own provenance line.
pub fn render_weekly_pack(input: &PackInput) -> String {
    let mut out = String::with_capacity(2048);
    out.push_str(&format!(
        "# Weekly Management Pack — {} ({})\n\n",
        input.week_label, input.plant
    ));

    // ── headline ──────────────────────────────────────────────────────────
    let docs = input.desk.map(|d| d.documents).unwrap_or(0);
    let exceptions = input.desk.map(|d| d.exceptions.len()).unwrap_or(0);
    let unmatched = input.recon.iter().filter(|l| l.status != MatchStatus::Matched).count();
    let matched = input.recon.len() - unmatched;
    out.push_str("## Headline\n\n");
    out.push_str(&format!(
        "| Metric | Value |\n|---|---|\n| Documents processed | {docs} |\n\
         | Exceptions raised | {exceptions} |\n| Invoices fully matched | {matched} |\n\
         | Reconciliation items needing action | {unmatched} |\n\n"
    ));
    out.push_str("_Source: document ingest + review desk + 3-way reconciliation._\n\n");

    // ── exceptions ────────────────────────────────────────────────────────
    out.push_str("## Exceptions needing a decision\n\n");
    match input.desk {
        Some(d) if !d.exceptions.is_empty() => {
            out.push_str("| Reason | Document |\n|---|---|\n");
            for e in &d.exceptions {
                out.push_str(&format!("| {} | `{}` |\n", e.reason, e.path));
            }
            out.push('\n');
            out.push_str(&format!(
                "_{} row(s); full queue in review-desk output._\n\n",
                d.exceptions.len()
            ));
        }
        _ => out.push_str("None.\n\n"),
    }

    // ── reconciliation ────────────────────────────────────────────────────
    out.push_str("## Reconciliation status\n\n");
    if input.recon.is_empty() {
        out.push_str("No invoices in scope this week.\n\n");
    } else {
        let summary = crate::reconcile::summarize(input.recon);
        out.push_str("| Status | Count |\n|---|---|\n");
        for (status, count) in summary {
            out.push_str(&format!("| {status} | {count} |\n"));
        }
        out.push('\n');
        // Detail only for actionable rows.
        let actionable: Vec<&ReconciledLine> = input
            .recon
            .iter()
            .filter(|l| l.status != MatchStatus::Matched)
            .collect();
        if !actionable.is_empty() {
            out.push_str("Actionable detail:\n\n");
            for l in actionable {
                out.push_str(&format!(
                    "- `{}` ({}): {}\n",
                    l.invoice_number, l.po_number, l.detail
                ));
            }
            out.push('\n');
        }
    }

    // ── notes ─────────────────────────────────────────────────────────────
    out.push_str("## Operational notes\n\n");
    if input.notes.is_empty() {
        out.push_str("None recorded.\n");
    } else {
        for n in input.notes {
            out.push_str(&format!("- {n}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review_desk::Exception;

    fn sample_desk() -> ReviewDeskReport {
        ReviewDeskReport {
            documents: 12,
            duplicate_groups: vec![],
            exceptions: vec![Exception {
                doc_id: "/in/inv9.pdf".into(),
                path: "/in/inv9.pdf".into(),
                kind: "Invoice".into(),
                reason: "missing_vendor".into(),
                detail: "x".into(),
            }],
        }
    }

    fn sample_recon() -> Vec<ReconciledLine> {
        vec![
            ReconciledLine {
                invoice_number: "INV-1".into(),
                vendor: "ACME".into(),
                po_number: "PO-1".into(),
                status: MatchStatus::Matched,
                qty_billed: 1.0,
                qty_received: 1.0,
                amount_expected_eur: 100.0,
                amount_billed_eur: 100.0,
                detail: String::new(),
            },
            ReconciledLine {
                invoice_number: "INV-2".into(),
                vendor: "BETA".into(),
                po_number: "PO-2".into(),
                status: MatchStatus::AmountMismatch,
                qty_billed: 1.0,
                qty_received: 1.0,
                amount_expected_eur: 50.0,
                amount_billed_eur: 90.0,
                detail: "invoice 90.00 vs expected 50.00".into(),
            },
        ]
    }

    #[test]
    fn pack_renders_all_sections_with_provenance() {
        let desk = sample_desk();
        let recon = sample_recon();
        let notes = vec!["Compressor C-2 bearing replaced.".to_string()];
        let md = render_weekly_pack(&PackInput {
            week_label: "2026-W35",
            plant: "Verona",
            desk: Some(&desk),
            recon: &recon,
            notes: &notes,
        });
        assert!(md.contains("# Weekly Management Pack — 2026-W35 (Verona)"));
        assert!(md.contains("| Documents processed | 12 |"));
        assert!(md.contains("| Exceptions raised | 1 |"));
        assert!(md.contains("missing_vendor"));
        assert!(md.contains("_Source:"));
        assert!(md.contains("INV-2") && md.contains("Actionable detail"));
        assert!(md.contains("- Compressor C-2 bearing replaced."));
    }

    #[test]
    fn deterministic_byte_identical_output() {
        let mk = || {
            let desk = sample_desk();
            let recon = sample_recon();
            render_weekly_pack(&PackInput {
                week_label: "W",
                plant: "P",
                desk: Some(&desk),
                recon: &recon,
                notes: &[],
            })
        };
        assert_eq!(mk(), mk());
    }

    #[test]
    fn empty_inputs_render_clean_not_broken() {
        let md = render_weekly_pack(&PackInput {
            week_label: "W0",
            plant: "P0",
            desk: None,
            recon: &[],
            notes: &[],
        });
        assert!(md.contains("Exceptions needing a decision") && md.contains("None."));
        assert!(md.contains("No invoices in scope this week."));
    }
}
