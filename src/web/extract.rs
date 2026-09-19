//! HTTP surface for structured-output extraction (Phase 19, B2).

use axum::{extract::State, Json};
use serde::Deserialize;

use crate::web::SharedState;

#[derive(Deserialize)]
pub struct ExtractRequest {
    pub text: String,
    /// JSON Schema object the extracted slots must conform to.
    pub schema: serde_json::Value,
}

/// POST /api/v1/extract — schema-constrained slot extraction. Never errors for
/// lack of a provider or a model that returns nothing parseable: it returns an
/// explicit `note` instead, so a harness can decide to fall back rather than
/// trust a fabricated object.
pub async fn extract_handler(
    State(state): State<SharedState>,
    Json(req): Json<ExtractRequest>,
) -> Json<serde_json::Value> {
    let cascade = state.shared.cascade.clone();
    match crate::extract::extract_slots(&cascade, &req.text, &req.schema).await {
        Ok((slots, provider)) => {
            Json(serde_json::json!({ "slots": slots, "provider": provider, "note": null }))
        }
        Err(e) => Json(serde_json::json!({
            "slots": serde_json::Value::Null,
            "provider": null,
            "note": e,
        })),
    }
}
