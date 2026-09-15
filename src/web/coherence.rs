use super::*;
use crate::web::SharedState;
use axum::{
    extract::{Json, Path, Query, State},
    http::{HeaderMap, StatusCode},
};

pub async fn coherence_snapshot_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<CoherenceSnapshot> {
    let uid = user_key(&headers);
    Json(state.with_user(&uid, |s| s.core.snapshot()))
}

/// GET /api/v1/coherence/edges?node_id=X — the named typed links (Phase 16 I6)
/// touching a node, both directions: `outgoing` (forward traversal by link
/// name) and `incoming` (inverse traversal — what cites/causes/precedes it).
#[derive(Deserialize)]
pub struct EdgeQuery {
    node_id: String,
}

#[derive(Serialize)]
pub struct EdgeProjection {
    pub id: String,
    pub relation: String,
    pub source_id: String,
    pub target_id: String,
    pub confidence: f32,
    pub weight: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
}

#[derive(Serialize)]
pub struct NodeEdgesResponse {
    pub node_id: String,
    pub outgoing: Vec<EdgeProjection>,
    pub incoming: Vec<EdgeProjection>,
}

pub async fn coherence_edges_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<EdgeQuery>,
) -> Json<NodeEdgesResponse> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let project = |e: &physis_core::relation::TypedEdge| EdgeProjection {
            id: e.id.clone(),
            relation: format!("{:?}", e.relation_type),
            source_id: e.source_id.clone(),
            target_id: e.target_id.clone(),
            confidence: e.confidence,
            weight: e.weight,
            provenance: e.provenance.clone(),
        };
        Json(NodeEdgesResponse {
            node_id: q.node_id.clone(),
            outgoing: s.core.edges_from(&q.node_id).into_iter().map(project).collect(),
            incoming: s.core.edges_to(&q.node_id).into_iter().map(project).collect(),
        })
    })
}

/// POST /api/v1/coherence/add-edge — write a named typed link (Phase 16 I6).
#[derive(Deserialize)]
pub struct AddEdgeRequest {
    source: String,
    /// RelationType variant name, e.g. "Causes", "Supports", "Precedes".
    relation: String,
    target: String,
    #[serde(default = "default_conf")]
    confidence: f32,
    #[serde(default = "default_w")]
    weight: f32,
}

fn default_conf() -> f32 {
    1.0
}
fn default_w() -> f32 {
    1.0
}

#[derive(Serialize)]
pub struct AddEdgeResponse {
    pub edge_id: String,
}

pub async fn coherence_add_edge_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<AddEdgeRequest>,
) -> Result<Json<AddEdgeResponse>, (StatusCode, Json<serde_json::Value>)> {
    use physis_core::relation::RelationType;
    let relation = match req.relation.as_str() {
        "Supports" => RelationType::Supports,
        "Contradicts" => RelationType::Contradicts,
        "Causes" => RelationType::Causes,
        "Influences" => RelationType::Influences,
        "Precedes" => RelationType::Precedes,
        "Follows" => RelationType::Follows,
        other => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("unknown relation '{other}' — Supports|Contradicts|Causes|Influences|Precedes|Follows")
                })),
            ));
        }
    };
    let uid = user_key(&headers);
    // Reject unknown endpoints so a typo'd id fails loudly instead of silently
    // recording an edge to nothing.
    let known = state.with_user(&uid, |s| {
        s.core.nodes.contains_key(&req.source) && s.core.nodes.contains_key(&req.target)
    });
    if !known {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "source and target must be existing node ids"})),
        ));
    }
    let edge_id = state.with_user(&uid, |s| {
        s.core
            .add_typed_edge(&req.source, relation, &req.target, req.confidence, req.weight)
    });
    // Audit the edge write (Phase 16 I9).
    state.with_user(&uid, |s| {
        s.audit.record(
            "edge:add",
            &edge_id,
            None,
            &uid,
            format!("{} → {} ({})", req.source, req.target, req.relation),
        );
        if let Some(db) = state.db() {
            let _ = s.audit.save(db, &uid);
        }
    });
    Ok(Json(AddEdgeResponse { edge_id }))
}

/// GET /api/v1/audit?limit=&node_id=&as_of= — the append-only mutation log
/// (Phase 16 I9/I10). Newest-first by default; filter by a single node's change
/// history (`node_id=`) or a point in time (`as_of=RFC3339`) for the
/// time-travel slice. The audit/trust layer an industrial buyer audits.
#[derive(Deserialize)]
pub struct AuditQuery {
    limit: Option<usize>,
    node_id: Option<String>,
    as_of: Option<String>,
}

pub async fn audit_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<AuditQuery>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let entries: Vec<&crate::audit::AuditEntry> = if let Some(cut) = q
            .as_of
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Utc))
        {
            s.audit.as_of(cut)
        } else if let Some(id) = &q.node_id {
            s.audit.for_subject(id)
        } else {
            s.audit.recent(q.limit.unwrap_or(50))
        };
        Json(serde_json::json!({ "count": entries.len(), "entries": entries }))
    })
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
                let mut view = node.observable(auto_cell(&shared, node));
                // Populate instances for the top search result (caller's focus)
                // Others get an empty Vec for efficiency.
                if view.fragment.is_none() {
                    view.fragment = node.extract_fragment();
                }
                Some(view)
            })
            .collect()
    });
    Json(CoherenceNodesResponse {
        count: results.len(),
        results,
        as_of: None,
    })
}

