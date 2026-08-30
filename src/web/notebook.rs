//! Notebook CRUD — per-tenant, validated, stored in sled.
//!
//! Layout lives on the server rather than in `localStorage` because a notebook
//! that exists only in one browser cannot be opened on the machine wired to the
//! projector, cannot be handed to a colleague, and vanishes with a cache clear.
//! Those are the three things a composed dashboard is *for*.
//!
//! Every write goes through [`crate::notebook::validate`]. The endpoint never
//! stores an unknown widget id "to be skipped at render": a stored reference
//! nobody checks is how a payload eventually rides in.

use super::*;
use crate::notebook::{validate, validate_id, Notebook};
use crate::web::SharedState;
use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
};

/// sled key for one tenant's notebook. Tenant-scoped like every other
/// per-user structure — one operator's composition is not another's.
fn key(uid: &str, id: &str) -> Vec<u8> {
    format!("notebook:{uid}:{id}").into_bytes()
}

fn prefix(uid: &str) -> Vec<u8> {
    format!("notebook:{uid}:").into_bytes()
}

fn errors_response(
    status: StatusCode,
    errors: Vec<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        status,
        Json(serde_json::json!({ "error": "notebook rejected", "errors": errors })),
    )
}

/// GET /api/v1/notebooks/registry — the widget catalogue.
///
/// Served from the same constant the validator checks against, so the picker
/// cannot offer a widget the server would refuse.
pub async fn registry_handler() -> Json<serde_json::Value> {
    Json(crate::notebook::registry_json())
}

/// GET /api/v1/notebooks — this tenant's notebooks, id and title only.
pub async fn list_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    let mut notebooks: Vec<serde_json::Value> = Vec::new();
    if let Some(db) = state.db() {
        for item in db.scan_prefix(prefix(&uid)).flatten() {
            if let Ok(nb) = serde_json::from_slice::<Notebook>(&item.1) {
                notebooks.push(serde_json::json!({
                    "id": nb.id,
                    "title": nb.title,
                    "cells": nb.cells.len(),
                    "updated_at": nb.updated_at,
                }));
            }
        }
    }
    // Stable order: sled iterates by key, which is already lexicographic, but
    // saying so keeps the contract if the store ever changes.
    notebooks.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    Json(serde_json::json!({
        "notebooks": notebooks,
        "persistent": state.db().is_some(),
    }))
}

/// GET /api/v1/notebooks/:id — one notebook.
///
/// A tenant who has never composed anything gets the starter composition
/// rather than a 404: a blank grid asks someone who has never seen the product
/// to know what it can show. The response says whether it was `saved`, so the
/// editor can tell "yours" from "ours".
pub async fn get_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let uid = user_key(&headers);
    let id = validate_id(&id).map_err(|e| errors_response(StatusCode::BAD_REQUEST, vec![e]))?;

    let stored: Option<Notebook> = state
        .db()
        .and_then(|db| db.get(key(&uid, &id)).ok().flatten())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());

    match stored {
        Some(nb) => Ok(Json(serde_json::json!({ "notebook": nb, "saved": true }))),
        None if id == "default" => {
            let mut nb = crate::notebook::starter();
            nb.id = id;
            Ok(Json(serde_json::json!({ "notebook": nb, "saved": false })))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("no notebook '{id}'") })),
        )),
    }
}

/// POST /api/v1/notebooks/:id — create or replace, validated.
pub async fn save_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(mut nb): Json<Notebook>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let uid = user_key(&headers);
    let id = validate_id(&id).map_err(|e| errors_response(StatusCode::BAD_REQUEST, vec![e]))?;
    // The path is the identity. A body claiming a different id would create a
    // notebook at a key the caller did not address — silently, and only
    // visible later as a duplicate.
    nb.id = id.clone();
    nb.updated_at = chrono::Utc::now().to_rfc3339();

    validate(&nb).map_err(|errors| errors_response(StatusCode::BAD_REQUEST, errors))?;

    let Some(db) = state.db() else {
        // Persistence is the whole point of storing layout server-side; a
        // success response with nothing written would be a lie the user only
        // discovers on their next visit.
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "state persistence is off — a notebook saved now would not survive"
            })),
        ));
    };
    let bytes = serde_json::to_vec(&nb).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("serialize: {e}") })),
        )
    })?;
    db.insert(key(&uid, &id), bytes).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("store: {e}") })),
        )
    })?;

    Ok(Json(serde_json::json!({
        "saved": true,
        "id": nb.id,
        "cells": nb.cells.len(),
        "updated_at": nb.updated_at,
    })))
}

/// DELETE /api/v1/notebooks/:id
pub async fn delete_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let uid = user_key(&headers);
    let id = validate_id(&id).map_err(|e| errors_response(StatusCode::BAD_REQUEST, vec![e]))?;
    let Some(db) = state.db() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "state persistence is off" })),
        ));
    };
    let removed = db
        .remove(key(&uid, &id))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("store: {e}") })),
            )
        })?
        .is_some();
    if !removed {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("no notebook '{id}'") })),
        ));
    }
    Ok(Json(serde_json::json!({ "deleted": true, "id": id })))
}
