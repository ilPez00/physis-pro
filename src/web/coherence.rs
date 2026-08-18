use super::*;
use crate::web::SharedState;
use axum::{extract::{Json, Query, State}, http::{StatusCode, HeaderMap}};

pub async fn coherence_snapshot_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<CoherenceSnapshot> {
    let uid = user_key(&headers);
    Json(state.with_user(&uid, |s| s.core.snapshot()))
}

/// Recall registered coherence nodes by content similarity — across every
/// machine that wrote into the shared graph. Unlike `/query` (mapper/ontology
/// surface), this reads the coherence nodes that `/coherence/register` creates.
pub async fn coherence_nodes_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<QueryParams>,
) -> Json<CoherenceNodesResponse> {
    let emb = embed_text(state.shared.clone(), params.q.clone()).await;
    let uid = user_key(&headers);
    let shared = state.shared.clone();
    // Project inside the same lock as the search: a recalled node is far more
    // useful when the caller can see its verdict and which cell it sits in.
    let results: Vec<NodeView> = state.with_user(&uid, |s| {
        s.core
            .search_nodes(&emb, params.max)
            .into_iter()
            .filter_map(|(id, _label, _score)| {
                let node = s.core.nodes.get(&id)?;
                Some(node.observable(auto_cell(&shared, node)))
            })
            .collect()
    });
    Json(CoherenceNodesResponse {
        count: results.len(),
        results,
    })
}

/// List ALL coherence nodes for the tenant (id + label + score), capped + sorted
/// by score desc. Unlike `/coherence/nodes` (similarity search, needs a query),
/// this enumerates the graph so a client can render/browse it without a query.
/// Per-tenant — NOT exposed via the public proxy.
pub async fn coherence_nodes_all_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<AllNodesParams>,
) -> Json<CoherenceNodesResponse> {
    let uid = user_key(&headers);
    let shared = state.shared.clone();
    let results: Vec<NodeView> = state.with_user(&uid, |s| {
        // Rank and cut BEFORE projecting: deriving the auto cell costs a pass
        // over the grid centroids per node, so classifying the whole store only
        // to discard all but `limit` would make browsing O(store) instead of
        // O(limit).
        let mut ranked: Vec<&CoherenceNode> = s.core.nodes.values().collect();
        ranked.sort_by(|a, b| {
            b.coherence_score
                .partial_cmp(&a.coherence_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        ranked.truncate(params.limit);
        ranked
            .into_iter()
            .map(|n| n.observable(auto_cell(&shared, n)))
            .collect()
    });
    Json(CoherenceNodesResponse {
        count: results.len(),
        results,
    })
}

pub async fn coherence_register_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CoherenceRegisterRequest>,
) -> Json<CoherenceRegisterResponse> {
    let embedding = embed_text(state.shared.clone(), req.input.clone()).await;
    let uid = user_key(&headers);
    // Retain the source text so the node can be recalled/browsed by content.
    let label = Some(req.input.clone());
    let (node_id, asserted) = state.with_user(&uid, |s| {
        let id = s.core.register_node_vec_labeled(embedding, label);
        // Apply the verdict AFTER registration, so re-registering an existing
        // label (dedupe returns the same id) updates its outcome rather than
        // being ignored — that is how a task reported optimistically at spawn
        // gets corrected when it later fails.
        if let Some(score) = req.coherence_score {
            s.core.assert_coherence(&id, score);
        }
        // Write through so the node survives a restart (dedupe makes a repeat
        // registration idempotent here too — same id, same key, same bytes).
        if let Some(db) = state.db() {
            if let Err(e) = s.core.persist_node(db, &uid, &id) {
                log::warn!("coherence node {id} not persisted: {e}");
            }
        }
        let asserted = s.core.nodes.get(&id).and_then(|n| n.asserted);
        (id, asserted)
    });
    Json(CoherenceRegisterResponse { node_id, asserted })
}

/// POST /api/v1/coherence/edit — exchange a node's content while keeping its
/// function (identity + structure). Re-embeds the new label, preserves id /
/// asserted / edges / provenance, re-derives the cell (auto), applies an
/// optional pin, and writes the node through to sled.
pub async fn coherence_edit_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CoherenceEditRequest>,
) -> Result<Json<CoherenceEditResponse>, (StatusCode, Json<ErrorResponse>)> {
    if req.id.trim().is_empty() || req.label.trim().is_empty() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "id and label are required",
        ));
    }
    // Validate the pin against the semiotic grid before mutating anything.
    if let Some(Some(pin)) = &req.cell {
        if HumanDomain::from_str(&pin.domain).is_none() || HumanMode::from_str(&pin.mode).is_none()
        {
            return Err(err_response(
                StatusCode::BAD_REQUEST,
                &format!(
                    "invalid cell pin {}×{} — expected a HUMAN domain × mode",
                    pin.domain, pin.mode
                ),
            ));
        }
    }
    let uid = user_key(&headers);

    // Old embedding → before-cell (computed before the swap).
    let before_embedding = state.with_user(&uid, |s| {
        s.core.nodes.get(&req.id).map(|n| n.embedding.clone())
    });
    let Some(before_embedding) = before_embedding else {
        return Err(err_response(
            StatusCode::NOT_FOUND,
            &format!("node {} not found", req.id),
        ));
    };

    let shared = state.shared.clone();
    let new_embedding = embed_text(shared.clone(), req.label.clone()).await;

    let auto_cell = shared
        .classifier
        .classify(&new_embedding)
        .first()
        .map(|c| CellPin {
            domain: c.domain.clone(),
            mode: c.mode.clone(),
        });
    let before_cell = shared
        .classifier
        .classify(&before_embedding)
        .first()
        .map(|c| CellPin {
            domain: c.domain.clone(),
            mode: c.mode.clone(),
        });

    let pin = match req.cell {
        None => crate::core::PinEdit::Keep,
        Some(None) => crate::core::PinEdit::Clear,
        Some(Some(p)) => crate::core::PinEdit::Set(p.domain, p.mode),
    };
    let outcome = state.with_user(&uid, |s| {
        let outcome = s.core.edit_node(
            &req.id,
            &req.label,
            new_embedding,
            pin,
            Some(shared.embedder_kind),
        );
        if let Ok(o) = &outcome {
            if let Some(db) = state.db() {
                if let Err(e) = s.core.persist_node(db, &uid, &o.node_id) {
                    log::warn!("edited node {} not persisted: {e}", o.node_id);
                }
            }
        }
        outcome
    });
    let outcome = outcome.map_err(|e| err_response(StatusCode::NOT_FOUND, &e.to_string()))?;

    // A pin is a labelled correction: the editor has told us both where the
    // node belongs and, when it differs from `auto_cell`, which cell the
    // classifier got wrong. `discovery.rs` can only find texts the ontology
    // cannot name at all; this is strictly better evidence, and it arrives as a
    // by-product of ordinary editing rather than a labelling task.
    if let Some((pd, pm)) = &outcome.pinned_cell {
        let pinned_key = cell_key(pd, pm);
        let mut quality = state.quality.lock().unwrap_or_else(|e| e.into_inner());
        match &auto_cell {
            Some(auto) if auto.domain == *pd && auto.mode == *pm => {
                // The human agreed with the classifier — confirmation, not noise.
                quality.report_success(&pinned_key);
            }
            Some(auto) => {
                quality.penalize_cell(&cell_key(&auto.domain, &auto.mode), PIN_DISSENT_SEVERITY);
                quality.report_success(&pinned_key);
            }
            // Nothing classified: the pin is the only signal, so there is no
            // cell to penalize — only the pinned one to reinforce.
            None => quality.report_success(&pinned_key),
        }
    }

    Ok(Json(CoherenceEditResponse {
        ok: true,
        node_id: outcome.node_id,
        label: outcome.label,
        auto_cell,
        pinned_cell: outcome
            .pinned_cell
            .map(|(domain, mode)| CellPin { domain, mode }),
        before_cell,
        asserted: outcome.asserted,
        coherence_score: outcome.coherence_score,
    }))
}

