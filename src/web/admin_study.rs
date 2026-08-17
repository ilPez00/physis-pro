use super::*;
use crate::web::SharedState;
use axum::{extract::{Json, Query, State, Request}, response::{IntoResponse, Response}, http::{StatusCode, HeaderMap}, routing::{get, post}};
use serde::{Deserialize, Serialize};

pub async fn study_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/study.html"))
}

pub async fn admin_review_handler(Json(documents): Json<Vec<ReviewDocument>>) -> Json<ReviewQueue> {
    Json(review_queue(&documents))
}

pub async fn report_shift_compute_handler(
    Query(q): Query<ReportFormatQuery>,
    Json(input): Json<crate::report::ShiftReportInput>,
) -> Response {
    report_response(&input, q.format.as_deref())
}

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

