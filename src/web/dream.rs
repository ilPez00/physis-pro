use super::*;
use crate::web::SharedState;
use axum::{
    extract::{Json, State},
    http::HeaderMap,
};

pub async fn dream_generate_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<DreamGenerateRequest>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        if s.goals.is_empty() {
            return Json(serde_json::json!({
                "dreams": [],
                "count": 0,
                "active": false,
                "reason": "No goals exist."
            }));
        }

        if !req.force && s.actor.is_working(&s.goals) {
            return Json(serde_json::json!({
                "dreams": [],
                "count": 0,
                "active": false,
                "reason": "PDCA cycle active — dreaming suppressed. Pass {'force': true} to override."
            }));
        }

        let goals = s.goals.clone();
        // Pass the tenant's scheduler: the 2-arg wrapper hands None for both
        // context sources, so the schedule-drift and countermeasure generators
        // — the ones that produce an actionable proposal rather than "mutate
        // with noise_std=x" — could never fire over HTTP. Experiences are not
        // part of RuntimeState yet, so that source stays None honestly.
        //
        // `min_cosine` opts into the ontological validation contract: a dream
        // must be nameable by the ontology grid (closest entry ≥ threshold) to
        // be proposed, with bounded regeneration for the rest. Without it the
        // raw batch is returned exactly as before, so existing callers see no
        // behavior change.
        let contracted = req.min_cosine.map(|min| {
            crate::dream::DreamContract::new(&state.shared.classifier, min, 2)
        });
        let (dreams, contract_report) = if let Some(contract) = contracted {
            let result = s.dreams.generate_dreams_contracted(
                &goals,
                req.count,
                Some(&s.scheduler),
                None,
                &contract,
            );
            let report = serde_json::json!({
                "rejected": result.rejected,
                "weakest_accepted_cosine": result.weakest_accepted,
                "min_cosine": req.min_cosine,
            });
            (result.dreams, Some(report))
        } else {
            (s.dreams.generate_dreams_with_context(&goals, req.count, Some(&s.scheduler), None), None)
        };
        // Serialize what a human can act on. `dream_type` and `description`
        // have always been produced by the engine (schedule drift,
        // countermeasures, grafts) and were simply dropped here, while the two
        // 768-d vectors per dream were sent instead — so every client saw the
        // arithmetic and none of the proposal. Vectors are available on request
        // via `?embeddings=1` for graph tooling; the default payload is now
        // small enough to render.
        Json(serde_json::json!({
            "dreams": dreams.iter().map(|d| {
                let mut obj = serde_json::json!({
                    "id": d.id,
                    "kind": d.dream_type,
                    "proposal": d.description,
                    "grade": d.grade,
                });
                if let Some(review) = &d.review {
                    obj["review"] = serde_json::json!({
                        "best_entry": review.best_entry,
                        "best_cell": review.best_cell,
                        "cosine": review.cosine,
                    });
                }
                if req.embeddings {
                    obj["source"] = serde_json::json!(d.source);
                    obj["embedding"] = serde_json::json!(d.embedding);
                }
                obj
            }).collect::<Vec<_>>(),
            "count": dreams.len(),
            "active": true,
            "contract": contract_report,
        }))
    })
}

/// Replay the nodes worth revisiting, named.
///
/// `/dream/generate` works over goals, which are pure vectors with no text —
/// its proposals can only ever say "mutate unknown". `core.dream()` runs over
/// *coherence nodes*, which carry labels and verdicts, so this is the view that
/// can actually tell an operator which piece of work is failing. It selects
/// reported failures first and falls back to sparse neighbourhoods for nodes
/// nobody has judged (see `PhysisCore::dream`).
///
/// A node already in the dream archive is skipped by the engine, so calling
/// this twice returns the new material rather than repeating itself.
pub async fn dream_replay_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<Vec<DreamReplayEntry>> {
    let uid = user_key(&headers);
    Json(replay_core(&state, &uid))
}

