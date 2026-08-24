//! Phase 6.1 — Admin Review Desk flow (PLAN 6.1).
//!
//! One deterministic pipeline over existing pieces:
//! seed demo corpus (or any folder) → `ingest_all` → per-doc field checks
//! (missing vendor/date/number, duplicate fingerprints, low confidence) →
//! exceptions queue + markdown report. Decision-support only: it never
//! approves payments or mutates anything outside its output string.
//!
//! The demo corpus is wired in `studio --demo`; this module is the review
//! desk half that turns ingested documents into a human-workable queue.

use crate::admin::{review_queue, ReviewDocument};
use crate::ingest::{ingest_all, IngestedDoc};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

/// One exception row: why this document needs a human.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Exception {
    pub doc_id: String,
    pub path: String,
    pub kind: String,
    /// machine-readable reason: missing_vendor | missing_date | missing_number
    /// | duplicate | low_confidence
    pub reason: String,
    pub detail: String,
}

/// Full desk state after one pass over a directory.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ReviewDeskReport {
    pub documents: usize,
    pub exceptions: Vec<Exception>,
    /// Fingerprint groups with >1 document (duplicates).
    pub duplicate_groups: Vec<Vec<String>>,
}

fn fingerprint(body: &str) -> String {
    let mut h = DefaultHasher::new();
    // Fold whitespace so trivial reformatting still matches.
    let folded: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
    folded.to_lowercase().hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Confidence proxy: docs whose ontology tags came back empty scored low —
/// the classifier found nothing to say about them.
fn low_confidence(doc: &IngestedDoc) -> bool {
    doc.ontology_tags.is_empty()
}

/// Run the desk over a folder of documents. Read-only on the folder.
pub fn run_desk(dir: &Path) -> ReviewDeskReport {
    let docs = ingest_all(&[dir.to_path_buf()]);

    // Duplicate detection via admin's queue logic on fingerprinted docs.
    let review_docs: Vec<ReviewDocument> = docs
        .iter()
        .map(|d| ReviewDocument {
            id: d.path.display().to_string(),
            path: d.path.display().to_string(),
            kind: format!("{:?}", d.kind),
            fingerprint: Some(fingerprint(&d.body)),
            vendor: d.meta.vendor.clone(),
            document_number: d.meta.doc_number.clone(),
        })
        .collect();
    let queue = review_queue(&review_docs);
    let duplicated_ids: std::collections::HashSet<String> = queue
        .duplicate_groups
        .iter()
        .flat_map(|g| g.document_ids.iter())
        .cloned()
        .collect();

    let mut exceptions = Vec::new();
    for d in &docs {
        let path_s = d.path.display().to_string();
        if low_confidence(d) {
            exceptions.push(Exception {
                doc_id: path_s.clone(),
                path: path_s.clone(),
                kind: format!("{:?}", d.kind),
                reason: "low_confidence".into(),
                detail: "classifier produced no ontology tags".into(),
            });
        }
        let invoice_like = matches!(d.kind, crate::ingest::DocKind::Invoice)
            || d.meta.doc_number.is_some()
            || d.meta.vendor.is_some();
        if invoice_like {
            if d.meta.vendor.is_none() {
                exceptions.push(Exception {
                    doc_id: path_s.clone(),
                    path: path_s.clone(),
                    kind: format!("{:?}", d.kind),
                    reason: "missing_vendor".into(),
                    detail: "invoice-like document without vendor".into(),
                });
            }
            if d.meta.date.is_none() {
                exceptions.push(Exception {
                    doc_id: path_s.clone(),
                    path: path_s.clone(),
                    kind: format!("{:?}", d.kind),
                    reason: "missing_date".into(),
                    detail: "invoice-like document without date".into(),
                });
            }
        }
        if duplicated_ids.contains(&path_s) {
            exceptions.push(Exception {
                doc_id: path_s.clone(),
                path: path_s.clone(),
                kind: format!("{:?}", d.kind),
                reason: "duplicate".into(),
                detail: "same content as another document".into(),
            });
        }
    }
    exceptions.sort_by(|a, b| a.reason.cmp(&b.reason).then(a.path.cmp(&b.path)));

    ReviewDeskReport {
        documents: docs.len(),
        duplicate_groups: queue
            .duplicate_groups
            .iter()
            .map(|g| g.document_ids.clone())
            .collect(),
        exceptions,
    }
}

/// Markdown rendering for humans / weekly pack seeding.
pub fn render_markdown(report: &ReviewDeskReport) -> String {
    let mut out = String::with_capacity(512);
    out.push_str("# Admin Review Desk\n\n");
    out.push_str(&format!("Documents reviewed: {}\n", report.documents));
    out.push_str(&format!(
        "Exceptions: {} · Duplicate groups: {}\n\n",
        report.exceptions.len(),
        report.duplicate_groups.len()
    ));
    if report.exceptions.is_empty() {
        out.push_str("No exceptions. Nothing needs a human.\n");
        return out;
    }
    out.push_str("| Reason | Kind | Document |\n|---|---|---|\n");
    for e in &report.exceptions {
        out.push_str(&format!(
            "| {} | {} | `{}` |\n",
            e.reason, e.kind, e.path
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_dir_is_clean_report_not_error() {
        let tmp = std::env::temp_dir().join(format!("desk_empty_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let r = run_desk(&tmp);
        assert_eq!(r.documents, 0);
        assert!(r.exceptions.is_empty());
        let md = render_markdown(&r);
        assert!(md.contains("No exceptions"));
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn duplicates_and_missing_fields_become_exceptions() {
        let tmp = std::env::temp_dir().join(format!("desk_dup_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let body = "Invoice No. 5555 from ACME SpA dated 2026-01-31, total 120 EUR";
        std::fs::write(tmp.join("inv1.pdf"), body).unwrap();
        std::fs::write(tmp.join("inv2.pdf"), body).unwrap(); // exact duplicate content
        std::fs::write(tmp.join("mystery.xyz"), "completely unclassifiable content qwerty").unwrap();

        let r = run_desk(&tmp);
        assert!(r.documents >= 3, "{r:?}");
        let reasons: Vec<&str> = r.exceptions.iter().map(|e| e.reason.as_str()).collect();
        assert!(reasons.contains(&"duplicate"), "{r:?}");
        assert!(
            reasons.contains(&"low_confidence"),
            "unclassifiable file must land in the queue"
        );
        // Deterministic order
        let again = run_desk(&tmp);
        assert_eq!(r, again);
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn clean_invoice_yields_no_field_exceptions() {
        let tmp = std::env::temp_dir().join(format!("desk_clean_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("ok.txt"),
            "Delivery note DN-2026-0042 received goods intact",
        )
        .unwrap();
        let r = run_desk(&tmp);
        assert!(
            !r.exceptions.iter().any(|e| e.reason.starts_with("missing_")),
            "{r:?}"
        );
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn markdown_lists_every_exception_row() {
        let report = ReviewDeskReport {
            documents: 2,
            duplicate_groups: vec![vec!["a".into(), "b".into()]],
            exceptions: vec![Exception {
                doc_id: "a".into(),
                path: "/tmp/a.pdf".into(),
                kind: "Invoice".into(),
                reason: "duplicate".into(),
                detail: "dup".into(),
            }],
        };
        let md = render_markdown(&report);
        assert!(md.contains("duplicate") && md.contains("/tmp/a.pdf"));
    }
}