/// GET /api/v1/quality — what the classifier has learned from feedback.
///
/// The tracker has always been wired into scoring (`adjust_score` demotes
/// penalized cells on every classify) but had no reader, so a cell could be
/// actively suppressed with nothing able to say so. Process-wide, not
/// per-tenant: the penalties apply to the shared classifier.
pub async fn quality_handler(State(state): State<SharedState>) -> Json<QualityResponse> {
    let quality = state.quality.lock().unwrap_or_else(|e| e.into_inner());
    Json(QualityResponse {
        penalties: quality.cell_penalties.clone(),
        boosts: quality.cell_boosts.clone(),
        failures: quality.failures.len(),
    })
}

/// POST /api/v1/coherence/delete — remove a node and every structure referencing
/// it (dedupe index, holarchy node + edges, persisted sled row).
pub async fn coherence_delete_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CoherenceDeleteRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let uid = user_key(&headers);
    let removed = state.with_user(&uid, |s| {
        let ok = s.core.delete_node(&req.id);
        if ok {
            if let Some(db) = state.db() {
                let key = crate::core::PhysisCore::node_key(&uid, &req.id);
                let _ = db.remove(key);
            }
        }
        ok
    });
    if !removed {
        return Err(err_response(
            StatusCode::NOT_FOUND,
            &format!("node {} not found", req.id),
        ));
    }
    Ok(Json(serde_json::json!({ "ok": true, "node_id": req.id })))
}

