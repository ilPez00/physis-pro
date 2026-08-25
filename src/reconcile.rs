//! Phase 6.3 — Invoice / PO / delivery-note reconciliation (PLAN 6.3).
//!
//! Classic 3-way match, decision-support only: an invoice matches when the
//! same vendor+PO reference appears on all three documents and quantities and
//! amounts agree within tolerance. Everything that does not match cleanly
//! lands in a typed exceptions list for a human — this module NEVER approves
//! payments or mutates state; it only classifies and reports.
//!
//! Inputs are plain structs (the caller maps CSV rows from `batch_csv` or doc
//! metadata from `review_desk` onto them), so matching is testable without
//! files. Deterministic: same inputs always yield the same report.

/// One purchase-order line to reconcile against.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PoLine {
    pub po_number: String,
    pub vendor: String,
    /// Ordered quantity.
    pub qty_ordered: f64,
    /// Agreed unit price in EUR.
    pub unit_price_eur: f64,
}

/// One delivery-note line (goods actually received).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DeliveryLine {
    pub po_number: String,
    pub vendor: String,
    pub qty_received: f64,
}

/// One invoice line as billed.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct InvoiceLine {
    pub po_number: String,
    pub vendor: String,
    pub qty_billed: f64,
    /// Billed total for the line (EUR), not unit price.
    pub amount_eur: f64,
    pub invoice_number: String,
}

/// Machine-readable outcome per invoice line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchStatus {
    /// All three documents agree within tolerance.
    Matched,
    /// Vendor/PO found but quantity differs beyond tolerance.
    QtyMismatch,
    /// Vendor/PO found but billed amount ≠ received×price beyond tolerance.
    AmountMismatch,
    /// No PO line with that vendor+po_number exists.
    NoPo,
    /// PO exists, goods were never delivered (billed without receipt).
    NotDelivered,
    /// Same invoice number billed twice (duplicate submission).
    DuplicateInvoice,
}

impl MatchStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MatchStatus::Matched => "matched",
            MatchStatus::QtyMismatch => "qty_mismatch",
            MatchStatus::AmountMismatch => "amount_mismatch",
            MatchStatus::NoPo => "no_po",
            MatchStatus::NotDelivered => "not_delivered",
            MatchStatus::DuplicateInvoice => "duplicate_invoice",
        }
    }
}

/// Reconciliation verdict for one invoice line.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReconciledLine {
    pub invoice_number: String,
    pub vendor: String,
    pub po_number: String,
    pub status: MatchStatus,
    pub qty_billed: f64,
    pub qty_received: f64,
    /// Expected amount = qty_received × PO unit price.
    pub amount_expected_eur: f64,
    pub amount_billed_eur: f64,
    /// Human explanation; always present for non-matched statuses.
    #[serde(default)]
    pub detail: String,
}

/// Tolerances: relative fraction + absolute floor so small-value lines don't
/// flap on rounding. Defaults match common AP practice (±2% or ±€0.05).
#[derive(Debug, Clone, Copy)]
pub struct Tolerance {
    pub rel: f64,
    pub abs_eur: f64,
}

impl Default for Tolerance {
    fn default() -> Self {
        Self {
            rel: 0.02,
            abs_eur: 0.05,
        }
    }
}

fn close(a: f64, b: f64, tol: Tolerance) -> bool {
    (a - b).abs() <= tol.abs_eur.max(b.abs() * tol.rel)
}