/// The one implementation of "what should I revisit" — shared by the JSON and
/// CSV surfaces so they can never drift.
fn replay_core(state: &SharedState, uid: &str) -> Vec<DreamReplayEntry> {
    state.with_user(uid, |s| {
        let results = s.core.dream();
        results
            .iter()
            .flat_map(|r| {
                r.nodes_tested
                    .iter()
                    .filter_map(|id| {
                        let node = s.core.nodes.get(id)?;
                        Some(DreamReplayEntry {
                            node_id: id.clone(),
                            // An unlabeled node has no readable handle; say so
                            // rather than emitting a blank row.
                            label: node
                                .label
                                .clone()
                                .unwrap_or_else(|| "(unlabeled)".to_string()),
                            asserted: node.asserted,
                            density: node.coherence_score,
                            outcome: r.outcome,
                            prevented_failure: r.prevented_failure,
                            // The decide half of the dream loop (Phase 16 I8):
                            // one concrete, typed action derived from the node's
                            // object kind — an operator or an agent can act on it.
                            typed_action: Some(crate::models::typed_action_for(
                                &node.kind,
                                Some(id.clone()),
                                r.outcome,
                                r.prevented_failure,
                            )),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    })
}

/// GET /api/v1/dream/replay.csv — the same replay as CSV, so an operator can
/// open the countermeasures in a spreadsheet or hand them to a workflow that
/// does not speak JSON (Phase 16 I11). Columns: node_id,label,outcome,
/// prevented_failure,action_kind,summary.
pub async fn dream_replay_csv_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> axum::response::Response {
    let uid = user_key(&headers);
    let entries = replay_core(&state, &uid);
    let mut out = String::from("node_id,label,outcome,prevented_failure,action_kind,summary\n");
    for e in &entries {
        let (kind, summary) = match &e.typed_action {
            Some(a) => (a.kind.as_str(), a.summary.clone()),
            None => ("", String::new()),
        };
        out.push_str(&format!(
            "{},{},{},{},{},{}\n",
            csv_field(&e.node_id),
            csv_field(&e.label),
            e.outcome,
            e.prevented_failure,
            kind,
            csv_field(&summary),
        ));
    }
    (
        [(axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8")],
        out,
    )
        .into_response()
}

/// Minimal RFC-4180 field quoting: wrap in quotes when the value carries a
/// comma, quote, or newline; double any inner quotes.
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// POST /api/v1/actions/dispatch — the **act** half of the dream loop
/// (Phase 16 I11): deliver typed actions (I8) to an outbound webhook.
///
/// The webhook is configured once with `PHYSIS_ACTIONS_WEBHOOK` — an air-gapped
/// site leaves it unset and the endpoint answers `configured: false` with an
/// honest note instead of an error, so a harness can fall back to the CSV
/// export rather than silently assuming delivery.
///
/// Body: `{ "node_id": "…" }` builds the node's typed action server-side, or
/// `{ "actions": [ {kind, object_id, object_kind, summary}, … ] }` delivers
/// pre-built actions (e.g. the `/dream/replay` payload) as-is.
#[derive(Deserialize)]
pub struct DispatchRequest {
    node_id: Option<String>,
    #[serde(default)]
    actions: Vec<crate::models::TypedAction>,
}

pub async fn dispatch_actions_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<DispatchRequest>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);

    // Build server-side from a node when that's all the caller has.
    let mut actions = req.actions;
    if let Some(node_id) = &req.node_id {
        let action = state.with_user(&uid, |s| {
            let node = s.core.nodes.get(node_id)?;
            let failing = node.rating() == Some(crate::models::CoherenceRating::Failure);
            Some(crate::models::typed_action_for(
                &node.kind,
                Some(node_id.clone()),
                if failing { -1.0 } else { 1.0 },
                false,
            ))
        });
        if let Some(a) = action {
            actions.push(a);
        }
    }

    let webhook = std::env::var("PHYSIS_ACTIONS_WEBHOOK")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let Some(url) = webhook else {
        return Json(serde_json::json!({
            "configured": false,
            "webhook": null,
            "delivered": 0,
            "skipped": actions.len(),
            "note": "PHYSIS_ACTIONS_WEBHOOK is not set — nothing was sent. Export the countermeasures via /api/v1/dream/replay.csv instead.",
        }));
    };

    let client = reqwest::Client::new();
    let mut results = Vec::with_capacity(actions.len());
    let mut delivered = 0usize;
    for a in &actions {
        let res = client
            .post(&url)
            .json(&serde_json::json!({
                "action": a,
                "at": chrono::Utc::now().to_rfc3339(),
                "actor": uid,
            }))
            .send()
            .await;
        match res {
            Ok(r) if r.status().is_success() => {
                delivered += 1;
                results.push(serde_json::json!({
                    "object_id": a.object_id, "kind": a.kind.as_str(), "ok": true,
                }));
            }
            Ok(r) => {
                results.push(serde_json::json!({
                    "object_id": a.object_id, "kind": a.kind.as_str(), "ok": false,
                    "error": format!("webhook returned {}", r.status()),
                }));
            }
            Err(e) => {
                results.push(serde_json::json!({
                    "object_id": a.object_id, "kind": a.kind.as_str(), "ok": false,
                    "error": format!("webhook unreachable: {e}"),
                }));
            }
        }
    }
    // Audit the dispatch so the loop is closed on paper too.
    state.with_user(&uid, |s| {
        s.audit.record(
            "action:dispatch",
            "actions",
            None,
            &uid,
            format!("delivered {delivered}/{} to {url}", actions.len()),
        );
        if let Some(db) = state.db() {
            let _ = s.audit.save(db, &uid);
        }
    });

    Json(serde_json::json!({
        "configured": true,
        "webhook": url,
        "delivered": delivered,
        "skipped": actions.len() - delivered,
        "results": results,
    }))
}

pub async fn dream_evaluate_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<DreamEvaluateRequest>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    let accepted = state.with_user(&uid, |s| s.dreams.evaluate_dream(&req.id, req.grade));
    Json(serde_json::json!({
        "accepted": accepted,
        "dream_id": req.id,
        "grade": req.grade,
    }))
}
