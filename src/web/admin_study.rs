use super::*;
use crate::web::SharedState;
use axum::{extract::{Json, Query, State}, response::Response, http::HeaderMap};

/// POST /api/v1/admin/review — group supplied documents into duplicate review
/// clusters. This is a stateless first step for the administration pilot.
pub async fn admin_review_handler(Json(documents): Json<Vec<ReviewDocument>>) -> Json<ReviewQueue> {
    Json(review_queue(&documents))
}

/// Stateless shift report: caller supplies all data (`ShiftReportInput`),
/// gets Italian Markdown (default) or HTML (`?format=html`) back.
/// Replica-safe like the other compute routes.
pub async fn report_shift_compute_handler(
    Query(q): Query<ReportFormatQuery>,
    Json(input): Json<crate::report::ShiftReportInput>,
) -> Response {
    report_response(&input, q.format.as_deref())
}

/// Stateful shift report from the live engine: every machine on the OEE board,
/// plus accepted/ungraded dream descriptions from the default tenant as
/// proposed countermeasures. Anomaly/RUL sections come via the stateless POST
/// (the monitor scores readings as they arrive; it doesn't keep a shift log).
pub async fn report_shift_live_handler(
    State(state): State<SharedState>,
    Query(q): Query<ReportFormatQuery>,
) -> Response {
    let oee: Vec<OeeReport> = {
        let board = state.oee.lock().unwrap_or_else(|e| e.into_inner());
        board
            .machine_ids()
            .iter()
            .filter_map(|id| board.report(id))
            .collect()
    };
    let countermeasures: Vec<String> = {
        let s = state.lock();
        s.dreams
            .dreams
            .iter()
            .filter(|d| d.grade.unwrap_or(1.0) > 0.5)
            .map(|d| d.description.clone())
            .take(10)
            .collect()
    };
    let input = crate::report::ShiftReportInput {
        oee,
        countermeasures,
        ..Default::default()
    };
    report_response(&input, q.format.as_deref())
}

/// POST /api/v1/study/log — persist study events for the caller's tenant. Each
/// event carries date/time, action (generated/edited/opened/deleted), path,
/// directory and place. Persisted to sled, capped at the newest 5000.
pub async fn study_log_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    use crate::study::StudyEvent;
    let uid = user_key(&headers);
    let events: Vec<StudyEvent> = req
        .get("events")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| serde_json::from_value::<StudyEvent>(e.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    let mut all = state
        .db()
        .map(|db| load_study(db, &uid))
        .unwrap_or_default();
    let accepted = events.len();
    all.extend(events);
    if all.len() > 5000 {
        all = all[all.len() - 5000..].to_vec();
    }
    if let Some(db) = state.db() {
        if let Err(e) = save_study(db, &uid, &all) {
            log::warn!("study log not persisted for {uid}: {e}");
        }
    }
    Json(serde_json::json!({ "stored": all.len(), "accepted": accepted }))
}

/// GET /api/v1/study/session — aggregate the tenant's Study Log into a
/// `StudySession` (timeline, file-type/dir/action tallies, rhythms).
pub async fn study_session_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    let events = state
        .db()
        .map(|db| load_study(db, &uid))
        .unwrap_or_default();
    let s = crate::study::session(&events);
    Json(
        serde_json::to_value(s)
            .unwrap_or_else(|_| serde_json::json!({ "error": "session encode" })),
    )
}

/// POST /api/v1/study/clear — wipe the tenant's Study Log (demo reset).
pub async fn study_clear_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    if let Some(db) = state.db() {
        let _ = db.remove(study_key(&uid));
    }
    Json(serde_json::json!({ "cleared": true, "uid": uid }))
}

/// sled key for a tenant's Study Log. One blob per tenant (newest-capped), so a
/// student's session history survives restarts and never bleeds across users.
pub fn study_key(uid: &str) -> String {
    format!("study:log:{uid}")
}

pub(crate) fn save_study(db: &sled::Db, uid: &str, events: &[crate::study::StudyEvent]) -> anyhow::Result<()> {
    db.insert(study_key(uid), serde_json::to_vec(events)?)?;
    Ok(())
}

pub(crate) fn load_study(db: &sled::Db, uid: &str) -> Vec<crate::study::StudyEvent> {
    db.get(study_key(uid))
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_slice::<Vec<crate::study::StudyEvent>>(&v).ok())
        .unwrap_or_default()
}

