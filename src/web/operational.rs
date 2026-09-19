//! HTTP surface for the normalized event stream (Phase 16, I3/I4).
//!
//! `POST /api/v1/events` is the **single ingestion path** every connector feeds:
//! dedupe by the stable content hash, append to the tenant's bounded event ring,
//! record a causal-ledger entry (so provenance survives into lineage/purposes),
//! optionally register a labeled coherence node. A new adapter that emits an
//! `OperationalEvent` inherits all four for free — no per-connector plumbing.
//!
//! `GET /api/v1/events` reads the ring back (newest first) and
//! `GET /api/v1/events/timeline` renders it through the same timeline pipeline
//! the shift report uses, so the report is a projection of the same store.

use axum::{extract::State, http::HeaderMap, Json};

use crate::operational::{load_events, push_capped, save_events, OperationalEvent};
use crate::operational_report::OperationalTimeline;
use crate::web::{user_key, SharedState};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct IngestEventsRequest {
    /// Events to ingest. Re-posted events (same `event_key`) are idempotent
    /// no-ops.
    pub events: Vec<OperationalEvent>,
    /// Register each *accepted* event as a labeled coherence node (graph
    /// visibility). Default off: batch telemetry usually wants the timeline +
    /// ledger without flooding the graph with one node per reading.
    #[serde(default)]
    pub register_nodes: bool,
    /// Anomaly thresholds (Phase 16 T2): `{"temp_max": 90.0, "vibration_max": 8.0, …}`.
    /// When set, accepted *machine* events whose scalar properties exceed a
    /// threshold are flagged as anomalies — recorded in the audit log and
    /// returned in the response. This is the half that makes the "a new adapter
    /// inherits anomaly detection" claim true: an adapter just ships readings.
    #[serde(default)]
    pub thresholds: std::collections::HashMap<String, f32>,
}

#[derive(Serialize)]
pub struct IngestEventsResponse {
    pub accepted: usize,
    pub skipped: usize,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_ids: Option<Vec<String>>,
    /// Anomalies detected on accepted machine events (empty when no thresholds
    /// were given or nothing exceeded them).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anomalies: Vec<serde_json::Value>,
}

/// POST /api/v1/events — one ingestion path, dedupe + ledger + store (+ optional
/// graph nodes). Returns counts, never fails on a bad event: a malformed row is
/// skipped and counted, so a partially-corrupt batch does not roll back the rest.
pub async fn post_events_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<IngestEventsRequest>,
) -> Json<IngestEventsResponse> {
    let uid = user_key(&headers);
    let total = req.events.len();

    // Phase 1 — mutate tenant state under its lock: dedupe, ledger, ring.
    let accepted_events = state.with_user(&uid, |s| {
        let mut accepted = Vec::with_capacity(req.events.len());
        for ev in req.events {
            if s.op_events.iter().any(|e| e.event_key == ev.event_key) {
                continue; // idempotent: already recorded
            }
            let mechanism = ev.ledger_mechanism();
            let actor = ev
                .actor
                .clone()
                .unwrap_or_else(|| ev.provenance.connector.clone());
            let input = ev.target.clone().or_else(|| ev.machine_id.clone());
            s.ledger
                .record_at(mechanism, &actor, ev.subject.clone(), input, ev.observed_at);
            push_capped(&mut s.op_events, ev.clone());
            accepted.push(ev);
        }
        if let Some(db) = state.db() {
            if let Err(e) = save_events(db, &uid, &s.op_events) {
                log::warn!("events not persisted for {uid}: {e}");
            }
            if let Err(e) = s.ledger.save(db, &uid) {
                log::warn!("ledger not persisted for {uid}: {e}");
            }
        }
        accepted
    });

    let skipped = total - accepted_events.len();

    // Phase 2 — optional anomaly detection (Phase 16 T2). Machine events with
    // scalar properties (temperature, vibration, rpm, …) are checked against
    // the caller's thresholds; each hit is audited and returned.
    let mut anomalies: Vec<serde_json::Value> = Vec::new();
    if !req.thresholds.is_empty() {
        for ev in &accepted_events {
            let Some(reading) = reading_from_event(ev) else {
                continue;
            };
            for a in crate::industrial::detect_anomalies(
                &ev.subject,
                std::slice::from_ref(&reading),
                &req.thresholds,
            ) {
                anomalies.push(serde_json::json!({
                    "machine_id": a.machine_id,
                    "reason": a.reason,
                    "severity": a.severity,
                }));
                state.with_user(&uid, |s| {
                    s.audit.record(
                        "anomaly",
                        &ev.subject,
                        Some("machine".into()),
                        &uid,
                        a.reason.clone(),
                    );
                    if let Some(db) = state.db() {
                        let _ = s.audit.save(db, &uid);
                    }
                });
            }
        }
    }

    // Phase 3 — optional graph visibility, off-thread (embedding is the slow
    // part and must not serialize other handlers behind the tenant lock).
    let mut node_ids: Option<Vec<String>> = None;
    if req.register_nodes && !accepted_events.is_empty() {
        node_ids = Some(Vec::with_capacity(accepted_events.len()));
        for ev in &accepted_events {
            let id = register_node(&state, &headers, ev).await;
            if let Some(ids) = &mut node_ids {
                ids.push(id);
            }
        }
    }

    Json(IngestEventsResponse {
        accepted: accepted_events.len(),
        skipped,
        total,
        node_ids,
        anomalies,
    })
}

