use super::*;
use crate::web::SharedState;
use axum::{
    extract::{Json, Query, State},
    http::{HeaderMap, StatusCode},
};

pub async fn holarchy_ingest_handler(
    State(state): State<SharedState>,
    Json(req): Json<HolarchyIngestRequest>,
) -> Json<serde_json::Value> {
    state.shared.ingest.push(3, req.data.as_bytes().to_vec()); // Default: ThoughtCapture
    Json(serde_json::json!({ "status": "pushed_to_ring" }))
}

/// Assert a relation between two coherence nodes (machines compose graphs, not
/// just nodes). Bridges both endpoints, then adds a holarchy edge. 404 if either
/// id is unknown in the tenant.
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

/// Node/edge counts for the caller's tenant.
///
/// This read the default tenant unconditionally while `/holarchy/edge` writes
/// through `with_user`, so an edge asserted by aion or praxis returned
/// `edge_added` and then showed up in nobody's snapshot — the write landed in
/// the tenant's graph and every reader looked at `_default`. Callers that send
/// no header still get `_default`, so the previous behaviour is unchanged.
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

/// Labelled neighbours of a coherence node — the walkable view of the holarchy.
///
/// `/holarchy/graph` exports `RawNodeKey`s, which are packed slotmap handles: a
/// client can see that an edge exists but not what it joins, so a graph *walk*
/// was impossible without this. Edges are treated as undirected here because
/// browsing asks "what is this connected to", not "which way did the assertion
/// point".
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

/// Full graph export for the caller's tenant (same tenancy fix as the snapshot
/// above: writes were per-tenant, this read was not).
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
