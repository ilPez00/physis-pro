//! Phase 6.1b/6.2b/6.4b — HTTP surface for the Phase-6 administration cores.
//!
//! Three stateless POST endpoints (replica-safe like the other compute
//! routes; no tenant state, no persistence):
//! - `POST /api/v1/admin/desk`      — run the review desk over a server-local
//!   folder (path must exist on the host running physis-pro)
//! - `POST /api/v1/admin/batch`     — validate a CSV batch, return rows + the
//!   reviewed export
//! - `POST /api/v1/admin/pack`      — render the weekly management pack from
//!   supplied inputs (caller may also feed it desk/recon output from the two
//!   endpoints above)

use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;

/// Request body for `/api/v1/admin/desk`.
#[derive(Debug, Deserialize)]
pub struct DeskRequest {
    /// Server-local directory of documents to review.
    pub dir: String,
}

pub async fn admin_desk_handler(Json(req): Json<DeskRequest>) -> Response {
    let dir = std::path::PathBuf::from(&req.dir);
    if !dir.is_dir() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            format!("dir not found on server: {}", req.dir),
        )
            .into_response();
    }
    // Blocking FS work in the handler is acceptable here: the review desk is
    // an operator-triggered batch job, not a hot path.
    let report = crate::review_desk::run_desk(&dir);
    axum::Json(serde_json::json!({
        "report": report,
        "markdown": crate::review_desk::render_markdown(&report),
    }))
    .into_response()
}

/// Request body for `/api/v1/admin/batch`.
#[derive(Debug, Deserialize)]
pub struct BatchRequest {
    /// Raw CSV text with header: vendor,doc_number,date,amount_eur.
    /// Read by the ingest-gated handler; kept unconditionally so the request
    /// shape (and client contract) is feature-independent.
    #[allow(dead_code)]
    pub csv_text: String,
}

#[cfg(feature = "ingest")]
pub async fn admin_batch_handler(Json(req): Json<BatchRequest>) -> Response {
    let rows = crate::batch_csv::import_csv(&req.csv_text);
    let valid = crate::batch_csv::valid_rows(&rows).len();
    let export = crate::batch_csv::export_reviewed_csv(&rows);
    axum::Json(serde_json::json!({
        "rows": rows,
        "valid": valid,
        "total": rows.len(),
        "export_csv": export,
    }))
    .into_response()
}

#[cfg(not(feature = "ingest"))]
pub async fn admin_batch_handler(Json(_req): Json<BatchRequest>) -> Response {
    (
        axum::http::StatusCode::NOT_IMPLEMENTED,
        "batch_csv requires the 'ingest' feature",
    )
        .into_response()
}

/// Request body for `/api/v1/admin/pack`. Reconciliation lines come in the
/// same shape `reconcile()` emits, so callers can chain endpoints directly.
#[derive(Debug, Deserialize)]
pub struct PackRequest {
    pub week_label: String,
    #[serde(default)]
    pub plant: String,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub recon: Vec<crate::reconcile::ReconciledLine>,
}

pub async fn admin_pack_handler(Json(req): Json<PackRequest>) -> Response {
    let md = crate::management_pack::render_weekly_pack(&crate::management_pack::PackInput {
        week_label: &req.week_label,
        plant: &req.plant,
        desk: None,
        recon: &req.recon,
        notes: &req.notes,
    });
    axum::Json(serde_json::json!({ "markdown": md })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn desk_rejects_missing_dir_with_400() {
        let resp = admin_desk_handler(Json(DeskRequest {
            dir: "/nonexistent/desk/dir".into(),
        }))
        .await;
        assert_eq!(resp.status(), axum::http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn desk_runs_on_real_temp_dir() {
        let tmp = std::env::temp_dir().join(format!("web_desk_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("a.txt"), "Delivery note DN-77 goods received").unwrap();
        let resp = admin_desk_handler(Json(DeskRequest {
            dir: tmp.display().to_string(),
        }))
        .await;
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[cfg(feature = "ingest")]
    #[tokio::test]
    async fn batch_validates_and_exports() {
        let resp = admin_batch_handler(Json(BatchRequest {
            csv_text: "vendor,doc_number,date,amount_eur\nACME,A-1,2026-01-01,10\n".into(),
        }))
        .await;
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[cfg(not(feature = "ingest"))]
    #[tokio::test]
    async fn batch_answers_501_without_ingest_feature() {
        let resp = admin_batch_handler(Json(BatchRequest {
            csv_text: "vendor,doc_number,date,amount_eur\nACME,A-1,2026-01-01,10\n".into(),
        }))
        .await;
        assert_eq!(resp.status(), axum::http::StatusCode::NOT_IMPLEMENTED);
    }

    #[tokio::test]
    async fn pack_renders_week_label() {
        let resp = admin_pack_handler(Json(PackRequest {
            week_label: "2026-W35".into(),
            plant: "Verona".into(),
            notes: vec![],
            recon: vec![],
        }))
        .await;
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }
}
