use super::*;
use crate::web::SharedState;
use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
};

/// GET /api/v1/axes — current axes: the builtin values derived from the live
/// ontology entries, plus any installed (promoted) axes with their time-decayed
/// activation and lineage.
pub async fn axes_list_handler(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let shared = state.shared.clone();
    let derived = tokio::task::spawn_blocking(move || {
        crate::axes::derive_axes(&shared.classifier.cell_centroids())
    })
    .await
    .expect("axes derivation completes");

    let now = chrono::Utc::now();
    let store = state.axes.lock().unwrap_or_else(|e| e.into_inner());
    let installed: Vec<serde_json::Value> = store
        .active()
        .into_iter()
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "role": a.role,
                "name": a.name,
                "operator": a.operator,
                "world_scope": a.world_scope,
                "created_at": a.created_at,
                "activation": crate::axes::decayed_activation(a, now),
            })
        })
        .collect();

    Json(serde_json::json!({ "builtin": derived, "installed": installed }))
}

/// POST /api/v1/axes/discover — propose NEW axis values (a new domain or mode)
/// from texts that no existing axis can name (axis-orphans), clustered. Anchors
/// include both the builtin derived axes AND installed ones, so a promoted axis's
/// region is no longer orphaned. Each proposal carries its lineage. No grid mutation.
pub async fn axes_discover_handler(
    State(state): State<SharedState>,
    Json(req): Json<AxesDiscoverRequest>,
) -> Json<serde_json::Value> {
    let shared = state.shared.clone();
    let role = parse_axis_role(&req.role);
    let tau = req.tau.unwrap_or(0.5);

    // Snapshot installed axes (clone out of the lock — not held across the await).
    let installed: Vec<crate::axes::AxisValue> = {
        let store = state.axes.lock().unwrap_or_else(|e| e.into_inner());
        store.axes.clone()
    };

    let proposals = tokio::task::spawn_blocking(move || {
        let centroids = shared.classifier.cell_centroids();
        let mut anchors = crate::axes::axis_anchors(&centroids, role);
        anchors.extend(crate::axes::installed_anchors(&installed, role));
        let embeddings: Vec<Vec<f32>> =
            req.texts.iter().map(|t| shared.embedder.embed(t)).collect();
        crate::axes::propose_axes(role, &req.texts, &embeddings, &anchors, tau, 0.70, 2, 8)
    })
    .await
    .expect("axes discovery completes");
    Json(serde_json::json!({ "role": role, "proposals": proposals }))
}

/// POST /api/v1/axes/promote — install a proposed axis. The anchor is the centroid
/// of the (re-embedded) sample texts. Refuses any axis without its cause: samples
/// AND operator are required (the Phase 6 lineage invariant). Persisted to sled.
pub async fn axes_promote_handler(
    State(state): State<SharedState>,
    Json(req): Json<AxesPromoteRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if req.samples.is_empty() || req.operator.trim().is_empty() || req.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                serde_json::json!({ "error": "promotion requires name, operator, and samples — an axis must carry its cause" }),
            ),
        );
    }
    let role = parse_axis_role(&req.role);
    let shared = state.shared.clone();
    let samples = req.samples.clone();
    let anchor = tokio::task::spawn_blocking(move || {
        let embs: Vec<Vec<f32>> = samples.iter().map(|t| shared.embedder.embed(t)).collect();
        if embs.is_empty() {
            return Vec::<f32>::new();
        }
        let dim = embs[0].len();
        let mut mean = vec![0.0f32; dim];
        for e in &embs {
            if e.len() != dim {
                continue;
            }
            for (i, v) in e.iter().enumerate() {
                mean[i] += v;
            }
        }
        let n = embs.len() as f32;
        mean.iter_mut().for_each(|v| *v /= n);
        mean
    })
    .await
    .expect("anchor embed completes");

    let now = chrono::Utc::now();
    let id = format!(
        "axis-{}-{}",
        now.timestamp_millis(),
        req.name
            .trim()
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric(), "-")
    );
    let av = crate::axes::AxisValue {
        id: id.clone(),
        role,
        name: req.name.trim().to_string(),
        anchor,
        hints: req.hints,
        operator: req.operator.trim().to_string(),
        samples: req.samples,
        world_scope: req.world_scope.unwrap_or_else(|| "global".to_string()),
        created_at: now.to_rfc3339(),
        last_seen: now.to_rfc3339(),
        activation: 1.0,
        archived: false,
    };
    {
        let mut store = state.axes.lock().unwrap_or_else(|e| e.into_inner());
        store.axes.push(av.clone());
        if let Some(db) = &state.db {
            store.save(db);
        }
    }
    log::info!(
        "[axes] promoted {:?} axis '{}' by {} (id={})",
        role,
        av.name,
        av.operator,
        id
    );
    (
        StatusCode::CREATED,
        Json(serde_json::json!({ "installed": av })),
    )
}

/// POST /api/v1/axes/demote — archive an installed axis (kept for provenance, not
/// deleted — exactly as failure nodes are kept). Persisted.
pub async fn axes_demote_handler(
    State(state): State<SharedState>,
    Json(req): Json<AxesDemoteRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut store = state.axes.lock().unwrap_or_else(|e| e.into_inner());
    match store.axes.iter_mut().find(|a| a.id == req.id) {
        Some(a) => {
            a.archived = true;
            if let Some(db) = &state.db {
                store.save(db);
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({ "archived": req.id })),
            )
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "axis id not found" })),
        ),
    }
}

/// GET /api/v1/axes/lineage?id=… — the cause of an installed axis: who installed
/// it, the source texts that earned it, and when. An axis with no lineage is
/// inadmissible — this is how you audit one.
pub async fn axes_lineage_handler(
    State(state): State<SharedState>,
    Query(q): Query<AxesLineageQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let store = state.axes.lock().unwrap_or_else(|e| e.into_inner());
    match store.axes.iter().find(|a| a.id == q.id) {
        Some(a) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "id": a.id, "role": a.role, "name": a.name,
                "operator": a.operator, "samples": a.samples,
                "world_scope": a.world_scope, "created_at": a.created_at,
                "archived": a.archived,
            })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "axis id not found" })),
        ),
    }
}
