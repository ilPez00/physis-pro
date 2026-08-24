use super::*;
use crate::shifts::{
    aggregate, append_shifts, clear_shifts, load_shifts, parse_shift_csv, parse_shift_json,
};
use crate::web::SharedState;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::Response,
};

/// POST /api/v1/shifts/import — CSV (`text/csv` body) or JSON
/// (`{"shifts": [...]}` / bare array). Partial acceptance: valid rows are
/// persisted for the caller's tenant, rejected rows come back with line
/// numbers and reasons.
pub async fn shifts_import_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let uid = user_key(&headers);
    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    let (records, errors) = if content_type.contains("json") {
        match serde_json::from_slice::<serde_json::Value>(&body) {
            Ok(serde_json::Value::Array(elements)) => parse_shift_json(&elements),
            Ok(value @ serde_json::Value::Object(_)) => match value.get("shifts") {
                Some(serde_json::Value::Array(elements)) => parse_shift_json(elements),
                _ => (
                    Vec::new(),
                    vec![crate::shifts::RowError {
                        line: 1,
                        error: "JSON body must be an array of shifts or {\"shifts\": [...]}".into(),
                    }],
                ),
            },
            Ok(_) => (
                Vec::new(),
                vec![crate::shifts::RowError {
                    line: 1,
                    error: "expected a JSON array".into(),
                }],
            ),
            Err(e) => (
                Vec::new(),
                vec![crate::shifts::RowError {
                    line: 1,
                    error: format!("invalid JSON: {e}"),
                }],
            ),
        }
    } else {
        match std::str::from_utf8(&body) {
            Ok(text) => parse_shift_csv(text),
            Err(_) => (
                Vec::new(),
                vec![crate::shifts::RowError {
                    line: 1,
                    error: "body is not UTF-8 text".into(),
                }],
            ),
        }
    };

    let accepted = records.len();
    let mut stored = load_count(&state, &uid);
    if accepted > 0 {
        if let Some(db) = state.db() {
            match append_shifts(db, &uid, records) {
                Ok(total) => stored = total,
                Err(e) => log::warn!("shift history not persisted for {uid}: {e}"),
            }
        }
    }

    axum::Json(crate::shifts::ImportReport {
        accepted,
        rejected: errors,
        stored,
    })
    .into_response()
}

/// GET /api/v1/shifts — the tenant's durable shift history, newest last.
/// Optional `?machine=CNC-01` narrows to one machine.
pub async fn shifts_list_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    let records = tenant_records(&state, &uid, q.get("machine").map(|m| m.as_str()));
    let machines: std::collections::BTreeSet<String> =
        records.iter().map(|r| r.shift.machine_id.clone()).collect();
    Json(serde_json::json!({
        "count": records.len(),
        "machines": machines,
        "shifts": records,
    }))
}

/// GET /api/v1/shifts/report — cumulative OEE per machine across the tenant's
/// whole history (or `?machine=` subset), plus the plant rollup. Default is
/// JSON; `?format=html` renders the customer-ready Italian document and
/// `?format=md` returns Markdown, both carrying a provenance metadata line.
pub async fn shifts_report_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Response {
    use crate::oee::plant_oee;
    let uid = user_key(&headers);
    let records = tenant_records(&state, &uid, q.get("machine").map(|m| m.as_str()));
    let merged = aggregate(&records);
    let reports: Vec<OeeReport> = merged.iter().map(|s| s.compute()).collect();
    let plant = (!reports.is_empty()).then(|| plant_oee(&reports));
    let meta = crate::report::ReportMetadata {
        source: "shift-history".into(),
        shift_count: records.len(),
        machine_count: reports.len(),
        span: crate::shifts::records_span(&records).map(|s| crate::report::ReportSpan {
            from: s.from,
            to: s.to,
        }),
        ..crate::report::ReportMetadata::new("")
    };

    match q.get("format").map(|f| f.as_str()) {
        Some("html") | Some("md") => {
            let input = crate::report::ShiftReportInput {
                oee: reports,
                plant,
                shift_label: q
                    .get("label")
                    .map(|l| l.to_string())
                    .or(Some("Rapporto storico impianto".into())),
                metadata: Some(meta),
                ..Default::default()
            };
            report_response(&input, q.get("format").map(|f| f.as_str()))
        }
        _ => Json(serde_json::json!({
            "machines": reports,
            "plant": plant,
            "metadata": meta,
        }))
        .into_response(),
    }
}

/// GET /api/v1/shifts/export — the tenant's history as a self-contained JSON
/// evidence bundle: raw records plus per-record OEE recomputed at export time,
/// so every headline number can be re-derived from first principles.
pub async fn shifts_export_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Json<crate::shifts::EvidenceBundle> {
    let uid = user_key(&headers);
    let records = tenant_records(&state, &uid, q.get("machine").map(|m| m.as_str()));
    Json(crate::shifts::evidence_bundle(
        &uid,
        q.get("machine").map(|m| m.as_str()),
        &records,
    ))
}

/// POST /api/v1/shifts/clear — wipe the tenant's shift history (demo reset).
pub async fn shifts_clear_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    if let Some(db) = state.db() {
        clear_shifts(db, &uid);
    }
    Json(serde_json::json!({ "cleared": true, "uid": uid }))
}

/// A tenant's stored shifts, optionally narrowed to one machine.
fn tenant_records(
    state: &SharedState,
    uid: &str,
    machine: Option<&str>,
) -> Vec<crate::shifts::ShiftRecord> {
    state
        .db()
        .map(|db| load_shifts(db, uid))
        .unwrap_or_default()
        .into_iter()
        .filter(|r| machine.map(|m| r.shift.machine_id == m).unwrap_or(true))
        .collect()
}

fn load_count(state: &SharedState, uid: &str) -> usize {
    state.db().map(|db| load_shifts(db, uid).len()).unwrap_or(0)
}
