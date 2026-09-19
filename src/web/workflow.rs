//! HTTP surface for the workflow scanner (the "who did what where" audit).
//!
//! `POST /api/v1/workflow/scan` scans a directory under the scan root, walks
//! the harness dirs the base scan skips, classifies every doc by audience
//! (agent / human / both), stamps timestamps/metadata, attributes git authors,
//! and returns the report. It also records the audit into the tenant's causal
//! ledger and pushes one `Workflow` event per doc through the single event
//! stream (`/api/v1/events`), so the audit survives in the timeline.

use axum::{extract::State, http::HeaderMap, Json};
use serde::Deserialize;

use crate::web::{err_response, user_key, ErrorResponse, SharedState};
use crate::workflow::WorkflowReport;

#[derive(Deserialize)]
pub struct WorkflowScanRequest {
    /// Directory to audit (confined to scan root).
    dir: String,
}

#[derive(serde::Serialize)]
pub struct WorkflowScanResponse {
    pub report: WorkflowReport,
    /// Number of workflow events recorded to the ledger/event ring.
    pub events_recorded: usize,
    /// Whether a causal-ledger entry was written for this audit run.
    pub ledger_recorded: bool,
}

/// POST /api/v1/workflow/scan — audit a directory's documentation coverage and
/// attribution. Path-traversal guarded like /scan; the run records a
/// `derived(workflow)` ledger event + one `Workflow` op-event per doc.
pub async fn workflow_scan_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<WorkflowScanRequest>,
) -> Result<Json<WorkflowScanResponse>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    let path = std::path::PathBuf::from(&req.dir);
    if !path.exists() {
        return Err(err_response(
            axum::http::StatusCode::BAD_REQUEST,
            &format!("directory not found: {}", req.dir),
        ));
    }
    let canonical = path.canonicalize().map_err(|e| {
        err_response(
            axum::http::StatusCode::BAD_REQUEST,
            &format!("cannot resolve path: {e}"),
        )
    })?;
    if !canonical.starts_with(state.scan_root()) {
        return Err(err_response(
            axum::http::StatusCode::FORBIDDEN,
            &format!("path outside scan root ({})", state.scan_root().display()),
        ));
    }
    let uid = user_key(&headers);

    // Scan off-thread: git attribution runs a subprocess.
    let root = canonical.clone();
    let docs = tokio::task::spawn_blocking(move || crate::workflow::scan_workflow(&root, None))
        .await
        .map_err(|e| {
            err_response(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                &e.to_string(),
            )
        })?;
    let report = crate::workflow::report(&canonical, docs);
    let events = crate::workflow::to_op_events(&report.docs);
    let events_recorded = events.len();

    // Record into the tenant's ledger + event ring (same store the single
    // ingestion path `/api/v1/events` writes).
    let ledger_recorded = state.with_user(&uid, |s| {
        for ev in events {
            if s.op_events.iter().any(|e| e.event_key == ev.event_key) {
                continue;
            }
            let mechanism = ev.ledger_mechanism();
            let actor = ev
                .actor
                .clone()
                .unwrap_or_else(|| ev.provenance.connector.clone());
            let input = ev.target.clone().or_else(|| ev.machine_id.clone());
            s.ledger
                .record_at(mechanism, &actor, ev.subject.clone(), input, ev.observed_at);
            crate::operational::push_capped(&mut s.op_events, ev.clone());
        }
        if let Some(db) = state.db() {
            if let Err(e) = crate::operational::save_events(db, &uid, &s.op_events) {
                log::warn!("workflow scan: events not persisted for {uid}: {e}");
            }
            if let Err(e) = s.ledger.save(db, &uid) {
                log::warn!("workflow scan: ledger not persisted for {uid}: {e}");
            }
        }
        let before = s.ledger.len();
        // One derived ledger entry naming the audit itself, so lineage walks
        // from the run to the docs it audited.
        s.ledger.record(
            crate::address::Mechanism::Derived,
            "workflow",
            canonical.display().to_string(),
            None,
        );
        if let Some(db) = state.db() {
            if let Err(e) = s.ledger.save(db, &uid) {
                log::warn!("workflow scan: ledger audit-entry persist failed: {e}");
            }
        }
        s.ledger.len() > before
    });

    Ok(Json(WorkflowScanResponse {
        report,
        events_recorded,
        ledger_recorded,
    }))
}
