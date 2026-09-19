//! HTTP surface for the declarative route table (Phase 19, B1).
//!
//! Routes are operator-level configuration (like axes), not tenant reasoning
//! state, so they live on `AppCtx` and are shared across tenants.

use axum::{
    extract::{Path as AxPath, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::routes::AgentRoute;
use crate::web::SharedState;

#[derive(Deserialize)]
pub struct RouteBody {
    pub route: AgentRoute,
}

#[derive(Deserialize)]
pub struct RouteMatchRequest {
    pub text: String,
}

#[derive(Serialize)]
pub struct RoutesResponse {
    pub routes: Vec<AgentRoute>,
}

#[derive(Serialize)]
pub struct RouteMatchResponse {
    pub domain: String,
    pub mode: String,
    /// Display-form cell, e.g. `FABRICATE/WORK`.
    pub cell: String,
    pub routes: Vec<AgentRoute>,
}

/// GET /api/v1/routes — every route in the table.
pub async fn list_routes(State(state): State<SharedState>) -> Json<RoutesResponse> {
    let table = state.routes.lock().unwrap_or_else(|e| e.into_inner());
    Json(RoutesResponse {
        routes: table.all().to_vec(),
    })
}

/// POST /api/v1/routes — upsert one route. Validation reports every problem at
/// once; on success the table is persisted before the response is sent, so a
/// caller that got `ok: true` owns the change across a restart.
pub async fn save_route(
    State(state): State<SharedState>,
    Json(body): Json<RouteBody>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut table = state.routes.lock().unwrap_or_else(|e| e.into_inner());
    let path = state.shared.config.data_dir.join("routes.json");
    match table.upsert(body.route) {
        Ok(()) => {
            let _ = table.save(&path);
            (
                StatusCode::OK,
                Json(serde_json::json!({ "ok": true, "saved": true })),
            )
        }
        Err(errs) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "errors": errs })),
        ),
    }
}

/// DELETE /api/v1/routes/{name} — remove a route by name.
pub async fn delete_route(
    State(state): State<SharedState>,
    AxPath(name): AxPath<String>,
) -> Json<serde_json::Value> {
    let mut table = state.routes.lock().unwrap_or_else(|e| e.into_inner());
    let removed = table.remove(&name);
    if removed {
        let path = state.shared.config.data_dir.join("routes.json");
        let _ = table.save(&path);
    }
    Json(serde_json::json!({ "ok": removed }))
}

/// POST /api/v1/routes/match — classify text and return the routes bound to the
/// winning cell. The classifier already applied overlays and salience, so this
/// is deterministic — an agent can route on the response without re-embedding.
pub async fn match_route(
    State(state): State<SharedState>,
    Json(req): Json<RouteMatchRequest>,
) -> Json<RouteMatchResponse> {
    // Text-only (image=None) so the image-slot 503 is unreachable here.
    let embedding =
        crate::web::wiki_semiotic::embed_input(state.shared.clone(), req.text.clone(), None)
            .await
            .unwrap_or_default();
    let results = state.shared.classifier.classify(&embedding);
    let (domain, mode) = match results.first() {
        Some(c) => (c.domain.clone(), c.mode.clone()),
        None => (String::new(), String::new()),
    };
    let table = state.routes.lock().unwrap_or_else(|e| e.into_inner());
    let routes = table
        .for_cell(&domain, &mode)
        .into_iter()
        .cloned()
        .collect();
    let cell = format!("{domain}/{mode}");
    Json(RouteMatchResponse {
        domain,
        mode,
        cell,
        routes,
    })
}