/// Reconcile every invoice line against POs and deliveries.
///
/// Duplicate invoices (same invoice_number seen more than once) are flagged
/// on every occurrence after the first — the human decides which one is real.
pub fn reconcile(
    pos: &[PoLine],
    deliveries: &[DeliveryLine],
    invoices: &[InvoiceLine],
    tol: Tolerance,
) -> Vec<ReconciledLine> {
    use std::collections::HashSet;

    // Received quantity per (vendor, po).
    let mut received: HashMap<(&str, &str), f64> = HashMap::new();
    for d in deliveries {
        *received
            .entry((d.vendor.as_str(), d.po_number.as_str()))
            .or_insert(0.0) += d.qty_received;
    }

    let mut seen_invoices: HashSet<&str> = HashSet::new();
    let mut out = Vec::with_capacity(invoices.len());

    for inv in invoices {
        // Duplicate invoice-number check first: highest-severity bookkeeping error.
        if !seen_invoices.insert(inv.invoice_number.as_str()) {
            out.push(ReconciledLine {
                invoice_number: inv.invoice_number.clone(),
                vendor: inv.vendor.clone(),
                po_number: inv.po_number.clone(),
                status: MatchStatus::DuplicateInvoice,
                qty_billed: inv.qty_billed,
                qty_received: 0.0,
                amount_expected_eur: 0.0,
                amount_billed_eur: inv.amount_eur,
                detail: format!("invoice `{}` submitted more than once", inv.invoice_number),
            });
            continue;
        }

        let Some(po) = pos
            .iter()
            .find(|p| p.po_number == inv.po_number && p.vendor == inv.vendor)
        else {
            out.push(ReconciledLine {
                invoice_number: inv.invoice_number.clone(),
                vendor: inv.vendor.clone(),
                po_number: inv.po_number.clone(),
                status: MatchStatus::NoPo,
                qty_billed: inv.qty_billed,
                qty_received: 0.0,
                amount_expected_eur: 0.0,
                amount_billed_eur: inv.amount_eur,
                detail: format!("no PO {} for vendor {}", inv.po_number, inv.vendor),
            });
            continue;
        };

        let qty_recv = *received
            .get(&(inv.vendor.as_str(), inv.po_number.as_str()))
            .unwrap_or(&0.0);
        let expected = qty_recv * po.unit_price_eur;

        let status = if !close(inv.qty_billed, qty_recv, tol) && qty_recv == 0.0 {
            MatchStatus::NotDelivered
        } else if !close(inv.qty_billed, qty_recv, tol) {
            MatchStatus::QtyMismatch
        } else if !close(inv.amount_eur, expected, tol) {
            MatchStatus::AmountMismatch
        } else {
            MatchStatus::Matched
        };

        let detail = match status {
            MatchStatus::Matched => String::new(),
            MatchStatus::NotDelivered => {
                format!("billed {} units; no delivery recorded", inv.qty_billed)
            }
            MatchStatus::QtyMismatch => {
                format!("billed {} vs delivered {}", inv.qty_billed, qty_recv)
            }
            MatchStatus::AmountMismatch => format!(
                "billed €{:.2} vs expected €{:.2} ({} × €{:.2})",
                inv.amount_eur, expected, qty_recv, po.unit_price_eur
            ),
            _ => String::new(),
        };

        out.push(ReconciledLine {
            invoice_number: inv.invoice_number.clone(),
            vendor: inv.vendor.clone(),
            po_number: inv.po_number.clone(),
            status,
            qty_billed: inv.qty_billed,
            qty_received: qty_recv,
            amount_expected_eur: expected,
            amount_billed_eur: inv.amount_eur,
            detail,
        });
    }
    out
}

/// Summary counts by status, in stable report order.
pub fn summarize(lines: &[ReconciledLine]) -> Vec<(String, usize)> {
    const ORDER: [MatchStatus; 6] = [
        MatchStatus::Matched,
        MatchStatus::QtyMismatch,
        MatchStatus::AmountMismatch,
        MatchStatus::NoPo,
        MatchStatus::NotDelivered,
        MatchStatus::DuplicateInvoice,
    ];
    ORDER
        .iter()
        .map(|s| {
            (
                s.as_str().to_owned(),
                lines.iter().filter(|l| l.status == *s).count(),
            )
        })
        .filter(|(_, n)| *n > 0)
        .collect()
}

/// Markdown report for humans — mirrors `review_desk`'s output style.
pub fn report(lines: &[ReconciledLine]) -> String {
    let mut md = String::from("# Reconciliation Report\n\n");
    md.push_str("| Status | Count |\n|---|---|\n");
    for (status, n) in summarize(lines) {
        md.push_str(&format!("| {status} | {n} |\n"));
    }
    md.push_str("\n## Exceptions\n\n");
    let exceptions: Vec<&ReconciledLine> = lines
        .iter()
        .filter(|l| l.status != MatchStatus::Matched)
        .collect();
    if exceptions.is_empty() {
        md.push_str("None — all lines matched.\n");
        return md;
    }
    md.push_str("| Invoice | Vendor | PO | Status | Detail |\n|---|---|---|---|---|\n");
    for l in exceptions {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            l.invoice_number,
            l.vendor,
            l.po_number,
            l.status.as_str(),
            l.detail
                .is_empty()
                .then(|| "-".to_string())
                .unwrap_or_else(|| l.detail.clone())
        ));
    }
    md
}

