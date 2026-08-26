use super::*;
use crate::web::SharedState;
use axum::{
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
};

/// POST /api/v1/spanner/sync — mirror the tenant's coherence graph into Spanner.
/// Upserts nodes + edges, then (when `mirror`) hard-deletes durable rows that
/// vanished locally. Returns the mutation counts.
pub async fn spanner_sync_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<SpannerSyncRequest>,
) -> Result<Json<SpannerSyncResponse>, (StatusCode, Json<ErrorResponse>)> {
    let uid = user_key(&headers);
    let sink = crate::spanner::SpannerSink::from_env().map_err(|e| {
        err_response(
            StatusCode::BAD_GATEWAY,
            &format!("spanner not configured: {e}"),
        )
    })?;

    // Export under the tenant lock, then do IO outside it (Spanner is slow).
    let (nodes, edges, ledger) = state.with_user(&uid, |s| {
        let ledger = if req.ledger {
            Some(s.ledger.clone())
        } else {
            None
        };
        (
            crate::spanner::export_nodes(&s.core),
            crate::spanner::export_edges(&s.core),
            ledger,
        )
    });

    let (nodes_upserted, edges_upserted) = join_spanner(&sink, &nodes, &edges)
        .await
        .map_err(|e| err_response(StatusCode::BAD_GATEWAY, &e.to_string()))?;

    // Ledger mirror (Phase 10b): upsert-only, namespaced `cause:`.
    let (cause_nodes, cause_edges) = match &ledger {
        Some(l) => crate::spanner::sync_ledger(&sink, l)
            .await
            .map_err(|e| err_response(StatusCode::BAD_GATEWAY, &e.to_string()))?,
        None => (0, 0),
    };

    if !req.mirror {
        return Ok(Json(SpannerSyncResponse {
            nodes_upserted,
            edges_upserted,
            nodes_deleted: 0,
            edges_deleted: 0,
            cause_nodes_upserted: cause_nodes,
            cause_edges_upserted: cause_edges,
        }));
    }

    // Orphan sweep: fetch remote, diff vs local, hard-delete.
    let stale_edges = crate::spanner::orphan_edge_keys(
        &sink
            .fetch_edges()
            .await
            .map_err(|e| err_response(StatusCode::BAD_GATEWAY, &e.to_string()))?,
        &edges,
    );
    let edges_deleted = sink
        .delete_edges(&stale_edges)
        .await
        .map_err(|e| err_response(StatusCode::BAD_GATEWAY, &e.to_string()))?;
    let stale_nodes = crate::spanner::orphan_node_ids(
        &sink
            .fetch_nodes()
            .await
            .map_err(|e| err_response(StatusCode::BAD_GATEWAY, &e.to_string()))?,
        &nodes,
    );
    let nodes_deleted = sink
        .delete_nodes(&stale_nodes)
        .await
        .map_err(|e| err_response(StatusCode::BAD_GATEWAY, &e.to_string()))?;

    Ok(Json(SpannerSyncResponse {
        nodes_upserted,
        edges_upserted,
        nodes_deleted,
        edges_deleted,
        cause_nodes_upserted: cause_nodes,
        cause_edges_upserted: cause_edges,
    }))
}

/// Upsert nodes + edges (Spanner IO is slow, but sequential here).
pub async fn join_spanner(
    sink: &crate::spanner::SpannerSink,
    nodes: &[crate::spanner::NodeRow],
    edges: &[crate::spanner::EdgeRow],
) -> anyhow::Result<(usize, usize)> {
    let n = sink.upsert_nodes(nodes).await?;
    let e = sink.upsert_edges(edges).await?;
    Ok((n, e))
}

/// POST /api/v1/spanner/hydrate — pull durable rows back from Spanner and
/// re-embed their labels under the live embedder into this tenant's core. This
/// is the "new device / new deploy" recovery path (and the model-swap migrate).
pub async fn spanner_hydrate_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<SpannerHydrateResponse>, (StatusCode, Json<ErrorResponse>)> {
    let uid = user_key(&headers);
    let sink = crate::spanner::SpannerSink::from_env().map_err(|e| {
        err_response(
            StatusCode::BAD_GATEWAY,
            &format!("spanner not configured: {e}"),
        )
    })?;
    let rows = sink
        .fetch_nodes()
        .await
        .map_err(|e| err_response(StatusCode::BAD_GATEWAY, &e.to_string()))?;
    let pulled = rows.len();

    let re_embedded = state.with_user(&uid, |s| {
        let embedder = state.shared.embedder.as_ref();
        crate::spanner::hydrate_into(&mut s.core, embedder, &rows)
    });
    // Persist the re-embedded nodes so the local store agrees with Spanner.
    state.with_user(&uid, |s| {
        if let Some(db) = state.db() {
            let ids: Vec<String> = s.core.nodes.keys().cloned().collect();
            for id in ids {
                let _ = s.core.persist_node(db, &uid, &id);
            }
        }
    });

    Ok(Json(SpannerHydrateResponse {
        pulled,
        re_embedded,
    }))
}
