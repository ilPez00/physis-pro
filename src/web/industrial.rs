use super::*;
use crate::web::SharedState;
use axum::{extract::{Json, Query, State, Request}, response::{IntoResponse, Response}, http::{StatusCode, HeaderMap}, routing::{get, post}};
use serde::{Deserialize, Serialize};

pub async fn oee_machine_handler(
    State(state): State<SharedState>,
    Json(req): Json<OeeMachineRequest>,
) -> Json<OkResponse> {
    {
        let mut board = state.oee.lock().unwrap_or_else(|e| e.into_inner());
        board.upsert_machine(
            &req.machine_id,
            req.shift_length_secs,
            req.ideal_cycle_time_secs,
        );
        if let Some(db) = state.db() {
            let _ = board.save(db);
        }
    }
    Json(OkResponse {
        ok: true,
        detail: format!("machine {} registered", req.machine_id),
    })
}

pub async fn oee_event_handler(
    State(state): State<SharedState>,
    Json(ev): Json<OeeEvent>,
) -> (StatusCode, Json<OkResponse>) {
    let mut board = state.oee.lock().unwrap_or_else(|e| e.into_inner());
    let (ok, detail) = match ev {
        OeeEvent::Stop {
            machine_id,
            reason,
            duration_secs,
            note,
        } => {
            let ok = board.record_stop(&machine_id, reason, duration_secs, note);
            (ok, format!("stop on {machine_id}"))
        }
        OeeEvent::Production {
            machine_id,
            good,
            scrap,
        } => {
            let ok = board.record_production(&machine_id, good, scrap);
            (ok, format!("+{good} good, +{scrap} scrap on {machine_id}"))
        }
        OeeEvent::StartShift { machine_id } => {
            let ok = board.start_shift(&machine_id);
            (ok, format!("shift started on {machine_id}"))
        }
    };
    if ok {
        if let Some(db) = state.db() {
            let _ = board.save(db);
        }
    }
    let code = if ok {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };
    let detail = if ok {
        detail
    } else {
        format!("unknown machine ({detail})")
    };
    (code, Json(OkResponse { ok, detail }))
}

pub async fn oee_machine_report_handler(
    State(state): State<SharedState>,
    Path(machine_id): Path<String>,
) -> Result<Json<OeeReport>, StatusCode> {
    state
        .oee
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .report(&machine_id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn oee_plant_handler(State(state): State<SharedState>) -> Json<PlantOee> {
    Json(state.oee.lock().unwrap_or_else(|e| e.into_inner()).plant())
}

pub async fn maintenance_observe_handler(
    State(state): State<SharedState>,
    Json(req): Json<ReadingRequest>,
) -> Json<OkResponse> {
    let reading = req.to_reading();
    let n = {
        let mut mon = state.maintenance.lock().unwrap_or_else(|e| e.into_inner());
        mon.observe_healthy(&reading);
        if let Some(db) = state.db() {
            let _ = mon.save(db);
        }
        mon.baseline(&req.machine_id)
            .map(|b| b.min_count())
            .unwrap_or(0)
    };
    Json(OkResponse {
        ok: true,
        detail: format!("baseline {} now {} samples", req.machine_id, n),
    })
}

pub async fn maintenance_check_handler(
    State(state): State<SharedState>,
    Json(req): Json<ReadingRequest>,
) -> Result<Json<MaintenanceCheckResponse>, StatusCode> {
    let reading = req.to_reading();
    let threshold = req.sigma_threshold.unwrap_or(DEFAULT_SIGMA_THRESHOLD);

    let (score, rul) = {
        let mut mon = state.maintenance.lock().unwrap_or_else(|e| e.into_inner());
        let score = mon
            .check(&reading, threshold)
            .ok_or(StatusCode::NOT_FOUND)?;
        let rul = match (&req.rul_metric, req.rul_limit) {
            (Some(m), Some(limit)) => mon.rul(&req.machine_id, m, limit),
            _ => None,
        };
        if let Some(db) = state.db() {
            let _ = mon.save(db);
        } // check() appended history
        (score, rul)
    };

    // Auto-kaizen seam: an anomaly becomes a coherence node so the dream loop
    // already sees the defect and can propose a countermeasure.
    let coherence_node_id = if score.is_anomaly {
        let text = score.describe();
        let embedding = embed_text(state.shared.clone(), text.clone()).await;
        let mut s = state.lock();
        let id = s.core.register_node_vec_labeled(embedding, Some(text));
        if let Some(db) = state.db() {
            let _ = s.core.persist_node(db, "_default", &id);
        }
        Some(id)
    } else {
        None
    };

    Ok(Json(MaintenanceCheckResponse {
        score,
        rul,
        coherence_node_id,
    }))
}

pub async fn maintenance_baseline_handler(
    State(state): State<SharedState>,
    Path(machine_id): Path<String>,
) -> Result<Json<HealthBaseline>, StatusCode> {
    state
        .maintenance
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .baseline(&machine_id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn oee_compute_handler(Json(shift): Json<OeeShift>) -> Json<OeeReport> {
    Json(shift.compute())
}

pub async fn oee_rollup_handler(Json(reports): Json<Vec<OeeReport>>) -> Json<PlantOee> {
    Json(plant_oee(&reports))
}

pub async fn maintenance_fold_handler(Json(req): Json<FoldRequest>) -> Json<HealthBaseline> {
    let mut baseline = req
        .baseline
        .unwrap_or_else(|| HealthBaseline::new(&req.reading.machine_id));
    baseline.observe(&req.reading.to_reading());
    Json(baseline)
}

pub async fn maintenance_score_handler(Json(req): Json<ScoreRequest>) -> Json<AnomalyScore> {
    let threshold = req.sigma_threshold.unwrap_or(DEFAULT_SIGMA_THRESHOLD);
    Json(req.baseline.score(&req.reading.to_reading(), threshold))
}

pub async fn maintenance_rul_handler(Json(req): Json<RulRequest>) -> Json<Option<RulEstimate>> {
    Json(project_time_to_limit(&req.samples, req.limit))
}

