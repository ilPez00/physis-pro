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
        let dreams = s.dreams.generate_dreams_with_context(
            &goals,
            req.count,
            Some(&s.scheduler),
            None,
        );
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
                if req.embeddings {
                    obj["source"] = serde_json::json!(d.source);
                    obj["embedding"] = serde_json::json!(d.embedding);
                }
                obj
            }).collect::<Vec<_>>(),
            "count": dreams.len(),
            "active": true,
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
    let out = state.with_user(&uid, |s| {
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
    });
    Json(out)
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