/// Build an industrial `MachineReading` from a machine event's scalar
/// properties, so a telemetry adapter's events can be anomaly-checked with no
/// extra wiring. Returns `None` for non-machine events.
fn reading_from_event(ev: &OperationalEvent) -> Option<crate::industrial::MachineReading> {
    if ev.kind != crate::operational::OperationalKind::Machine {
        return None;
    }
    let f = |k: &str| ev.properties.get(k).and_then(|v| v.parse::<f32>().ok());
    let status = match ev.status {
        crate::operational::OperationalStatus::Warning => crate::industrial::MachineStatus::Warning,
        crate::operational::OperationalStatus::Failure => {
            crate::industrial::MachineStatus::Critical
        }
        _ => crate::industrial::MachineStatus::Online,
    };
    let mut raw = std::collections::HashMap::new();
    for (k, v) in &ev.properties {
        if let Ok(n) = v.parse::<f32>() {
            raw.insert(k.clone(), n);
        }
    }
    Some(crate::industrial::MachineReading {
        machine_id: ev.subject.clone(),
        timestamp: ev.observed_at,
        temperature: f("temperature"),
        pressure: f("pressure"),
        vibration: f("vibration"),
        rpm: f("rpm"),
        status,
        raw,
    })
}

/// Register one labeled coherence node (deduped by label), stamped with the
/// typed-ontology axis from the event (Phase 16 I5): kind = the event's source
/// family, properties = the event's scalar properties. Best-effort: a graph
/// write failing must not fail the event ingest.
async fn register_node(state: &SharedState, headers: &HeaderMap, ev: &OperationalEvent) -> String {
    let label = format!("{} — {:?} [{:?}]", ev.subject, ev.kind, ev.status);
    // Text-only (image=None) so the image-slot 503 is unreachable here.
    let embedding =
        crate::web::wiki_semiotic::embed_input(state.shared.clone(), label.clone(), None)
            .await
            .unwrap_or_default();
    let uid = user_key(headers);
    state.with_user(&uid, |s| {
        let id = s.core.register_node_vec_labeled(embedding, Some(label));
        if let Some(node) = s.core.nodes.get_mut(&id) {
            node.kind = ev.kind.as_str().to_string();
            node.properties = ev
                .properties
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            if let Some(actor) = &ev.actor {
                node.properties
                    .insert("actor".into(), serde_json::Value::String(actor.clone()));
            }
        }
        if let Some(db) = state.db() {
            if let Err(e) = s.core.persist_node(db, &uid, &id) {
                log::warn!("event node {id} not persisted: {e}");
            }
        }
        id
    })
}

/// GET /api/v1/events?limit=N — the tenant's stored events, newest first.
pub async fn get_events_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<EventsQuery>,
) -> Json<Vec<OperationalEvent>> {
    let uid = user_key(&headers);
    let mut events = state.with_user(&uid, |s| s.op_events.clone());
    events.reverse(); // newest first
    if let Some(limit) = params.limit {
        events.truncate(limit);
    }
    Json(events)
}

/// GET /api/v1/events/timeline — the stored events run through the shared
/// timeline pipeline, so the shift report is a projection of this store.
pub async fn events_timeline_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<OperationalTimeline> {
    let uid = user_key(&headers);
    let events = state.with_user(&uid, |s| s.op_events.clone());
    Json(OperationalTimeline::from_events(&events))
}

#[derive(Deserialize)]
pub struct EventsQuery {
    limit: Option<usize>,
}

/// Re-hydrate a tenant's stored event ring from sled (called from `with_user`
/// and the default-tenant construction).
pub fn rehydrate_events(rt: &mut crate::web::RuntimeState, db: &sled::Db, uid: &str) {
    rt.op_events = load_events(db, uid);
}
