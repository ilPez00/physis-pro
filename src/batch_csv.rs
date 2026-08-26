//! Phase 6.2 — CSV batch import with validation + reviewed export (PLAN 6.2).
//!
//! Schema-validated CSV rows in, per-row errors out, reviewed rows export
//! back to CSV. Deterministic and dependency-light: the `csv` crate behind
//! the existing `ingest` feature does parsing; validation is plain data
//! checks so bad rows never abort good ones.
//!
//! Wire shape matches the review-desk philosophy: decision support, explicit
//! row-level errors, nothing silently dropped.

/// One CSV row after validation.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct BatchRow {
    pub line: usize,
    pub vendor: String,
    pub doc_number: String,
    pub date: String,
    pub amount_eur: f64,
    /// "ok" or one of the validation reason codes.
    pub status: String,
    /// Human explanation when status != "ok".
    #[serde(default)]
    pub error: Option<String>,
}

/// Import + validate CSV text. Header required:
/// `vendor,doc_number,date,amount_eur`.
pub fn import_csv(text: &str) -> Vec<BatchRow> {
    let mut out = Vec::new();
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(text.as_bytes());
    for (i, rec) in rdr.records().enumerate() {
        let line = i + 2; // header is line 1
        let rec = match rec {
            Ok(r) => r,
            Err(e) => {
                out.push(BatchRow {
                    line,
                    vendor: String::new(),
                    doc_number: String::new(),
                    date: String::new(),
                    amount_eur: 0.0,
                    status: "malformed".into(),
                    error: Some(format!("unparseable CSV record: {e}")),
                });
                continue;
            }
        };
        let vendor = rec.get(0).unwrap_or("").trim().to_string();
        let doc_number = rec.get(1).unwrap_or("").trim().to_string();
        let date = rec.get(2).unwrap_or("").trim().to_string();
        let amount_raw = rec.get(3).unwrap_or("").trim().to_string();

        let mut status = "ok".to_string();
        let mut error: Option<String> = None;

        if vendor.is_empty() || doc_number.is_empty() || date.is_empty() {
            status = "missing_field".into();
            let missing = [
                ("vendor", vendor.is_empty()),
                ("doc_number", doc_number.is_empty()),
                ("date", date.is_empty()),
            ]
            .iter()
            .filter(|(_, m)| *m)
            .map(|(n, _)| *n)
            .collect::<Vec<_>>()
            .join(",");
            error = Some(format!("required fields empty: {missing}"));
        } else if chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").is_err() {
            status = "bad_date".into();
            error = Some(format!("date `{date}` not ISO YYYY-MM-DD"));
        }

        let amount_eur = parse_amount(&amount_raw);
        if amount_eur.is_none() {
            status = if status == "ok" {
                "bad_amount".into()
            } else {
                status
            };
            error = Some(format!(
                "amount `{amount_raw}` not numeric; {}",
                error.unwrap_or_default()
            ));
        }

        out.push(BatchRow {
            line,
            vendor,
            doc_number,
            date,
            amount_eur: amount_eur.unwrap_or(0.0),
            status,
            error,
        });
    }
    out
}

/// Only rows whose status is exactly `ok`.
pub fn valid_rows(rows: &[BatchRow]) -> Vec<&BatchRow> {
    rows.iter().filter(|r| r.status == "ok").collect()
}

/// Export rows back to CSV with a review column. Round-trips through
/// `import_csv`'s sibling importer on the customer side.
pub fn export_reviewed_csv(rows: &[BatchRow]) -> String {
    let mut w = csv::WriterBuilder::new().from_writer(Vec::new());
    let _ = w.write_record([
        "line",
        "vendor",
        "doc_number",
        "date",
        "amount_eur",
        "status",
        "error",
    ]);
    for r in rows {
        let _ = w.write_record([
            &r.line.to_string(),
            &r.vendor,
            &r.doc_number,
            &r.date,
            &format!("{:.2}", r.amount_eur),
            &r.status,
            r.error.as_deref().unwrap_or(""),
        ]);
    }
    String::from_utf8_lossy(&w.into_inner().unwrap_or_default()).into_owned()
}

/// Amount parser: accepts plain dots (1234.56), plain commas (1234,56), and
/// European thousands format (1.234,56). Returns None when not numeric.
fn parse_amount(raw: &str) -> Option<f64> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    if t.contains(',') {
        // European: dot-thousands removed, comma becomes decimal point.
        t.replace('.', "").replace(',', ".").parse::<f64>().ok()
    } else {
        t.parse::<f64>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "vendor,doc_number,date,amount_eur\n\
ACME SpA,INV-001,2026-01-31,120.50\n\
Beta Srl,,2026-02-02,88\n\
Gamma Ltd,INV-003,31/01/2026,10\n\
Delta Spa,INV-004,2026-03-03,abc\n";

    #[test]
    fn good_rows_pass_bad_rows_get_reasons() {
        let rows = import_csv(SAMPLE);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].status, "ok");
        assert_eq!(rows[1].status, "missing_field");
        assert!(rows[1].error.as_deref().unwrap().contains("doc_number"));
        assert_eq!(rows[2].status, "bad_date");
        assert!(
            rows[3].status.contains("amount") || rows[3].status == "bad_amount",
            "{rows:?}"
        );
        // line numbers account for the header
        assert_eq!(rows[0].line, 2);
        assert_eq!(rows[3].line, 5);
    }

    #[test]
    fn european_decimal_comma_is_accepted() {
        let rows = import_csv("vendor,doc_number,date,amount_eur\nA,B,2026-01-01,\"1.234,56\"\n");
        assert_eq!(rows[0].status, "ok");
        assert!((rows[0].amount_eur - 1234.56).abs() < 0.01);
    }

    #[test]
    fn valid_rows_filter_and_export_round_trips() {
        let rows = import_csv(SAMPLE);
        let ok = valid_rows(&rows);
        assert_eq!(ok.len(), 1);
        assert_eq!(ok[0].doc_number, "INV-001");

        let csv_out = export_reviewed_csv(&rows);
        assert!(csv_out.starts_with("line,vendor,doc_number,date,amount_eur,status,error"));
        assert!(csv_out.contains("missing_field"));
        assert!(csv_out.contains("120.50"));
    }

    #[test]
    fn empty_input_yields_no_rows_not_panic() {
        assert!(import_csv("").is_empty());
        assert_eq!(
            export_reviewed_csv(&[]),
            "line,vendor,doc_number,date,amount_eur,status,error\n"
        );
    }
}
