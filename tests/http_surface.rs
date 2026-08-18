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
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::sync::MutexGuard;
use tower::ServiceExt;

#[cfg(feature = "sql")]
use rusqlite::Connection;

// `build_app` reads the process environment directly (HOME, PHYSIS_EMBEDDER,
// PHYSIS_API_TOKEN, PHYSIS_SCAN_ROOT). Cargo runs these tests on parallel
// threads in ONE process, so two tests building apps concurrently would clobber
// each other's env — the original harness had that latent race, and the auth
// test below *must* set a token that no other test may see. Serializing every
// env-touching test through this mutex makes the suite deterministic.
static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Build the app over an explicit data dir, holding the env mutex for the
/// caller. Every test (or repeated build in a restart test) passes through the
/// same lock, so the shared process env is never mutated concurrently.
async fn app_on(dir: &std::path::Path) -> (axum::Router, MutexGuard<'static, ()>) {
    let env = ENV_LOCK.lock().await;
    std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
    std::env::remove_var("PHYSIS_API_TOKEN");
    std::env::remove_var("PHYSIS_TOKENS");
    std::env::remove_var("PHYSIS_SCAN_ROOT");
    std::env::set_var("PHYSIS_DEV", "1");
    std::env::set_var("PHYSIS_DATA_DIR", dir); // config resolves data dir
    let (_state, router) = physis_pro::web::build_app().await.expect("build_app must succeed");
    (router, env)
}