/// Structured, agent-facing query over the live coherence graph — the
/// "objects" API PRODUCT_RETHINK.md called for. Filter nodes by outcome
/// verdict, semiotic cell, and/or label substring, then project to `NodeView`.
/// Reused by both the HTTP handler and the `physis_coherence_query` MCP tool so
/// the two surfaces never diverge.
pub fn query_coherence_nodes(
    nodes: &std::collections::HashMap<String, CoherenceNode>,
    classifier: &CellClassifier,
    verdict: Option<crate::models::CoherenceRating>,
    cell: Option<(String, String)>,
    q: Option<&str>,
    limit: usize,
) -> Vec<NodeView> {
    let q = q.map(|s| s.to_ascii_lowercase());
    let mut matched: Vec<&CoherenceNode> = nodes
        .values()
        .filter(|n| {
            if let Some(want) = verdict {
                if n.rating() != Some(want) {
                    return false;
                }
            }
            if let Some((d, m)) = &cell {
                // A pin outranks the classifier: the node occupies `cell_pin`
                // when set, else the classifier's top hit.
                let occupied = n
                    .cell_pin
                    .clone()
                    .or_else(|| {
                        classifier
                            .classify(&n.embedding)
                            .first()
                            .map(|c| (c.domain.clone(), c.mode.clone()))
                    });
                if occupied.as_ref() != Some(&(d.clone(), m.clone())) {
                    return false;
                }
            }
            if let Some(q) = &q {
                if !n
                    .label
                    .as_deref()
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .contains(q)
                {
                    return false;
                }
            }
            true
        })
        .collect();
    // Highest-density first — same ordering the browse endpoint uses.
    matched.sort_by(|a, b| {
        b.coherence_score
            .partial_cmp(&a.coherence_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    matched
        .into_iter()
        .take(limit)
        .map(|n| {
            let mut view = n.observable(classifier.classify(&n.embedding).first().map(|c| (c.domain.clone(), c.mode.clone())));
            if view.fragment.is_none() {
                view.fragment = n.extract_fragment();
            }
            view
        })
        .collect()
}

#[derive(Deserialize)]
pub struct CoherenceQueryParams {
    /// Only nodes with this asserted outcome: `success` | `inert` | `failure`.
    #[serde(default)]
    verdict: Option<String>,
    /// Only nodes in this semiotic cell, given as `domain|mode` (e.g. `Finance|Procure`).
    #[serde(default)]
    cell: Option<String>,
    /// Substring match against the node label (case-insensitive).
    #[serde(default)]
    q: Option<String>,
    #[serde(default = "default_all_limit")]
    limit: usize,
}

/// `GET /api/v1/coherence/query` — typed facet query over the live coherence
/// graph. Returns the same `NodeView` projection as `/coherence/nodes/all`, so
/// clients can ask "every failed node in Finance|Procure" instead of pulling
/// the whole store and filtering client-side.
pub async fn coherence_facet_query_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<CoherenceQueryParams>,
) -> Json<CoherenceNodesResponse> {
    let verdict = match params.verdict.as_deref() {
        None | Some("") => None,
        Some("success") => Some(crate::models::CoherenceRating::Success),
        Some("inert") => Some(crate::models::CoherenceRating::Inert),
        Some("failure") => Some(crate::models::CoherenceRating::Failure),
        // An unrecognised verdict is not fatal — treat as "no verdict filter".
        Some(_) => None,
    };
    let cell = params.cell.as_ref().and_then(|c| {
        let mut it = c.splitn(2, '|');
        match (it.next(), it.next()) {
            (Some(d), Some(m)) if !d.is_empty() && !m.is_empty() => {
                Some((d.to_string(), m.to_string()))
            }
            _ => None,
        }
    });
    let uid = user_key(&headers);
    let results: Vec<NodeView> = state.with_user(&uid, |s| {
        query_coherence_nodes(
            &s.core.nodes,
            &state.shared.classifier,
            verdict,
            cell.clone(),
            params.q.as_deref(),
            params.limit,
        )
    });
    Json(CoherenceNodesResponse {
        count: results.len(),
        results,
        as_of: None,
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
    // Parse the optional point-in-time cutoff once. A bad RFC3339 value is not
    // fatal — we surface an empty/erroneous view rather than 500ing the browse.
    let as_of: Option<chrono::DateTime<chrono::Utc>> = params
        .as_of
        .as_ref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));
    let results: Vec<NodeView> = state.with_user(&uid, |s| {
        // Rank and cut BEFORE projecting: deriving the auto cell costs a pass
        // over the grid centroids per node, so classifying the whole store only
        // to discard all but `limit` would make browsing O(store) instead of
        // O(limit).
        let mut ranked: Vec<&CoherenceNode> = s.core.nodes.values().collect();
        // Point-in-time replay: keep only nodes that existed at or before `as_of`.
        if let Some(cut) = as_of {
            ranked.retain(|n| n.created_at <= cut);
        }
        ranked.sort_by(|a, b| {
            b.coherence_score
                .partial_cmp(&a.coherence_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        ranked.truncate(params.limit);
        ranked
            .into_iter()
            .map(|n| {
                let mut view = n.observable(auto_cell(&shared, n));
                if view.fragment.is_none() {
                    view.fragment = n.extract_fragment();
                }
                view
            })
            .collect()
    });
    Json(CoherenceNodesResponse {
        count: results.len(),
        results,
        as_of: params.as_of.clone(),
    })
}

/// GET /api/v1/coherence/nodes/{id}/instances — find all nodes that share
/// conceptual similarity with a given node. Returns `ConceptualInstance`
/// projections with id, label, score, origin path, and cell.
pub async fn coherence_node_instances_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(node_id): Path<String>,
    Query(params): Query<InstanceParams>,
) -> Result<Json<InstanceResponse>, (StatusCode, Json<ErrorResponse>)> {
    let uid = user_key(&headers);
    let shared = state.shared.clone();

    let instances: Vec<crate::models::ConceptualInstance> = state.with_user(&uid, |s| {
        s.core.find_conceptual_instances(
            &node_id,
            &shared.classifier,
            params.threshold.unwrap_or(0.3),
            params.limit.unwrap_or(100),
        )
    });

    if instances.is_empty() {
        // Check if the node exists at all
        let exists = state.with_user(&uid, |s| s.core.nodes.contains_key(&node_id));
        if !exists {
            return Err(err_response(StatusCode::NOT_FOUND, &format!("node {} not found", node_id)));
        }
    }

    Ok(Json(InstanceResponse {
        node_id,
        count: instances.len(),
        threshold: params.threshold.unwrap_or(0.3),
        instances,
    }))
}

#[derive(Deserialize)]
pub struct InstanceParams {
    /// Minimum cosine similarity threshold (default 0.3).
    #[serde(default = "default_threshold")]
    threshold: Option<f32>,
    /// Maximum number of instances to return (default 100).
    #[serde(default = "default_limit")]
    limit: Option<usize>,
}

fn default_threshold() -> Option<f32> { Some(0.3) }
fn default_limit() -> Option<usize> { Some(100) }

#[derive(Serialize)]
pub struct InstanceResponse {
    pub node_id: String,
    pub count: usize,
    pub threshold: f32,
    pub instances: Vec<crate::models::ConceptualInstance>,
}

pub async fn coherence_register_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CoherenceRegisterRequest>,
) -> Json<CoherenceRegisterResponse> {
    let embedding = embed_text(state.shared.clone(), req.input.clone()).await;
    // Auto-stamp the object kind from the winning cell (Phase 16 I7) when the
    // caller did not supply one. Computed before `embedding` is moved into
    // registration.
    let auto_kind = if req.kind.is_none() {
        state
            .shared
            .classifier
            .classify(&embedding)
            .first()
            .and_then(|c| crate::models::object_type_for(&c.domain, &c.mode))
    } else {
        None
    };
    let uid = user_key(&headers);
    // Retain the source text so the node can be recalled/browsed by content.
    let label = Some(req.input.clone());
    let (node_id, asserted) = state.with_user(&uid, |s| {
        let id = s.core.register_node_vec_labeled(embedding, label);
        // Stamp the typed-ontology axis (Phase 16 I5): object kind + properties.
        // `register_node_vec_labeled` dedupes by label, so this updates the
        // existing node rather than creating a second one.
        if let Some(node) = s.core.nodes.get_mut(&id) {
            if let Some(kind) = &req.kind {
                node.kind = kind.clone();
            } else if node.kind.is_empty() {
                if let Some(t) = auto_kind {
                    node.kind = t.to_string();
                }
            }
            if let Some(props) = &req.properties {
                node.properties = props.clone();
            }
        }
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
        // Audit the mutation (Phase 16 I9): the append-only log records every
        // registration and every verdict, sealed, with the actor.
        let node_kind = s.core.nodes.get(&id).and_then(|n| {
            if n.kind.is_empty() {
                None
            } else {
                Some(n.kind.clone())
            }
        });
        s.audit.record(
            "register",
            &id,
            node_kind.clone(),
            &uid,
            format!("label: {}", req.input),
        );
        if req.coherence_score.is_some() {
            s.audit.record(
                "assert",
                &id,
                node_kind,
                &uid,
                format!("verdict {}", req.coherence_score.unwrap_or(0.0)),
            );
        }
        if let Some(db) = state.db() {
            let _ = s.audit.save(db, &uid);
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
            // Audit the content edit (Phase 16 I9).
            s.audit.record(
                "edit",
                &o.node_id,
                s.core
                    .nodes
                    .get(&o.node_id)
                    .and_then(|n| if n.kind.is_empty() { None } else { Some(n.kind.clone()) }),
                &uid,
                format!("label → {}", req.label),
            );
            if let Some(db) = state.db() {
                let _ = s.audit.save(db, &uid);
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
    let boosts: HashMap<String, f32> = quality
        .cell_boosts
        .iter()
        .map(|(k, b)| (k.clone(), *b))
        .collect();
    Json(quality_response(
        quality.cell_penalties.clone(),
        boosts,
        &quality.failures,
    ))
}

/// Pure payload builder for [`quality_handler`] — the trend/aggregation logic
/// is unit-tested here rather than driven through HTTP (no shipped endpoint
/// creates failure *records* cheaply; pin dissents only `penalize_cell`).
pub(crate) fn quality_response(
    penalties: HashMap<String, f32>,
    boosts: HashMap<String, f32>,
    failures: &[crate::quality::QualityFailure],
) -> QualityResponse {
    let resolved = failures.iter().filter(|f| f.resolved).count();
    let mean_severity = if failures.is_empty() {
        0.0
    } else {
        failures.iter().map(|f| f.severity).sum::<f32>() / failures.len() as f32
    };
    // Newest first: the dashboard reads the recent window, not the archive.
    let mut ordered: Vec<&crate::quality::QualityFailure> = failures.iter().collect();
    ordered.sort_by_key(|f| std::cmp::Reverse(f.timestamp));
    let records = ordered
        .into_iter()
        .map(|f| QualityFailureView {
            feedback: {
                let mut s: String = f.feedback.chars().take(160).collect();
                if f.feedback.chars().count() > 160 {
                    s.push('…');
                }
                s
            },
            corrected_to: f.correct_domain.clone(),
            id: f.id.clone(),
            at: f.timestamp.to_rfc3339(),
            top_cell: display_cell(&f.top_cell),
            top_score: f.top_score,
            severity: f.severity,
            resolved: f.resolved,
        })
        .collect();

    QualityResponse {
        penalties,
        boosts,
        failures: failures.len(),
        open: failures.len() - resolved,
        resolved,
        mean_severity,
        records,
    }
}

/// `DOMAIN\x00MODE` store key → `DOMAIN/MODE` display form (QA 8.5 parity with
/// the physis-core summary renderer; the NUL never reaches a browser).
fn display_cell(key: &str) -> String {
    key.replace('\0', "/")
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
            // Audit the deletion before the node is gone (Phase 16 I9).
            s.audit.record("delete", &req.id, None, &uid, "node deleted");
            if let Some(db) = state.db() {
                let _ = s.audit.save(db, &uid);
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

// ── Epistemic Evolution & GUI Web Handlers ───────────────────────────

use physis_core::coherence_query::{EpistemicQuery, EpistemicQueryResult};
use physis_core::contradiction::{Contradiction, ContradictionParty, ResolutionStatus};
use physis_core::epistemic::EpistemicEvent;
use physis_core::explanation::ExplanationReport;
use physis_core::hypothesis::{
    Evidence, EvidencePolarity, Hypothesis, HypothesisStatus, Prediction,
};
use crate::ported::quality::FitnessContext;

#[derive(Debug, Deserialize)]
pub struct HypothesesQueryParams {
    pub status: Option<String>,
    pub limit: Option<usize>,
    pub min_fitness: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateHypothesisRequest {
    pub statement: String,
    pub assumptions: Option<Vec<String>>,
    pub confidence: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct AddEvidenceRequest {
    pub source: String,
    pub polarity: String, // "Supports" or "Contradicts"
    pub confidence: f32,
    pub claim: String,
    pub context: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct RecordPredictionRequest {
    pub statement: String,
    pub expected_outcome: Option<String>,
    pub actual_outcome: Option<String>,
    pub correct: Option<bool>,
    pub confidence: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct TransitionHypothesisRequest {
    pub status: String,
    pub reason: String,
    pub trigger_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateContradictionRequest {
    pub claim_a: ContradictionParty,
    pub claim_b: ContradictionParty,
    pub resolution: Option<ResolutionStatus>,
    pub explanation: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResolveContradictionRequest {
    pub resolution: ResolutionStatus,
    pub explanation: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReplayQueryParams {
    pub subject_id: String,
    pub at: String,
}

#[derive(Debug, Deserialize)]
pub struct FitnessFeedbackRequest {
    pub interpretation: String,
    pub is_success: bool,
    pub feedback: Option<String>,
    pub context: FitnessContext,
}

#[derive(Debug, Deserialize)]
pub struct DiscoverOntologyRequest {
    pub texts: Vec<String>,
    pub coverage_threshold: Option<f32>,
    pub cluster_sim: Option<f32>,
    pub min_cluster: Option<usize>,
}

/// GET /api/v1/hypotheses — list competing hypotheses ranked by fitness.
pub async fn hypotheses_list_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<HypothesesQueryParams>,
) -> Json<Vec<Hypothesis>> {
    let uid = user_key(&headers);
    let results = state.with_user(&uid, |s| {
        let mut list: Vec<Hypothesis> = s.core.hypotheses.values().cloned().collect();
        if let Some(ref status_str) = params.status {
            list.retain(|h| format!("{:?}", h.status).eq_ignore_ascii_case(status_str));
        }
        if let Some(min_fit) = params.min_fitness {
            list.retain(|h| h.fitness >= min_fit);
        }
        list.sort_by(|a, b| {
            b.fitness
                .partial_cmp(&a.fitness)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if let Some(lim) = params.limit {
            list.truncate(lim);
        }
        list
    });
    Json(results)
}

/// POST /api/v1/hypotheses — register a new hypothesis.
pub async fn hypothesis_create_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CreateHypothesisRequest>,
) -> (StatusCode, Json<Hypothesis>) {
    let embedding = embed_text(state.shared.clone(), req.statement.clone()).await;
    let uid = user_key(&headers);
    let created = state.with_user(&uid, |s| {
        let mut hyp = Hypothesis::new(req.statement, embedding);
        if let Some(assumptions) = req.assumptions {
            hyp.assumptions = assumptions;
        }
        if let Some(conf) = req.confidence {
            hyp.confidence = conf;
        }
        let id = s.core.register_hypothesis(hyp.clone());
        s.core.hypothesis(&id).cloned().unwrap_or(hyp)
    });
    (StatusCode::CREATED, Json(created))
}

/// GET /api/v1/hypotheses/:id — retrieve a single hypothesis with all evidence.
pub async fn hypothesis_get_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Hypothesis>, (StatusCode, Json<ErrorResponse>)> {
    let uid = user_key(&headers);
    let hyp = state.with_user(&uid, |s| s.core.hypothesis(&id).cloned());
    match hyp {
        Some(h) => Ok(Json(h)),
        None => Err(err_response(
            StatusCode::NOT_FOUND,
            &format!("Hypothesis {} not found", id),
        )),
    }
}

/// POST /api/v1/hypotheses/:id/evidence — add supporting or contradicting evidence.
pub async fn hypothesis_evidence_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(req): Json<AddEvidenceRequest>,
) -> Result<Json<Hypothesis>, (StatusCode, Json<ErrorResponse>)> {
    let uid = user_key(&headers);
    let updated = state.with_user(&uid, |s| {
        let hyp = s.core.hypothesis_mut(&id)?;
        let pol = if req.polarity.eq_ignore_ascii_case("Contradicts") {
            EvidencePolarity::Contradicts
        } else {
            EvidencePolarity::Supports
        };
        let ev = Evidence {
            source: req.source,
            polarity: pol,
            confidence: req.confidence,
            claim: req.claim,
            observed_at: Some(chrono::Utc::now()),
            embedding: Vec::new(),
            context: req.context.unwrap_or_default(),
            intake_id: None,
        };
        match pol {
            EvidencePolarity::Supports => hyp.add_supporting_evidence(ev),
            EvidencePolarity::Contradicts => hyp.add_contradicting_evidence(ev),
        }
        Some(hyp.clone())
    });
    match updated {
        Some(h) => Ok(Json(h)),
        None => Err(err_response(
            StatusCode::NOT_FOUND,
            &format!("Hypothesis {} not found", id),
        )),
    }
}

/// POST /api/v1/hypotheses/:id/prediction — record a testable prediction outcome.
pub async fn hypothesis_prediction_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(req): Json<RecordPredictionRequest>,
) -> Result<Json<Hypothesis>, (StatusCode, Json<ErrorResponse>)> {
    let uid = user_key(&headers);
    let updated = state.with_user(&uid, |s| {
        let hyp = s.core.hypothesis_mut(&id)?;
        let pred = Prediction {
            statement: req.statement,
            expected_outcome: req.expected_outcome,
            made_at: chrono::Utc::now(),
            observed_at: if req.actual_outcome.is_some() {
                Some(chrono::Utc::now())
            } else {
                None
            },
            actual_outcome: req.actual_outcome,
            correct: req.correct,
            confidence: req.confidence.unwrap_or(0.8),
        };
        hyp.record_prediction(pred);
        Some(hyp.clone())
    });
    match updated {
        Some(h) => Ok(Json(h)),
        None => Err(err_response(
            StatusCode::NOT_FOUND,
            &format!("Hypothesis {} not found", id),
        )),
    }
}

/// POST /api/v1/hypotheses/:id/transition — explicitly update status with trigger logging.
pub async fn hypothesis_transition_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(req): Json<TransitionHypothesisRequest>,
) -> Result<Json<Hypothesis>, (StatusCode, Json<ErrorResponse>)> {
    let uid = user_key(&headers);
    let new_status = match req.status.to_lowercase().as_str() {
        "candidate" => HypothesisStatus::Candidate,
        "supported" => HypothesisStatus::Supported,
        "contradicted" => HypothesisStatus::Contradicted,
        "confirmed" => HypothesisStatus::Confirmed,
        "inert" => HypothesisStatus::Inert,
        "failed" => HypothesisStatus::Failed,
        "superseded" => HypothesisStatus::Superseded,
        "isolated" => HypothesisStatus::Isolated,
        "certified" => HypothesisStatus::Certified,
        _ => {
            return Err(err_response(
                StatusCode::BAD_REQUEST,
                &format!("Invalid status: {}", req.status),
            ))
        }
    };

    let updated = state.with_user(&uid, |s| {
        s.core
            .transition_hypothesis(&id, new_status, &req.reason, req.trigger_id);
        s.core.hypothesis(&id).cloned()
    });
    match updated {
        Some(h) => Ok(Json(h)),
        None => Err(err_response(
            StatusCode::NOT_FOUND,
            &format!("Hypothesis {} not found", id),
        )),
    }
}

/// GET /api/v1/explanation/:id — get structured explanation report for hypothesis.
pub async fn explanation_get_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<ExplanationReport>, (StatusCode, Json<ErrorResponse>)> {
    let uid = user_key(&headers);
    let report = state.with_user(&uid, |s| s.core.full_explanation_report(&id));
    match report {
        Some(r) => Ok(Json(r)),
        None => Err(err_response(
            StatusCode::NOT_FOUND,
            &format!("No explanation available for {}", id),
        )),
    }
}

/// POST /api/v1/coherence/query — epistemic queries (contradictions, gaps, precedents).
pub async fn coherence_query_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(query): Json<EpistemicQuery>,
) -> Json<EpistemicQueryResult> {
    let uid = user_key(&headers);
    let res = state.with_user(&uid, |s| s.core.query_coherence(&query));
    Json(res)
}

/// GET /api/v1/contradictions — list all recorded contradictions.
pub async fn contradictions_list_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<Vec<Contradiction>> {
    let uid = user_key(&headers);
    let list = state.with_user(&uid, |s| s.core.contradictions.clone());
    Json(list)
}

/// POST /api/v1/contradictions — record a new conflicting claim pair.
pub async fn contradiction_create_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CreateContradictionRequest>,
) -> (StatusCode, Json<Contradiction>) {
    let uid = user_key(&headers);
    let created = state.with_user(&uid, |s| {
        let mut contra = Contradiction::new(req.claim_a, req.claim_b);
        if let Some(res) = req.resolution {
            contra.resolution = res;
        }
        if let Some(exp) = req.explanation {
            contra.explanation = Some(exp);
        }
        let id = s.core.record_contradiction(contra.clone());
        s.core
            .contradictions
            .iter()
            .find(|c| c.id == id)
            .cloned()
            .unwrap_or(contra)
    });
    (StatusCode::CREATED, Json(created))
}

/// POST /api/v1/contradictions/:id/resolve — resolve a contradiction.
pub async fn contradiction_resolve_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(req): Json<ResolveContradictionRequest>,
) -> Result<Json<Contradiction>, (StatusCode, Json<ErrorResponse>)> {
    let uid = user_key(&headers);
    let updated = state.with_user(&uid, |s| {
        let c = s.core.contradictions.iter_mut().find(|c| c.id == id)?;
        c.resolution = req.resolution;
        if let Some(exp) = req.explanation {
            c.explanation = Some(exp);
        }
        Some(c.clone())
    });
    match updated {
        Some(c) => Ok(Json(c)),
        None => Err(err_response(
            StatusCode::NOT_FOUND,
            &format!("Contradiction {} not found", id),
        )),
    }
}

/// GET /api/v1/epistemic/audit — get epistemic audit timeline.
pub async fn epistemic_audit_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<Vec<EpistemicEvent>> {
    let uid = user_key(&headers);
    let events = state.with_user(&uid, |s| s.core.epistemic_audit.events.clone());
    Json(events)
}

/// GET /api/v1/epistemic/replay — reconstruct belief at past timestamp.
pub async fn epistemic_replay_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<ReplayQueryParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let uid = user_key(&headers);
    let target_time = match chrono::DateTime::parse_from_rfc3339(&params.at) {
        Ok(dt) => dt.with_timezone(&chrono::Utc),
        Err(e) => {
            return Err(err_response(
                StatusCode::BAD_REQUEST,
                &format!("Invalid timestamp {}: {}", params.at, e),
            ))
        }
    };

    let belief = state.with_user(&uid, |s| {
        s.core
            .reconstruct_belief_at(&params.subject_id, target_time)
    });

    Ok(Json(serde_json::json!({
        "subject_id": params.subject_id,
        "at": target_time,
        "reconstructed_status": belief,
    })))
}

/// GET /api/v1/processes — list operational cycles and process task graph.
pub async fn processes_list_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    let cycles = state.with_user(&uid, |s| s.core.process_cycles.clone());
    Json(serde_json::json!({
        "cycles_count": cycles.len(),
        "cycles": cycles,
    }))
}

/// POST /api/v1/ontology/discover — unsupervised discovery of candidate ontology domains.
pub async fn ontology_discover_handler(
    State(_state): State<SharedState>,
    Json(req): Json<DiscoverOntologyRequest>,
) -> Json<physis_core::discovery::DiscoveryReport> {
    let loader = physis_core::ontology::OntologyLoader::load_all();
    let emb = physis_core::embed::RandomProjectionEmbedder::new(64);
    let classifier = physis_core::classify::CellClassifier::build(&loader, &emb);

    let config = physis_core::discovery::DiscoveryConfig {
        coverage_threshold: req.coverage_threshold.unwrap_or(0.85),
        cluster_sim: req.cluster_sim.unwrap_or(0.70),
        min_cluster: req.min_cluster.unwrap_or(2),
        max_hints: 6,
        auto_retune: true,
    };

    let report = physis_core::discovery::discover(&req.texts, &classifier, &emb, &config);
    Json(report)
}

/// POST /api/v1/fitness/feedback — record contextual feedback for an interpretation.
pub async fn fitness_feedback_handler(
    State(state): State<SharedState>,
    Json(req): Json<FitnessFeedbackRequest>,
) -> Json<serde_json::Value> {
    let mut quality = state.quality.lock().unwrap_or_else(|e| e.into_inner());
    let cell_key = if let Some(dom) = &req.context.domain {
        dom.clone()
    } else {
        req.context.key()
    };
    log::info!(
        "Fitness feedback for interpretation '{}' (success={}, note={:?}): target cell={}",
        req.interpretation,
        req.is_success,
        req.feedback,
        cell_key
    );
    if req.is_success {
        quality.report_success(&cell_key);
    } else {
        quality.penalize_cell(&cell_key, 0.8);
    }
    let standing = quality.cell_standing(&cell_key);

    Json(serde_json::json!({
        "status": "ok",
        "cell_key": cell_key,
        "cell_standing": standing,
    }))
}

// ── LLM Coherence Harness Endpoints ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct HarnessEvaluateRequest {
    pub text: String,
    pub context: Option<String>,
    #[serde(default)]
    pub config: Option<crate::ai::harness::HarnessConfig>,
}

#[derive(Deserialize)]
pub struct HarnessGenerateRequest {
    pub prompt: String,
    pub context: Option<String>,
    #[serde(default)]
    pub config: Option<crate::ai::harness::HarnessConfig>,
}

/// GET /api/v1/harness/config — retrieve available providers and default harness rules.
pub async fn harness_config_handler(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let providers = state.shared.cascade.providers_list();
    Json(serde_json::json!({
        "default_config": crate::ai::harness::HarnessConfig::default(),
        "available_providers": providers,
    }))
}

/// POST /api/v1/harness/evaluate — evaluate candidate output against semiotic grid, coherence, and contradictions.
pub async fn harness_evaluate_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<HarnessEvaluateRequest>,
) -> Result<Json<crate::ai::harness::CoherenceEvaluation>, (StatusCode, Json<ErrorResponse>)> {
    let uid = user_key(&headers);
    let cfg = req.config.unwrap_or_default();
    let res = state.with_user(&uid, |s| {
        let quality = state.quality.lock().unwrap_or_else(|e| e.into_inner());
        crate::ai::harness::CoherenceHarness::evaluate(
            &req.text,
            req.context.as_deref(),
            &cfg,
            &mut s.core,
            &state.shared.classifier,
            state.shared.embedder.as_ref(),
            &quality,
        )
    });
    Ok(Json(res))
}

/// POST /api/v1/harness/generate — execute LLM generation through the Physis Coherence Harness.
pub async fn harness_generate_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<HarnessGenerateRequest>,
) -> Result<Json<crate::ai::harness::HarnessResult>, (StatusCode, Json<ErrorResponse>)> {
    let uid = user_key(&headers);
    let cfg = req.config.unwrap_or_default();
    let cascade = state.shared.cascade.clone();

    let result = crate::ai::harness::CoherenceHarness::generate_with_harness(
        &req.prompt,
        req.context.as_deref(),
        &cfg,
        &state,
        &uid,
        &cascade,
    )
    .await
    .map_err(|e| err_response(StatusCode::BAD_REQUEST, &e))?;

    Ok(Json(result))
}

// ── Communities rollup (PLAN 9.4b parity with the studio endpoint) ─────────
//
// The studio (physis-core, :19877) has had `GET /api/v1/communities` since the
// Phase-9.4b landing; the main server never got one, so the Pulse dashboard
// would have had to call a second process for its community grid. This is the
// same rollup over the main server's tenant state, reusing the Phase-9.4
// clean-room `crate::communities` detector directly.
//
// Read-only invariant (adopted from the pattern source as an engineering rule):
// this handler READS the core and writes only its own JSON response.

/// Build the communities payload from labeled node labels + resolved edges.
/// Split out from the handler so it is unit-testable without an HTTP stack.
///
/// Edge endpoints are labels already (`resolved_edges` resolves ids → labels),
/// so detection runs in label space. Labeled nodes that carry no edge are not
/// seen by the detector — they are appended as one singleton community with
/// full cohesion rather than vanishing from the rollup.
fn communities_payload(
    labeled: &[String],
    edges: &crate::communities::EdgeList,
) -> serde_json::Value {
    let report = crate::communities::detect_communities(edges, 16);

    let mut out: Vec<serde_json::Value> = (0..report.n_communities())
        .map(|i| {
            let mut members: Vec<String> = report
                .members(i as u32)
                .into_iter()
                .map(String::from)
                .collect();
            members.sort();
            serde_json::json!({
                "id": i,
                "members": members,
                "cohesion": report.cohesion[i],
            })
        })
        .collect();

    // Singletons: labeled nodes absent from every detected community. The
    // report's `nodes` holds exactly the edge endpoints, so set difference is
    // enough — no second graph pass.
    let connected: std::collections::HashSet<&str> =
        report.nodes.iter().map(|s| s.as_str()).collect();
    let mut singles: Vec<String> = labeled
        .iter()
        .filter(|l| !connected.contains(l.as_str()))
        .cloned()
        .collect();
    singles.sort();
    if !singles.is_empty() {
        out.push(serde_json::json!({
            "id": out.len(),
            "members": singles,
            "cohesion": 1.0,
        }));
    }

    // Largest first so a client renders the shape of the graph without having
    // to sort; ties break on first member for determinism.
    out.sort_by_key(|c| {
        std::cmp::Reverse((
            c["members"].as_array().map(|m| m.len()).unwrap_or(0),
            c["members"]
                .as_array()
                .and_then(|m| m.first())
                .and_then(|f| f.as_str())
                .unwrap_or("")
                .to_string(),
        ))
    });

    serde_json::json!({ "communities": out })
}

/// GET /api/v1/communities — label-propagation communities over this tenant's
/// labeled coherence nodes, largest first, isolated nodes as singletons.
pub async fn communities_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    let (labeled, edges) = state.with_user(&uid, |s| {
        // `resolved_edges` yields coherence IDS (bridge lookup), not labels —
        // map them into label space so detection and display share one vocabulary.
        let names: std::collections::HashMap<String, String> = s
            .core
            .nodes
            .values()
            .filter_map(|n| n.label.as_ref().map(|l| (n.id.clone(), l.clone())))
            .collect();
        let labeled: Vec<String> = names.values().cloned().collect();
        let edges: crate::communities::EdgeList = s
            .core
            .resolved_edges()
            .into_iter()
            .filter_map(|(a, b, _w)| match (a, b) {
                (Some(a), Some(b)) if a != b => match (names.get(&a), names.get(&b)) {
                    (Some(la), Some(lb)) if la != lb => Some((la.clone(), lb.clone())),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        (labeled, edges)
    });
    Json(communities_payload(&labeled, &edges))
}

#[cfg(test)]
mod communities_tests {
    use super::*;

    #[test]
    fn two_cliques_and_a_singleton() {
        let e = |a: &str, b: &str| (a.to_string(), b.to_string());
        let edges: crate::communities::EdgeList =
            vec![e("a", "b"), e("b", "c"), e("a", "c"), e("x", "y")];
        let labeled = vec![
            "a".into(),
            "b".into(),
            "c".into(),
            "x".into(),
            "y".into(),
            "solo".into(),
        ];
        let v = communities_payload(&labeled, &edges);
        let comms = v["communities"].as_array().unwrap();
        assert_eq!(comms.len(), 3, "{v:?}");
        // Largest first: the triangle, then the pair, then the singleton.
        assert_eq!(comms[0]["members"].as_array().unwrap().len(), 3);
        assert_eq!(comms[1]["members"].as_array().unwrap().len(), 2);
        assert_eq!(comms[2]["members"], serde_json::json!(["solo"]));
        assert_eq!(comms[2]["cohesion"], serde_json::json!(1.0));
    }

    #[test]
    fn empty_tenant_yields_empty_rollup_not_error() {
        let v = communities_payload(&[], &Vec::new());
        assert_eq!(v["communities"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn all_isolated_nodes_collapse_into_one_singleton_community() {
        let edges: crate::communities::EdgeList = Vec::new();
        let v = communities_payload(&["q/1".to_string(), "q/2".to_string()], &edges);
        let comms = v["communities"].as_array().unwrap();
        assert_eq!(comms.len(), 1, "{v:?}");
        assert_eq!(comms[0]["members"], serde_json::json!(["q/1", "q/2"]));
    }
}

#[cfg(test)]
mod quality_tests {
    use super::*;

    fn failure(
        id: &str,
        hours_ago: i64,
        severity: f32,
        resolved: bool,
    ) -> crate::quality::QualityFailure {
        crate::quality::QualityFailure {
            id: id.into(),
            timestamp: chrono::Utc::now() - chrono::Duration::hours(hours_ago),
            feedback: format!("feedback {id}"),
            feedback_embedding: vec![0.0; 384],
            top_cell: "BOND\u{0}PLAY".to_string(),
            top_score: 0.42,
            correct_domain: None,
            correct_embedding: None,
            severity,
            resolved,
        }
    }

    #[test]
    fn empty_ledger_reads_as_zeroed_not_erroring() {
        let r = quality_response(HashMap::new(), HashMap::new(), &[]);
        assert_eq!(r.failures, 0);
        assert_eq!(r.open, 0);
        assert_eq!(r.resolved, 0);
        assert_eq!(r.mean_severity, 0.0);
        assert!(r.records.is_empty());
    }

    #[test]
    fn records_are_newest_first_and_aggregates_add_up() {
        let failures = vec![
            failure("old", 48, 0.3, true),
            failure("mid", 24, 0.5, false),
            failure("new", 1, 0.8, false),
        ];
        let mut penalties = HashMap::new();
        penalties.insert("BOND\u{0}PLAY".to_string(), 0.25);
        let r = quality_response(penalties, HashMap::new(), &failures);
        assert_eq!(r.failures, 3);
        assert_eq!(r.open, 2);
        assert_eq!(r.resolved, 1);
        assert!(
            (r.mean_severity - 0.5333333).abs() < 1e-4,
            "{}",
            r.mean_severity
        );
        // Newest first: the dashboard reads the recent window.
        let ids: Vec<&str> = r.records.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids, ["new", "mid", "old"], "{ids:?}");
    }

    #[test]
    fn views_carry_display_cells_and_never_embeddings() {
        let failures = vec![failure("one", 2, 0.8, false)];
        let v = serde_json::to_value(quality_response(HashMap::new(), HashMap::new(), &failures))
            .unwrap();
        let rec = &v["records"][0];
        // The NUL-keyed store cell is rendered DOMAIN/MODE (QA 8.5 parity).
        assert_eq!(rec["top_cell"], serde_json::json!("BOND/PLAY"));
        let payload = v.to_string();
        assert!(!payload.contains("embedding"), "no vectors over the wire");
        assert!(!payload.contains('\u{0}'), "no NUL bytes reach the browser");
    }

    #[test]
    fn long_feedback_is_truncated_with_ellipsis() {
        let mut f = failure("long", 0, 0.5, false);
        f.feedback = "x".repeat(400);
        let r = quality_response(HashMap::new(), HashMap::new(), &[f]);
        let view = &r.records[0];
        assert_eq!(view.feedback.chars().count(), 161); // 160 + ellipsis
        assert!(view.feedback.ends_with('…'));
    }
}
