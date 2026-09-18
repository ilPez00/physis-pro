//! Phase 30 W1: typed-edge identity chain over HTTP.
//!
//! Nodes persist across restarts; W1 extends the same guarantee to typed
//! edges: write-through on add, rehydrate on load, cascade on node delete,
//! full 15-variant vocabulary, stable ids through the atlas payload, tenant
//! isolation. No LLM anywhere — RP embedder, routing/tenancy/persistence
//! assertions only.
#![cfg(feature = "web")]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::sync::MutexGuard;
use tower::ServiceExt;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn app() -> (
    axum::Router,
    tempfile::TempDir,
    MutexGuard<'static, ()>,
) {
    let env = ENV_LOCK.lock().await;
    let dir = tempfile::tempdir().expect("temp data dir");
    std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
    std::env::remove_var("PHYSIS_API_TOKEN");
    std::env::remove_var("PHYSIS_TOKENS");
    std::env::set_var("PHYSIS_SCAN_ROOT", dir.path());
    std::env::set_var("PHYSIS_DEV", "1");
    std::env::set_var("PHYSIS_DATA_DIR", dir.path());
    let (_state, router) = physis_pro::web::build_app()
        .await
        .expect("build_app must succeed");
    (router, dir, env)
}

async fn send(router: &axum::Router, mut req: Request<Body>) -> (StatusCode, Value) {
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            40001,
        ))));
    let resp = router.clone().oneshot(req).await.expect("router responds");
    let status = resp.status();
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
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
        .uri(path)
        .header("x-physis-user", user)
        .body(Body::empty())
        .unwrap()
}

async fn register(router: &axum::Router, user: &str, input: &str) -> String {
    let (status, body) = send(
        router,
        post("/api/v1/coherence/register", user, json!({"input": input})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["node_id"].as_str().expect("node_id").to_string()
}

#[tokio::test]
async fn full_vocab_accepts_and_unknown_rejects() {
    let (router, _dir, _env) = app().await;
    let a = register(&router, "t", "w1 vocab source node").await;
    let b = register(&router, "t", "w1 vocab target node").await;
    for rel in [
        "Supports", "Contradicts", "Causes", "Influences", "Precedes", "Follows",
        "DependsOn", "Requires", "Produces", "Measures", "Observes", "Explains",
        "Predicts", "Invalidates", "DerivedFrom",
    ] {
        let (status, body) = send(
            &router,
            post(
                "/api/v1/coherence/add-edge",
                "t",
                json!({"source": a, "relation": rel, "target": b}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{rel}: {body}");
        assert!(body["edge_id"].as_str().is_some(), "{rel}: {body}");
    }
    let (status, body) = send(
        &router,
        post(
            "/api/v1/coherence/add-edge",
            "t",
            json!({"source": a, "relation": "Befriends", "target": b}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn evidence_and_provenance_round_trip_through_projection() {
    let (router, _dir, _env) = app().await;
    let a = register(&router, "t", "w1 evidence source").await;
    let b = register(&router, "t", "w1 evidence target").await;
    let (status, body) = send(
        &router,
        post(
            "/api/v1/coherence/add-edge",
            "t",
            json!({
                "source": a, "relation": "Supports", "target": b,
                "evidence": "notes/deleuze.md:72-81",
                "provenance": "human",
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let eid = body["edge_id"].as_str().unwrap().to_string();
    let (_, body) = send(
        &router,
        get(&format!("/api/v1/coherence/edges?node_id={a}"), "t"),
    )
    .await;
    let out = body["outgoing"].as_array().expect("outgoing");
    let e = out.iter().find(|e| e["id"] == eid).expect("edge by id");
    assert_eq!(e["evidence"], "notes/deleuze.md:72-81");
    assert_eq!(e["provenance"], "human");
}

#[tokio::test]
async fn delete_cascades_edges_and_atlas_carries_edge_ids() {
    let (router, _dir, _env) = app().await;
    let a = register(&router, "t", "w1 cascade source").await;
    let b = register(&router, "t", "w1 cascade target").await;
    let (status, body) = send(
        &router,
        post(
            "/api/v1/coherence/add-edge",
            "t",
            json!({"source": a, "relation": "Causes", "target": b}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let eid = body["edge_id"].as_str().unwrap().to_string();

    // Atlas carries the typed edge with its stable id before the delete.
    let (_, body) = send(
        &router,
        get("/api/v1/atlas/graph?limit=200&k=4&min_sim=0.99", "t"),
    )
    .await;
    let edges = body["edges"].as_array().expect("edges");
    let typed = edges.iter().find(|e| e["id"] == eid);
    assert!(typed.is_some(), "typed edge id survives into atlas: {edges:?}");
    assert_eq!(typed.unwrap()["relation"], "Causes");

    // Deleting the source drops the edge from the projection.
    let (status, _) = send(
        &router,
        post("/api/v1/coherence/delete", "t", json!({"id": a})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = send(
        &router,
        get(&format!("/api/v1/coherence/edges?node_id={b}"), "t"),
    )
    .await;
    assert_eq!(body["incoming"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn edges_are_tenant_isolated() {
    let (router, _dir, _env) = app().await;
    let a = register(&router, "alice", "w1 tenant source").await;
    let b = register(&router, "alice", "w1 tenant target").await;
    let (status, _) = send(
        &router,
        post(
            "/api/v1/coherence/add-edge",
            "alice",
            json!({"source": a, "relation": "Precedes", "target": b}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = send(
        &router,
        get(&format!("/api/v1/coherence/edges?node_id={a}"), "bob"),
    )
    .await;
    assert_eq!(body["outgoing"].as_array().unwrap().len(), 0);
}
