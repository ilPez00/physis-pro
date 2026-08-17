use super::*;
use crate::web::SharedState;
use axum::{extract::{Json, Query, State, Request}, response::{IntoResponse, Response}, http::{StatusCode, HeaderMap}, routing::{get, post}};
use serde::{Deserialize, Serialize};

pub async fn holarchy_ingest_handler(
    State(state): State<SharedState>,
    Json(req): Json<HolarchyIngestRequest>,
) -> Json<serde_json::Value> {
    state.shared.ingest.push(3, req.data.as_bytes().to_vec()); // Default: ThoughtCapture
    Json(serde_json::json!({ "status": "pushed_to_ring" }))
}

pub async fn holarchy_edge_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<EdgeWriteRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let uid = user_key(&headers);
    let ok = state.with_user(&uid, |s| {
        s.core
            .add_edge_between(&req.source, &req.target, req.weight, req.semantic_type)
    });
    if ok {
        Ok(Json(serde_json::json!({ "status": "edge_added" })))
    } else {
        Err(err_response(
            StatusCode::NOT_FOUND,
            "source or target node id not found",
        ))
    }
}

pub async fn holarchy_snapshot_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    let (nodes_count, edges_count) = state.with_user(&uid, |s| {
        (s.core.holarchy.nodes.len(), s.core.holarchy.edges.len())
    });
    Json(serde_json::json!({
        "nodes": nodes_count,
        "edges": edges_count,
    }))
}

pub async fn holarchy_trajectory_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<crate::gantt::Trajectory> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let goals_progress: Vec<(String, f32)> =
            s.goals.iter().map(|g| (g.id.clone(), g.progress)).collect();
        Json(compute_trajectory(&s.scheduler, &goals_progress))
    })
}

pub async fn holarchy_neighbors_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<NeighborsParams>,
) -> Json<Vec<NeighborEdge>> {
    let uid = user_key(&headers);
    let out = state.with_user(&uid, |s| {
        let edges = s.core.resolved_edges();
        edges
            .iter()
            .filter_map(|(src, tgt, weight)| {
                let (a, b) = (src.as_deref()?, tgt.as_deref()?);
                let other = if a == params.id {
                    b
                } else if b == params.id {
                    a
                } else {
                    return None;
                };
                let node = s.core.nodes.get(other)?;
                Some(NeighborEdge {
                    id: other.to_string(),
                    label: node.label.clone().unwrap_or_default(),
                    weight: *weight,
                    asserted: node.asserted,
                })
            })
            .collect::<Vec<_>>()
    });
    Json(out)
}

pub async fn holarchy_graph_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<GraphResponse> {
    let uid = user_key(&headers);
    let (nodes, edges) = state.with_user(&uid, |s| {
        let nodes: Vec<_> = s
            .core
            .holarchy
            .nodes
            .iter()
            .map(|(k, p)| (crate::graph::RawNodeKey::from(k), *p))
            .collect();
        (nodes, s.core.holarchy.edges.clone())
    });
    Json(GraphResponse { nodes, edges })
}