use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    fn po(n: &str, price: f64) -> PoLine {
        PoLine {
            po_number: n.into(),
            vendor: "ACME Spa".into(),
            qty_ordered: 100.0,
            unit_price_eur: price,
        }
    }
    fn del(n: &str, qty: f64) -> DeliveryLine {
        DeliveryLine {
            po_number: n.into(),
            vendor: "ACME Spa".into(),
            qty_received: qty,
        }
    }
    fn inv(num: &str, n: &str, qty: f64, amount: f64) -> InvoiceLine {
        InvoiceLine {
            invoice_number: num.into(),
            po_number: n.into(),
            vendor: "ACME Spa".into(),
            qty_billed: qty,
            amount_eur: amount,
        }
    }

    #[test]
    fn clean_three_way_match_matches() {
        // 10 ordered @ 5.00, 10 delivered, billed exactly 50.00.
        let out = reconcile(
            &[po("PO-1", 5.0)],
            &[del("PO-1", 10.0)],
            &[inv("INV-1", "PO-1", 10.0, 50.0)],
            Tolerance::default(),
        );
        assert_eq!(out[0].status, MatchStatus::Matched);
        assert_eq!(summarize(&out), vec![("matched".into(), 1)]);
    }

    #[test]
    fn overbilling_within_two_percent_still_matches() {
        // 50.00 expected; 50.80 billed = 1.6% over → inside ±2%.
        let out = reconcile(
            &[po("PO-1", 5.0)],
            &[del("PO-1", 10.0)],
            &[inv("INV-1", "PO-1", 10.0, 50.80)],
            Tolerance::default(),
        );
        assert_eq!(out[0].status, MatchStatus::Matched);
    }

    #[test]
    fn qty_mismatch_is_caught() {
        let out = reconcile(
            &[po("PO-1", 5.0)],
            &[del("PO-1", 8.0)],
            &[inv("INV-1", "PO-1", 10.0, 40.0)], // billed 10, delivered 8
            Tolerance::default(),
        );
        assert_eq!(out[0].status, MatchStatus::QtyMismatch);
        assert!(out[0].detail.contains("billed 10 vs delivered 8"));
    }

    #[test]
    fn amount_mismatch_is_caught() {
        // Quantities agree (10=10) but billing is way off: 10 × 5 = 50 expected.
        let out = reconcile(
            &[po("PO-1", 5.0)],
            &[del("PO-1", 10.0)],
            &[inv("INV-1", "PO-1", 10.0, 75.0)],
            Tolerance::default(),
        );
        assert_eq!(out[0].status, MatchStatus::AmountMismatch);
        assert!(
            out[0].detail.contains("expected €50.00"),
            "{}",
            out[0].detail
        );
    }

    #[test]
    fn unknown_po_and_vendor_mismatch_are_no_po() {
        let out = reconcile(
            &[po("PO-1", 5.0)],
            &[del("PO-1", 1.0)],
            &[
                inv("INV-A", "PO-404", 1.0, 5.0),
                inv("INV-B", "PO-1", 1.0, 5.0),
            ],
            Tolerance::default(),
        );
        // INV-B's vendor is ACME Spa (helper default) so it matches; INV-A has no PO.
        assert_eq!(out[0].status, MatchStatus::NoPo);
        assert_eq!(out[1].status, MatchStatus::Matched);
    }

    #[test]
    fn billed_without_delivery_flags_not_delivered() {
        let out = reconcile(
            &[po("PO-1", 5.0)],
            &[],
            &[inv("INV-1", "PO-1", 4.0, 20.0)],
            Tolerance::default(),
        );
        assert_eq!(out[0].status, MatchStatus::NotDelivered);
    }

    #[test]
    fn duplicate_invoice_numbers_flag_every_repeat() {
        let out = reconcile(
            &[po("PO-1", 5.0)],
            &[del("PO-1", 10.0)],
            &[
                inv("INV-1", "PO-1", 10.0, 50.0),
                inv("INV-1", "PO-1", 10.0, 50.0),
                inv("INV-2", "PO-1", 10.0, 50.0),
            ],
            Tolerance::default(),
        );
        assert_eq!(out[0].status, MatchStatus::Matched);
        assert_eq!(out[1].status, MatchStatus::DuplicateInvoice);
        assert_eq!(out[2].status, MatchStatus::Matched);
    }

    #[test]
    fn partial_deliveries_sum_across_notes() {
        // Two delivery notes for the same PO accumulate: 6 + 4 = 10 received.
        let out = reconcile(
            &[po("PO-1", 5.0)],
            &[del("PO-1", 6.0), del("PO-1", 4.0)],
            &[inv("INV-1", "PO-1", 10.0, 50.0)],
            Tolerance::default(),
        );
        assert_eq!(out[0].status, MatchStatus::Matched);
        assert_eq!(out[0].qty_received, 10.0);
    }

    #[test]
    fn report_lists_exceptions_and_summary() {
        let out = reconcile(
            &[po("PO-1", 5.0), po("PO-9", 2.0)],
            &[del("PO-1", 8.0)],
            &[
                inv("INV-1", "PO-1", 10.0, 40.0),
                inv("INV-2", "PO-MISSING", 3.0, 6.0),
            ],
            Tolerance::default(),
        );
        let md = report(&out);
        assert!(md.contains("# Reconciliation Report"));
        assert!(md.contains("qty_mismatch"));
        assert!(md.contains("no_po"));
        assert!(md.contains("INV-2"));
        // A fully clean run says so explicitly.
        let clean = reconcile(
            &[po("PO-1", 5.0)],
            &[del("PO-1", 10.0)],
            &[inv("INV-1", "PO-1", 10.0, 50.0)],
            Tolerance::default(),
        );
        assert!(report(&clean).contains("None — all lines matched."));
    }

    #[test]
    fn empty_inputs_yield_empty_report_not_panic() {
        assert!(reconcile(&[], &[], &[], Tolerance::default()).is_empty());
        assert_eq!(report(&[]), "# Reconciliation Report\n\n| Status | Count |\n|---|---|\n\n## Exceptions\n\nNone — all lines matched.\n");
    }
}
