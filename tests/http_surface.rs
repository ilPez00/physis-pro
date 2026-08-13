//! HTTP-surface tests: drive the real router in-process.
//!
//! Every wiring bug found in this codebase lived here and was invisible to the
//! engine tests — the rate limiter constructed but never layered in, three
//! handlers reading the default tenant while their writers were per-tenant,
//! `/dream/generate` dropping the proposal it had just produced, and a
//! persistence call site that compiled clean and saved nothing. None of those
//! are type errors; all of them are one request away from being obvious.
//!
//! `PHYSIS_EMBEDDER=random-projection` keeps these fast: the vectors are
//! non-semantic, which is fine because nothing here asserts on *meaning*, only
//! on routing, tenancy, and persistence.
#![cfg(feature = "web")]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

/// Build the app once per test with an isolated data dir, so tests never share
/// sled state with each other or with a developer's real `~/.physis`.
async fn app() -> (axum::Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp data dir");
    std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
    std::env::set_var("PHYSIS_DATA_DIR", dir.path());
    std::env::set_var("HOME", dir.path()); // config resolves ~/.physis
    let (_state, router) = physis_pro::web::build_app().await;
    (router, dir)
}

async fn send(router: &axum::Router, mut req: Request<Body>) -> (StatusCode, Value) {
    // The rate-limit middleware extracts ConnectInfo for the peer IP. A real
    // server supplies it via `into_make_service_with_connect_info`; `oneshot`
    // does not, and without it the extractor rejects every request — which is
    // precisely the 500-on-everything failure the middleware comment warns
    // about, so injecting it here keeps the test exercising the real stack.
    req.extensions_mut().insert(axum::extract::ConnectInfo(
        std::net::SocketAddr::from(([127, 0, 0, 1], 40000)),
    ));
    let resp = router.clone().oneshot(req).await.expect("router responds");
    let status = resp.status();
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

fn post(path: &str, user: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .header("x-physis-user", user)
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get(path: &str, user: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(path)
        .header("x-physis-user", user)
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn health_and_embedder_are_public() {
    // These two are the ops surface: they must answer without a token, and the
    // embedder must admit when it is degraded rather than look healthy.
    let (router, _dir) = app().await;

    let (status, _) = send(&router, get("/health", "_default")).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(&router, get("/api/v1/embedder", "_default")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["kind"], "random-projection");
    assert_eq!(body["degraded"], true, "a non-semantic embedder must say so");
}

#[tokio::test]
async fn coherence_nodes_are_isolated_per_tenant() {
    // Regression: /reconstruct, /context/filter and /compress/logs all read the
    // default tenant regardless of X-Physis-User, so a client that wrote under
    // its own id got empty results back and no error.
    let (router, _dir) = app().await;

    let (status, body) = send(
        &router,
        post("/api/v1/coherence/register", "alice", json!({"input": "alice's private note"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["node_id"].is_string());

    let (_, alice) = send(&router, get("/api/v1/coherence/nodes/all?limit=10", "alice")).await;
    assert_eq!(alice["count"], 1, "alice sees her own node");

    let (_, bob) = send(&router, get("/api/v1/coherence/nodes/all?limit=10", "bob")).await;
    assert_eq!(bob["count"], 0, "bob must not see alice's node");

    // The read half of the loop has to honour the header too.
    let (_, recon) = send(
        &router,
        post("/api/v1/reconstruct", "alice", json!({"input": "private note"})),
    )
    .await;
    assert!(
        recon["count"].as_u64().unwrap_or(0) >= 1,
        "reconstruct must read the caller's tenant, not _default"
    );

    let (_, empty) = send(
        &router,
        post("/api/v1/reconstruct", "bob", json!({"input": "private note"})),
    )
    .await;
    assert_eq!(empty["count"], 0, "and must stay empty for another tenant");
}

#[tokio::test]
async fn a_verdict_is_stored_and_can_be_corrected() {
    // The asserted axis travels on the wire and re-registering the same label
    // updates it — that is how a task reported optimistically at spawn gets
    // corrected when it later fails.
    let (router, _dir) = app().await;

    let (_, first) = send(
        &router,
        post("/api/v1/coherence/register", "t", json!({"input": "deploy step", "coherence_score": 1.0})),
    )
    .await;
    assert_eq!(first["asserted"], 1.0);

    let (_, again) = send(
        &router,
        post("/api/v1/coherence/register", "t", json!({"input": "deploy step", "coherence_score": -1.0})),
    )
    .await;
    assert_eq!(again["node_id"], first["node_id"], "dedupe returns the same node");
    assert_eq!(again["asserted"], -1.0, "and the verdict is corrected");

    // Out-of-range verdicts are clamped rather than stored raw.
    let (_, clamped) = send(
        &router,
        post("/api/v1/coherence/register", "t", json!({"input": "deploy step", "coherence_score": 9.0})),
    )
    .await;
    assert_eq!(clamped["asserted"], 1.0);
}

#[tokio::test]
async fn dreams_return_a_readable_proposal_not_raw_vectors() {
    // Regression: the handler serialized two 768-d vectors per dream and
    // dropped `description`, so every client saw the arithmetic and none of
    // the reasoning.
    let (router, _dir) = app().await;

    send(
        &router,
        post("/api/v1/goals", "d", json!({"prompt": "reduce nozzle clogs before the next batch"})),
    )
    .await;

    let (status, body) = send(
        &router,
        post("/api/v1/dream/generate", "d", json!({"count": 2, "force": true})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let dreams = body["dreams"].as_array().expect("dreams array");
    assert!(!dreams.is_empty(), "a tenant with a goal can dream");

    let first = &dreams[0];
    let proposal = first["proposal"].as_str().unwrap_or("");
    assert!(!proposal.is_empty(), "a dream must state its proposal");
    assert!(first["kind"].is_string());
    assert!(
        first.get("embedding").is_none(),
        "vectors stay out of the default payload"
    );
    // The goal keeps the text it was embedded from, so proposals can name it.
    assert!(
        proposal.contains("nozzle"),
        "proposal should name the goal, got {proposal:?}"
    );

    // Opt in and the vectors come back, for graph tooling that wants them.
    let (_, raw) = send(
        &router,
        post("/api/v1/dream/generate", "d", json!({"count": 1, "force": true, "embeddings": true})),
    )
    .await;
    assert!(raw["dreams"][0]["embedding"].is_array());
}

#[tokio::test]
async fn dream_replay_names_failures_and_skips_successes() {
    // The thesis, over HTTP: replay what was reported broken, leave what was
    // reported working alone.
    let (router, _dir) = app().await;

    for (text, score) in [
        ("nozzle clogged mid print", -1.0),
        ("bed levelling ran clean", 1.0),
    ] {
        send(
            &router,
            post("/api/v1/coherence/register", "r", json!({"input": text, "coherence_score": score})),
        )
        .await;
    }

    let (status, body) = send(&router, post("/api/v1/dream/replay", "r", json!({}))).await;
    assert_eq!(status, StatusCode::OK);
    let labels: Vec<&str> = body
        .as_array()
        .expect("replay array")
        .iter()
        .filter_map(|e| e["label"].as_str())
        .collect();
    assert!(labels.iter().any(|l| l.contains("clogged")), "failures are replayed");
    assert!(
        !labels.iter().any(|l| l.contains("levelling")),
        "a reported success is not replayed, however typical it looks"
    );
}

#[tokio::test]
async fn holarchy_edges_are_visible_to_the_tenant_that_wrote_them() {
    // Regression: /holarchy/edge wrote per-tenant while /holarchy/snapshot read
    // _default, so an edge returned "edge_added" and appeared in no snapshot.
    let (router, _dir) = app().await;

    let mut ids = Vec::new();
    for text in ["source node", "target node"] {
        let (_, body) = send(
            &router,
            post("/api/v1/coherence/register", "g", json!({"input": text})),
        )
        .await;
        ids.push(body["node_id"].as_str().unwrap().to_string());
    }

    let (status, _) = send(
        &router,
        post("/api/v1/holarchy/edge", "g", json!({"source": ids[0], "target": ids[1]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, snap) = send(&router, get("/api/v1/holarchy/snapshot", "g")).await;
    assert_eq!(snap["edges"], 1, "the writer can see its own edge");

    let (_, other) = send(&router, get("/api/v1/holarchy/snapshot", "elsewhere")).await;
    assert_eq!(other["edges"], 0, "another tenant cannot");

    // The walkable view resolves ids to labels; /holarchy/graph only exposes
    // packed RawNodeKeys, which is why this endpoint had to exist.
    let (_, neighbours) = send(
        &router,
        get(&format!("/api/v1/holarchy/neighbors?id={}", ids[0]), "g"),
    )
    .await;
    assert_eq!(neighbours[0]["label"], "target node");

    // Walking to a dead end is normal, not an error.
    let (status, body) = send(&router, get("/api/v1/holarchy/neighbors?id=nope", "g")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().map(Vec::len), Some(0));
}

#[tokio::test]
async fn the_semiotic_grid_reports_every_loaded_ontology() {
    // Regression: the handler enumerated eight collections by hand and omitted
    // custom_domains, where the 20 EXTRA_ONTOLOGIES live — so it reported 27 of
    // 70 cells populated while classify used all of them.
    let (router, _dir) = app().await;

    let (status, body) = send(&router, get("/api/v1/semiotic/grid", "_default")).await;
    assert_eq!(status, StatusCode::OK);
    let cells = body["cells"].as_array().expect("cells");
    assert_eq!(cells.len(), 70, "5 domains x 14 modes");

    let empty = cells
        .iter()
        .filter(|c| c["entries"].as_array().map(Vec::is_empty).unwrap_or(true))
        .count();
    assert_eq!(empty, 0, "every cell must report the entries classify scores against");

    let maintain = cells
        .iter()
        .find(|c| c["domain"] == "FABRICATE" && c["mode"] == "MAINTAIN")
        .expect("FABRICATE x MAINTAIN exists");
    assert!(
        !maintain["entries"].as_array().unwrap().is_empty(),
        "the cell that classify demonstrably uses cannot read as empty"
    );
}
