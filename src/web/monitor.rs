//! Monitor ingest — one endpoint a phone can sync a morning into.
//!
//! `POST /api/v1/machines/{name}/reduce` (Phase 8b) is the general boundary,
//! and a client *could* call it four times with four hand-built payloads. This
//! exists because a phone on mobile data should not make four round trips to
//! record one morning, and because deciding which sensor becomes which
//! primitive is a product decision: two clients re-implementing it is how two
//! clients end up disagreeing about what a workout was.
//!
//! So the device posts what it observed, and the engine decides what that
//! meant — the same split the machines exist for.

use super::*;
use crate::monitor::{translate, SensorBatch};
use crate::web::SharedState;
use axum::{
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
};

/// POST /api/v1/monitor/ingest
///
/// Tenant-scoped: a device syncs into its owner's graph. Everything it produces
/// goes through the same reducers a manual `/machines/*/reduce` would, so a
/// morning recorded by a phone and a morning typed in by hand are the same kind
/// of fact.
pub async fn monitor_ingest_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(batch): Json<SensorBatch>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    if batch.device.trim().is_empty() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "device is required — two phones are two witnesses and the graph should know which",
        ));
    }
    // A batch is one sync, not one day of raw sampling. The cap is generous for
    // a phone that was offline overnight and a hard stop for a client looping.
    const MAX_SAMPLES: usize = 5_000;
    if batch.samples.len() > MAX_SAMPLES {
        return Err(err_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            &format!(
                "{} samples in one batch (max {MAX_SAMPLES}) — sync more often rather than bigger",
                batch.samples.len()
            ),
        ));
    }

    let uid = user_key(&headers);
    let shared = state.shared.clone();
    let translation = translate(&batch);
    let summary = translation.summary();

    // Nothing understood is worth saying out loud rather than answering 200 and
    // moving on: a collector that syncs every ten minutes and translates to
    // nothing every time is misconfigured, not quiet.
    if translation.is_empty() {
        return Ok(Json(serde_json::json!({
            "device": batch.device,
            "understood": summary,
            "registered": 0,
            "note": "nothing in this batch reached a reducer — check the collector's sensors",
        })));
    }

    let now = chrono::Utc::now();
    let device = batch.device.clone();
    let (registered, scores) = state.with_user(&uid, |s| {
        let embedder = shared.embedder.as_ref();
        let before = s.core.nodes.len();
        let mut scores = serde_json::Map::new();

        // Each reducer registers and bridges its own primitives, so a machine's
        // meaning stays in the machine. The one direct registration below is an
        // observation, not an interpretation — see the note on it.
        if !translation.events.is_empty() {
            // Two different acts, and the monitor needs both.
            //
            // Remember each observation: chronos keeps *rhythms*, so feeding it
            // alone would record "lesson every 7d" on the third Tuesday and
            // nothing at all on the first — losing the very fact the day was
            // worth noticing. Registering a labeled observation is the core's
            // own primitive (the same act as /coherence/register), not a
            // machine's job, so this is not the handler doing a machine's work.
            for event in &translation.events {
                let label = crate::monitor::observation_label(event);
                let id = s.core.register_node_from_text(&label, embedder);
                s.core.bridge_node(&id);
            }
            // Then let chronos say what recurs across them.
            let score =
                crate::machines::chronos::reduce(&mut s.core, embedder, &translation.events, now);
            scores.insert("chronos".into(), serde_json::json!(score));
        }
        if !translation.sessions.is_empty() {
            let ids = crate::machines::soma::register_sessions(
                &mut s.core,
                embedder,
                &translation.sessions,
            );
            scores.insert("soma".into(), serde_json::json!(ids.len()));
        }
        if !translation.waypoints.is_empty() {
            // Fixes become *places*, not a trace: clustering happens before
            // registration, so the graph never holds every coordinate the
            // device ever reported.
            let places = crate::machines::topos::detect_places(&translation.waypoints, 0.2, 2);
            let ids = crate::machines::topos::register_places(&mut s.core, embedder, &places);
            scores.insert("topos".into(), serde_json::json!(ids.len()));
        }
        if !translation.readings.is_empty() {
            let score = crate::machines::empedocles::reduce(
                &mut s.core,
                embedder,
                &translation.readings,
                1.0,
            );
            scores.insert("empedocles".into(), serde_json::json!(score));
        }

        let registered = s.core.nodes.len().saturating_sub(before);

        // Why these nodes exist: a device observed them. Without this the graph
        // would hold a workout with no account of how it was known.
        s.ledger.record(
            crate::address::Mechanism::Received,
            "monitor",
            format!("device:{device}"),
            None,
        );
        (registered, scores)
    });

    // Persist what the sync produced, and report the outcome rather than
    // assuming it: a morning that survived the request and not the restart is
    // the worst of both.
    let mut persisted = 0usize;
    let mut persist_error: Option<String> = None;
    if let Some(db) = state.db() {
        state.with_user(&uid, |s| {
            let ids: Vec<String> = s.core.nodes.keys().cloned().collect();
            for id in ids {
                match s.core.persist_node(db, &uid, &id) {
                    Ok(()) => persisted += 1,
                    Err(e) => {
                        persist_error.get_or_insert_with(|| e.to_string());
                    }
                }
            }
            if let Err(e) = s.ledger.save(db, &uid) {
                log::warn!("monitor: ledger persist failed: {e}");
                persist_error.get_or_insert_with(|| e.to_string());
            }
        });
    }

    Ok(Json(serde_json::json!({
        "device": batch.device,
        "understood": summary,
        "registered": registered,
        "machines": scores,
        "persisted": persisted,
        "persist_error": persist_error,
        "tenant": uid,
    })))
}