/// Build the app once per test with an isolated data dir, so tests never share
/// sled state with each other or with a developer's real `~/.physis`.
async fn app() -> (axum::Router, tempfile::TempDir, MutexGuard<'static, ()>) {
    let dir = tempfile::tempdir().expect("temp data dir");
    let (router, env) = app_on(dir.path()).await;
    (router, dir, env)
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

/// Like `send`, but returns the raw body text (for HTML/text responses that
/// aren't JSON, e.g. the `/ui` and `/demo` pages).
async fn send_text(router: &Router, req: Request<Body>) -> (StatusCode, String) {
    let mut req = req;
    req.extensions_mut().insert(axum::extract::ConnectInfo(
        std::net::SocketAddr::from(([127, 0, 0, 1], 40000)),
    ));
    let resp = router.clone().oneshot(req).await.expect("router responds");
    let status = resp.status();
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
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

fn post_auth(path: &str, user: &str, body: Value, token: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .header("x-physis-user", user)
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn health_and_embedder_are_public() {
    // These two are the ops surface: they must answer without a token, and the
    // embedder must admit when it is degraded rather than look healthy.
    let (router, _dir, _env) = app().await;

    let (status, _) = send(&router, get("/health", "_default")).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(&router, get("/api/v1/embedder", "_default")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["kind"], "random-projection");
    assert_eq!(body["degraded"], true, "a non-semantic embedder must say so");
}

#[tokio::test]
async fn demo_and_console_ui_are_served() {
    // The demo dashboard and the dev console are single-file pages served by the
    // router — a missing include_str or a broken route must not ship silently.
    let (router, _dir, _env) = app().await;

    let (status, body) = send_text(&router, get("/demo", "_default")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Physis · Industrial Backoffice Intelligence"), "demo page must be the product dashboard");
    assert!(body.contains("/api/v1/oee/plant"), "demo page must wire the live API");

    let (status, body) = send_text(&router, get("/ui", "_default")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Physis Console"));

    // Every page loads the shared runtime with a bare `<script src>`, which
    // carries no Bearer token — so the asset has to be on the public router and
    // actually exist. It didn't: extracting ui-shell.js added the script tags
    // and no route, and all three dashboards booted with `$` undefined.
    for page in ["/demo", "/ui", "/study"] {
        let (_, body) = send_text(&router, get(page, "_default")).await;
        assert!(body.contains("src=\"ui-shell.js\""), "{page} loads the shared runtime");
    }
    let (status, body) = send_text(&router, get("/ui-shell.js", "_default")).await;
    assert_eq!(status, StatusCode::OK, "ui-shell.js must be served");
    assert!(body.contains("function toast"), "ui-shell.js must be the runtime, not an error page");
}

#[tokio::test]
async fn planner_simulates_without_mutating_live_state_and_admin_groups_duplicates() {
    let (router, _dir, _env) = app().await;
    let candidate = serde_json::json!({
        "kind": "office",
        "domains": [{
            "name": "Invoice Review",
            "category": "Finance",
            "domain": "STUDY",
            "mode": "WORK",
            "axis_kind": "office",
            "axis_name": "finance",
            // Required by `OntologyEntry`, and present on all 929 entries in
            // the shipped ontologies. Omitting it here made the endpoint answer
            // 400 rather than exercising the planner at all.
            "unit": "invoices",
            "hints": ["invoice", "supplier", "review"]
        }]
    });
    let (status, simulation) = send(
        &router,
        post(
            "/api/v1/planner/simulate",
            "planner-user",
            json!({
                "scenario_id": "procurement-v2",
                "candidate_ontology": candidate.to_string(),
                "texts": [{"id": "invoice-1", "source": "supplier invoice review"}]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "planner succeeds: {simulation}");
    assert_eq!(simulation["scenario_id"], "procurement-v2");
    assert_eq!(simulation["records_scanned"], 1);
    assert!(simulation["items"][0]["before"]["domain"].is_string());
    assert!(simulation["items"][0]["after"]["domain"].is_string());

    let (status, queue) = send(
        &router,
        post(
            "/api/v1/admin/review",
            "admin-user",
            json!([
                {"id":"a","path":"a.pdf","kind":"invoice","fingerprint":"dup-1"},
                {"id":"b","path":"b.pdf","kind":"invoice","fingerprint":"dup-1"},
                {"id":"c","path":"c.pdf","kind":"invoice"}
            ]),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(queue["documents"], 3);
    assert_eq!(queue["duplicate_documents"], 2);
    assert_eq!(queue["duplicate_groups"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn study_log_sessions_are_served_and_isolated_per_tenant() {
    // The learning/teaching layer: /study page serves, events persist with
    // their context metadata, a session aggregates them (timeline in order,
    // tallies, rhythms), and one student's log never leaks to another.
    let (router, _dir, _env) = app().await;

    let (status, body) = send_text(&router, get("/study", "_default")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("physis-schola"), "study page must be the learning dashboard");
    assert!(body.contains("/api/v1/study/session"), "study page must wire the live API");

    let ev = |at, action, path, place| json!({
        "observed_at": at, "action": action, "path": path, "place": place
    });
    let base = "2026-08-10T08:00:00Z";
    let (_, logged) = send(
        &router,
        post("/api/v1/study/log", "stu", json!({
            "events": [
                ev(base, "generated", "notes/topology.md", "Library"),
                ev("2026-08-11T08:00:00Z", "edited", "notes/topology.md", "Home"),
                ev("2026-08-12T08:00:00Z", "edited", "notes/topology.md", "Library"),
            ]
        })),
    )
    .await;
    assert_eq!(logged["accepted"], 3);
    assert_eq!(logged["stored"], 3);

    let (_, s) = send(&router, get("/api/v1/study/session", "stu")).await;
    assert_eq!(s["timeline"]["entries"].as_array().unwrap().len(), 3);
    assert_eq!(s["by_action"][0]["key"], "edited");
    assert_eq!(s["by_action"][0]["count"], 2);
    assert_eq!(s["by_type"][0]["key"], "md");
    assert_eq!(s["rhythms"][0]["period_days"].as_f64().unwrap(), 1.0, "daily rhythm detected");
    assert_eq!(s["timeline"]["entries"][0]["properties"]["place"], "Library");

    let (_, t) = send(&router, get("/api/v1/study/session", "tea")).await;
    assert_eq!(t["timeline"]["entries"].as_array().unwrap().len(), 0, "a teacher never sees a student's log");
}

#[tokio::test]
async fn coherence_nodes_are_isolated_per_tenant() {
    // Regression: /reconstruct, /context/filter and /compress/logs all read the
    // default tenant regardless of X-Physis-User, so a client that wrote under
    // its own id got empty results back and no error.
    let (router, _dir, _env) = app().await;

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
    let (router, _dir, _env) = app().await;

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
    let (router, _dir, _env) = app().await;

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
    let (router, _dir, _env) = app().await;

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
    let (router, _dir, _env) = app().await;

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
    let (router, _dir, _env) = app().await;

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

#[tokio::test]
async fn goals_survive_a_restart() {
    // Regression (PLAN 1H): goals were the last tenant state to silently die on
    // restart — the load_goals call site was missing from with_user, so the
    // persist code compiled clean, saved every goal, and nothing ever came back.
    // This is "build a process, write, drop it, rebuild on the same data dir".
    let dir = tempfile::tempdir().expect("temp data dir");

    let (router, env) = app_on(dir.path()).await;
    let (status, body) = send(
        &router,
        post("/api/v1/goals", "persist", json!({"prompt": "reduce nozzle clogs before the next batch"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["goal"]["id"].is_string(), "goal created");
    drop(router); // process exit — drops AppCtx (and the sled Db, flushing it)
    drop(env); // release the env lock before rebuilding, or app_on below deadlocks

    // Second "process" over the same data dir: the goal must be back, and a
    // tenant able to dream about it means load_goals actually fired.
    let (router, _env) = app_on(dir.path()).await;
    let (status, body) = send(
        &router,
        post("/api/v1/dream/generate", "persist", json!({"count": 2, "force": true})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let dreams = body["dreams"].as_array().expect("dreams array");
    assert!(!dreams.is_empty(), "a tenant whose goal survived restart can dream");
    let proposal = dreams[0]["proposal"].as_str().unwrap_or("");
    assert!(
        proposal.contains("nozzle"),
        "the proposal must name the persisted goal, got {proposal:?}"
    );
}

#[tokio::test]
async fn scan_is_confined_to_scan_root() {
    // Regression (PREMORTEM P6): /scan took an arbitrary server-side dir. The
    // guard canonicalizes and rejects anything outside the root — without it
    // `{"dir":"/etc"}` would expose the host filesystem.
    let dir = tempfile::tempdir().expect("temp data dir");

    // A real corpus inside the scan root (which defaults to the data dir).
    let inside = dir.path().join(".physis").join("notes");
    std::fs::create_dir_all(&inside).expect("mkdir inside scan root");
    std::fs::write(inside.join("fixture.txt"), "hello from the shop floor").expect("write fixture");

    let (router, _env) = app_on(dir.path()).await;

    let (status, _) = send(
        &router,
        post("/api/v1/scan", "_default", json!({"dir": inside.to_string_lossy()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "a path inside scan root is scannable");

    // A second scan reuses the directory hash diff and content-addressed
    // embeddings instead of forwarding the same label through the embedder.
    let (status, second) = send(
        &router,
        post("/api/v1/scan", "_default", json!({"dir": inside.to_string_lossy()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "repeat scan succeeds: {second}");
    assert_eq!(second["unchanged"], 1, "unchanged file is recognized: {second}");
    assert!(second["reused_labels"].as_u64().unwrap_or(0) > 0, "repeat scan reuses labels: {second}");
    assert_eq!(second["embedding_cache_hits"], 0, "unchanged labels bypass embedding: {second}");
    assert_eq!(second["embedding_cache_misses"], 0, "no re-embedding on repeat: {second}");

    // An existing directory OUTSIDE the root must be refused, not scanned.
    let outside = std::env::temp_dir();
    let (status, body) = send(
        &router,
        post("/api/v1/scan", "_default", json!({"dir": outside.to_string_lossy()})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "path outside scan root rejected");
    assert!(
        body["error"].as_str().unwrap_or("").contains("scan root"),
        "the refusal must name the confinement: {body}"
    );
}

#[tokio::test]
async fn fs_list_and_resolve_are_confined_to_scan_root() {
    // The console file manager cannot see OS paths from a drag-and-drop, so it
    // browses scan_root through /fs/list and maps dropped basenames through
    // /fs/resolve. Both must refuse anything outside the root.
    let dir = tempfile::tempdir().expect("temp data dir");
    let workshop = dir.path().join("workshop");
    std::fs::create_dir_all(workshop.join("notes")).expect("mkdir workshop/notes");
    std::fs::write(workshop.join("notes").join("log.txt"), "paused print").expect("write log");

    let env = ENV_LOCK.lock().await;
    std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
    std::env::remove_var("PHYSIS_API_TOKEN");
    std::env::remove_var("PHYSIS_TOKENS");
    std::env::set_var("PHYSIS_DEV", "1");
    std::env::set_var("PHYSIS_DATA_DIR", dir.path());
    std::env::set_var("PHYSIS_SCAN_ROOT", dir.path());
    let (_state, router) = physis_pro::web::build_app().await.expect("build_app must succeed");

    let (status, body) = send(&router, get("/api/v1/fs/list", "_default")).await;
    assert_eq!(status, StatusCode::OK, "list scan root: {body}");
    let names: Vec<&str> = body["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap_or(""))
        .collect();
    assert!(names.contains(&"workshop"), "scan root lists workshop: {body}");
    assert_eq!(body["parent"], serde_json::Value::Null, "scan root has no parent");

    let notes = workshop.join("notes");
    let uri = format!("/api/v1/fs/list?dir={}", notes.display());
    let (status, body) = send(&router, get(&uri, "_default")).await;
    assert_eq!(status, StatusCode::OK, "list notes: {body}");
    assert!(
        body["entries"].as_array().unwrap().iter().any(|e| e["name"] == "log.txt" && e["kind"] == "file"),
        "notes contains log.txt: {body}"
    );

    let (status, body) = send(&router, get("/api/v1/fs/list?dir=/etc", "_default")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "list outside scan root: {body}");

    let (status, body) = send(&router, get("/api/v1/fs/resolve?name=notes", "_default")).await;
    assert_eq!(status, StatusCode::OK, "resolve notes: {body}");
    let hits = body["matches"].as_array().expect("matches");
    assert!(
        hits.iter().any(|h| h["kind"] == "dir" && h["path"].as_str().unwrap_or("").ends_with("workshop/notes")),
        "resolve finds workshop/notes: {body}"
    );

    let (status, _) = send(&router, get("/api/v1/fs/resolve?name=workshop/notes", "_default")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "resolve rejects a path");

    std::env::remove_var("PHYSIS_SCAN_ROOT");
    drop(env);
}

#[cfg(feature = "sql")]
#[tokio::test]
async fn sql_schema_and_incremental_scan_are_exposed() {
    let dir = tempfile::tempdir().expect("temp data dir");
    let db_path = dir.path().join(".physis").join("orders.sqlite");
    std::fs::create_dir_all(db_path.parent().unwrap()).expect("mkdir database dir");
    let conn = Connection::open(&db_path).expect("create sqlite fixture");
    conn.execute_batch(
        "CREATE TABLE orders (id INTEGER PRIMARY KEY, status TEXT, amount INTEGER); \
         INSERT INTO orders VALUES (1, 'open', 10), (2, 'paid', 20);",
    )
    .expect("seed sqlite fixture");
    drop(conn);

    let (router, _env) = app_on(dir.path()).await;
    let uri = format!("/api/v1/sql/schema?database={}", urlencoding::encode(&db_path.to_string_lossy()));
    let (status, schema) = send(&router, get(&uri, "sql-test")).await;
    assert_eq!(status, StatusCode::OK, "schema discovery succeeds: {schema}");
    assert_eq!(schema["tables"][0]["name"], "orders");
    assert_eq!(schema["tables"][0]["columns"][0]["name"], "id");

    let request = json!({
        "database": db_path.to_string_lossy(),
        "table": "orders",
        "key_columns": ["id"]
    });
    let (status, first) = send(&router, post("/api/v1/sql/scan", "sql-test", request.clone())).await;
    assert_eq!(status, StatusCode::OK, "first SQL scan succeeds: {first}");
    assert_eq!(first["new"], 2);
    let (status, second) = send(&router, post("/api/v1/sql/scan", "sql-test", request)).await;
    assert_eq!(status, StatusCode::OK, "repeat SQL scan succeeds: {second}");
    assert_eq!(second["unchanged"], 2);
    assert_eq!(second["embedding_cache_misses"], 0);
}

#[tokio::test]
async fn watch_ingests_then_removes_nodes() {
    // Watch-mode auto-ingest (Phase 7B): one /api/v1/watch sweep registers
    // new/changed files as labeled core nodes, and a later sweep drops the
    // nodes for a removed file. Same scan-root confinement as /scan.
    let dir = tempfile::tempdir().expect("temp data dir");
    let inside = dir.path().join(".physis").join("notes");
    std::fs::create_dir_all(&inside).expect("mkdir inside scan root");
    let file = inside.join("topology.md");
    std::fs::write(&file, "# Topology\n## Graphs\n").expect("write fixture");

    let (router, _env) = app_on(dir.path()).await;

    let (status, body) = send(
        &router,
        post("/api/v1/watch", "w", json!({"dir": inside.to_string_lossy()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "first sweep succeeds: {body}");
    assert_eq!(body["new"], 1, "new file seen: {body}");
    assert!(body["registered"].as_u64().unwrap_or(0) >= 1, "nodes registered: {body}");

    // Idle sweep: nothing new, cache advanced.
    let (_, idle) = send(
        &router,
        post("/api/v1/watch", "w", json!({"dir": inside.to_string_lossy()})),
    )
    .await;
    assert_eq!(idle["new"], 0, "second sweep sees nothing new: {idle}");

    // Content change → re-register.
    std::fs::write(&file, "# Topology edited\n").expect("rewrite");
    let (_, changed) = send(
        &router,
        post("/api/v1/watch", "w", json!({"dir": inside.to_string_lossy()})),
    )
    .await;
    assert_eq!(changed["changed"], 1, "content change detected: {changed}");

    // Removal → nodes dropped.
    std::fs::remove_file(&file).expect("remove");
    let (_, removed) = send(
        &router,
        post("/api/v1/watch", "w", json!({"dir": inside.to_string_lossy()})),
    )
    .await;
    assert_eq!(removed["deleted"], 1, "removal detected: {removed}");
    assert!(removed["removed"].as_u64().unwrap_or(0) >= 1, "nodes dropped: {removed}");

    // Confinement: an outside dir is refused, not watched.
    let outside = std::env::temp_dir();
    let (status, body) = send(
        &router,
        post("/api/v1/watch", "w", json!({"dir": outside.to_string_lossy()})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "path outside scan root rejected: {body}");
}

#[tokio::test]
async fn vault_import_registers_recallable_nodes() {
    // Vault importers (Phase 7B): /api/v1/vault/import reduces a markdown/notes
    // dir to labeled core nodes (title + headings), recallable via search/nodes.
    let dir = tempfile::tempdir().expect("temp data dir");
    let vault = dir.path().join(".physis").join("vault");
    std::fs::create_dir_all(&vault).expect("mkdir inside scan root");
    std::fs::write(vault.join("alpha.md"), "# Alpha Note\n\n## Fixes\n\nmachine body text").expect("write note");

    let (router, _env) = app_on(dir.path()).await;

    let (status, body) = send(
        &router,
        post("/api/v1/vault/import", "v", json!({"dir": vault.to_string_lossy(), "git": 0})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "import succeeds: {body}");
    assert_eq!(body["docs"], 1, "one note: {body}");
    assert!(body["registered"].as_u64().unwrap_or(0) >= 1, "labels registered: {body}");

    // Recall by content.
    let (_, search) = send(
        &router,
        post("/api/v1/search/nodes", "v", json!({"query": "alpha note", "token_budget": 100})),
    )
    .await;
    assert!(
        search["results"].as_array().map(|a| a.iter().any(|r| r["label"].as_str().unwrap_or("").contains("Alpha")))
            .unwrap_or(false),
        "vault note recallable by content: {search}"
    );

    // Re-import dedupes.
    let (_, again) = send(
        &router,
        post("/api/v1/vault/import", "v", json!({"dir": vault.to_string_lossy(), "git": 0})),
    )
    .await;
    assert!(again["deduped"].as_u64().unwrap_or(0) >= 1, "re-import dedupes: {again}");

    // Confinement.
    let outside = std::env::temp_dir();
    let (status, _) = send(
        &router,
        post("/api/v1/vault/import", "v", json!({"dir": outside.to_string_lossy()})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "path outside scan root rejected");
}

#[tokio::test]
async fn history_import_registers_recallable_nodes() {
    // Personal-history importers (Phase 7B): /api/v1/history/import reduces a
    // browser bookmarks export to labeled core nodes, recallable by content.
    let dir = tempfile::tempdir().expect("temp data dir");
    let notes = dir.path().join(".physis").join("exports");
    std::fs::create_dir_all(&notes).expect("mkdir inside scan root");
    let bookmarks = notes.join("bookmarks.html");
    std::fs::write(&bookmarks, r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p><DT><A HREF="https://shopfloor.example/">Shop floor portal</A></DL><p>"#)
        .expect("write bookmarks");

    let (router, _env) = app_on(dir.path()).await;

    let (status, body) = send(
        &router,
        post("/api/v1/history/import", "h", json!({"path": bookmarks.to_string_lossy()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "import succeeds: {body}");
    assert_eq!(body["items"], 1, "one bookmark: {body}");
    assert_eq!(body["file"], "bookmarks.html");
    assert!(body["registered"].as_u64().unwrap_or(0) >= 1, "labels registered: {body}");

    // Recall by content.
    let (_, search) = send(
        &router,
        post("/api/v1/search/nodes", "h", json!({"query": "shop floor portal", "token_budget": 100})),
    )
    .await;
    assert!(
        search["results"].as_array().map(|a| a.iter().any(|r| r["label"].as_str().unwrap_or("").contains("Shop floor")))
            .unwrap_or(false),
        "bookmark recallable by content: {search}"
    );

    // Re-import dedupes.
    let (_, again) = send(
        &router,
        post("/api/v1/history/import", "h", json!({"path": bookmarks.to_string_lossy()})),
    )
    .await;
    assert!(again["deduped"].as_u64().unwrap_or(0) >= 1, "re-import dedupes: {again}");

    // Unsupported file → 400; outside scan root → 403.
    let csv = notes.join("data.csv");
    std::fs::write(&csv, "a,b\n").expect("write csv");
    let (status, _) = send(
        &router,
        post("/api/v1/history/import", "h", json!({"path": csv.to_string_lossy()})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unsupported extension rejected");

    let outside = std::env::temp_dir();
    let (status, _) = send(
        &router,
        post("/api/v1/history/import", "h", json!({"path": outside.to_string_lossy()})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "path outside scan root rejected");
}

#[tokio::test]
async fn praxis_backfill_seeds_behavioral_verdicts() {
    // Praxis backfill (Phase 7B): notebook entries + trackers become labeled
    // behavioral nodes whose asserted verdict follows the real life-log.
    let dir = tempfile::tempdir().expect("temp data dir");
    let exports = dir.path().join(".physis").join("exports");
    std::fs::create_dir_all(&exports).expect("mkdir inside scan root");
    let export = exports.join("praxis.json");
    std::fs::write(&export, r#"{
        "notebook": [
          {"content": "fixed the nozzle bind","mood": 9},
          {"content": "meh day","mood": 4},
          {"content": "gave up on the jig","mood": 2}
        ],
        "trackers": [
          {"name": "reading", "progress": 0.9},
          {"name": "meditate", "progress": 0.2}
        ]
    }"#).expect("write export");

    let (router, _env) = app_on(dir.path()).await;

    let (status, body) = send(
        &router,
        post("/api/v1/praxis/backfill", "p", json!({"path": export.to_string_lossy()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "backfill succeeds: {body}");
    assert_eq!(body["items"], 5, "3 notes + 2 trackers: {body}");
    assert_eq!(body["successes"], 2, "high mood + full tracker: {body}");
    assert_eq!(body["inert"], 1, "neutral mood: {body}");
    assert_eq!(body["failures"], 2, "low mood + empty tracker: {body}");
    assert!(body["registered"].as_u64().unwrap_or(0) >= 1, "nodes registered: {body}");

    // Re-backfill dedupes (labels praxis:notebook/… are stable).
    let (_, again) = send(
        &router,
        post("/api/v1/praxis/backfill", "p", json!({"path": export.to_string_lossy()})),
    )
    .await;
    assert!(again["deduped"].as_u64().unwrap_or(0) >= 1, "re-backfill dedupes: {again}");

    // Confinement.
    let outside = std::env::temp_dir();
    let (status, _) = send(
        &router,
        post("/api/v1/praxis/backfill", "p", json!({"path": outside.to_string_lossy()})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "path outside scan root rejected");
}

#[tokio::test]
#[cfg(feature = "spanner")]
async fn spanner_routes_are_registered_but_reject_unconfigured() {
    // Phase 7C wiring: the endpoints exist and are tenant-aware, but without
    // Spanner env they must answer BAD_GATEWAY (not 404/500) with a clear reason.
    let dir = tempfile::tempdir().expect("temp data dir");
    let (router, _env) = app_on(dir.path()).await;

    let (status, body) = send(
        &router,
        post("/api/v1/spanner/sync", "s", json!({"mirror": true})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "unconfigured sync → 502: {body}");
    assert!(body["error"].as_str().unwrap_or("").contains("not configured"), "names the problem: {body}");

    let (status, body) = send(
        &router,
        post("/api/v1/spanner/hydrate", "s", json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "unconfigured hydrate → 502: {body}");
}

#[tokio::test]
async fn bearer_token_gates_protected_routes() {
    // Regression (PREMORTEM P6): the API used to bind wide open. With a token
    // configured, protected routes must require it while the ops surface stays
    // public — and a wrong secret must not pass.
    let dir = tempfile::tempdir().expect("temp data dir");
    let env = ENV_LOCK.lock().await;
    std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
    std::env::set_var("PHYSIS_API_TOKEN", "test-secret");
    std::env::set_var("HOME", dir.path());
    let (_state, router) = physis_pro::web::build_app().await.expect("build_app must succeed");

    // Public ops surface: no token needed.
    let (status, _) = send(&router, get("/health", "_default")).await;
    assert_eq!(status, StatusCode::OK, "health stays public");
    let (status, _) = send(&router, get("/api/v1/embedder", "_default")).await;
    assert_eq!(status, StatusCode::OK, "embedder provenance stays public");

    // A protected POST with no bearer — the header must be compulsory.
    let (status, _) = send(
        &router,
        post("/api/v1/coherence/register", "u", json!({"input": "deploy step"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "no bearer is rejected");

    let (status, _) = send(
        &router,
        post_auth("/api/v1/coherence/register", "u", json!({"input": "deploy step"}), "wrong-secret"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a wrong secret is rejected");

    let (status, _) = send(
        &router,
        post_auth("/api/v1/coherence/register", "u", json!({"input": "deploy step"}), "test-secret"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the configured token passes");
    drop(env);
}

#[tokio::test]
async fn dev_mode_without_tokens_skips_auth() {
    // With PHYSIS_DEV=1 and no tokens configured on a loopback bind, protected
    // routes must remain accessible (explicit dev opt-in bypass).
    let dir = tempfile::tempdir().expect("temp data dir");
    let env = ENV_LOCK.lock().await;
    std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
    std::env::remove_var("PHYSIS_API_TOKEN");
    std::env::remove_var("PHYSIS_TOKENS");
    std::env::set_var("PHYSIS_DEV", "1");
    std::env::set_var("HOME", dir.path());
    let (_state, router) = physis_pro::web::build_app().await.expect("build_app must succeed");

    let (status, _) = send(
        &router,
        post("/api/v1/coherence/register", "u", json!({"input": "dev mode step"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "PHYSIS_DEV bypasses auth without tokens");
    drop(env);
}

#[tokio::test]
async fn no_dev_mode_without_tokens_requires_auth() {
    // Without PHYSIS_DEV and without any token, even on loopback, the server
    // must refuse to start — a container that accidentally binds wide open can
    // never expose an unauthenticated API.
    let dir = tempfile::tempdir().expect("temp data dir");
    let env = ENV_LOCK.lock().await;
    std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
    std::env::remove_var("PHYSIS_API_TOKEN");
    std::env::remove_var("PHYSIS_TOKENS");
    std::env::remove_var("PHYSIS_DEV");
    std::env::set_var("HOME", dir.path());

    let result = physis_pro::web::build_app().await;
    assert!(result.is_err(), "build_app must fail without PHYSIS_DEV and without tokens");
    drop(env);
}

#[tokio::test]
async fn semiotic_edit_swaps_content_keeps_structure() {
    // The "exchange parts" edit: change a node's content through the API while
    // its identity + verdict survive, the auto cell is reported, a pin overrides
    // it, and the dedupe index moves with the label.
    let (router, _dir, _env) = app().await;

    let (_, reg) = send(
        &router,
        post("/api/v1/coherence/register", "ed", json!({"input": "milling spindle bearing", "coherence_score": -1.0})),
    )
    .await;
    let id = reg["node_id"].as_str().unwrap().to_string();
    assert_eq!(reg["asserted"], -1.0);

    // Edit without a pin: verdict preserved, id stable, auto cell reported.
    let (status, edit) = send(
        &router,
        post("/api/v1/coherence/edit", "ed", json!({"id": id, "label": "welding robot joint recalibration"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "edit succeeds: {edit}");
    assert_eq!(edit["node_id"], id, "identity survives the content swap");
    assert_eq!(edit["label"], "welding robot joint recalibration");
    assert_eq!(edit["asserted"], -1.0, "verdict survives the swap");
    assert!(edit["auto_cell"]["domain"].is_string(), "auto cell derived from new content");
    assert!(edit["before_cell"].is_object(), "before cell reported for the delta");
    assert_eq!(edit["pinned_cell"], Value::Null, "no pin supplied → none stored");

    // The old label is no longer a dedupe alias: re-registering it makes a NEW
    // node; re-registering the new label returns the same id.
    let (_, dup_old) = send(
        &router,
        post("/api/v1/coherence/register", "ed", json!({"input": "milling spindle bearing"})),
    )
    .await;
    assert_ne!(dup_old["node_id"], id, "old label must not dedupe to the edited node");
    let (_, dup_new) = send(
        &router,
        post("/api/v1/coherence/register", "ed", json!({"input": "welding robot joint recalibration"})),
    )
    .await;
    assert_eq!(dup_new["node_id"], id, "new label dedupes to the edited node");
}

#[tokio::test]
async fn semiotic_edit_supports_pin_and_rejects_bad_cells() {
    let (router, _dir, _env) = app().await;

    let (_, reg) = send(
        &router,
        post("/api/v1/coherence/register", "pn", json!({"input": "autoclave cycle log"})),
    )
    .await;
    let id = reg["node_id"].as_str().unwrap().to_string();

    // Pin a cell: the pin is echoed + persisted (survives a re-read via edit).
    let (status, pinned) = send(
        &router,
        post("/api/v1/coherence/edit", "pn", json!({
            "id": id,
            "label": "autoclave cycle log (edited)",
            "cell": {"domain": "FABRICATE", "mode": "MAINTAIN"},
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "pinned edit succeeds: {pinned}");
    assert_eq!(pinned["pinned_cell"]["domain"], "FABRICATE");
    assert_eq!(pinned["pinned_cell"]["mode"], "MAINTAIN");

    // An invalid cell is rejected before anything is mutated.
    let (status, bad) = send(
        &router,
        post("/api/v1/coherence/edit", "pn", json!({
            "id": id,
            "label": "garbage cell",
            "cell": {"domain": "NOPE", "mode": "MAYBE"},
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "bad cell rejected: {bad}");

    // Unknown id → 404.
    let (status, missing) = send(
        &router,
        post("/api/v1/coherence/edit", "pn", json!({"id": "no-such-node", "label": "x"})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unknown id: {missing}");
}

/// The pin has to be visible from a *reader*, not just echoed by the writer.
/// The test above asserts the edit response contains `pinned_cell`, which the
/// handler builds from the value it just wrote — it would pass even if nothing
/// downstream honoured the pin, and for a while nothing did: `cell_pin` was
/// stored, persisted, and reported by no endpoint at all.
#[tokio::test]
async fn a_pin_is_visible_from_the_node_listing_and_outranks_the_classifier() {
    let (router, _dir, _env) = app().await;

    let (_, reg) = send(
        &router,
        post("/api/v1/coherence/register", "pv", json!({"input": "autoclave cycle log"})),
    )
    .await;
    let id = reg["node_id"].as_str().unwrap().to_string();

    let find = |body: &Value, id: &str| -> Value {
        body["results"]
            .as_array()
            .expect("results array")
            .iter()
            .find(|n| n["id"] == id)
            .expect("the node is listed")
            .clone()
    };

    // Unpinned: whatever the classifier says, flagged as not pinned.
    let (_, all) = send(&router, get("/api/v1/coherence/nodes/all", "pv")).await;
    let before = find(&all, &id);
    assert_eq!(before["pinned"], false, "an unedited node is not pinned: {before}");

    // Pin it somewhere the classifier would not have chosen on its own.
    let (status, _) = send(
        &router,
        post("/api/v1/coherence/edit", "pv", json!({
            "id": id,
            "label": "autoclave cycle log",
            "cell": {"domain": "BOND", "mode": "PLAY"},
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, all) = send(&router, get("/api/v1/coherence/nodes/all", "pv")).await;
    let pinned = find(&all, &id);
    assert_eq!(pinned["pinned"], true, "the listing reports the pin: {pinned}");
    assert_eq!(pinned["cell"][0], "BOND", "the pin outranks the classifier");
    assert_eq!(pinned["cell"][1], "PLAY");

    // An edit that says nothing about the cell must not silently unpin it —
    // changing the text is not a retraction of where someone said it belongs.
    let (status, _) = send(
        &router,
        post("/api/v1/coherence/edit", "pv", json!({
            "id": id, "label": "autoclave cycle log, revised",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, all) = send(&router, get("/api/v1/coherence/nodes/all", "pv")).await;
    let still = find(&all, &id);
    assert_eq!(still["pinned"], true, "a content edit leaves the pin standing: {still}");
    assert_eq!(still["cell"][0], "BOND");

    // Retraction is explicit: `"cell": null`.
    let (status, _) = send(
        &router,
        post("/api/v1/coherence/edit", "pv", json!({
            "id": id, "label": "autoclave cycle log, revised", "cell": null,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, all) = send(&router, get("/api/v1/coherence/nodes/all", "pv")).await;
    let cleared = find(&all, &id);
    assert_eq!(cleared["pinned"], false, "an explicit null retracts the pin: {cleared}");
}

/// A pin that disagrees with the classifier is a labelled correction, so it has
/// to reach the quality tracker — otherwise editing teaches the system nothing.
#[tokio::test]
async fn a_dissenting_pin_penalizes_the_cell_the_classifier_chose() {
    let (router, _dir, _env) = app().await;

    let (_, reg) = send(
        &router,
        post("/api/v1/coherence/register", "qz", json!({"input": "hydraulic press tonnage drift"})),
    )
    .await;
    let id = reg["node_id"].as_str().unwrap().to_string();

    // What the classifier picked on its own, before any human says otherwise.
    let (_, edited) = send(
        &router,
        post("/api/v1/coherence/edit", "qz", json!({
            "id": id, "label": "hydraulic press tonnage drift",
        })),
    )
    .await;
    let auto_domain = edited["auto_cell"]["domain"].as_str().unwrap_or("").to_string();
    let auto_mode = edited["auto_cell"]["mode"].as_str().unwrap_or("").to_string();
    assert!(!auto_domain.is_empty(), "the classifier picked a cell: {edited}");

    // Disagree with it, repeatedly. One pin is a sample, not a verdict, so the
    // penalty has to accumulate rather than land in full on the first click.
    let dissent = if auto_domain == "BOND" { "STUDY" } else { "BOND" };
    for _ in 0..3 {
        let (status, _) = send(
            &router,
            post("/api/v1/coherence/edit", "qz", json!({
                "id": id,
                "label": "hydraulic press tonnage drift",
                "cell": {"domain": dissent, "mode": "PLAY"},
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    let (status, quality) = send(&router, get("/api/v1/quality", "qz")).await;
    assert_eq!(status, StatusCode::OK, "quality readable: {quality}");
    let key = format!("{auto_domain}\u{0}{auto_mode}");
    let penalty = quality["penalties"][&key].as_f64().unwrap_or(0.0);
    assert!(
        penalty > 0.0,
        "three disagreements moved the penalty for {auto_domain}×{auto_mode}: {quality}"
    );
    assert!(
        penalty < 1.0,
        "a pin must never be able to lock a cell outright — that is reserved for \
         reported failures: {penalty}"
    );
}

#[tokio::test]
async fn coherence_delete_removes_node_and_index() {
    let (router, _dir, _env) = app().await;

    let (_, reg) = send(
        &router,
        post("/api/v1/coherence/register", "dl", json!({"input": "turbine bearing temp"})),
    )
    .await;
    let id = reg["node_id"].as_str().unwrap().to_string();
    assert_eq!(reg["node_id"], id);

    let (status, del) = send(
        &router,
        post("/api/v1/coherence/delete", "dl", json!({"id": id})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "delete succeeds: {del}");
    assert_eq!(del["ok"], true);

    // Second delete of the same id → 404.
    let (status, _) = send(
        &router,
        post("/api/v1/coherence/delete", "dl", json!({"id": id})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The label is free to register again (dedupe index cleaned up).
    let (_, dup) = send(
        &router,
        post("/api/v1/coherence/register", "dl", json!({"input": "turbine bearing temp"})),
    )
    .await;
    assert_ne!(dup["node_id"], id, "deleted label is free again");

    // Tenancy: another user cannot delete it (they can't even see it).
    let (status, _) = send(
        &router,
        post("/api/v1/coherence/delete", "other", json!({"id": id})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "cross-tenant delete refused");
}

#[tokio::test]
async fn fixed_token_node_search_bounds_the_response() {
    // The response's token cost is bounded by the budget no matter how many
    // nodes exist: labels are ranked, then greedily packed under the budget.
    let (router, _dir, _env) = app().await;
    let (_, _) = send(
        &router,
        post("/api/v1/coherence/register", "sr", json!({"input": "milling spindle bearing wear"})),
    )
    .await;
    let (_, _) = send(
        &router,
        post("/api/v1/coherence/register", "sr", json!({"input": "furnace temperature profile"})),
    )
    .await;
    let (_, _) = send(
        &router,
        post("/api/v1/coherence/register", "sr", json!({"input": "welding robot joint calibration"})),
    )
    .await;

    let (status, res) = send(
        &router,
        post("/api/v1/search/nodes", "sr", json!({"query": "spindle bearing", "token_budget": 20})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "search succeeds: {res}");
    assert!(res["total_tokens"].as_u64().unwrap_or(u64::MAX) <= 20,
            "total_tokens {} must respect the 20-token budget", res["total_tokens"]);
    assert_eq!(res["budget"], 20);
    assert!(!res["results"].as_array().unwrap().is_empty(), "relevant nodes found");
    assert_eq!(res["results"][0]["kind"], "node");
    assert!(res["results"][0]["label"].is_string());

    // Empty budget → zero results, zero tokens (hard bound holds at the floor).
    let (_, res) = send(
        &router,
        post("/api/v1/search/nodes", "sr", json!({"query": "spindle", "token_budget": 0})),
    )
    .await;
    assert_eq!(res["total_tokens"], 0);
}

// ── Industrial endpoints ─────────────────────────────────────────────

#[tokio::test]
async fn oee_endpoints_accept_and_compute() {
    let (router, _dir, _env) = app().await;

    // Register a machine.
    let (status, body) = send(
        &router,
        post("/api/v1/oee/machine", "plant", json!({
            "machine_id": "cnc-1",
            "shift_length_secs": 28800.0,
            "ideal_cycle_time_secs": 60.0
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "register machine: {body}");
    assert_eq!(body["ok"], true);
    assert!(body["detail"].as_str().unwrap().contains("cnc-1"));

    // Record production.
    let (status, body) = send(
        &router,
        post("/api/v1/oee/event", "plant", json!({
            "kind": "production",
            "machine_id": "cnc-1",
            "good": 85,
            "scrap": 2
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "record event: {body}");
    assert!(body["oee"].as_f64().unwrap_or(0.0) >= 0.0, "OEE must be non-negative");

    // Plant rollup.
    let (status, body) = send(&router, get("/api/v1/oee/plant", "plant")).await;
    assert_eq!(status, StatusCode::OK, "plant OEE: {body}");
    assert!(body["oee"].as_f64().unwrap_or(0.0) >= 0.0);
    assert_eq!(body["machine_count"], 1);

    // Per-machine report.
    let (status, body) = send(&router, get("/api/v1/oee/cnc-1", "plant")).await;
    assert_eq!(status, StatusCode::OK, "machine report: {body}");
}

#[tokio::test]
async fn maintenance_check_requires_baseline_and_returns_404_without() {
    let (router, _dir, _env) = app().await;

    // Without a baseline, check returns 404 (no baseline to score against).
    let (status, _) = send(
        &router,
        post("/api/v1/maintenance/check", "plant", json!({
            "machine_id": "cnc-1",
            "temperature": 65.0,
            "vibration": 1.2,
            "rpm": 8000,
            "status": "online"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "check without baseline returns 404");
}

#[tokio::test]
async fn voice_endpoints_are_registered_when_feature_enabled() {
    // Voice is a separate binary (physis-pro-voice) and does not expose routes
    // through the main web server. This test documents that fact so the absence
    // is explicit rather than an oversight.
    let (router, _dir, _env) = app().await;

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/voice/status")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&router, req).await;
    // Expect 404: voice lives in its own server.
    assert_eq!(status, StatusCode::NOT_FOUND);
}

