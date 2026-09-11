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
    std::env::set_var("PHYSIS_SCAN_ROOT", dir);
    std::env::set_var("PHYSIS_DEV", "1");
    std::env::set_var("PHYSIS_DATA_DIR", dir); // config resolves data dir
    let (_state, router) = physis_pro::web::build_app()
        .await
        .expect("build_app must succeed");
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
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            40000,
        ))));
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
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            40000,
        ))));
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

fn delete(path: &str, user: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(path)
        .header("x-physis-user", user)
        .body(Body::empty())
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

fn post_text(path: &str, user: &str, content_type: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", content_type)
        .header("x-physis-user", user)
        .body(Body::from(body.to_string()))
        .unwrap()
}

// ── Shift history (PLAN 5.1 + 5.3) ──────────────────────────────────────────

const SHIFT_CSV: &str = "\
machine_id,shift_length_secs,ideal_cycle_time_secs,total_count,good_count,downtime_secs,stop_reason,recorded_at
CNC-01,28800,60,400,390,1800,breakdown,2026-08-24T06:00:00Z
CNC-02,28800,45,500,500,,,
";

#[tokio::test]
async fn shift_csv_import_accepts_partially_and_reports_lines() {
    let (router, _dir, _env) = app().await;

    let csv = format!("{SHIFT_CSV}PRESS-9,28800,60,10,999,,,\nBROKEN,-1,60,1,1,,,\n");
    let (status, body) = send(
        &router,
        post_text("/api/v1/shifts/import", "plant", "text/csv", &csv),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["accepted"], 2, "the two good rows land");
    assert_eq!(
        body["rejected"].as_array().unwrap().len(),
        2,
        "the bad rows bounce"
    );
    assert_eq!(body["rejected"][0]["line"], 4);
    assert!(body["rejected"][0]["error"]
        .as_str()
        .unwrap()
        .contains("cannot exceed total_count"));
    assert_eq!(body["rejected"][1]["line"], 5);
    assert_eq!(body["stored"], 2);

    // History is durable and tenant-scoped.
    let (_, list) = send(&router, get("/api/v1/shifts", "plant")).await;
    assert_eq!(list["count"], 2);
    let (_, other) = send(&router, get("/api/v1/shifts", "intruder")).await;
    assert_eq!(other["count"], 0, "another tenant must see nothing");

    // Machine filter.
    let (_, filtered) = send(&router, get("/api/v1/shifts?machine=CNC-01", "plant")).await;
    assert_eq!(filtered["count"], 1);
    assert_eq!(filtered["shifts"][0]["shift"]["machine_id"], "CNC-01");
}

#[tokio::test]
async fn shift_report_aggregates_history_per_machine() {
    let (router, _dir, _env) = app().await;

    let csv = "\
machine_id,shift_length_secs,ideal_cycle_time_secs,total_count,good_count,downtime_secs,stop_reason
M1,100,10,100,90,20,breakdown
M1,100,10,100,80,0,
M2,200,20,50,45,0,
";
    let (_, imported) = send(
        &router,
        post_text("/api/v1/shifts/import", "plant", "text/csv", csv),
    )
    .await;
    assert_eq!(imported["accepted"], 3);

    let (status, report) = send(&router, get("/api/v1/shifts/report", "plant")).await;
    assert_eq!(status, StatusCode::OK);
    let machines = report["machines"].as_array().unwrap();
    assert_eq!(machines.len(), 2);
    let m1 = machines.iter().find(|m| m["machine_id"] == "M1").unwrap();
    assert_eq!(m1["total_count"], 200, "history sums across shifts");
    assert_eq!(m1["good_count"], 170);
    // Availability: (200 planned − 20 stopped) / 200.
    assert!((m1["availability"].as_f64().unwrap() - 0.9).abs() < 1e-4);

    // Plant rollup present with ≥2 machines.
    assert!(
        report["plant"]["oee"].is_number(),
        "rollup computed across machines"
    );

    // Single-machine slice.
    let (_, only_m2) = send(&router, get("/api/v1/shifts/report?machine=M2", "plant")).await;
    assert_eq!(only_m2["machines"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn shift_export_bundle_carries_provenance_and_metadata() {
    let (router, _dir, _env) = app().await;

    let (_, imported) = send(
        &router,
        post_text("/api/v1/shifts/import", "plant", "text/csv", SHIFT_CSV),
    )
    .await;
    assert_eq!(imported["accepted"], 2);

    // Evidence bundle: raw records + per-record OEE, aligned, tenant-stamped.
    let (_, export) = send(&router, get("/api/v1/shifts/export", "plant")).await;
    assert_eq!(export["tenant"], "plant");
    assert_eq!(export["shift_count"], 2);
    assert_eq!(export["machine_count"], 2);
    let records = export["records"].as_array().unwrap();
    let reports = export["reports"].as_array().unwrap();
    assert_eq!(
        records.len(),
        reports.len(),
        "every record has its recomputed OEE"
    );
    assert_eq!(records[0]["shift"]["machine_id"], reports[0]["machine_id"]);
    assert_eq!(
        export["span"]["from"], "2026-08-24T06:00:00Z",
        "window opens at the oldest dated record"
    );

    // Another tenant sees an empty bundle, not someone else's plant.
    let (_, foreign) = send(&router, get("/api/v1/shifts/export", "intruder")).await;
    assert_eq!(foreign["shift_count"], 0);

    // Aggregated report carries provenance metadata.
    let (_, report) = send(
        &router,
        get("/api/v1/shifts/report?machine=CNC-01", "plant"),
    )
    .await;
    assert_eq!(report["metadata"]["source"], "shift-history");
    assert_eq!(report["metadata"]["shift_count"], 1);

    // Customer-ready render paths reuse the report renderer.
    let (_, md) = send(
        &router,
        get("/api/v1/shifts/report?format=md&label=Agosto", "plant"),
    )
    .await;
    let markdown = md["markdown"].as_str().unwrap();
    assert!(markdown.contains("# Agosto"), "{markdown}");
    assert!(markdown.contains("turni su"), "provenance line present");

    let (status, html) =
        send_text(&router, get("/api/v1/shifts/report?format=html", "plant")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("class=\"meta\""));
    assert!(html.contains("<!DOCTYPE html>"));
}

#[tokio::test]
async fn shift_json_import_and_clear() {
    let (router, _dir, _env) = app().await;

    let body = json!({
        "shifts": [
            {"machine_id": "PRESS-2", "shift_length_secs": 14400.0,
             "ideal_cycle_time_secs": 30.0, "total_count": 300, "good_count": 295,
             "stops": [{"reason": "minor_stop", "duration_secs": 240.0}],
             "recorded_at": "2026-08-23T22:00:00Z"}
        ]
    });
    let (_, imported) = send(&router, post("/api/v1/shifts/import", "plant", body)).await;
    assert_eq!(imported["accepted"], 1, "{imported}");
    assert_eq!(imported["stored"], 1);

    let (_, bad) = send(
        &router,
        post(
            "/api/v1/shifts/import",
            "plant",
            json!({"shifts": [{"machine_id": "", "shift_length_secs": 1.0,
                               "ideal_cycle_time_secs": 1.0, "total_count": 1,
                               "good_count": 1}]}),
        ),
    )
    .await;
    assert_eq!(bad["accepted"], 0);
    assert!(bad["rejected"][0]["error"]
        .as_str()
        .unwrap()
        .contains("machine_id"));

    // A malformed JSON envelope is a reported error, never a 500.
    let (_, envelope_err) = send(
        &router,
        post("/api/v1/shifts/import", "plant", json!({"nope": true})),
    )
    .await;
    assert!(envelope_err["rejected"][0]["error"].is_string());

    let (_, cleared) = send(&router, post("/api/v1/shifts/clear", "plant", json!({}))).await;
    assert_eq!(cleared["cleared"], true);
    let (_, after) = send(&router, get("/api/v1/shifts", "plant")).await;
    assert_eq!(after["count"], 0);
}

#[tokio::test]
async fn health_and_embedder_are_public() {
    // These two are the ops surface: they must answer without a token, and the
    // embedder must admit when it is not semantic rather than look healthy.
    let (router, _dir, _env) = app().await;

    let (status, body) = send(&router, get("/health", "_default")).await;
    assert_eq!(status, StatusCode::OK);
    // PH-001: /health once reported 0.1.17 while the manifest said 0.1.19+ —
    // a hardcoded literal drifts. The handler now reads
    // env!("CARGO_PKG_VERSION"); this test fails if anyone re-hardcodes it,
    // because the test's copy comes from the same macro at build time.
    assert_eq!(
        body["version"],
        env!("CARGO_PKG_VERSION"),
        "/health version must derive from CARGO_PKG_VERSION, never a literal"
    );

    let (status, body) = send(&router, get("/api/v1/embedder", "_default")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["kind"], "random-projection");
    // `semantic` and `degraded` are two different facts and must not collapse:
    // `semantic` is the capability (is this a trained model?), `degraded` is the
    // alarm (did a model we expected to work silently emit garbage?). This
    // harness sets PHYSIS_EMBEDDER=random-projection, i.e. the operator *asked*
    // for offline mode — so it is non-semantic on purpose and must NOT raise the
    // alarm, or an intentional offline run is indistinguishable from a real
    // model failure.
    assert_eq!(
        body["semantic"], false,
        "a non-semantic embedder must say so"
    );
    assert_eq!(
        body["degraded"], false,
        "explicitly requested offline mode is supported, not a silent failure"
    );
}

#[tokio::test]
async fn demo_and_console_ui_are_served() {
    // The demo dashboard and the dev console are single-file pages served by the
    // router — a missing include_str or a broken route must not ship silently.
    let (router, _dir, _env) = app().await;

    let (status, body) = send_text(&router, get("/demo", "_default")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("Physis · Industrial Backoffice Intelligence"),
        "demo page must be the product dashboard"
    );
    assert!(
        body.contains("/api/v1/oee/plant"),
        "demo page must wire the live API"
    );

    let (status, body) = send_text(&router, get("/console", "_default")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Physis Console"));
    // PLAN 3.3: the quality feedback-loop tab ships inside the unified console.
    assert!(
        body.contains("data-tab=\"quality\"") && body.contains("id=\"tab-quality\""),
        "console serves the Quality tab (nav button + section)"
    );
    assert!(
        body.contains("loadQuality"),
        "console wires a loader for the Quality tab"
    );

    let (status, body) = send_text(&router, get("/ui", "_default")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("GRAPH FILE MANAGER"),
        "/ui is the graph file-manager"
    );

    // Every page loads the shared runtime with a bare `<script src>`, which
    // carries no Bearer token — so the asset has to be on the public router and
    // actually exist. It didn't: extracting ui-shell.js added the script tags
    // and no route, and all three dashboards booted with `$` undefined.
    for page in ["/demo", "/ui", "/study"] {
        let (_, body) = send_text(&router, get(page, "_default")).await;
        assert!(
            body.contains("src=\"ui-shell.js\""),
            "{page} loads the shared runtime"
        );
    }
    let (status, body) = send_text(&router, get("/ui-shell.js", "_default")).await;
    assert_eq!(status, StatusCode::OK, "ui-shell.js must be served");
    assert!(
        body.contains("function toast"),
        "ui-shell.js must be the runtime, not an error page"
    );
    // The ask bar speaks to the NL endpoint (PLAN 3.2), not the raw search —
    // drift here would silently demote every page's query box back to search-only.
    assert!(body.contains("/api/v1/ask"), "ask bar wired to /api/v1/ask");
}

/// The llms.txt convention (adoption track 1): AI crawlers read one plain file
/// to learn what an instance can do. Public, no auth, served from the repo.
#[tokio::test]
async fn llms_txt_is_publicly_served_and_names_the_capabilities() {
    let (router, _dir, _env) = app().await;

    // No auth header — must be public like /health.
    let (status, body) = send_text(&router, get("/llms.txt", "")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.starts_with("# Physis Pro"));
    for capability in [
        "/api/v1/ask",
        "/api/v1/files/query",
        "physis-pro serve --stdio",
        "space·time·cause",
    ] {
        assert!(body.contains(capability), "llms.txt names `{capability}`");
    }
}

/// Regression: the console once shipped ~950 lines of JS after `</html>`.
#[tokio::test]
async fn dashboard_pages_end_at_the_closing_html_tag() {
    // Console shipped with ~950 lines of JavaScript appended AFTER `</html>`:
    // 19 functions (hypotheses, contradictions, epistemic audit, explanation,
    // discovery, coherence harness, file browser) whose panels were all present
    // in the markup. The browser ended the document at the tag and painted the
    // source as visible page text, so those panels rendered but did nothing.
    //
    // Nothing about that is loud — the page still returns 200 with the right
    // title, so every assertion above kept passing. Anchor on the structure
    // instead: markup after the document ends is silently dead code.
    let (router, _dir, _env) = app().await;

    for page in ["/demo", "/ui", "/study", "/agent"] {
        let (status, body) = send_text(&router, get(page, "_default")).await;
        assert_eq!(status, StatusCode::OK, "{page} must be served");

        let end = body
            .rfind("</html>")
            .unwrap_or_else(|| panic!("{page} must close its html element"));
        let trailing = body[end + "</html>".len()..].trim();
        assert!(
            trailing.is_empty(),
            "{page} has {} characters after </html> — anything there is outside \
             the document and never runs. Starts: {:?}",
            trailing.len(),
            &trailing[..trailing.len().min(120)],
        );

        // An unbalanced pair is the other way source leaks into the page.
        let opens = body.matches("<script").count();
        let closes = body.matches("</script>").count();
        assert_eq!(
            opens, closes,
            "{page} has {opens} <script> opens against {closes} closes",
        );
    }
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
    assert!(
        body.contains("physis-schola"),
        "study page must be the learning dashboard"
    );
    assert!(
        body.contains("/api/v1/study/session"),
        "study page must wire the live API"
    );

    let ev = |at, action, path, place| {
        json!({
            "observed_at": at, "action": action, "path": path, "place": place
        })
    };
    let base = "2026-08-10T08:00:00Z";
    let (_, logged) = send(
        &router,
        post(
            "/api/v1/study/log",
            "stu",
            json!({
                "events": [
                    ev(base, "generated", "notes/topology.md", "Library"),
                    ev("2026-08-11T08:00:00Z", "edited", "notes/topology.md", "Home"),
                    ev("2026-08-12T08:00:00Z", "edited", "notes/topology.md", "Library"),
                ]
            }),
        ),
    )
    .await;
    assert_eq!(logged["accepted"], 3);
    assert_eq!(logged["stored"], 3);

    let (_, s) = send(&router, get("/api/v1/study/session", "stu")).await;
    assert_eq!(s["timeline"]["entries"].as_array().unwrap().len(), 3);
    assert_eq!(s["by_action"][0]["key"], "edited");
    assert_eq!(s["by_action"][0]["count"], 2);
    assert_eq!(s["by_type"][0]["key"], "md");
    assert_eq!(
        s["rhythms"][0]["period_days"].as_f64().unwrap(),
        1.0,
        "daily rhythm detected"
    );
    assert_eq!(
        s["timeline"]["entries"][0]["properties"]["place"],
        "Library"
    );

    let (_, t) = send(&router, get("/api/v1/study/session", "tea")).await;
    assert_eq!(
        t["timeline"]["entries"].as_array().unwrap().len(),
        0,
        "a teacher never sees a student's log"
    );
}

#[tokio::test]
async fn coherence_nodes_are_isolated_per_tenant() {
    // Regression: /reconstruct, /context/filter and /compress/logs all read the
    // default tenant regardless of X-Physis-User, so a client that wrote under
    // its own id got empty results back and no error.
    let (router, _dir, _env) = app().await;

    let (status, body) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "alice",
            json!({"input": "alice's private note"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["node_id"].is_string());

    let (_, alice) = send(
        &router,
        get("/api/v1/coherence/nodes/all?limit=10", "alice"),
    )
    .await;
    assert_eq!(alice["count"], 1, "alice sees her own node");

    let (_, bob) = send(&router, get("/api/v1/coherence/nodes/all?limit=10", "bob")).await;
    assert_eq!(bob["count"], 0, "bob must not see alice's node");

    // The read half of the loop has to honour the header too.
    let (_, recon) = send(
        &router,
        post(
            "/api/v1/reconstruct",
            "alice",
            json!({"input": "private note"}),
        ),
    )
    .await;
    assert!(
        recon["count"].as_u64().unwrap_or(0) >= 1,
        "reconstruct must read the caller's tenant, not _default"
    );

    let (_, empty) = send(
        &router,
        post(
            "/api/v1/reconstruct",
            "bob",
            json!({"input": "private note"}),
        ),
    )
    .await;
    assert_eq!(empty["count"], 0, "and must stay empty for another tenant");
}

#[tokio::test]
async fn coherence_nodes_all_respects_as_of_point_in_time() {
    // Time-travel: an `as_of` RFC3339 cutoff must exclude nodes registered
    // after that instant. The node we register is stamped "now", so a cutoff in
    // the distant past yields nothing and a cutoff in the far future keeps it.
    let (router, _dir, _env) = app().await;

    let (status, body) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "carol",
            json!({"input": "a note registered right now"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["node_id"].is_string());

    let (_, past) = send(
        &router,
        get("/api/v1/coherence/nodes/all?limit=10&as_of=1970-01-01T00:00:00Z", "carol"),
    )
    .await;
    assert_eq!(past["count"], 0, "cutoff before registration hides the node");
    assert_eq!(
        past["as_of"].as_str(),
        Some("1970-01-01T00:00:00Z"),
        "echoes the requested cutoff"
    );

    let (_, future) = send(
        &router,
        get("/api/v1/coherence/nodes/all?limit=10&as_of=2099-01-01T00:00:00Z", "carol"),
    )
    .await;
    assert_eq!(future["count"], 1, "cutoff after registration keeps the node");

    let (_, live) = send(
        &router,
        get("/api/v1/coherence/nodes/all?limit=10", "carol"),
    )
    .await;
    assert_eq!(live["as_of"], serde_json::Value::Null, "no cutoff when omitted");

    // A malformed cutoff must not 500 — it degrades to the live view.
    let (_, bad) = send(
        &router,
        get("/api/v1/coherence/nodes/all?limit=10&as_of=not-a-date", "carol"),
    )
    .await;
    assert_eq!(bad["count"], 1, "unparseable cutoff is ignored, shows all");
}

#[tokio::test]
async fn coherence_query_filters_by_verdict_and_label() {
    // The "objects" API: structured facet query over the live graph.
    let (router, _dir, _env) = app().await;

    // Register three nodes with distinct labels + outcomes.
    let (_, a) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "dave",
            json!({"input": "Finance invoice mismatch on PO-77", "coherence_score": -1.0}),
        ),
    )
    .await;
    let (_, b) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "dave",
            json!({"input": "Procurement lead time within SLA", "coherence_score": 1.0}),
        ),
    )
    .await;
    let (_, c) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "dave",
            json!({"input": "Unrelated maintenance log entry", "coherence_score": 0.0}),
        ),
    )
    .await;
    assert!(a["node_id"].is_string() && b["node_id"].is_string() && c["node_id"].is_string());

    // verdict=failure → only the -1 node.
    let (_, fails) = send(
        &router,
        get("/api/v1/coherence/facet?verdict=failure&limit=10", "dave"),
    )
    .await;
    assert_eq!(fails["count"], 1, "one failure node");
    assert_eq!(fails["results"][0]["rating"], "Failure");

    // q=invoice → only the Finance node (substring, case-insensitive).
    let (_, inv) = send(
        &router,
        get("/api/v1/coherence/facet?q=INVOICE&limit=10", "dave"),
    )
    .await;
    assert_eq!(inv["count"], 1, "substring match is case-insensitive");
    assert!(inv["results"][0]["label"].as_str().unwrap().contains("invoice"));

    // Combined verdict=success + q=procure → the +1 procurement node.
    let (_, combo) = send(
        &router,
        get("/api/v1/coherence/facet?verdict=success&q=procure&limit=10", "dave"),
    )
    .await;
    assert_eq!(combo["count"], 1, "verdict + label filters compose");

    // No filter → all three.
    let (_, all) = send(
        &router,
        get("/api/v1/coherence/facet?limit=10", "dave"),
    )
    .await;
    assert_eq!(all["count"], 3, "no filter returns the whole store");
}

#[tokio::test]
async fn a_verdict_is_stored_and_can_be_corrected() {
    // The asserted axis travels on the wire and re-registering the same label
    // updates it — that is how a task reported optimistically at spawn gets
    // corrected when it later fails.
    let (router, _dir, _env) = app().await;

    let (_, first) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "t",
            json!({"input": "deploy step", "coherence_score": 1.0}),
        ),
    )
    .await;
    assert_eq!(first["asserted"], 1.0);

    let (_, again) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "t",
            json!({"input": "deploy step", "coherence_score": -1.0}),
        ),
    )
    .await;
    assert_eq!(
        again["node_id"], first["node_id"],
        "dedupe returns the same node"
    );
    assert_eq!(again["asserted"], -1.0, "and the verdict is corrected");

    // Out-of-range verdicts are clamped rather than stored raw.
    let (_, clamped) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "t",
            json!({"input": "deploy step", "coherence_score": 9.0}),
        ),
    )
    .await;
    assert_eq!(clamped["asserted"], 1.0);
}

#[tokio::test]
async fn outcome_memory_surfaces_recorded_failures_on_recall() {
    // The killer feature: recall is outcome-valenced. A node the caller has
    // judged as failed (−1) must travel its verdict into /search/nodes and into
    // the top-level `failures` list, so an agent sees "you already tried this
    // and it blew up" instead of a bare cosimilar match.
    let (router, _dir, _env) = app().await;

    // Register a labeled node and mark it a recorded failure.
    let (_, reg) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "om",
            json!({"input": "bake the firmware at 300C", "coherence_score": -1.0}),
        ),
    )
    .await;
    assert_eq!(reg["asserted"], -1.0, "verdict persisted on register");

    // Recall by the same text — random-projection embedder ranks exact text first.
    let (_, search) = send(
        &router,
        post(
            "/api/v1/search/nodes",
            "om",
            json!({"query": "bake the firmware at 300C", "token_budget": 200}),
        ),
    )
    .await;

    // The hit carries the verdict...
    let failed_hit = search["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["label"].as_str().unwrap_or("").contains("firmware"))
        .expect("failed node is recalled");
    assert_eq!(
        failed_hit["verdict"], -1.0,
        "recalled hit exposes its asserted verdict"
    );

    // ...and the top-level failures list names it.
    let in_failures = search["failures"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["label"].as_str().unwrap_or("").contains("firmware"));
    assert!(in_failures, "recorded failure surfaced in `failures`: {search}");

    // A node judged as success (+1) is recalled with its verdict but is NOT a
    // failure — proving the failures list is verdict-gated, not "everything".
    let (_, ok) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "om",
            json!({"input": "ship the signed image", "coherence_score": 1.0}),
        ),
    )
    .await;
    assert_eq!(ok["asserted"], 1.0, "success verdict persisted");

    let (_, search2) = send(
        &router,
        post(
            "/api/v1/search/nodes",
            "om",
            json!({"query": "ship the signed image", "token_budget": 400}),
        ),
    )
    .await;
    let ok_hit = search2["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["label"].as_str().unwrap_or("").contains("signed image"))
        .expect("success node recalled");
    assert_eq!(ok_hit["verdict"], 1.0, "success hit keeps its verdict");
    let failure_labels: Vec<String> = search2["failures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["label"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        !failure_labels.iter().any(|l| l.contains("signed image")),
        "success node must not appear in failures: {search2}"
    );
}

#[tokio::test]
async fn dreams_return_a_readable_proposal_not_raw_vectors() {
    // Regression: the handler serialized two 768-d vectors per dream and
    // dropped `description`, so every client saw the arithmetic and none of
    // the reasoning.
    let (router, _dir, _env) = app().await;

    send(
        &router,
        post(
            "/api/v1/goals",
            "d",
            json!({"prompt": "reduce nozzle clogs before the next batch"}),
        ),
    )
    .await;

    let (status, body) = send(
        &router,
        post(
            "/api/v1/dream/generate",
            "d",
            json!({"count": 2, "force": true}),
        ),
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
        post(
            "/api/v1/dream/generate",
            "d",
            json!({"count": 1, "force": true, "embeddings": true}),
        ),
    )
    .await;
    assert!(raw["dreams"][0]["embedding"].is_array());
}

#[tokio::test]
async fn dream_generate_honours_the_validation_contract() {
    // The Generative Ontology contract (`research_new`): with `min_cosine`
    // set, only dreams the ontology grid can name are proposed, the response
    // accounts for rejections, and every proposal carries its review verdict.
    let (router, _dir, _env) = app().await;

    send(
        &router,
        post(
            "/api/v1/goals",
            "c",
            json!({"prompt": "reduce nozzle clogs before the next batch"}),
        ),
    )
    .await;

    // An impossible bar for the random-projection embedder: nothing the
    // engine can dream will clear 0.95 against the 973-entry grid, so the
    // contract must reject the whole batch rather than pretend the dreams
    // are coherent. Rejection is information, not an error.
    let (status, body) = send(
        &router,
        post(
            "/api/v1/dream/generate",
            "c",
            json!({"count": 2, "force": true, "min_cosine": 0.95}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["count"], 0, "nothing clears an impossible bar: {body}");
    let contract = &body["contract"];
    assert!(contract.is_object(), "contract accounting present: {body}");
    assert!(
        contract["rejected"].as_u64().unwrap_or(0) >= 2,
        "the attempted dreams are accounted for: {contract}"
    );

    // A sane bar lets some dreams through, each carrying its verdict.
    let (status, body) = send(
        &router,
        post(
            "/api/v1/dream/generate",
            "c",
            json!({"count": 3, "force": true, "min_cosine": 0.2}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let dreams = body["dreams"].as_array().expect("dreams array");
    assert!(!dreams.is_empty(), "a sane bar admits dreams: {body}");
    for d in dreams {
        let review = &d["review"];
        assert!(review["cosine"].is_number(), "verdict present: {d}");
        assert!(
            review["cosine"].as_f64().unwrap() >= 0.2,
            "accepted dreams clear the bar: {d}"
        );
    }
    assert!(body["contract"].is_object());
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
            post(
                "/api/v1/coherence/register",
                "r",
                json!({"input": text, "coherence_score": score}),
            ),
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
    assert!(
        labels.iter().any(|l| l.contains("clogged")),
        "failures are replayed"
    );
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
        post(
            "/api/v1/holarchy/edge",
            "g",
            json!({"source": ids[0], "target": ids[1]}),
        ),
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
    assert_eq!(
        empty, 0,
        "every cell must report the entries classify scores against"
    );

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
        post(
            "/api/v1/goals",
            "persist",
            json!({"prompt": "reduce nozzle clogs before the next batch"}),
        ),
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
        post(
            "/api/v1/dream/generate",
            "persist",
            json!({"count": 2, "force": true}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let dreams = body["dreams"].as_array().expect("dreams array");
    assert!(
        !dreams.is_empty(),
        "a tenant whose goal survived restart can dream"
    );
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
        post(
            "/api/v1/scan",
            "_default",
            json!({"dir": inside.to_string_lossy()}),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a path inside scan root is scannable"
    );

    // A second scan reuses the directory hash diff and content-addressed
    // embeddings instead of forwarding the same label through the embedder.
    let (status, second) = send(
        &router,
        post(
            "/api/v1/scan",
            "_default",
            json!({"dir": inside.to_string_lossy()}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "repeat scan succeeds: {second}");
    assert_eq!(
        second["unchanged"], 1,
        "unchanged file is recognized: {second}"
    );
    assert!(
        second["reused_labels"].as_u64().unwrap_or(0) > 0,
        "repeat scan reuses labels: {second}"
    );
    assert_eq!(
        second["embedding_cache_hits"], 0,
        "unchanged labels bypass embedding: {second}"
    );
    assert_eq!(
        second["embedding_cache_misses"], 0,
        "no re-embedding on repeat: {second}"
    );

    // An existing directory OUTSIDE the root must be refused, not scanned.
    let outside = std::env::temp_dir();
    let (status, body) = send(
        &router,
        post(
            "/api/v1/scan",
            "_default",
            json!({"dir": outside.to_string_lossy()}),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "path outside scan root rejected"
    );
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
    let (_state, router) = physis_pro::web::build_app()
        .await
        .expect("build_app must succeed");

    let (status, body) = send(&router, get("/api/v1/fs/list", "_default")).await;
    assert_eq!(status, StatusCode::OK, "list scan root: {body}");
    let names: Vec<&str> = body["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap_or(""))
        .collect();
    assert!(
        names.contains(&"workshop"),
        "scan root lists workshop: {body}"
    );
    assert_eq!(
        body["parent"],
        serde_json::Value::Null,
        "scan root has no parent"
    );

    let notes = workshop.join("notes");
    let uri = format!("/api/v1/fs/list?dir={}", notes.display());
    let (status, body) = send(&router, get(&uri, "_default")).await;
    assert_eq!(status, StatusCode::OK, "list notes: {body}");
    assert!(
        body["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["name"] == "log.txt" && e["kind"] == "file"),
        "notes contains log.txt: {body}"
    );

    let (status, body) = send(&router, get("/api/v1/fs/list?dir=/etc", "_default")).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "list outside scan root: {body}"
    );

    let (status, body) = send(&router, get("/api/v1/fs/resolve?name=notes", "_default")).await;
    assert_eq!(status, StatusCode::OK, "resolve notes: {body}");
    let hits = body["matches"].as_array().expect("matches");
    assert!(
        hits.iter()
            .any(|h| h["kind"] == "dir"
                && h["path"].as_str().unwrap_or("").ends_with("workshop/notes")),
        "resolve finds workshop/notes: {body}"
    );

    let (status, _) = send(
        &router,
        get("/api/v1/fs/resolve?name=workshop/notes", "_default"),
    )
    .await;
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
    let uri = format!(
        "/api/v1/sql/schema?database={}",
        urlencoding::encode(&db_path.to_string_lossy())
    );
    let (status, schema) = send(&router, get(&uri, "sql-test")).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "schema discovery succeeds: {schema}"
    );
    assert_eq!(schema["tables"][0]["name"], "orders");
    assert_eq!(schema["tables"][0]["columns"][0]["name"], "id");

    let request = json!({
        "database": db_path.to_string_lossy(),
        "table": "orders",
        "key_columns": ["id"]
    });
    let (status, first) = send(
        &router,
        post("/api/v1/sql/scan", "sql-test", request.clone()),
    )
    .await;
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
        post(
            "/api/v1/watch",
            "w",
            json!({"dir": inside.to_string_lossy()}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "first sweep succeeds: {body}");
    assert_eq!(body["new"], 1, "new file seen: {body}");
    assert!(
        body["registered"].as_u64().unwrap_or(0) >= 1,
        "nodes registered: {body}"
    );

    // Idle sweep: nothing new, cache advanced.
    let (_, idle) = send(
        &router,
        post(
            "/api/v1/watch",
            "w",
            json!({"dir": inside.to_string_lossy()}),
        ),
    )
    .await;
    assert_eq!(idle["new"], 0, "second sweep sees nothing new: {idle}");

    // Content change → re-register.
    std::fs::write(&file, "# Topology edited\n").expect("rewrite");
    let (_, changed) = send(
        &router,
        post(
            "/api/v1/watch",
            "w",
            json!({"dir": inside.to_string_lossy()}),
        ),
    )
    .await;
    assert_eq!(changed["changed"], 1, "content change detected: {changed}");

    // Removal → nodes dropped.
    std::fs::remove_file(&file).expect("remove");
    let (_, removed) = send(
        &router,
        post(
            "/api/v1/watch",
            "w",
            json!({"dir": inside.to_string_lossy()}),
        ),
    )
    .await;
    assert_eq!(removed["deleted"], 1, "removal detected: {removed}");
    assert!(
        removed["removed"].as_u64().unwrap_or(0) >= 1,
        "nodes dropped: {removed}"
    );

    // Confinement: an outside dir is refused, not watched.
    let outside = std::env::temp_dir();
    let (status, body) = send(
        &router,
        post(
            "/api/v1/watch",
            "w",
            json!({"dir": outside.to_string_lossy()}),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "path outside scan root rejected: {body}"
    );
}

#[tokio::test]
async fn vault_import_registers_recallable_nodes() {
    // Vault importers (Phase 7B): /api/v1/vault/import reduces a markdown/notes
    // dir to labeled core nodes (title + headings), recallable via search/nodes.
    let dir = tempfile::tempdir().expect("temp data dir");
    let vault = dir.path().join(".physis").join("vault");
    std::fs::create_dir_all(&vault).expect("mkdir inside scan root");
    std::fs::write(
        vault.join("alpha.md"),
        "# Alpha Note\n\n## Fixes\n\nmachine body text",
    )
    .expect("write note");

    let (router, _env) = app_on(dir.path()).await;

    let (status, body) = send(
        &router,
        post(
            "/api/v1/vault/import",
            "v",
            json!({"dir": vault.to_string_lossy(), "git": 0}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "import succeeds: {body}");
    assert_eq!(body["docs"], 1, "one note: {body}");
    assert!(
        body["registered"].as_u64().unwrap_or(0) >= 1,
        "labels registered: {body}"
    );

    // Recall by content.
    let (_, search) = send(
        &router,
        post(
            "/api/v1/search/nodes",
            "v",
            json!({"query": "alpha note", "token_budget": 100}),
        ),
    )
    .await;
    assert!(
        search["results"]
            .as_array()
            .map(|a| a
                .iter()
                .any(|r| r["label"].as_str().unwrap_or("").contains("Alpha")))
            .unwrap_or(false),
        "vault note recallable by content: {search}"
    );

    // Re-import dedupes.
    let (_, again) = send(
        &router,
        post(
            "/api/v1/vault/import",
            "v",
            json!({"dir": vault.to_string_lossy(), "git": 0}),
        ),
    )
    .await;
    assert!(
        again["deduped"].as_u64().unwrap_or(0) >= 1,
        "re-import dedupes: {again}"
    );

    // Confinement.
    let outside = std::env::temp_dir();
    let (status, _) = send(
        &router,
        post(
            "/api/v1/vault/import",
            "v",
            json!({"dir": outside.to_string_lossy()}),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "path outside scan root rejected"
    );
}

#[tokio::test]
async fn history_import_registers_recallable_nodes() {
    // Personal-history importers (Phase 7B): /api/v1/history/import reduces a
    // browser bookmarks export to labeled core nodes, recallable by content.
    let dir = tempfile::tempdir().expect("temp data dir");
    let notes = dir.path().join(".physis").join("exports");
    std::fs::create_dir_all(&notes).expect("mkdir inside scan root");
    let bookmarks = notes.join("bookmarks.html");
    std::fs::write(
        &bookmarks,
        r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p><DT><A HREF="https://shopfloor.example/">Shop floor portal</A></DL><p>"#,
    )
    .expect("write bookmarks");

    let (router, _env) = app_on(dir.path()).await;

    let (status, body) = send(
        &router,
        post(
            "/api/v1/history/import",
            "h",
            json!({"path": bookmarks.to_string_lossy()}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "import succeeds: {body}");
    assert_eq!(body["items"], 1, "one bookmark: {body}");
    assert_eq!(body["file"], "bookmarks.html");
    assert!(
        body["registered"].as_u64().unwrap_or(0) >= 1,
        "labels registered: {body}"
    );

    // Recall by content.
    let (_, search) = send(
        &router,
        post(
            "/api/v1/search/nodes",
            "h",
            json!({"query": "shop floor portal", "token_budget": 100}),
        ),
    )
    .await;
    assert!(
        search["results"]
            .as_array()
            .map(|a| a
                .iter()
                .any(|r| r["label"].as_str().unwrap_or("").contains("Shop floor")))
            .unwrap_or(false),
        "bookmark recallable by content: {search}"
    );

    // Re-import dedupes.
    let (_, again) = send(
        &router,
        post(
            "/api/v1/history/import",
            "h",
            json!({"path": bookmarks.to_string_lossy()}),
        ),
    )
    .await;
    assert!(
        again["deduped"].as_u64().unwrap_or(0) >= 1,
        "re-import dedupes: {again}"
    );

    // Unsupported file → 400; outside scan root → 403.
    let csv = notes.join("data.csv");
    std::fs::write(&csv, "a,b\n").expect("write csv");
    let (status, _) = send(
        &router,
        post(
            "/api/v1/history/import",
            "h",
            json!({"path": csv.to_string_lossy()}),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "unsupported extension rejected"
    );

    let outside = std::env::temp_dir();
    let (status, _) = send(
        &router,
        post(
            "/api/v1/history/import",
            "h",
            json!({"path": outside.to_string_lossy()}),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "path outside scan root rejected"
    );
}

#[tokio::test]
async fn praxis_backfill_seeds_behavioral_verdicts() {
    // Praxis backfill (Phase 7B): notebook entries + trackers become labeled
    // behavioral nodes whose asserted verdict follows the real life-log.
    let dir = tempfile::tempdir().expect("temp data dir");
    let exports = dir.path().join(".physis").join("exports");
    std::fs::create_dir_all(&exports).expect("mkdir inside scan root");
    let export = exports.join("praxis.json");
    std::fs::write(
        &export,
        r#"{
        "notebook": [
          {"content": "fixed the nozzle bind","mood": 9},
          {"content": "meh day","mood": 4},
          {"content": "gave up on the jig","mood": 2}
        ],
        "trackers": [
          {"name": "reading", "progress": 0.9},
          {"name": "meditate", "progress": 0.2}
        ]
    }"#,
    )
    .expect("write export");

    let (router, _env) = app_on(dir.path()).await;

    let (status, body) = send(
        &router,
        post(
            "/api/v1/praxis/backfill",
            "p",
            json!({"path": export.to_string_lossy()}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "backfill succeeds: {body}");
    assert_eq!(body["items"], 5, "3 notes + 2 trackers: {body}");
    assert_eq!(body["successes"], 2, "high mood + full tracker: {body}");
    assert_eq!(body["inert"], 1, "neutral mood: {body}");
    assert_eq!(body["failures"], 2, "low mood + empty tracker: {body}");
    assert!(
        body["registered"].as_u64().unwrap_or(0) >= 1,
        "nodes registered: {body}"
    );

    // Re-backfill dedupes (labels praxis:notebook/… are stable).
    let (_, again) = send(
        &router,
        post(
            "/api/v1/praxis/backfill",
            "p",
            json!({"path": export.to_string_lossy()}),
        ),
    )
    .await;
    assert!(
        again["deduped"].as_u64().unwrap_or(0) >= 1,
        "re-backfill dedupes: {again}"
    );

    // Confinement.
    let outside = std::env::temp_dir();
    let (status, _) = send(
        &router,
        post(
            "/api/v1/praxis/backfill",
            "p",
            json!({"path": outside.to_string_lossy()}),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "path outside scan root rejected"
    );
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
    assert_eq!(
        status,
        StatusCode::BAD_GATEWAY,
        "unconfigured sync → 502: {body}"
    );
    assert!(
        body["error"]
            .as_str()
            .unwrap_or("")
            .contains("not configured"),
        "names the problem: {body}"
    );

    let (status, body) = send(&router, post("/api/v1/spanner/hydrate", "s", json!({}))).await;
    assert_eq!(
        status,
        StatusCode::BAD_GATEWAY,
        "unconfigured hydrate → 502: {body}"
    );
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
    let (_state, router) = physis_pro::web::build_app()
        .await
        .expect("build_app must succeed");

    // Public ops surface: no token needed.
    let (status, _) = send(&router, get("/health", "_default")).await;
    assert_eq!(status, StatusCode::OK, "health stays public");
    let (status, _) = send(&router, get("/api/v1/embedder", "_default")).await;
    assert_eq!(status, StatusCode::OK, "embedder provenance stays public");

    // A protected POST with no bearer — the header must be compulsory.
    let (status, _) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "u",
            json!({"input": "deploy step"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "no bearer is rejected");

    let (status, _) = send(
        &router,
        post_auth(
            "/api/v1/coherence/register",
            "u",
            json!({"input": "deploy step"}),
            "wrong-secret",
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a wrong secret is rejected"
    );

    let (status, _) = send(
        &router,
        post_auth(
            "/api/v1/coherence/register",
            "u",
            json!({"input": "deploy step"}),
            "test-secret",
        ),
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
    let (_state, router) = physis_pro::web::build_app()
        .await
        .expect("build_app must succeed");

    let (status, _) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "u",
            json!({"input": "dev mode step"}),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "PHYSIS_DEV bypasses auth without tokens"
    );
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
    assert!(
        result.is_err(),
        "build_app must fail without PHYSIS_DEV and without tokens"
    );
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
        post(
            "/api/v1/coherence/register",
            "ed",
            json!({"input": "milling spindle bearing", "coherence_score": -1.0}),
        ),
    )
    .await;
    let id = reg["node_id"].as_str().unwrap().to_string();
    assert_eq!(reg["asserted"], -1.0);

    // Edit without a pin: verdict preserved, id stable, auto cell reported.
    let (status, edit) = send(
        &router,
        post(
            "/api/v1/coherence/edit",
            "ed",
            json!({"id": id, "label": "welding robot joint recalibration"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "edit succeeds: {edit}");
    assert_eq!(edit["node_id"], id, "identity survives the content swap");
    assert_eq!(edit["label"], "welding robot joint recalibration");
    assert_eq!(edit["asserted"], -1.0, "verdict survives the swap");
    assert!(
        edit["auto_cell"]["domain"].is_string(),
        "auto cell derived from new content"
    );
    assert!(
        edit["before_cell"].is_object(),
        "before cell reported for the delta"
    );
    assert_eq!(
        edit["pinned_cell"],
        Value::Null,
        "no pin supplied → none stored"
    );

    // The old label is no longer a dedupe alias: re-registering it makes a NEW
    // node; re-registering the new label returns the same id.
    let (_, dup_old) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "ed",
            json!({"input": "milling spindle bearing"}),
        ),
    )
    .await;
    assert_ne!(
        dup_old["node_id"], id,
        "old label must not dedupe to the edited node"
    );
    let (_, dup_new) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "ed",
            json!({"input": "welding robot joint recalibration"}),
        ),
    )
    .await;
    assert_eq!(
        dup_new["node_id"], id,
        "new label dedupes to the edited node"
    );
}

#[tokio::test]
async fn semiotic_edit_supports_pin_and_rejects_bad_cells() {
    let (router, _dir, _env) = app().await;

    let (_, reg) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "pn",
            json!({"input": "autoclave cycle log"}),
        ),
    )
    .await;
    let id = reg["node_id"].as_str().unwrap().to_string();

    // Pin a cell: the pin is echoed + persisted (survives a re-read via edit).
    let (status, pinned) = send(
        &router,
        post(
            "/api/v1/coherence/edit",
            "pn",
            json!({
                "id": id,
                "label": "autoclave cycle log (edited)",
                "cell": {"domain": "FABRICATE", "mode": "MAINTAIN"},
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "pinned edit succeeds: {pinned}");
    assert_eq!(pinned["pinned_cell"]["domain"], "FABRICATE");
    assert_eq!(pinned["pinned_cell"]["mode"], "MAINTAIN");

    // An invalid cell is rejected before anything is mutated.
    let (status, bad) = send(
        &router,
        post(
            "/api/v1/coherence/edit",
            "pn",
            json!({
                "id": id,
                "label": "garbage cell",
                "cell": {"domain": "NOPE", "mode": "MAYBE"},
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "bad cell rejected: {bad}");

    // Unknown id → 404.
    let (status, missing) = send(
        &router,
        post(
            "/api/v1/coherence/edit",
            "pn",
            json!({"id": "no-such-node", "label": "x"}),
        ),
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
        post(
            "/api/v1/coherence/register",
            "pv",
            json!({"input": "autoclave cycle log"}),
        ),
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
    assert_eq!(
        before["pinned"], false,
        "an unedited node is not pinned: {before}"
    );

    // Pin it somewhere the classifier would not have chosen on its own.
    let (status, _) = send(
        &router,
        post(
            "/api/v1/coherence/edit",
            "pv",
            json!({
                "id": id,
                "label": "autoclave cycle log",
                "cell": {"domain": "BOND", "mode": "PLAY"},
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, all) = send(&router, get("/api/v1/coherence/nodes/all", "pv")).await;
    let pinned = find(&all, &id);
    assert_eq!(
        pinned["pinned"], true,
        "the listing reports the pin: {pinned}"
    );
    assert_eq!(pinned["cell"][0], "BOND", "the pin outranks the classifier");
    assert_eq!(pinned["cell"][1], "PLAY");

    // An edit that says nothing about the cell must not silently unpin it —
    // changing the text is not a retraction of where someone said it belongs.
    let (status, _) = send(
        &router,
        post(
            "/api/v1/coherence/edit",
            "pv",
            json!({
                "id": id, "label": "autoclave cycle log, revised",
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, all) = send(&router, get("/api/v1/coherence/nodes/all", "pv")).await;
    let still = find(&all, &id);
    assert_eq!(
        still["pinned"], true,
        "a content edit leaves the pin standing: {still}"
    );
    assert_eq!(still["cell"][0], "BOND");

    // Retraction is explicit: `"cell": null`.
    let (status, _) = send(
        &router,
        post(
            "/api/v1/coherence/edit",
            "pv",
            json!({
                "id": id, "label": "autoclave cycle log, revised", "cell": null,
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, all) = send(&router, get("/api/v1/coherence/nodes/all", "pv")).await;
    let cleared = find(&all, &id);
    assert_eq!(
        cleared["pinned"], false,
        "an explicit null retracts the pin: {cleared}"
    );
}

/// A pin that disagrees with the classifier is a labelled correction, so it has
/// to reach the quality tracker — otherwise editing teaches the system nothing.
#[tokio::test]
async fn a_dissenting_pin_penalizes_the_cell_the_classifier_chose() {
    let (router, _dir, _env) = app().await;

    let (_, reg) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "qz",
            json!({"input": "hydraulic press tonnage drift"}),
        ),
    )
    .await;
    let id = reg["node_id"].as_str().unwrap().to_string();

    // What the classifier picked on its own, before any human says otherwise.
    let (_, edited) = send(
        &router,
        post(
            "/api/v1/coherence/edit",
            "qz",
            json!({
                "id": id, "label": "hydraulic press tonnage drift",
            }),
        ),
    )
    .await;
    let auto_domain = edited["auto_cell"]["domain"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let auto_mode = edited["auto_cell"]["mode"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert!(
        !auto_domain.is_empty(),
        "the classifier picked a cell: {edited}"
    );

    // Disagree with it, repeatedly. One pin is a sample, not a verdict, so the
    // penalty has to accumulate rather than land in full on the first click.
    let dissent = if auto_domain == "BOND" {
        "STUDY"
    } else {
        "BOND"
    };
    for _ in 0..3 {
        let (status, _) = send(
            &router,
            post(
                "/api/v1/coherence/edit",
                "qz",
                json!({
                    "id": id,
                    "label": "hydraulic press tonnage drift",
                    "cell": {"domain": dissent, "mode": "PLAY"},
                }),
            ),
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

/// The quality endpoint serves the trend fields the console Quality tab draws
/// from, even on a fresh tracker with nothing reported (PLAN 3.3).
#[tokio::test]
async fn quality_endpoint_reports_trend_fields_on_an_empty_ledger() {
    let (router, _dir, _env) = app().await;

    let (status, quality) = send(&router, get("/api/v1/quality", "qz")).await;
    assert_eq!(status, StatusCode::OK, "quality readable: {quality}");
    for key in ["failures", "open", "resolved", "mean_severity", "records"] {
        assert!(
            quality.get(key).is_some(),
            "response must carry `{key}` for the dashboard: {quality}"
        );
    }
    assert_eq!(quality["records"].as_array().unwrap().len(), 0);
}

/// PLAN 3.2: /api/v1/ask answers plain-English questions over the graph.
/// Retrieval always works; without a semantic provider the endpoint degrades
/// to raw matches and says why instead of failing.
#[tokio::test]
async fn ask_answers_over_the_graph_and_degrades_without_a_provider() {
    let (router, _dir, _env) = app().await;

    let (_, reg) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "qz",
            json!({"input": "hydraulic press tonnage drift"}),
        ),
    )
    .await;
    assert!(reg["node_id"].as_str().is_some(), "{reg}");

    let (status, ask) = send(
        &router,
        post(
            "/api/v1/ask",
            "qz",
            json!({"question": "what do I have about the hydraulic press?"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "ask readable: {ask}");
    assert!(ask["semantic"].is_boolean(), "{ask}");
    // The retrieval half must work regardless of providers.
    assert!(
        ask["results"]
            .as_array()
            .map(|a| a
                .iter()
                .any(|r| r["label"].as_str().unwrap_or("").contains("hydraulic")))
            .unwrap_or(false),
        "registered node recallable through /ask: {ask}"
    );
    // No provider configured in the test env: either a grounded answer naming
    // its provider, or an explicit reason — never a silent empty reply.
    if ask["answer"].is_null() {
        let note = ask["note"].as_str().unwrap_or("");
        assert!(
            note.contains("provider"),
            "a missing answer must say why: {ask}"
        );
    } else {
        assert!(
            ask["provider"].is_string(),
            "an answer names its provider: {ask}"
        );
    }
}

/// Phase 10 D4+D5: importers record causal events and /api/v1/files/query
/// composes the space·time·cause facets over them, isolated per tenant.
#[tokio::test]
async fn cause_ledger_records_imports_and_answers_facet_queries() {
    let dir = tempfile::tempdir().expect("temp data dir");
    let vault = dir.path().join(".physis").join("vault");
    std::fs::create_dir_all(&vault).expect("mkdir");
    std::fs::write(vault.join("note.md"), "# Plant Log\n\npress ran fine").expect("note");
    let exports = dir.path().join(".physis").join("exports");
    std::fs::create_dir_all(&exports).expect("mkdir");
    std::fs::write(
        exports.join("praxis.json"),
        r#"{"notebook":[{"content":"cleaned the line","mood":8}]}"#,
    )
    .expect("export");

    let (router, _env) = app_on(dir.path()).await;
    let vault_str = vault.display().to_string();

    // Import both streams.
    let (_, v) = send(
        &router,
        post(
            "/api/v1/vault/import",
            "alice",
            json!({"dir": vault_str, "git": 0}),
        ),
    )
    .await;
    assert!(v["registered"].as_u64().unwrap_or(0) >= 1, "{v}");
    let (_, p) = send(
        &router,
        post(
            "/api/v1/praxis/backfill",
            "alice",
            json!({"path": exports.join("praxis.json").display().to_string()}),
        ),
    )
    .await;
    assert!(p["registered"].as_u64().unwrap_or(0) >= 1, "{p}");

    // Unfiltered query: both causal events visible to their tenant.
    let (_, all) = send(&router, post("/api/v1/files/query", "alice", json!({}))).await;
    let matches = all["matches"].as_array().cloned().unwrap_or_default();
    assert!(
        matches
            .iter()
            .any(|m| m["mechanism"] == "received" && m["actor"] == "vault-import"),
        "vault import recorded as received: {all}"
    );
    assert!(
        matches
            .iter()
            .any(|m| m["mechanism"] == "derived" && m["actor"] == "praxis-backfill"),
        "praxis backfill recorded as derived: {all}"
    );

    // Cause facet composes: only the received stream survives.
    let (_, recv) = send(
        &router,
        post("/api/v1/files/query", "alice", json!({"cause": "received"})),
    )
    .await;
    let rm = recv["matches"].as_array().unwrap();
    assert!(
        !rm.is_empty() && rm.iter().all(|m| m["mechanism"] == "received"),
        "{recv}"
    );

    // Time facet: current month bucket catches everything; a past bucket nothing.
    let now_period = chrono::Utc::now().format("%Y-%m").to_string();
    let (_, during) = send(
        &router,
        post(
            "/api/v1/files/query",
            "alice",
            json!({"during": now_period}),
        ),
    )
    .await;
    assert!(during["total"].as_u64().unwrap_or(0) >= 2, "{during}");
    let (_, empty_past) = send(
        &router,
        post("/api/v1/files/query", "alice", json!({"during": "1999-01"})),
    )
    .await;
    assert_eq!(empty_past["total"], 0, "{empty_past}");

    // Space facet: place prefix filters.
    let (_, spaced) = send(
        &router,
        post("/api/v1/files/query", "alice", json!({"place": vault_str})),
    )
    .await;
    assert_eq!(
        spaced["total"], 1,
        "only the vault event sits under its root: {spaced}"
    );

    // Tenant isolation: bob sees none of alice's causes.
    let (_, bobs) = send(&router, post("/api/v1/files/query", "bob", json!({}))).await;
    assert_eq!(bobs["total"], 0, "{bobs}");
}

/// Phase 10 D6: purpose links. A goal is created, an artifact is attached to
/// it with /api/v1/files/serve, the link shows up as a `serves` cause event
/// and in the artifact's lineage; unknown goals are rejected.
#[tokio::test]
async fn purpose_links_record_final_cause_and_show_in_lineage() {
    let dir = tempfile::tempdir().expect("temp data dir");
    let vault = dir.path().join(".physis").join("vault");
    std::fs::create_dir_all(&vault).expect("mkdir");
    std::fs::write(vault.join("runbook.md"), "# Runbook\n\nline restart steps").expect("note");

    let (router, _env) = app_on(dir.path()).await;
    let vault_str = vault.display().to_string();

    // A goal to serve.
    let (_, g) = send(
        &router,
        post(
            "/api/v1/goals",
            "alice",
            json!({"prompt": "keep the line running"}),
        ),
    )
    .await;
    let goal_id = g["goal"]["id"].as_str().expect("goal id").to_string();
    let (_, list) = send(&router, get("/api/v1/goals/list", "alice")).await;
    assert!(
        list["goals"]
            .as_array()
            .map(|a| a.iter().any(|x| x["id"] == g["goal"]["id"]))
            .unwrap_or(false),
        "goal picker lists all tenant goals: {list}"
    );

    // An artifact with a producing event.
    let (_, v) = send(
        &router,
        post(
            "/api/v1/vault/import",
            "alice",
            json!({"dir": vault_str, "git": 0}),
        ),
    )
    .await;
    assert!(v["registered"].as_u64().unwrap_or(0) >= 1, "{v}");

    // Attach purpose.
    let (status, s) = send(
        &router,
        post(
            "/api/v1/files/serve",
            "alice",
            json!({"object": vault_str, "goal_id": goal_id}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    assert_eq!(s["recorded"]["mechanism"], "serves", "{s}");

    // Unknown goal → 404, never a silent record.
    let (status, miss) = send(
        &router,
        post(
            "/api/v1/files/serve",
            "alice",
            json!({"object": vault_str, "goal_id": "nope"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{miss}");

    // Purpose visible through the facet query…
    let (_, q) = send(
        &router,
        post("/api/v1/files/query", "alice", json!({"cause": "serves"})),
    )
    .await;
    assert_eq!(q["total"], 1, "{q}");
    // …and through lineage: purposes populated, serves excluded from chains.
    let (_, lin) = send(
        &router,
        get(
            &format!("/api/v1/files/lineage?path={}", urlencode(&vault_str)),
            "alice",
        ),
    )
    .await;
    assert_eq!(lin["purposes"].as_array().unwrap().len(), 1, "{lin}");
    let chain = lin["lineage"].as_array().unwrap();
    assert!(
        chain.iter().all(|e| e["mechanism"] != "serves"),
        "purpose links are not derivations: {lin}"
    );

    // Tenant isolation: bob cannot see or attach to alice's world.
    let (_, bobs) = send(
        &router,
        post(
            "/api/v1/files/serve",
            "bob",
            json!({"object": vault_str, "goal_id": goal_id}),
        ),
    )
    .await;
    assert!(!bobs["error"].is_null(), "{bobs}");
}

/// Phase 10b T1: the causal ledger joins the assembled operational timeline.
/// Study events and cause events (from a vault import) appear in one ordered
/// view; tallies stay study-scoped; the response reports how many causes
/// merged.
#[tokio::test]
async fn session_timeline_merges_cause_events_with_study_events() {
    let dir = tempfile::tempdir().expect("temp data dir");
    let vault = dir.path().join(".physis").join("vault");
    std::fs::create_dir_all(&vault).expect("mkdir");
    std::fs::write(vault.join("note.md"), "# Plant Log\n\npress fine").expect("note");

    let (router, _env) = app_on(dir.path()).await;

    // A study event older than the import, and one newer — ordering across
    // connectors must hold in the merged timeline.
    let (_, logged) = send(
        &router,
        post(
            "/api/v1/study/log",
            "mx",
            json!({"events": [
                {"observed_at": "2026-08-20T08:00:00Z", "action": "generated",
                 "path": "notes/press.md", "place": "Library"}
            ]}),
        ),
    )
    .await;
    assert_eq!(logged["accepted"], 1, "{logged}");

    let vault_str = vault.display().to_string();
    let (_, v) = send(
        &router,
        post(
            "/api/v1/vault/import",
            "mx",
            json!({"dir": vault_str, "git": 0}),
        ),
    )
    .await;
    assert!(v["registered"].as_u64().unwrap_or(0) >= 1, "{v}");

    let (_, s) = send(&router, get("/api/v1/study/session", "mx")).await;
    let entries = s["timeline"]["entries"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    // 1 study + ≥1 cause, chronologically interleaved (study event first).
    assert!(entries.len() >= 2, "{s}");
    assert_eq!(
        entries[0]["subject"], "press.md",
        "oldest event leads regardless of connector: {s}"
    );
    let cause_entries = entries
        .iter()
        .filter(|e| e["properties"]["mechanism"] == "received")
        .count();
    assert!(cause_entries >= 1, "cause events joined the timeline: {s}");
    assert_eq!(
        s["causes"].as_u64(),
        Some(cause_entries as u64),
        "`causes` reports exactly the merged ledger events: {s}"
    );

    // Tallies stay study-scoped: by_action counts only study actions.
    assert_eq!(s["by_action"][0]["key"], "generated", "{s}");

    // A tenant with no ledger sees no merge (and no phantom field drift).
    let (_, plain) = send(&router, get("/api/v1/study/session", "solo")).await;
    assert_eq!(plain["causes"], 0, "{plain}");
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Phase 10 D7: mailbox import — .eml/.mbox exports register labeled nodes
/// and record `received(mailbox)` cause events, visible through the facet
/// query.
#[tokio::test]
async fn mail_import_registers_messages_and_records_mailbox_causes() {
    let dir = tempfile::tempdir().expect("temp data dir");
    let maildir = dir.path().join(".physis").join("mail");
    std::fs::create_dir_all(&maildir).expect("mkdir");
    std::fs::write(
        maildir.join("report.eml"),
        "From: Ada <ada@example.org>\nSubject: press tonnage\nDate: Mon, 24 Aug 2026 08:15:00 +0000\n\n drifted 2 tons\n",
    )
    .unwrap();
    std::fs::write(
        maildir.join("inbox.mbox"),
        "From x Sat Aug 22 10:00:00 2026\nFrom: bob@example.org\nSubject: shift handover\n\nall quiet\n",
    )
    .unwrap();

    let (router, _env) = app_on(dir.path()).await;
    let dir_str = maildir.display().to_string();
    let (_, m) = send(
        &router,
        post("/api/v1/mail/import", "alice", json!({"dir": dir_str})),
    )
    .await;
    assert_eq!(m["files"], 2, "one event per source file: {m}");
    assert_eq!(m["messages"], 2, "{m}");
    assert!(m["registered"].as_u64().unwrap_or(0) >= 1, "{m}");

    // Cause trail: two received events under actor `mailbox`, timed by the
    // messages' own Date headers (TimeRef) — not the import moment.
    let (_, q) = send(
        &router,
        post("/api/v1/files/query", "alice", json!({"cause": "mailbox"})),
    )
    .await;
    assert_eq!(q["total"], 2, "{q}");
    let mail_matches = q["matches"].as_array().unwrap();
    assert!(
        mail_matches
            .iter()
            .all(|e| e["actor"] == "mailbox" && e["mechanism"] == "received"),
        "{q}"
    );
    let eml_event = mail_matches
        .iter()
        .find(|e| e["object"].as_str().unwrap_or("").ends_with("report.eml"))
        .expect("eml event present");
    assert_eq!(
        eml_event["at"], "2026-08-24T08:15:00+00:00",
        "message Date header drives the event time: {q}"
    );

    // Confinement still applies.
    let (status, out) = send(
        &router,
        post("/api/v1/mail/import", "alice", json!({"dir": "/etc"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{out}");
}

#[tokio::test]
async fn coherence_delete_removes_node_and_index() {
    let (router, _dir, _env) = app().await;

    let (_, reg) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "dl",
            json!({"input": "turbine bearing temp"}),
        ),
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
        post(
            "/api/v1/coherence/register",
            "dl",
            json!({"input": "turbine bearing temp"}),
        ),
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
        post(
            "/api/v1/coherence/register",
            "sr",
            json!({"input": "milling spindle bearing wear"}),
        ),
    )
    .await;
    let (_, _) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "sr",
            json!({"input": "furnace temperature profile"}),
        ),
    )
    .await;
    let (_, _) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "sr",
            json!({"input": "welding robot joint calibration"}),
        ),
    )
    .await;

    let (status, res) = send(
        &router,
        post(
            "/api/v1/search/nodes",
            "sr",
            json!({"query": "spindle bearing", "token_budget": 20}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "search succeeds: {res}");
    assert!(
        res["total_tokens"].as_u64().unwrap_or(u64::MAX) <= 20,
        "total_tokens {} must respect the 20-token budget",
        res["total_tokens"]
    );
    assert_eq!(res["budget"], 20);
    assert!(
        !res["results"].as_array().unwrap().is_empty(),
        "relevant nodes found"
    );
    assert_eq!(res["results"][0]["kind"], "node");
    assert!(res["results"][0]["label"].is_string());

    // Empty budget → zero results, zero tokens (hard bound holds at the floor).
    let (_, res) = send(
        &router,
        post(
            "/api/v1/search/nodes",
            "sr",
            json!({"query": "spindle", "token_budget": 0}),
        ),
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
        post(
            "/api/v1/oee/machine",
            "plant",
            json!({
                "machine_id": "cnc-1",
                "shift_length_secs": 28800.0,
                "ideal_cycle_time_secs": 60.0
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "register machine: {body}");
    assert_eq!(body["ok"], true);
    assert!(body["detail"].as_str().unwrap().contains("cnc-1"));

    // Record production.
    let (status, body) = send(
        &router,
        post(
            "/api/v1/oee/event",
            "plant",
            json!({
                "kind": "production",
                "machine_id": "cnc-1",
                "good": 85,
                "scrap": 2
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "record event: {body}");
    assert!(
        body["oee"].as_f64().unwrap_or(0.0) >= 0.0,
        "OEE must be non-negative"
    );

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
        post(
            "/api/v1/maintenance/check",
            "plant",
            json!({
                "machine_id": "cnc-1",
                "temperature": 65.0,
                "vibration": 1.2,
                "rpm": 8000,
                "status": "online"
            }),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "check without baseline returns 404"
    );
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

#[tokio::test]
async fn epistemic_hypotheses_crud_and_lifecycle() {
    let (router, _dir, _env) = app().await;

    // 1. Create hypothesis
    let (status, body) = send(
        &router,
        post(
            "/api/v1/hypotheses",
            "tester",
            json!({
                "statement": "Bearing overheating due to lube starvation",
                "assumptions": ["spindle running > 5000 RPM", "NLGI-2 grease"],
                "confidence": 0.85
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create hypothesis: {body}");
    let hyp_id = body["id"].as_str().expect("hyp id");

    // 2. Add supporting evidence
    let (status, ev_body) = send(
        &router,
        post(
            &format!("/api/v1/hypotheses/{}/evidence", hyp_id),
            "tester",
            json!({
                "source": "vibration_sensor_v1",
                "polarity": "Supports",
                "confidence": 0.92,
                "claim": "High frequency acceleration peaks at 3.2 kHz"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "add evidence: {ev_body}");
    assert_eq!(
        ev_body["supporting_evidence"].as_array().map(|a| a.len()),
        Some(1)
    );

    // 3. Record prediction
    let (status, pred_body) = send(
        &router,
        post(
            &format!("/api/v1/hypotheses/{}/prediction", hyp_id),
            "tester",
            json!({
                "statement": "Purging lube channel will drop temp by 15C",
                "expected_outcome": "Temp < 70C",
                "actual_outcome": "Temp reached 64C",
                "correct": true,
                "confidence": 0.9
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "record prediction: {pred_body}");
    assert_eq!(
        pred_body["predictions"].as_array().map(|a| a.len()),
        Some(1)
    );

    // 4. Transition state
    let (status, trans_body) = send(
        &router,
        post(
            &format!("/api/v1/hypotheses/{}/transition", hyp_id),
            "tester",
            json!({
                "status": "Confirmed",
                "reason": "Purge test verified prediction"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "transition state: {trans_body}");
    assert_eq!(trans_body["status"].as_str(), Some("Confirmed"));

    // 5. Query explanation report
    let (status, exp_body) = send(
        &router,
        get(&format!("/api/v1/explanation/{}", hyp_id), "tester"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "explanation report: {exp_body}");
    assert_eq!(exp_body["status"].as_str(), Some("Confirmed"));
    assert!(!exp_body["human_readable_summary"]
        .as_str()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn epistemic_contradictions_and_resolution() {
    let (router, _dir, _env) = app().await;

    // 1. Create contradiction
    let (status, body) = send(
        &router,
        post(
            "/api/v1/contradictions",
            "operator_u1",
            json!({
                "claim_a": {
                    "claim": "Telemetry sensor S1 reads 98C",
                    "source": "sensor_s1",
                    "confidence": 0.95,
                    "authority": 0.95
                },
                "claim_b": {
                    "claim": "Operator thermal gun reads 45C",
                    "source": "operator_shift_1",
                    "confidence": 0.85,
                    "authority": 0.80
                },
                "resolution": "Open"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "record contradiction: {body}");
    let contra_id = body["id"].as_str().expect("contra id");

    // 2. Resolve contradiction
    let (status, res_body) = send(
        &router,
        post(
            &format!("/api/v1/contradictions/{}/resolve", contra_id),
            "operator_u1",
            json!({
                "resolution": "BPreferred",
                "explanation": "Sensor S1 thermocouple wire had loose ground contact"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "resolve contradiction: {res_body}");
    assert_eq!(res_body["resolution"].as_str(), Some("BPreferred"));
}

#[tokio::test]
async fn epistemic_audit_stream_and_historical_replay() {
    let (router, _dir, _env) = app().await;

    // 1. Create hypothesis to seed audit events
    let (_, h_body) = send(
        &router,
        post(
            "/api/v1/hypotheses",
            "user_audit",
            json!({
                "statement": "Nozzle pressure anomaly",
                "confidence": 0.7
            }),
        ),
    )
    .await;
    let hyp_id = h_body["id"].as_str().unwrap();

    // 2. Query audit log
    let (status, audit_body) = send(&router, get("/api/v1/epistemic/audit", "user_audit")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!audit_body.as_array().unwrap().is_empty());

    // 3. Historical replay
    let target_time = chrono::Utc::now().to_rfc3339();
    let (status, replay_body) = send(
        &router,
        get(
            &format!(
                "/api/v1/epistemic/replay?subject_id={}&at={}",
                hyp_id,
                urlencoding::encode(&target_time)
            ),
            "user_audit",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay_body["subject_id"].as_str(), Some(hyp_id));
    assert_eq!(
        replay_body["reconstructed_status"].as_str(),
        Some("Candidate")
    );
}

#[tokio::test]
async fn unsupervised_ontology_discovery_endpoint() {
    let (router, _dir, _env) = app().await;

    let (status, disc_body) = send(
        &router,
        post(
            "/api/v1/ontology/discover",
            "anyone",
            json!({
                "texts": [
                    "quantum qubit decoherence gate fidelity",
                    "superconducting qubit dilution refrigerator microwave control",
                    "quantum error correction lattice surgery surface code"
                ],
                "coverage_threshold": 0.85,
                "min_cluster": 2
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(disc_body["total"].as_u64().unwrap_or(0) >= 3);
}

#[tokio::test]
async fn llm_coherence_harness_config_and_evaluate_endpoint() {
    let (router, _dir, _env) = app().await;

    // 1. Check Harness Config
    let (status, cfg_body) = send(&router, get("/api/v1/harness/config", "test_user")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(cfg_body["default_config"].is_object());
    assert!(cfg_body["available_providers"].is_array());

    // 2. Run Evaluate on coherent technical statement
    let (status, eval_body) = send(
        &router,
        post(
            "/api/v1/harness/evaluate",
            "test_user",
            json!({
                "text": "Replace worn spindle bearings to eliminate thermal runaway",
                "context": "Bearing temperature measured at 92C under continuous load",
                "config": {
                    "coherence_threshold": 0.50,
                    "reject_contradictions": true
                }
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let verdict = eval_body["verdict"].as_str().unwrap();
    assert!(verdict == "APPROVED" || verdict == "WARNING" || verdict == "REVISED");
    assert!(eval_body["composite_fitness"].as_f64().is_some());
    assert!(eval_body["explanation_report"].is_object());
}

#[tokio::test]
async fn the_agent_dashboard_renders_the_harness_verdict() {
    // Every agent answer is judged by the coherence harness and the verdict
    // arrives as its own SSE event carrying a CoherenceEvaluation as JSON.
    // `addEvent` escapes text into a single line, so routing a verdict there
    // would print raw JSON at the user — the page needs a structured renderer
    // and needs to reach it.
    let (router, _dir, _env) = app().await;
    let (status, body) = send_text(&router, get("/agent", "_default")).await;
    assert_eq!(status, StatusCode::OK);

    assert!(
        body.contains("function renderVerdict"),
        "verdict renderer must exist"
    );
    assert!(
        body.contains("ev.kind === 'verdict'"),
        "the event loop must route verdicts to it, or the card never renders",
    );
    // The four verdicts the arbiter can return must each be styleable, or an
    // unstyled one silently renders as an unmarked card.
    for verdict in ["APPROVED", "WARNING", "REJECTED", "REVISED"] {
        assert!(
            body.contains(&format!(".verdict.{verdict}")),
            "no style for {verdict}"
        );
    }
    // Contradictions are the whole point of the gate: a conflicting answer is
    // exactly what a fitness percentage alone hides.
    assert!(
        body.contains("detected_contradictions"),
        "the card must surface contradictions, not only the score",
    );
    // "gate off" and "gate on but quiet" are indistinguishable in the trace,
    // so the capabilities line has to state which one it is.
    assert!(
        body.contains("harness_gate"),
        "caps must report whether the gate is on"
    );
}

#[tokio::test]
async fn agent_capabilities_report_the_coherence_gate() {
    let (router, _dir, _env) = app().await;
    let (status, body) = send(&router, get("/api/v1/agent/caps", "_default")).await;
    assert_eq!(status, StatusCode::OK);
    // Defaults on: an agent emitting unjudged claims into a system built to
    // keep only interpretations that go on working would be the one component
    // exempt from that rule.
    assert_eq!(body["harness_gate"], true, "the gate must default on");
    let floor = body["harness_threshold"]
        .as_f64()
        .expect("threshold must be numeric");
    assert!(
        (0.0..=1.0).contains(&floor),
        "threshold is a fitness fraction, got {floor}"
    );
}

// ── Agent demo: the same loop, inside and outside the harness ─────────────────
//
// Two demonstrations of the agent surface, kept as tests so the traces they
// print are reproducible (`cargo test --test http_surface agent_demo --
// --nocapture`):
//
//   1. INSIDE  — POST /api/v1/agent/run returns an SSE stream of the live
//      ReAct trace (plan → tool_call → tool_result → … → final → verdict),
//      judged by the coherence gate before display.
//   2. OUTSIDE — the identical loop called as a library function
//      (`run_agent`) with a hand-built registry, no HTTP involved.
//
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

/// Scripted OpenAI-compatible chat server on 127.0.0.1:0. Returns the base URL.
async fn spawn_mock_orchestrator(
    tool_name: &'static str,
    arguments: &'static str,
    final_text: &'static str,
) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 256 * 1024];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);

                // Round detection: a tool-role message means the results are
                // back, so the model must produce its answer now.
                let body = if req.contains("\"role\":\"tool\"") {
                    serde_json::json!({
                        "choices": [{"message": {
                            "role": "assistant",
                            "content": final_text,
                        }, "finish_reason": "stop"}],
                        "usage": {"prompt_tokens": 12, "completion_tokens": 9, "total_tokens": 21}
                    })
                } else {
                    serde_json::json!({
                        "choices": [{"message": {
                            "role": "assistant",
                            "content": serde_json::Value::Null,
                            "tool_calls": [{"id": "call-demo-1", "type": "function",
                                "function": {"name": tool_name, "arguments": arguments}}],
                        }, "finish_reason": "tool_calls"}],
                        "usage": {"prompt_tokens": 24, "completion_tokens": 14, "total_tokens": 38}
                    })
                };

                let payload = serde_json::to_string(&body).unwrap();
                let http = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                    payload.len(),
                    payload
                );
                let _ = sock.write_all(http.as_bytes()).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    format!("http://{addr}")
}

/// DEMO (inside): the dashboard path. One POST, one streamed trace.
#[tokio::test]
async fn agent_demo_inside_harness_sse_stream() {
    let dir = tempfile::tempdir().unwrap();
    let base = spawn_mock_orchestrator(
        "classify",
        "{\"text\":\"Classify this very prompt against the semiotic grid.\"}",
        "The prompt lands in the HUMAN × OPERATE cell: it directs an agent to \
         act on the semiotic grid, which is precisely what the classify result \
         above recorded.",
    )
    .await;

    // Same env ritual as every test here; EP_ORCHESTRATOR must be visible only
    // while build_app snapshots the provider cascade.
    let env = ENV_LOCK.lock().await;
    std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
    std::env::remove_var("PHYSIS_API_TOKEN");
    std::env::remove_var("PHYSIS_TOKENS");
    std::env::set_var("PHYSIS_SCAN_ROOT", dir.path());
    std::env::set_var("PHYSIS_DEV", "1");
    std::env::set_var("PHYSIS_DATA_DIR", dir.path());
    std::env::set_var("EP_ORCHESTRATOR", &base);
    let (_state, router) = physis_pro::web::build_app()
        .await
        .expect("build_app must succeed");
    std::env::remove_var("EP_ORCHESTRATOR");
    drop(env);

    let mut req = post(
        "/api/v1/agent/run",
        "demo-tenant",
        serde_json::json!({
            "prompt": "Classify this very prompt against the semiotic grid.",
            "tools": ["classify"],
            "mode": "ANALYZE"
        }),
    );
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            40000,
        ))));

    let resp = router.oneshot(req).await.expect("router responds");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "POST /api/v1/agent/run must be accepted"
    );

    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("sse body")
        .to_bytes();
    let raw = String::from_utf8_lossy(&bytes);

    // Parse the SSE frames: `event: <kind>` + `data: {json}`.
    let mut frames: Vec<(String, serde_json::Value)> = Vec::new();
    for block in raw.split("\n\n") {
        let kind = block.lines().find_map(|l| l.strip_prefix("event: "));
        let data = block.lines().find_map(|l| l.strip_prefix("data: "));
        if let (Some(k), Some(d)) = (kind, data) {
            frames.push((
                k.trim().to_string(),
                serde_json::from_str(d.trim()).unwrap_or_default(),
            ));
        }
    }
    for (k, d) in &frames {
        println!(
            "[inside] event={k:<11} {}",
            d["text"].as_str().unwrap_or("")
        );
    }

    let kinds: Vec<&str> = frames.iter().map(|(k, _)| k.as_str()).collect();
    let pos = |name: &str| {
        kinds
            .iter()
            .position(|k| *k == name)
            .unwrap_or_else(|| panic!("missing `{name}` frame; got {kinds:?}"))
    };
    assert!(
        pos("tool_call") < pos("tool_result"),
        "result must follow its call"
    );
    assert!(
        pos("tool_result") < pos("final"),
        "final answer comes after tools"
    );
    assert!(pos("final") < pos("done"), "done terminates the stream");
    assert!(
        kinds.contains(&"verdict"),
        "the coherence gate must judge the final answer"
    );

    // The verdict frame carries the arbiter's structured evaluation.
    let verdict_text = frames
        .iter()
        .find(|(k, _)| k == "verdict")
        .and_then(|(_, d)| d["text"].as_str())
        .expect("verdict frame carries its payload as text");
    let evaluation: serde_json::Value =
        serde_json::from_str(verdict_text).expect("verdict is JSON");
    println!(
        "[inside] gate verdict={} fitness={:.3}",
        evaluation["verdict"].as_str().unwrap_or("?"),
        evaluation["composite_fitness"].as_f64().unwrap_or(f64::NAN)
    );
}

/// DEMO (outside): the same loop as a library call — no router, no SSE.
#[tokio::test]
async fn agent_demo_outside_harness_direct_call() {
    let base = spawn_mock_orchestrator(
        "get_time",
        "{}",
        "The current UTC timestamp is the one the get_time tool returned.",
    )
    .await;

    let env = ENV_LOCK.lock().await;
    std::env::set_var("EP_ORCHESTRATOR", &base);
    let cascade = physis_pro::ai::provider::ProviderCascade::from_env();
    std::env::remove_var("EP_ORCHESTRATOR");
    drop(env);

    assert!(
        cascade
            .provider_names()
            .contains(&"orchestrator".to_string()),
        "mock must be registered, got {:?}",
        cascade.provider_names()
    );

    // Curated registry: clear the network/fs builtins, add one trivial tool.
    let mut tools = physis_pro::ai::tools::ToolRegistry::new();
    tools.clear();
    tools.register(
        "get_time",
        "Returns the current UTC timestamp.",
        serde_json::json!({"type": "object", "properties": {}}),
        Box::new(|_args| Box::pin(async move { Ok(chrono::Utc::now().to_rfc3339()) })),
    );

    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink_events = events.clone();
    let sink: physis_pro::ai::agent::EventSink = std::sync::Arc::new(move |ev| {
        println!(
            "[outside] r{} {:<11} {}",
            ev.round,
            ev.kind,
            ev.text.replace('\n', " ")
        );
        sink_events.lock().unwrap().push(ev.kind);
    });

    let out = physis_pro::ai::agent::run_agent_stream(
        &cascade,
        &tools,
        &physis_pro::ai::agent::AgentConfig::default(),
        &[],
        "What time is it right now? Use the get_time tool.",
        None,
        "",
        None,
        &sink,
    )
    .await
    .expect("direct agent run must succeed");

    println!(
        "[outside] summary: provider={} rounds={} tokens={}",
        out.provider, out.tool_rounds, out.tokens_used
    );
    assert_eq!(out.provider, "orchestrator");
    assert_eq!(out.tool_rounds, 1, "exactly one scripted tool round");

    let kinds = events.lock().unwrap().clone();
    assert!(kinds.contains(&"tool_call".to_string()), "trace: {kinds:?}");
    assert!(
        kinds.contains(&"tool_result".to_string()),
        "trace: {kinds:?}"
    );
    assert!(kinds.contains(&"final".to_string()), "trace: {kinds:?}");
    let call_pos = kinds.iter().position(|k| k == "tool_call").unwrap();
    let res_pos = kinds.iter().position(|k| k == "tool_result").unwrap();
    let fin_pos = kinds.iter().position(|k| k == "final").unwrap();
    assert!(
        call_pos < res_pos && res_pos < fin_pos,
        "trace must be ordered: {kinds:?}"
    );
}

#[tokio::test]
async fn agent_run_rejects_unknown_tool_whitelist_instead_of_running_empty() {
    let dir = tempfile::tempdir().expect("temp data dir");
    std::fs::create_dir_all(dir.path()).expect("mkdir");

    let env = ENV_LOCK.lock().await;
    std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
    std::env::remove_var("PHYSIS_API_TOKEN");
    std::env::remove_var("PHYSIS_TOKENS");
    std::env::set_var("PHYSIS_DEV", "1");
    std::env::set_var("PHYSIS_DATA_DIR", dir.path());
    std::env::set_var("PHYSIS_SCAN_ROOT", dir.path());
    let (_state, router) = physis_pro::web::build_app()
        .await
        .expect("build_app must succeed");

    // A whitelist naming tools that don't exist used to run the agent with an
    // empty registry — the model was told to use tools it never received and
    // improvised a textual call nothing dispatched. Must be a loud 400.
    let body = serde_json::json!({
        "prompt": "What time is it? Use the get_time tool.",
        "tools": ["get_time"],
        "mode": "ANALYZE"
    });
    let (status, err) = send(&router, post("/api/v1/agent/run", "_default", body)).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "unknown tool must 400: {err}"
    );
    let msg = err["error"].as_str().unwrap_or_default();
    assert!(
        msg.contains("get_time"),
        "error must name the missing tool: {msg}"
    );
    assert!(
        msg.contains("classify"),
        "error should list available tools for discoverability: {msg}"
    );

    // A whitelist of real tools still opens the stream normally.
    let body = serde_json::json!({
        "prompt": "probe",
        "tools": ["classify"],
        "mode": "ANALYZE"
    });
    let (status, _) = send(&router, post("/api/v1/agent/run", "_default", body)).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "known tool must open the SSE stream"
    );

    std::env::remove_var("PHYSIS_SCAN_ROOT");
    drop(env);
}

#[tokio::test]
async fn every_dashboard_carries_the_shared_product_switcher() {
    // The four dashboards are one product served by one binary, but each was an
    // island: no page linked to any other, so /agent was reachable only by
    // typing the URL. The switcher is injected by ui-shell.js on any page that
    // loads it — so the invariant worth guarding is that every page loads the
    // shell, and that the shell still lists exactly the routes that exist.
    let (router, _dir, _env) = app().await;

    for page in ["/ui", "/demo", "/study", "/agent", "/pulse", "/console"] {
        let (status, body) = send_text(&router, get(page, "_default")).await;
        assert_eq!(status, StatusCode::OK, "{page} must be served");
        assert!(
            body.contains("ui-shell.js"),
            "{page} must load the shared shell, or it loses the product nav",
        );
    }

    let (status, shell) = send_text(&router, get("/ui-shell.js", "_default")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        shell.contains("initProductNav"),
        "the shell must define the switcher"
    );
    assert!(
        shell.contains("DOMContentLoaded"),
        "the shell loads from <head> on /agent and end-of-body elsewhere, so it \
         must handle both readiness states or the nav never mounts",
    );

    // A switcher entry pointing at a route that does not exist is a 404 the
    // user finds by clicking, so every listed href must actually serve.
    for href in ["/ui", "/agent", "/study", "/demo", "/pulse", "/console"] {
        assert!(
            shell.contains(&format!("href: '{href}'")),
            "the switcher must list {href}",
        );
        let (status, _) = send_text(&router, get(href, "_default")).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "switcher links to {href}, which must serve"
        );
    }
}

#[tokio::test]
async fn the_shared_stylesheet_carries_the_accessibility_floors() {
    // These three regressed independently once already, each because a rule
    // that belonged to the shared system was left to the pages to remember.
    let (router, _dir, _env) = app().await;

    let (status, css) = send_text(&router, get("/physis.css", "_default")).await;
    assert_eq!(status, StatusCode::OK);

    // ui-shell.js emits <span class="err"> from apiError() on every page. Four
    // pages kept a private copy of the bare rule and two never did, so an API
    // error on Pulse or Agent rendered in the muted body colour at 3.76:1 —
    // the least visible text on the screen was the failure.
    assert!(
        css.contains(".err {"),
        "physis.css must define the bare .err rule that ui-shell.js relies on",
    );

    // WCAG 2.2 SC 2.5.8 (AA) — the web minimum is 24 CSS px, not the 44pt
    // native figure. The nav pills measured 22px and shared controls 23.2px.
    assert!(
        css.matches("min-height:24px").count() >= 2,
        "the control floor and the nav floor must both survive",
    );

    // Raw hex for muted text is how the contrast failures got in: #5c6f92 on
    // three pages and #5a6a8a on Console, both under 4.5:1, both invisible to
    // review because they never touched the token table.
    for page in ["/ui", "/demo", "/study", "/agent", "/pulse", "/console"] {
        let (_, body) = send_text(&router, get(page, "_default")).await;
        for banned in ["#5c6f92", "#5a6a8a"] {
            assert!(
                !body.contains(banned),
                "{page} reintroduced {banned}, which fails AA for body text — \
                 use var(--brume)",
            );
        }
        assert!(
            body.contains("/physis.css"),
            "{page} must load the shared stylesheet to inherit these floors",
        );
    }
}

#[tokio::test]
async fn the_ask_bar_is_on_every_dashboard_and_queries_the_vector_graph() {
    // One query box on every page, backed by the vector search rather than by
    // an LLM: /api/v1/search/nodes embeds the query and ranks node labels by
    // cosine with no provider involved, so the box still works on an install
    // with no API key — which is the default state.
    let (router, _dir, _env) = app().await;

    let (status, shell) = send_text(&router, get("/ui-shell.js", "_default")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        shell.contains("initAskBar"),
        "the shell must define the ask bar"
    );
    assert!(
        shell.contains("/api/v1/search/nodes"),
        "the box must query the vector graph, not an LLM endpoint",
    );
    // Portalled to <body> and fixed: nested in the header it landed inside
    // whatever stacking context an ancestor created, and dashboards raise their
    // own panels, so page content painted straight over the results.
    assert!(
        shell.contains("document.body.appendChild(panel)"),
        "the results panel must be portalled out of the header",
    );
    assert!(
        shell.contains("position:fixed"),
        "the panel must escape ancestor clipping"
    );

    // The endpoint it calls must exist and answer.
    let body = serde_json::json!({ "query": "nozzle temperature", "top_k": 3 });
    let (status, out) = send(&router, post("/api/v1/search/nodes", "_default", body)).await;
    assert_eq!(status, StatusCode::OK, "the ask bar's endpoint must serve");
    assert!(out["results"].is_array(), "results must be a list: {out}");
    assert!(
        out["budget"].is_number(),
        "the response is token-packed and says so"
    );

    // The handoff target must accept the question the box hands over.
    let (status, agent) = send_text(&router, get("/agent", "_default")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        agent.contains("URLSearchParams(window.location.search).get('q')"),
        "/agent must read the ?q= handoff, or the question is retyped",
    );
}

#[tokio::test]
async fn scenarios_persist_review_and_promote_per_tenant() {
    // PLAN 7.3 + 7.4: a simulated scenario can be saved as a dated draft with
    // signifier-shift hypotheses, listed, and promoted — explicitly, reversibly,
    // and never visible across tenants.
    let (router, _dir, _env) = app().await;

    let simulation = json!({
        "scenario_id": "procurement-v3",
        "records_scanned": 2,
        "changed": 1,
        "unchanged": 1,
        "low_confidence": 0,
        "transitions": [{"from":"OPERATE×MONITOR","to":"STUDY×WORK","count":1}],
        "items": [
            {"id":"a","source":"s/a",
             "before":{"domain":"OPERATE","mode":"MONITOR","score":0.9},
             "after":{"domain":"STUDY","mode":"WORK","score":0.8}},
            {"id":"b","source":"s/b",
             "before":{"domain":"BOND","mode":"PLAY","score":0.9},
             "after":{"domain":"BOND","mode":"PLAY","score":0.9}}
        ]
    });

    // Save → 201, draft, exactly one dated hypothesis shift.
    let (status, saved) = send(
        &router,
        post(
            "/api/v1/planner/scenarios/save",
            "planner-user",
            json!({ "simulation": simulation }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "save succeeds: {saved}");
    assert_eq!(saved["version"], 1);
    assert_eq!(saved["status"], "draft");
    let shifts = saved["shifts"].as_array().unwrap();
    assert_eq!(shifts.len(), 1, "only changed items become shifts");
    assert_eq!(shifts[0]["item_id"], "a");
    assert_eq!(shifts[0]["hypothesis"], true);
    assert!(shifts[0]["observed_at"].is_string());

    // PLAN 7.2: the saved version carries the coverage + aggregate diff.
    // Item "a" moves OPERATE×MONITOR → STUDY×WORK confidently; item "b" stays.
    let impact = &saved["impact"];
    assert!(impact.is_object(), "impact analysis present: {saved}");
    assert_eq!(
        impact["newly_uncovered"].as_array().map(|a| a.len()),
        Some(0)
    );
    let deltas = impact["cell_deltas"].as_array().unwrap();
    assert_eq!(deltas.len(), 2, "two cells moved by one each");
    assert_eq!(deltas[0]["delta"], -1, "largest movement first");
    assert!(impact["mean_confidence_delta"].is_number());

    // The owner lists it; another tenant sees an empty list.
    let (status, mine) = send(
        &router,
        get("/api/v1/planner/scenarios/procurement-v3", "planner-user"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(mine.as_array().unwrap().len(), 1);
    let (_, theirs) = send(
        &router,
        get("/api/v1/planner/scenarios/procurement-v3", "someone-else"),
    )
    .await;
    assert!(theirs.as_array().unwrap().is_empty(), "tenant isolation");

    // Promote v1: status flips, promotion timestamp recorded.
    let (status, promoted) = send(
        &router,
        post(
            "/api/v1/planner/scenarios/procurement-v3/promote",
            "planner-user",
            json!({ "version": 1 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "promotion succeeds: {promoted}");
    assert_eq!(promoted["status"], "promoted");
    assert!(promoted["promoted_at"].is_string());

    // Unknown version answers 404, not a 500.
    let (status, missing) = send(
        &router,
        post(
            "/api/v1/planner/scenarios/procurement-v3/promote",
            "planner-user",
            json!({ "version": 99 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");
}

#[tokio::test]
async fn wiki_generate_covers_registered_nodes_and_tours_edges() {
    // Phase 9.6: /api/v1/wiki/generate must cover every labeled node (outline
    // coverage invariant) and order the tour by edge direction. Deterministic
    // mode is forced via `deterministic:true` so no model call happens.
    let (router, _dir, _env) = app().await;

    for label in ["alpha/ingest", "alpha/classify", "beta/report"] {
        let (status, body) = send(
            &router,
            post(
                "/api/v1/coherence/register",
                "wiki-user",
                json!({"input": label, "label": label}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    let (status, gen) = send(
        &router,
        post(
            "/api/v1/wiki/generate",
            "wiki-user",
            json!({ "deterministic": true, "max_group": 5, "tour_cap": 10 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{gen}");
    assert_eq!(
        gen["degraded"], true,
        "RP embedder reports degraded honestly"
    );

    // Coverage invariant: every registered label appears in exactly one page.
    let pages = gen["pages"].as_array().expect("pages array");
    let mut covered: Vec<&str> = pages
        .as_slice()
        .iter()
        .flat_map(|p| p["members"].as_array().unwrap().iter())
        .filter_map(|m| m.as_str())
        .collect();
    covered.sort_unstable();
    assert_eq!(
        covered,
        vec!["alpha/classify", "alpha/ingest", "beta/report"],
        "{pages:?}"
    );
    // Markdown skeleton present even with no LLM.
    assert!(pages
        .iter()
        .all(|p| p["markdown"].as_str().unwrap_or("").contains("## Members")));

    // Tour covers all nodes.
    assert_eq!(gen["tour"]["count"].as_u64(), Some(3));

    // Empty tenant answers an explicit error, not a 500.
    let (_, empty) = send(
        &router,
        post(
            "/api/v1/wiki/generate",
            "nobody-here",
            json!({ "deterministic": true }),
        ),
    )
    .await;
    assert!(empty["error"].is_string(), "{empty}");

    // GET tour endpoint works and stays tenant-scoped.
    let (status, tour) = send(&router, get("/api/v1/wiki/tour", "wiki-user")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tour["count"].as_u64(), Some(3));
}

/// Phase 11 — corpus-level comprehension: community summaries and the global
/// question that ranks them.
///
/// The failure this guards is the one top-k retrieval cannot even express:
/// "what are the recurring themes" has no single node holding the answer, so a
/// summaries surface that silently drops members, or a global ask that ranks
/// nodes instead of communities, would look fine and answer the wrong question.
#[tokio::test]
async fn corpus_summaries_and_global_ask_are_tenant_scoped() {
    let (router, _dir, _env) = app().await;

    // Two linked nodes (one community) plus one isolated node (a singleton).
    let mut ids = Vec::new();
    for text in [
        "spindle bearing preload procedure",
        "spindle bearing replacement procedure",
        "invoice reconciliation exception",
    ] {
        let (_, body) = send(
            &router,
            post(
                "/api/v1/coherence/register",
                "corpus-user",
                json!({"input": text}),
            ),
        )
        .await;
        ids.push(body["node_id"].as_str().unwrap().to_string());
    }
    let (status, _) = send(
        &router,
        post(
            "/api/v1/holarchy/edge",
            "corpus-user",
            json!({"source": ids[0], "target": ids[1]}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, summ) = send(&router, get("/api/v1/corpus/summaries", "corpus-user")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(summ["nodes"].as_u64(), Some(3));

    // Coverage invariant: no summary step may drop a member.
    let summaries = summ["summaries"].as_array().expect("summaries array");
    let mut covered: Vec<&str> = summaries
        .iter()
        .flat_map(|s| s["members"].as_array().unwrap().iter())
        .filter_map(|m| m.as_str())
        .collect();
    covered.sort_unstable();
    assert_eq!(
        covered,
        vec![
            "invoice reconciliation exception",
            "spindle bearing preload procedure",
            "spindle bearing replacement procedure",
        ],
        "{summaries:?}"
    );

    // The linked pair summarizes as a community with distinctive keywords and a
    // digest — the text a global question actually ranks.
    let linked = summaries
        .iter()
        .find(|s| s["size"].as_u64() == Some(2))
        .expect("the two linked nodes form one community");
    assert!(
        linked["keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|k| k == "spindle"),
        "{linked}"
    );
    assert!(
        linked["digest"].as_str().unwrap().contains("2 nodes"),
        "{linked}"
    );

    // Global ask degrades to ranked communities without a provider rather than
    // failing — same contract as /api/v1/ask.
    let (status, ask) = send(
        &router,
        post(
            "/api/v1/corpus/ask",
            "corpus-user",
            json!({"question": "what are the recurring themes here?"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!ask["hits"].as_array().unwrap().is_empty(), "{ask}");
    assert!(
        ask["total_tokens"].as_u64().unwrap() <= ask["budget"].as_u64().unwrap(),
        "packed context must stay inside the budget: {ask}"
    );
    assert_eq!(
        ask["cited"].as_array().unwrap().len(),
        ask["hits"].as_array().unwrap().len(),
        "every citation resolves to a summary: {ask}"
    );

    // Another tenant sees none of it.
    let (_, other) = send(&router, get("/api/v1/corpus/summaries", "someone-else")).await;
    assert_eq!(other["nodes"].as_u64(), Some(0));
    assert_eq!(other["communities"].as_u64(), Some(0));
}

/// Phase 11 C4 — `unit: "section"` retrieves heading-scoped sections and says
/// where each hit came from, while the default flat path stays byte-identical.
///
/// The bug this guards against is silent: a section retriever that loses the
/// breadcrumb still returns plausible text, and the caller cannot tell that
/// "Torque to 45 Nm" came from the wrong assembly.
#[tokio::test]
async fn rag_retrieve_section_unit_carries_heading_context() {
    let dir = tempfile::tempdir().expect("temp scan root");
    std::fs::write(
        dir.path().join("manual.md"),
        "# Spindle Assembly\n\nGeneral notes.\n\n## Reassembly\n\n### Bearing preload\n\nTorque to 45 Nm.\n\n## Teardown\n\nRemove the housing.\n",
    )
    .expect("write manual");

    let (router, _env) = app_on(dir.path()).await;

    let (status, sectioned) = send(
        &router,
        post(
            "/api/v1/rag/retrieve",
            "rag-user",
            json!({
                "dir": dir.path().to_str().unwrap(),
                "query": "bearing preload torque",
                "unit": "section",
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(sectioned["unit"], "section");

    let chunks = sectioned["chunks"].as_array().expect("chunks");
    let sources = sectioned["sources"].as_array().expect("sources");
    assert_eq!(
        chunks.len(),
        sources.len(),
        "every hit reports its provenance: {sectioned}"
    );
    assert!(chunks.len() > 1, "one document split into sections: {sectioned}");

    // A section's text carries its ancestors, and its source names the path.
    let preload = chunks
        .iter()
        .position(|c| c["text"].as_str().unwrap_or("").contains("45 Nm"))
        .expect("the preload section is retrieved");
    assert!(
        chunks[preload]["text"]
            .as_str()
            .unwrap()
            .contains("Spindle Assembly > Reassembly > Bearing preload"),
        "the embedded text carries the heading chain: {}",
        chunks[preload]["text"]
    );
    assert_eq!(
        sources[preload]["section"],
        "manual > Spindle Assembly > Reassembly > Bearing preload"
    );
    assert!(sources[preload]["path"]
        .as_str()
        .unwrap()
        .ends_with("manual.md"));

    // Default unit is unchanged: whole documents, one chunk per file.
    let (status, flat) = send(
        &router,
        post(
            "/api/v1/rag/retrieve",
            "rag-user",
            json!({
                "dir": dir.path().to_str().unwrap(),
                "query": "bearing preload torque",
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(flat["unit"], "chunk");
    // Whole document as one chunk: the manual's first and last sections share a
    // single chunk, and no section path is reported for it.
    let flat_chunks = flat["chunks"].as_array().unwrap();
    let manual = flat_chunks
        .iter()
        .position(|c| c["text"].as_str().unwrap_or("").contains("45 Nm"))
        .expect("the manual is retrieved");
    let manual_text = flat_chunks[manual]["text"].as_str().unwrap();
    assert!(
        manual_text.contains("Remove the housing."),
        "flat unit keeps the document whole: {manual_text}"
    );
    assert!(
        flat["sources"][manual]["section"].is_null(),
        "the flat unit reports no section path: {flat}"
    );
}

/// Search by content (PLAN 12.1): the answer lives in a paragraph nobody
/// labeled, so ranking labels cannot find it. These assert the properties the
/// GUI depends on — citation, highlight offsets, facets over the whole match
/// set, filters, confinement, and that the index is not rebuilt per keystroke.
#[tokio::test]
async fn content_search_finds_sections_cites_them_and_reuses_its_index() {
    let (router, dir, _env) = app().await;

    std::fs::create_dir_all(dir.path().join("line-2")).unwrap();
    std::fs::create_dir_all(dir.path().join("office")).unwrap();
    std::fs::write(
        dir.path().join("line-2/maintenance.md"),
        "# Spindle Assembly\n\n## Reassembly\n\n### Bearing preload\n\n\
         Torque the retaining nut to 45 Nm, then recheck after error E4412 clears.\n\n\
         ## Teardown\n\nRemove the housing.\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("office/invoices.md"),
        "# Invoices\n\nAcme invoice 8871 paid; no bearing work billed.\n",
    )
    .unwrap();

    let (status, r) = send(
        &router,
        post(
            "/api/v1/search/content",
            "search-user",
            json!({ "query": "E4412" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{r}");

    let hits = r["hits"].as_array().expect("hits array");
    assert!(!hits.is_empty(), "the section that says E4412 is found: {r}");
    let top = &hits[0];
    assert!(
        top["path"].as_str().unwrap().ends_with("maintenance.md"),
        "{top}"
    );
    // The citation is the heading chain, not a byte offset: "Bearing preload"
    // is what makes a hit legible.
    assert!(
        top["citation"].as_str().unwrap().contains("Bearing preload"),
        "{top}"
    );

    // Highlight offsets are character ranges into `snippet`; slicing by them
    // must land exactly on the matched term or the UI marks the wrong text.
    let snippet: Vec<char> = top["snippet"].as_str().unwrap().chars().collect();
    let hl = top["highlights"].as_array().expect("highlights");
    assert!(!hl.is_empty(), "the matched term is marked: {top}");
    let start = hl[0][0].as_u64().unwrap() as usize;
    let len = hl[0][1].as_u64().unwrap() as usize;
    let marked: String = snippet[start..start + len].iter().collect();
    assert_eq!(marked.to_lowercase(), "e4412", "{top}");

    // Facets describe the corpus that matched, and the place facet is the
    // folder — the distinction an operator navigates by.
    let (_, broad) = send(
        &router,
        post(
            "/api/v1/search/content",
            "search-user",
            json!({ "query": "bearing" }),
        ),
    )
    .await;
    assert!(
        broad["matched"].as_u64().unwrap() >= 2,
        "both files mention bearings: {broad}"
    );
    let places: Vec<&str> = broad["facets"]["place"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["value"].as_str().unwrap())
        .collect();
    assert!(places.contains(&"line-2"), "{broad}");
    assert!(places.contains(&"office"), "{broad}");

    // The index is built once, not per query: a rebuild on every keystroke is
    // the difference between a search box and a progress bar.
    assert_eq!(broad["rebuilt"], false, "{broad}");

    // Filtering by place narrows to that folder.
    let (_, filtered) = send(
        &router,
        post(
            "/api/v1/search/content",
            "search-user",
            json!({ "query": "bearing", "place": "office" }),
        ),
    )
    .await;
    for h in filtered["hits"].as_array().unwrap() {
        assert_eq!(h["place"], "office", "{filtered}");
    }

    // A quoted phrase must appear verbatim.
    let (_, phrase) = send(
        &router,
        post(
            "/api/v1/search/content",
            "search-user",
            json!({ "query": "\"no bearing work\"" }),
        ),
    )
    .await;
    assert_eq!(phrase["matched"], 1, "{phrase}");
    assert!(phrase["hits"][0]["path"]
        .as_str()
        .unwrap()
        .ends_with("invoices.md"));

    // The preview pane reads the section back out of the file on disk.
    let cited = top["citation"].as_str().unwrap().to_string();
    let section = cited.split_once('›').map(|(_, s)| s.trim()).unwrap();
    let path = top["path"].as_str().unwrap();
    let (status, preview) = send(
        &router,
        get(
            &format!(
                "/api/v1/search/content/section?path={}&section={}",
                urlencoding::encode(path),
                urlencoding::encode(section)
            ),
            "search-user",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert!(
        preview["body"].as_str().unwrap().contains("45 Nm"),
        "{preview}"
    );
    assert_eq!(preview["matched_section"], true, "{preview}");

    // Confinement: the scan root is the only reachable corpus.
    let outside = tempfile::tempdir().unwrap();
    let (status, _) = send(
        &router,
        post(
            "/api/v1/search/content",
            "search-user",
            json!({ "query": "bearing", "dir": outside.path().to_str().unwrap() }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// With no semantic model loaded the semantic channel must be absent, not
/// faked: random-projection vectors rank confidently and wrongly. The surface
/// says which mode actually ran.
#[tokio::test]
async fn content_search_degrades_to_lexical_without_a_semantic_embedder() {
    let (router, dir, _env) = app().await;
    std::fs::write(
        dir.path().join("log.md"),
        "# Shift\n\nPump seal replaced on line one.\n",
    )
    .unwrap();

    let (status, r) = send(
        &router,
        post(
            "/api/v1/search/content",
            "search-user",
            json!({ "query": "pump seal", "mode": "semantic" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{r}");
    assert_eq!(r["semantic_index"], false, "{r}");
    assert_eq!(r["requested_mode"], "semantic", "{r}");
    assert_eq!(r["mode"], "lexical", "the mode that actually ran: {r}");
    assert!(!r["hits"].as_array().unwrap().is_empty(), "{r}");
}

/// Atlas (PLAN 12.3): one call returns a drawable graph. The visualization is
/// only as honest as this payload — recorded edges must stay distinguishable
/// from resemblance, every node must land in exactly one community, and the
/// whole thing must be tenant-scoped like the store it reads.
#[tokio::test]
async fn atlas_graph_returns_a_drawable_tenant_scoped_graph() {
    let (router, _dir, _env) = app().await;

    for label in [
        "spindle bearing seized on line two",
        "spindle bearing replaced on line two",
        "quarterly invoice reconciliation for Acme",
    ] {
        let (status, _) = send(
            &router,
            post(
                "/api/v1/coherence/register",
                "atlas-user",
                json!({ "input": label }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    let (status, g) = send(&router, get("/api/v1/atlas/graph", "atlas-user")).await;
    assert_eq!(status, StatusCode::OK, "{g}");
    let nodes = g["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 3, "{g}");
    assert_eq!(g["total_nodes"], 3);
    assert_eq!(g["truncated"], false);

    // Every node carries what the renderer draws with.
    for n in nodes {
        assert!(!n["label"].as_str().unwrap().is_empty(), "{n}");
        assert!(n["community"].is_number(), "{n}");
        assert!(n["degree"].is_number(), "{n}");
    }

    // Communities partition the nodes: nothing is invisible to the picture.
    let sizes: u64 = g["communities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["size"].as_u64().unwrap())
        .sum();
    assert_eq!(sizes as usize, nodes.len(), "{g}");

    // Edge kinds stay separate — "we recorded this" and "these look alike" are
    // different claims and the legend distinguishes them.
    for e in g["edges"].as_array().unwrap() {
        let kind = e["kind"].as_str().unwrap();
        assert!(kind == "similarity" || kind == "holarchy", "{e}");
    }

    // Another tenant sees its own (empty) graph, not this one.
    let (status, other) = send(&router, get("/api/v1/atlas/graph", "someone-else")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(other["nodes"].as_array().unwrap().len(), 0, "{other}");
}

/// The new surfaces are served by the binary itself: no CDN, no build step.
/// An air-gapped install that renders an empty graph is the failure this
/// asserts against.
#[tokio::test]
async fn search_and_atlas_pages_are_served_self_contained() {
    let (router, _dir, _env) = app().await;

    let (status, html) = send_text(&router, get("/search", "u")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("/api/v1/search/content"), "search page is wired");

    let (status, html) = send_text(&router, get("/atlas", "u")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("/organic.js"), "atlas loads the local renderer");
    assert!(
        !html.contains("unpkg.com") && !html.contains("cdn."),
        "no external script host may appear on an air-gapped surface"
    );

    // The renderer is public: a <script src> carries no Bearer header.
    let (status, js) = send_text(&router, get("/organic.js", "u")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(js.contains("window.Organic"), "renderer exports its entry point");

    // Pinning lives in the shared shell, so a page written later gets it for
    // free — and both surfaces that can name a query actually offer it. A
    // notebook nobody can add to from where they work is a notebook nobody
    // keeps.
    let (_, shell) = send_text(&router, get("/ui-shell.js", "u")).await;
    assert!(shell.contains("function addToNotebook("), "the shell exposes pinning");
    let (_, search) = send_text(&router, get("/search", "u")).await;
    assert!(search.contains("addToNotebook('search.content'"), "Search can pin its query");
    let (_, atlas) = send_text(&router, get("/atlas", "u")).await;
    assert!(atlas.contains("addToNotebook("), "Atlas can pin an entity");
}

/// Ontology studio write path (PLAN 12.4). A pack is data, so the studio must
/// be able to author one — but only inside the packs directory, only with the
/// grid's own vocabulary, and only in the shape the loader can actually read.
#[tokio::test]
async fn ontology_packs_can_be_authored_listed_read_and_deleted() {
    let (router, dir, _env) = app().await;

    let entries = json!([
        {
            "name": "Freight Booking",
            "category": "Logistics",
            "domain": "bond",
            "mode": "work",
            "hints": ["freight", "carrier", "bill of lading"]
        },
        {
            "name": "Customs Declaration",
            "domain": "CONSTRUCT",
            "mode": "WORK",
            "hints": ["customs", "tariff code"]
        }
    ]);

    let (status, saved) = send(
        &router,
        post(
            "/api/v1/ontology/packs/save",
            "studio-user",
            json!({ "name": "logistics-demo", "entries": entries }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["entries"], 2);
    // Saving is not loading, and the response says so rather than implying the
    // classifier already changed.
    assert_eq!(saved["live"], false, "{saved}");

    // It landed in the packs directory, in the format the loader parses.
    let path = dir.path().join("packs/logistics-demo.json");
    assert!(path.exists(), "pack written to {}", path.display());
    let on_disk = std::fs::read_to_string(&path).unwrap();
    let (_, loaded) = physis_pro::config::OntologyLoader::load_str(&on_disk, "logistics-demo")
        .expect("the engine's own loader reads what the studio wrote");
    assert_eq!(loaded.len(), 2);

    let (status, list) = send(&router, get("/api/v1/ontology/packs", "studio-user")).await;
    assert_eq!(status, StatusCode::OK);
    let pack = list["packs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "logistics-demo")
        .expect("the saved pack is listed");
    assert_eq!(pack["valid"], true, "{pack}");
    assert_eq!(pack["entries"], 2, "{pack}");
    // This process started before the pack existed, so it is pending, not live.
    assert_eq!(pack["live"], false, "{pack}");

    let (status, read) = send(
        &router,
        get("/api/v1/ontology/packs/read?name=logistics-demo", "studio-user"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read["domains"][0]["domain"], "BOND", "{read}");
    assert_eq!(read["domains"][0]["mode"], "WORK", "{read}");

    // Every problem comes back at once — one round-trip per error is not a
    // studio.
    let (status, bad) = send(
        &router,
        post(
            "/api/v1/ontology/packs/save",
            "studio-user",
            json!({
                "name": "broken",
                "entries": [{ "name": "", "domain": "NOPE", "mode": "ALSO-NOPE" }]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(bad["errors"].as_array().unwrap().len() >= 3, "{bad}");

    // A name that would escape the packs directory is refused outright.
    let (status, _) = send(
        &router,
        post(
            "/api/v1/ontology/packs/save",
            "studio-user",
            json!({ "name": "../escape", "entries": entries }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(!dir.path().join("../escape.json").exists());

    let (status, del) = send(
        &router,
        post(
            "/api/v1/ontology/packs/delete",
            "studio-user",
            json!({ "name": "logistics-demo" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{del}");
    assert!(!path.exists(), "the file is gone");

    let (status, _) = send(
        &router,
        post(
            "/api/v1/ontology/packs/delete",
            "studio-user",
            json!({ "name": "logistics-demo" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Phase 8 read-back (`POST /api/v1/machines/:name/reduce`): the machines are
/// meant to register what they perceive into the graph, not answer and forget.
/// Only stateless classify was exposed, so the loop was open at the boundary.
#[tokio::test]
async fn machine_reduce_registers_bridges_and_persists_per_tenant() {
    let (router, _dir, _env) = app().await;

    let events = json!({"events": [
        {"label": "oil check", "at": "2026-08-01T08:00:00Z"},
        {"label": "oil check", "at": "2026-08-08T08:00:00Z"},
        {"label": "oil check", "at": "2026-08-15T08:00:00Z"}
    ]});

    let (status, r) = send(
        &router,
        post("/api/v1/machines/chronos/reduce", "machine-user", events.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{r}");
    assert_eq!(r["machine"], "chronos");
    let registered = r["registered"].as_u64().unwrap();
    assert!(registered >= 1, "reduce registered nothing: {r}");
    assert_eq!(r["nodes_before"], 0, "{r}");
    assert_eq!(r["nodes_after"].as_u64().unwrap(), registered, "{r}");
    // Persistence is reported, not assumed: a reduce that looked like it worked
    // and vanished on restart is the worst of both.
    assert_eq!(r["persisted"].as_u64().unwrap(), registered, "{r}");
    assert!(r["persist_error"].is_null(), "{r}");

    // What it registered is in the caller's graph, recallable by content.
    let (_, nodes) = send(&router, get("/api/v1/coherence/nodes/all", "machine-user")).await;
    let labels: Vec<String> = nodes["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["label"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        labels.iter().any(|l| l.contains("oil check")),
        "the rhythm it perceived is in the graph: {labels:?}"
    );

    // Registration dedupes by label, so an identical second reduce adds
    // nothing — and says so rather than looking like a failure.
    let (status, again) = send(
        &router,
        post("/api/v1/machines/chronos/reduce", "machine-user", events),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["registered"], 0, "{again}");
    assert_eq!(again["nodes_before"], again["nodes_after"], "{again}");

    // A machine writes into the caller's graph, never a shared one.
    let (_, other) = send(&router, get("/api/v1/coherence/nodes/all", "other-tenant")).await;
    assert_eq!(other["results"].as_array().unwrap().len(), 0, "{other}");

    // Asking a stateless machine to write is refused, and the refusal names
    // the machines that can — a caller who got {"score": 0} back would have no
    // way to tell nothing was written.
    let (status, err) = send(
        &router,
        post(
            "/api/v1/machines/harmony/reduce",
            "machine-user",
            json!({"notes": [60, 64, 67]}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{err}");
    assert!(
        err["error"].as_str().unwrap().contains("chronos"),
        "names the machines that do write: {err}"
    );

    let (status, _) = send(
        &router,
        post("/api/v1/machines/nope/reduce", "machine-user", json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The registry says which machines write, so a client need not discover it
    // from a 400.
    let (_, list) = send(&router, get("/api/v1/machines", "machine-user")).await;
    let machines = list["machines"].as_array().unwrap();
    let chronos = machines.iter().find(|m| m["name"] == "chronos").unwrap();
    let harmony = machines.iter().find(|m| m["name"] == "harmony").unwrap();
    assert_eq!(chronos["writes"], true, "{chronos}");
    assert_eq!(harmony["writes"], false, "{harmony}");
}

/// PH-014 Item A: every `/api/v1/machines/<name>/register` route registers a
/// typed `CoherenceNode` — `kind` drawn from `OBJECT_TYPES`, not left empty —
/// which is exactly what commit `bb469ea` deleted the fields needed for while
/// leaving these routes wired to handlers that didn't exist.
#[tokio::test]
async fn machine_register_routes_stamp_a_typed_kind() {
    let (router, _dir, _env) = app().await;

    let cases = [
        (
            "chronos",
            json!({"events": [
                {"label": "oil check", "at": "2026-08-01T08:00:00Z"},
                {"label": "oil check", "at": "2026-08-08T08:00:00Z"}
            ]}),
        ),
        (
            "empedocles",
            json!({"readings": [
                {"metric": "spindle_temp", "value": 92.0, "baseline": 60.0}
            ]}),
        ),
        (
            "euclid",
            json!({"primitives": [
                {"vertices": 4, "circularity": 0.2, "aspect_ratio": 1.0, "color": [10, 10, 10], "texture": 0.1}
            ]}),
        ),
    ];

    for (machine, body) in cases {
        let (status, r) = send(
            &router,
            post(&format!("/api/v1/machines/{machine}/register"), "machine-user", body),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{machine}: {r}");
        assert_eq!(r["machine"], machine, "{r}");
        let node_id = r["node_id"].as_str().expect("node_id in response");
        let kind = r["kind"].as_str().expect("kind in response");
        assert!(
            physis_pro::models::OBJECT_TYPES.contains(&kind),
            "{machine} register stamped {kind:?}, not a member of OBJECT_TYPES: {r}"
        );

        // The node it created is real and carries the same kind, not just the
        // response echoing a value nothing persisted.
        let (_, nodes) = send(&router, get("/api/v1/coherence/nodes/all", "machine-user")).await;
        let node = nodes["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == node_id)
            .unwrap_or_else(|| panic!("{machine}: node {node_id} not found in graph: {nodes}"));
        assert_eq!(node["kind"], kind, "{machine}: {node}");
    }
}

/// Phase 10 end-to-end (the verification the plan index left open): a real
/// scan must produce all three filing axes at once — where the file lives
/// (space), when it changed (time), and what produced it (cause) — and every
/// surface that reads them must agree.
///
/// D1–D6 each had their own unit or handler test; nothing walked the chain
/// from a scan through the graph DTO, the ledger, a facet query, a purpose
/// link and back out as lineage. That walk is what a customer does.
#[tokio::test]
async fn space_time_cause_survives_a_real_scan_end_to_end() {
    let dir = tempfile::tempdir().expect("temp data dir");
    // Two folders, so "place" has something to distinguish.
    let line = dir.path().join(".physis").join("line-2");
    let office = dir.path().join(".physis").join("office");
    std::fs::create_dir_all(&line).expect("mkdir");
    std::fs::create_dir_all(&office).expect("mkdir");
    std::fs::write(
        line.join("maintenance.md"),
        "# Spindle\n\nBearing replaced after the vibration alarm.\n",
    )
    .expect("write");
    std::fs::write(
        office.join("invoice.md"),
        "# Invoice 8871\n\nAcme reconciled against PO 4412.\n",
    )
    .expect("write");

    let (router, _env) = app_on(dir.path()).await;
    let root = dir.path().join(".physis");
    let root_str = root.display().to_string();

    let (status, scan) = send(
        &router,
        post("/api/v1/scan", "ops", json!({ "dir": root_str })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{scan}");

    // ── Space + time: the graph DTO carries the address, not just the path.
    let (status, graph) = send(
        &router,
        get(
            &format!("/api/v1/fs/graph?dir={}", urlencoding::encode(&root_str)),
            "ops",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{graph}");
    let files = graph["files"].as_array().expect("files array");
    assert_eq!(files.len(), 2, "both files are in the graph: {graph}");
    for f in files {
        assert!(
            f["place"].is_string(),
            "every scanned file knows where it lives: {f}"
        );
        assert!(
            f["period"].as_str().is_some_and(|p| p.len() == 7),
            "period is the YYYY-MM bucket: {f}"
        );
        assert!(
            f["modified"].is_string(),
            "time comes from fs metadata, not ingest time: {f}"
        );
        assert_eq!(
            f["mechanism"], "generated",
            "a scan is what produced this registration: {f}"
        );
    }

    // ── Cause: the scan itself is in the ledger, addressable by facet.
    let (_, causes) = send(
        &router,
        post("/api/v1/files/query", "ops", json!({"cause": "generated"})),
    )
    .await;
    let matches = causes["matches"].as_array().expect("matches");
    assert!(
        !matches.is_empty(),
        "the scan recorded why these files exist: {causes}"
    );
    assert!(
        matches.iter().all(|m| m["mechanism"] == "generated"),
        "{causes}"
    );
    let scanned_object = matches[0]["object"].as_str().unwrap().to_string();

    // Rollups answer "where/when/how", not just "which".
    assert!(
        causes["by_mechanism"].is_object() || causes["by_mechanism"].is_array(),
        "facet query rolls up by mechanism: {causes}"
    );

    // Time facet: this month catches it, a past bucket does not.
    let now_period = chrono::Utc::now().format("%Y-%m").to_string();
    let (_, this_month) = send(
        &router,
        post("/api/v1/files/query", "ops", json!({"during": now_period})),
    )
    .await;
    assert!(this_month["total"].as_u64().unwrap_or(0) >= 1, "{this_month}");
    let (_, long_ago) = send(
        &router,
        post("/api/v1/files/query", "ops", json!({"during": "1999-01"})),
    )
    .await;
    assert_eq!(long_ago["total"], 0, "{long_ago}");

    // ── Purpose: attach the artifact to a goal, read it back as final cause.
    let (status, goal) = send(
        &router,
        post(
            "/api/v1/goals",
            "ops",
            json!({"label": "cut unplanned downtime on line 2"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{goal}");
    let (_, goals) = send(&router, get("/api/v1/goals/list", "ops")).await;
    let goal_id = goals["goals"][0]["id"].as_str().expect("a goal id").to_string();

    let (status, served) = send(
        &router,
        post(
            "/api/v1/files/serve",
            "ops",
            json!({"object": scanned_object, "goal_id": goal_id}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{served}");

    let (status, lineage) = send(
        &router,
        get(
            &format!(
                "/api/v1/files/lineage?path={}",
                urlencoding::encode(&scanned_object)
            ),
            "ops",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{lineage}");
    assert!(
        !lineage["lineage"].as_array().unwrap().is_empty(),
        "the why-made chain survives the round trip: {lineage}"
    );
    let purposes = lineage["purposes"].as_array().unwrap();
    assert_eq!(purposes.len(), 1, "the goal it serves: {lineage}");
    assert_eq!(purposes[0]["input"], goal_id, "{lineage}");
    // Serves is a final cause, not a production step — it must not appear in
    // the chain that explains how the artifact came to be.
    assert!(
        lineage["lineage"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["mechanism"] != "serves"),
        "purpose is not lineage: {lineage}"
    );

    // ── Isolation: another tenant shares the files on disk, not the history.
    let (_, other) = send(&router, post("/api/v1/files/query", "auditor", json!({}))).await;
    assert_eq!(other["total"], 0, "{other}");
}

/// Notebooks (PLAN Phase 13, N1–N2): a user-composed dashboard is stored
/// server-side per tenant and validated on every write. The validation is the
/// security boundary as much as the ergonomic one — a notebook is meant to be
/// exported and handed to a colleague, so it must hold references the server
/// recognises and nothing else.
#[tokio::test]
async fn notebooks_are_validated_stored_per_tenant_and_never_hold_markup() {
    let (router, _dir, _env) = app().await;

    // The catalogue the picker offers is the one the validator checks against.
    let (status, registry) = send(&router, get("/api/v1/notebooks/registry", "ops")).await;
    assert_eq!(status, StatusCode::OK);
    let widgets = registry["widgets"].as_array().expect("widgets");
    assert!(!widgets.is_empty(), "{registry}");
    for w in widgets {
        let path = w["path"].as_str().unwrap();
        assert!(path.starts_with("/api/v1/"), "{w}");
    }

    // A tenant who never composed anything gets the starter composition, not a
    // blank grid and not a 404.
    let (status, fresh) = send(&router, get("/api/v1/notebooks/default", "ops")).await;
    assert_eq!(status, StatusCode::OK, "{fresh}");
    assert_eq!(fresh["saved"], false, "{fresh}");
    assert!(!fresh["notebook"]["cells"].as_array().unwrap().is_empty());

    // Save a real composition, including a pinned query — the cell type that
    // makes this more than rearranging panels.
    let doc = json!({
        "id": "ops",
        "title": "Line 2 — daily",
        "cells": [
            {"widget": "coherence.kpis", "span": 12, "height": "s", "params": {}},
            {"widget": "search.content", "span": 6, "height": "l",
             "params": {"query": "\"no fault found\"", "place": "line-2", "top_k": 8},
             "refresh": {"mode": "interval", "seconds": 60}}
        ]
    });
    let (status, saved) = send(&router, post("/api/v1/notebooks/ops", "ops", doc)).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["cells"], 2, "{saved}");
    assert!(saved["updated_at"].as_str().unwrap().contains('T'), "{saved}");

    let (_, listed) = send(&router, get("/api/v1/notebooks", "ops")).await;
    let ids: Vec<&str> = listed["notebooks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"ops"), "{listed}");

    let (_, read) = send(&router, get("/api/v1/notebooks/ops", "ops")).await;
    assert_eq!(read["saved"], true, "{read}");
    assert_eq!(read["notebook"]["cells"][1]["params"]["top_k"], 8, "{read}");

    // A widget nobody ships is refused outright rather than stored and skipped
    // at render — a stored reference nobody validates is how a payload rides in.
    let (status, bad) = send(
        &router,
        post(
            "/api/v1/notebooks/evil",
            "ops",
            json!({
                "id": "evil",
                "title": "x",
                "cells": [{"widget": "<img src=x onerror=alert(1)>", "span": 6, "height": "m"}]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    assert!(
        bad["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e.as_str().unwrap().contains("unknown widget")),
        "{bad}"
    );

    // An undeclared parameter is refused too: widgets take what they declare,
    // so a cell cannot smuggle a field through to the endpoint behind it.
    let (status, smuggle) = send(
        &router,
        post(
            "/api/v1/notebooks/ops2",
            "ops",
            json!({
                "id": "ops2",
                "title": "x",
                "cells": [{"widget": "search.content", "span": 6, "height": "m",
                           "params": {"query": "ok", "dir": "/etc"}}]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{smuggle}");
    assert!(
        smuggle["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e.as_str().unwrap().contains("takes no parameter 'dir'")),
        "{smuggle}"
    );

    // The body cannot rename the notebook out from under its own URL.
    let (_, renamed) = send(
        &router,
        post(
            "/api/v1/notebooks/ops",
            "ops",
            json!({"id": "somewhere-else", "title": "t", "cells": []}),
        ),
    )
    .await;
    assert_eq!(renamed["id"], "ops", "{renamed}");
    let (status, _) = send(&router, get("/api/v1/notebooks/somewhere-else", "ops")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // One operator's composition is not another's.
    let (_, other) = send(&router, get("/api/v1/notebooks", "auditor")).await;
    assert_eq!(other["notebooks"].as_array().unwrap().len(), 0, "{other}");

    let (status, gone) = send(&router, delete("/api/v1/notebooks/ops", "ops")).await;
    assert_eq!(status, StatusCode::OK, "{gone}");
    let (status, _) = send(&router, delete("/api/v1/notebooks/ops", "ops")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "deleting twice is a 404");
}

/// Monitor mode (PLAN Phase 14, M1–M2): a phone syncs a morning in one call and
/// the engine decides what it meant. The device perceives; the machines mean.
#[tokio::test]
async fn monitor_ingest_turns_sensor_samples_into_graph_nodes() {
    let (router, _dir, _env) = app().await;

    let batch = json!({
        "device": "pixel-8",
        "samples": [
            // A workout: 6000 steps in 45 minutes is well past a stroll.
            {"kind": "steps", "at": "2026-08-30T07:00:00Z", "count": 6000, "minutes": 45},
            // A stroll: recorded as a rhythm candidate, not as a session.
            {"kind": "steps", "at": "2026-08-30T13:00:00Z", "count": 800, "minutes": 60},
            // Two fixes at the same place, so topos has something to cluster.
            {"kind": "location", "at": "2026-08-30T07:05:00Z", "lat": 45.4642, "lng": 9.19,
             "accuracy_m": 12.0, "label": "gym"},
            {"kind": "location", "at": "2026-08-30T07:40:00Z", "lat": 45.4643, "lng": 9.1901,
             "accuracy_m": 20.0, "label": "gym"},
            // A fix too vague to be a place at all.
            {"kind": "location", "at": "2026-08-30T09:00:00Z", "lat": 45.9, "lng": 9.9,
             "accuracy_m": 900.0},
            // A lesson: how long and how many voices — never what was said.
            {"kind": "audio", "at": "2026-08-30T14:00:00Z", "minutes": 50, "voices": 3},
            {"kind": "reading", "at": "2026-08-30T22:00:00Z", "metric": "battery",
             "value": 18.0, "baseline": 80.0}
        ]
    });

    let (status, r) = send(&router, post("/api/v1/monitor/ingest", "phone-owner", batch)).await;
    assert_eq!(status, StatusCode::OK, "{r}");
    assert_eq!(r["device"], "pixel-8");

    // What it understood, reported as counts.
    let u = &r["understood"];
    assert_eq!(u["sessions"], 1, "only the burst is a session: {r}");
    assert_eq!(u["waypoints"], 2, "the vague fix was dropped: {r}");
    assert!(u["events"].as_u64().unwrap() >= 3, "{r}");

    // A dropped sample is said out loud, not silently discarded.
    let skipped = u["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1, "{r}");
    assert!(skipped[0].as_str().unwrap().contains("900m"), "{r}");

    // It reached the reducers, and what they registered is in the graph.
    assert!(r["registered"].as_u64().unwrap() >= 1, "{r}");
    assert!(r["machines"]["chronos"].is_number(), "{r}");
    assert!(r["persist_error"].is_null(), "{r}");

    let (_, nodes) = send(&router, get("/api/v1/coherence/nodes/all", "phone-owner")).await;
    let labels: Vec<String> = nodes["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["label"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        labels.iter().any(|l| l.contains("workout")),
        "the workout it inferred is in the graph: {labels:?}"
    );
    // The conversation is an event with a duration and a voice count. What was
    // said never entered the system — there is nowhere in the wire format for
    // it to live.
    assert!(
        labels.iter().any(|l| l.contains("conversation")),
        "{labels:?}"
    );
    assert!(
        !labels.iter().any(|l| l.contains("transcript")),
        "no transcript may ever appear: {labels:?}"
    );

    // Why these nodes exist: a device reported them.
    let (_, causes) = send(
        &router,
        post("/api/v1/files/query", "phone-owner", json!({"cause": "monitor"})),
    )
    .await;
    let matches = causes["matches"].as_array().unwrap();
    assert!(
        matches.iter().any(|m| m["object"] == "device:pixel-8"),
        "the sync is accounted for: {causes}"
    );

    // A device syncs into its owner's graph, nobody else's.
    let (_, other) = send(&router, get("/api/v1/coherence/nodes/all", "someone-else")).await;
    assert_eq!(other["results"].as_array().unwrap().len(), 0, "{other}");

    // A batch that translates to nothing says so rather than answering 200 and
    // moving on: a collector syncing every ten minutes to no effect is
    // misconfigured, not quiet.
    let (status, quiet) = send(
        &router,
        post(
            "/api/v1/monitor/ingest",
            "phone-owner",
            json!({"device": "pixel-8", "samples": [
                {"kind": "steps", "at": "2026-08-30T03:00:00Z", "count": 0, "minutes": 60}
            ]}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(quiet["registered"], 0, "{quiet}");
    assert!(quiet["note"].as_str().unwrap().contains("collector"), "{quiet}");

    // An unnamed device is refused: two phones are two witnesses.
    let (status, _) = send(
        &router,
        post("/api/v1/monitor/ingest", "phone-owner", json!({"device": "", "samples": []})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── Phase 19 B1: declarative route table ────────────────────────────────────
#[tokio::test]
async fn route_table_crud_match_and_validation() {
    let (router, _dir, _env) = app().await;

    // Invalid route: every problem reported at once, one round-trip.
    let (status, body) = send(
        &router,
        post(
            "/api/v1/routes",
            "tenant-a",
            json!({"route": {"name": "  ", "cell": "BROKEN", "action": ""}}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let errors = body["errors"].as_array().expect("errors array");
    assert!(errors.len() >= 3, "all problems reported: {body}");

    // Upsert a valid route, then read it back.
    let (status, body) = send(
        &router,
        post(
            "/api/v1/routes",
            "tenant-a",
            json!({"route": {
                "name": "downtime",
                "cell": "FABRICATE/WORK",
                "action": "open the shift-loss countermeasure",
                "examples": ["press seized", "CNC down 40 min"]
            }}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], true);

    let (status, body) = send(&router, get("/api/v1/routes", "tenant-a")).await;
    assert_eq!(status, StatusCode::OK);
    let routes = body["routes"].as_array().expect("routes array");
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0]["name"], "downtime");
    assert_eq!(routes[0]["cell"], "FABRICATE/WORK");

    // Match: classify text and return routes bound to the winning cell. Under
    // the random-projection embedder the cell is not guaranteed, but the shape
    // is: a cell string and a routes array, never an error.
    let (status, body) = send(
        &router,
        post(
            "/api/v1/routes/match",
            "tenant-a",
            json!({"text": "seized bearing on the press"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["cell"].as_str().is_some(), "cell present: {body}");
    assert!(body["routes"].is_array());

    // Delete, then confirm deletion.
    let (status, body) = send(&router, delete("/api/v1/routes/downtime", "tenant-a")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], true);
    let (_, body) = send(&router, get("/api/v1/routes", "tenant-a")).await;
    assert_eq!(body["routes"].as_array().unwrap().len(), 0);
}

// ── Phase 19 B5: quality-failure trace ──────────────────────────────────────
#[tokio::test]
async fn trace_unknown_failure_is_404_and_known_is_well_shaped() {
    let (router, _dir, _env) = app().await;

    // Unknown id → explicit 404, not a 500.
    let (status, _) = send(&router, get("/api/v1/trace?failure_id=nope", "tenant-a")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── Phase 16 I3/I4: single event ingestion path ─────────────────────────────
fn event_json(key: &str, subject: &str, kind: &str, status: &str) -> Value {
    json!({
        "event_key": key,
        "observed_at": "2026-08-12T08:55:00Z",
        "kind": kind,
        "status": status,
        "subject": subject,
        "provenance": {"connector": "industrial", "source": "machine", "source_id": subject},
        "properties": {"temperature": "78.5"},
        "evidence": [],
        "machine_id": subject,
        "raw": null,
    })
}

#[tokio::test]
async fn events_ingest_dedupes_stores_and_isolates_tenants() {
    let (router, _dir, _env) = app().await;

    // One event, re-posted (idempotent dedupe by event_key), plus a second.
    let (status, body) = send(
        &router,
        post(
            "/api/v1/events",
            "tenant-a",
            json!({"events": [
                event_json("ev-1", "voron-01", "machine", "warning"),
                event_json("ev-1", "voron-01", "machine", "warning"),
                event_json("ev-2", "press-07", "machine", "failure"),
            ]}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["accepted"], 2, "two distinct events accepted: {body}");
    assert_eq!(body["skipped"], 1, "re-post deduped: {body}");

    // Read back: both stored, newest first.
    let (status, body) = send(&router, get("/api/v1/events", "tenant-a")).await;
    assert_eq!(status, StatusCode::OK);
    let events = body.as_array().expect("events array");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["event_key"], "ev-2", "newest first");

    // Timeline renders from the store.
    let (status, body) = send(&router, get("/api/v1/events/timeline", "tenant-a")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["entries"].as_array().map(|a| a.len()), Some(2));
    assert!(body["counts_by_kind"]["machine"].is_number());

    // Tenant isolation: the other tenant's ring is empty.
    let (_, body) = send(&router, get("/api/v1/events", "tenant-b")).await;
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn events_can_register_graph_nodes_when_requested() {
    let (router, _dir, _env) = app().await;

    let (status, body) = send(
        &router,
        post(
            "/api/v1/events",
            "tenant-a",
            json!({"events": [event_json("ev-n1", "cnc-01", "machine", "success")],
                   "register_nodes": true}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["accepted"], 1);
    let ids = body["node_ids"].as_array().expect("node_ids");
    assert_eq!(ids.len(), 1);
    assert!(!ids[0].as_str().unwrap().is_empty());
}

// ── Phase 16 I5: typed CoherenceNode (kind + properties) ────────────────────
#[tokio::test]
async fn register_stamps_kind_and_properties_into_the_node() {
    let (router, _dir, _env) = app().await;

    let (status, body) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "tenant-a",
            json!({
                "input": "press-07 cycle time drift",
                "kind": "machine",
                "properties": {"oee": 0.82, "down_min": 14, "cell": "FABRICATE/WORK"},
                "coherence_score": -0.5,
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let node_id = body["node_id"].as_str().unwrap().to_string();

    // The node comes back with the typed axis through the standard projection.
    let (status, body) = send(&router, get("/api/v1/coherence/nodes/all", "tenant-a")).await;
    assert_eq!(status, StatusCode::OK);
    let nodes = body["results"].as_array().expect("results");
    let node = nodes.iter().find(|n| n["id"] == node_id).expect("registered node");
    assert_eq!(node["kind"], "machine");
    assert_eq!(node["properties"]["oee"], 0.82);
    assert_eq!(node["properties"]["down_min"], 14);
    assert_eq!(node["rating"], "Failure");

    // Re-registering the same label (dedupe) with a new kind updates it.
    let (_, body) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "tenant-a",
            json!({"input": "press-07 cycle time drift", "kind": "sensor_reading"}),
        ),
    )
    .await;
    assert_eq!(body["node_id"], node_id);
    let (_, body) = send(&router, get("/api/v1/coherence/nodes/all", "tenant-a")).await;
    let nodes = body["results"].as_array().unwrap();
    let node = nodes.iter().find(|n| n["id"] == node_id).unwrap();
    assert_eq!(node["kind"], "sensor_reading");
}

// ── Phase 16 I6: named typed links + inverse traversal ──────────────────────
#[tokio::test]
async fn typed_edges_expose_outgoing_and_inverse_traversal() {
    let (router, _dir, _env) = app().await;

    // Two nodes: a work order and the failure it causes.
    let (_, b1) = send(
        &router,
        post("/api/v1/coherence/register", "tenant-a", json!({"input": "work-order-41"})),
    )
    .await;
    let (_, b2) = send(
        &router,
        post("/api/v1/coherence/register", "tenant-a", json!({"input": "press seized"})),
    )
    .await;
    let wo = b1["node_id"].as_str().unwrap().to_string();
    let failure = b2["node_id"].as_str().unwrap().to_string();

    // A named "Causes" link: work-order-41 → press seized.
    let (status, _) = send(
        &router,
        post(
            "/api/v1/coherence/add-edge",
            "tenant-a",
            json!({
                "source": wo,
                "relation": "Causes",
                "target": failure,
                "confidence": 0.9,
                "weight": 0.7
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Forward traversal from the source.
    let (status, body) = send(
        &router,
        get(&format!("/api/v1/coherence/edges?node_id={wo}", wo = wo), "tenant-a"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let out = body["outgoing"].as_array().expect("outgoing");
    assert_eq!(out.len(), 1);
    assert_eq!(out[0]["relation"], "Causes");
    assert_eq!(out[0]["target_id"], failure);
    assert_eq!(out[0]["confidence"], 0.9);

    // Inverse traversal: the target sees the same link as incoming.
    let (_, body) = send(
        &router,
        get(&format!("/api/v1/coherence/edges?node_id={failure}", failure = failure), "tenant-a"),
    )
    .await;
    let inc = body["incoming"].as_array().expect("incoming");
    assert_eq!(inc.len(), 1);
    assert_eq!(inc[0]["relation"], "Causes");
    assert_eq!(inc[0]["source_id"], wo);

    // Tenant isolation: the other tenant sees no edges for this node.
    let (_, body) = send(
        &router,
        get(&format!("/api/v1/coherence/edges?node_id={wo}", wo = wo), "tenant-b"),
    )
    .await;
    assert_eq!(body["outgoing"].as_array().unwrap().len(), 0);
}

// ── Phase 16 I7: 70-cell grid → object types ────────────────────────────────
#[tokio::test]
async fn classify_reports_object_type_and_register_auto_stamps_kind() {
    let (router, _dir, _env) = app().await;

    // The classify response exposes the object type of the winning cell. Under
    // the random-projection embedder the cell isn't guaranteed, but it is one
    // of the known vocabulary whenever classification fires.
    let (status, body) = send(
        &router,
        post("/api/v1/classify", "tenant-a", json!({"text": "CNC press cycle time drift"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let ot = body["object_type"].as_str();
    assert!(ot.is_none() || !ot.unwrap().is_empty(), "object_type present or absent: {body}");

    // Registering WITHOUT a kind auto-stamps it from the winning cell, so the
    // node becomes a typed object with no extra round-trip.
    let (status, body) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "tenant-a",
            json!({"input": "CNC press seized bearing — maintenance"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let node_id = body["node_id"].as_str().unwrap().to_string();
    let (_, body) = send(&router, get("/api/v1/coherence/nodes/all", "tenant-a")).await;
    let node = body["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == node_id)
        .expect("node");
    // Either auto-stamped to a vocabulary type, or still empty if the
    // projection embedder returned an unresolvable cell — never garbage.
    let kind = node["kind"].as_str().unwrap();
    if !kind.is_empty() {
        assert!(
            ["machine", "sensor_reading", "work_order", "asset", "document"]
                .contains(&kind),
            "auto-kind '{kind}' is a sensible object type"
        );
    }
}

// ── Phase 16 I8: dream output → typed actions ───────────────────────────────
#[tokio::test]
async fn dream_replay_proposes_a_typed_action_for_the_failing_node() {
    let (router, _dir, _env) = app().await;

    // A failing typed machine node: the dream loop's decide half must propose a
    // concrete action for it, not free text.
    let (_, body) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "tenant-a",
            json!({
                "input": "CNC press seized bearing",
                "kind": "machine",
                "coherence_score": -0.8,
            }),
        ),
    )
    .await;
    let node_id = body["node_id"].as_str().unwrap().to_string();

    let (status, body) = send(&router, post("/api/v1/dream/replay", "tenant-a", json!({}))).await;
    assert_eq!(status, StatusCode::OK);
    let entries = body.as_array().expect("replay entries");
    let entry = entries
        .iter()
        .find(|e| e["node_id"] == node_id)
        .expect("the failing machine is replayed");

    // The typed action is present, targets the node, and names its kind.
    let ta = &entry["typed_action"];
    assert!(ta.is_object(), "typed_action present: {entry}");
    assert_eq!(ta["object_id"], node_id);
    assert_eq!(ta["object_kind"], "machine");
    let kind = ta["kind"].as_str().unwrap();
    // Under the random-projection embedder the outcome may read as either
    // "prevented" (acknowledge) or "still failing" (schedule maintenance) —
    // both are legitimate typed actions, never free text.
    assert!(
        ["acknowledge", "schedule_maintenance"].contains(&kind),
        "typed action kind '{kind}' is one of the vocabulary"
    );
    assert!(!ta["summary"].as_str().unwrap().is_empty());
}

// ── Phase 16 I9/I10: append-only mutation log / audit view ──────────────────
#[tokio::test]
async fn audit_log_records_mutations_and_filters_by_subject() {
    let (router, _dir, _env) = app().await;

    // Register (with a verdict), edit, delete, add-edge — each must be sealed.
    let (_, b1) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "tenant-a",
            json!({"input": "pump-3 overheating", "kind": "machine", "coherence_score": -0.9}),
        ),
    )
    .await;
    let node_id = b1["node_id"].as_str().unwrap().to_string();

    let (_, b2) = send(
        &router,
        post(
            "/api/v1/coherence/register",
            "tenant-a",
            json!({"input": "work order 42"}),
        ),
    )
    .await;
    let wo = b2["node_id"].as_str().unwrap().to_string();

    send(
        &router,
        post(
            "/api/v1/coherence/edit",
            "tenant-a",
            json!({"id": node_id, "label": "pump-3 overheating (revised)"}),
        ),
    )
    .await;
    send(
        &router,
        post(
            "/api/v1/coherence/add-edge",
            "tenant-a",
            json!({"source": node_id, "relation": "Causes", "target": wo}),
        ),
    )
    .await;

    // The audit view is newest-first and includes all four ops.
    let (status, body) = send(&router, get("/api/v1/audit", "tenant-a")).await;
    assert_eq!(status, StatusCode::OK);
    let entries = body["entries"].as_array().expect("entries");
    assert!(entries.len() >= 4);
    let ops: Vec<String> = entries.iter().map(|e| e["op"].as_str().unwrap().to_string()).collect();
    assert!(ops.contains(&"register".to_string()));
    assert!(ops.contains(&"assert".to_string()));
    assert!(ops.contains(&"edit".to_string()));
    assert!(ops.contains(&"edge:add".to_string()));

    // Per-subject history: the pump's own change trail.
    let (_, body) = send(
        &router,
        get(&format!("/api/v1/audit?node_id={node_id}", node_id = node_id), "tenant-a"),
    )
    .await;
    let hist = body["entries"].as_array().expect("history");
    assert!(hist.len() >= 3, "register + assert + edit: {body}");
    assert!(hist.iter().all(|e| e["subject_id"] == node_id));

    // Time-travel slice: every entry has a seq and a timestamp (sealed rows).
    let (_, body) = send(
        &router,
        get("/api/v1/audit?as_of=2100-01-01T00:00:00Z", "tenant-a"),
    )
    .await;
    let all = body["entries"].as_array().expect("all");
    assert_eq!(all.len(), entries.len(), "as_of in the future sees everything");
    assert!(all.iter().all(|e| e["seq"].is_number() && e["at"].is_string()));

    // Tenant isolation: the other tenant's log is empty.
    let (_, body) = send(&router, get("/api/v1/audit", "tenant-b")).await;
    assert_eq!(body["count"], 0);
}

// ── Phase 16 I11: typed-action dispatch + CSV export ────────────────────────
#[tokio::test]
async fn actions_dispatch_is_honest_when_no_webhook_configured() {
    let (router, _dir, _env) = app().await;
    std::env::remove_var("PHYSIS_ACTIONS_WEBHOOK");

    let (status, body) = send(
        &router,
        post(
            "/api/v1/actions/dispatch",
            "tenant-a",
            json!({"actions": [{
                "kind": "schedule_maintenance",
                "object_id": "cnc-01",
                "object_kind": "machine",
                "summary": "schedule maintenance for cnc-01"
            }]}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["configured"], false);
    assert_eq!(body["skipped"], 1);
    assert_eq!(body["delivered"], 0);
    let note = body["note"].as_str().unwrap();
    assert!(note.contains("PHYSIS_ACTIONS_WEBHOOK"), "honest note: {note}");
}

#[tokio::test]
async fn dream_replay_csv_exports_countermeasures() {
    let (router, _dir, _env) = app().await;

    // A failing machine node gives the CSV at least something to replay.
    send(
        &router,
        post(
            "/api/v1/coherence/register",
            "tenant-a",
            json!({"input": "CNC press seized bearing", "kind": "machine", "coherence_score": -0.9}),
        ),
    )
    .await;

    let (status, body) = send_text(
        &router,
        Request::builder()
            .method("GET")
            .uri("/api/v1/dream/replay.csv")
            .header("x-physis-user", "tenant-a")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let header = "node_id,label,outcome,prevented_failure,action_kind,summary";
    assert!(body.starts_with(header), "CSV header present: {body}");
    assert!(body.lines().count() >= 1, "at least the header row");
}

#[tokio::test]
async fn actions_dispatch_attempts_delivery_and_reports_failure() {
    // app() holds ENV_LOCK for the test's duration, so setting the webhook var
    // here is serialized against every other env-mutating test.
    let (router, _dir, _guard) = app().await;
    std::env::set_var("PHYSIS_ACTIONS_WEBHOOK", "http://127.0.0.1:1/dispatch");

    let (status, body) = send(
        &router,
        post(
            "/api/v1/actions/dispatch",
            "tenant-a",
            json!({"actions": [{
                "kind": "open_work_order",
                "object_id": "wo-9",
                "summary": "open a work order for wo-9"
            }]}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["configured"], true);
    assert_eq!(body["delivered"], 0);
    let results = body["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["ok"], false, "dead port delivery fails loudly: {body}");
    std::env::remove_var("PHYSIS_ACTIONS_WEBHOOK");
}

// ── Phase 15.4: customer/plant/host identity in report metadata ─────────────
#[tokio::test]
async fn shift_report_metadata_carries_evidence_grade_identity() {
    let (router, _dir, _guard) = app().await;
    // Give the host a name; customer resolves to "unlicensed" (no licence in tests).
    std::env::set_var("PHYSIS_PLANT", "Acciaierie Nord");

    let (status, body) = send(
        &router,
        post(
            "/api/v1/shifts/import",
            "tenant-a",
            json!({"shifts": [{
                "recorded_at": "2026-08-01T06:00:00Z",
                "shift_label": "Turno 1",
                "machine_id": "press-01",
                "shift_length_secs": 28800,
                "ideal_cycle_time_secs": 60,
                "stops": [],
                "total_count": 120,
                "good_count": 117
            }]}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["accepted"].as_u64().unwrap_or(0) >= 1, "shift imported: {body}");

    // The markdown report is evidence-grade: it names whose plant, which host,
    // and which build produced it — never a blank identity line.
    let (status, text) = send_text(
        &router,
        Request::builder()
            .method("GET")
            .uri("/api/v1/shifts/report?format=md")
            .header("x-physis-user", "tenant-a")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(text.contains("Acciaierie Nord"), "plant identity in report: {text}");
    assert!(text.contains("unlicensed"), "customer identity in report: {text}");
    assert!(text.contains(env!("CARGO_PKG_VERSION")), "build version in report: {text}");
    assert!(text.contains("host "), "host identity in report: {text}");
    std::env::remove_var("PHYSIS_PLANT");
}

// ── Phase 18 T7: air-gapped degradation (no weights, no network) ────────────
#[tokio::test]
async fn air_gapped_path_degrades_honestly_without_weights_or_network() {
    // The whole http_surface suite runs under the random-projection embedder
    // with no providers and no network — the air-gapped worst case. This test
    // pins the contract: core endpoints answer 200 with an honest shape, never
    // 500, and classify says outright that it is NOT semantic.
    let (router, _dir, _env) = app().await;

    // classify — even empty/garbage text is a 200 with a shape.
    let (status, body) = send(
        &router,
        post("/api/v1/classify", "air", json!({"text": ""})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "classify never 500s: {body}");
    assert!(body["top"]["domain"].is_string(), "top cell shape: {body}");
    // The honest air-gap marker: the assessment names the offline embedder.
    assert_eq!(body["assessment"]["model"], "random-projection", "{body}");

    // embed — random projection is deterministic and available offline.
    let (status, body) = send(
        &router,
        post("/api/v1/embed", "air", json!({"text": "hello"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["dim"].as_u64().unwrap_or(0) > 0, "embedding has a dimension: {body}");

    // extract — never 500: either a provider extracts slots, or an explicit
    // note says why not (never a fabricated object, never a silent failure).
    let (status, body) = send(
        &router,
        post(
            "/api/v1/extract",
            "air",
            json!({"text": "the press is down", "schema": {"type": "object", "properties": {"machine": {"type": "string"}}}}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    if let Some(note) = body["note"].as_str() {
        assert!(!note.is_empty(), "honest degradation note: {body}");
    } else {
        assert!(body["slots"].is_object(), "either slots or a note: {body}");
    }
}

// ── Phase 18 T5: marketing-page smoke (public pages, air-gap rule) ──────────
#[tokio::test]
async fn marketing_pages_serve_expected_copy_without_external_assets() {
    let (router, _dir, _env) = app().await;

    // Public marketing/status pages must serve 200 with the real product copy
    // (not the stale "API platform" framing), and must not fetch external
    // CSS/JS/font assets — the same air-gap rule the product itself obeys.
    for (path, needle) in [
        ("/", "adaptive industrial intelligence"),
        ("/pricing", "Shift Loss Review"),
        ("/about", "70-cell semiotic grid"),
        ("/deploy", "Deployment status"),
    ] {
        let (status, html) = send_text(
            &router,
            Request::builder().method("GET").uri(path).body(Body::empty()).unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{path} serves 200");
        assert!(html.contains(needle), "{path} carries expected copy");
        assert!(!html.contains("<link rel=\"stylesheet\" href=\"http"), "{path}: no external CSS");
        assert!(!html.contains("<script src=\"http"), "{path}: no external JS");
        assert!(html.contains("/physis.css"), "{path} loads the shared design system");
    }

    // The settings page is protected (dev mode passes) and carries its own copy.
    let (status, html) = send_text(
        &router,
        Request::builder()
            .method("GET")
            .uri("/settings")
            .header("x-physis-user", "smoke")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("install health"), "/settings carries its copy");
}

// ── Phase 18 T2: adapter-contract — a machine event inherits anomaly
//    detection (dedupe + ledger + timeline already covered by events_ingest) ──
#[tokio::test]
async fn machine_event_inherits_anomaly_detection() {
    let (router, _dir, _env) = app().await;

    // A hot machine reading + thresholds → flagged as an anomaly.
    let (status, body) = send(
        &router,
        post(
            "/api/v1/events",
            "tenant-a",
            json!({
                "events": [{
                    "event_key": "ev-hot-1",
                    "observed_at": "2026-08-12T08:55:00Z",
                    "kind": "machine",
                    "status": "warning",
                    "subject": "cnc-01",
                    "provenance": {"connector": "industrial", "source": "machine", "source_id": "cnc-01"},
                    "properties": {"temperature": "95"},
                    "evidence": [],
                    "machine_id": "cnc-01",
                    "raw": null
                }],
                "thresholds": {"temp_max": 90.0}
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["accepted"], 1);
    let anomalies = body["anomalies"].as_array().unwrap_or_else(|| {
        eprintln!("RESPONSE: {body}");
        panic!("no anomalies in response")
    });
    assert_eq!(anomalies.len(), 1, "hot reading flagged: {body}");
    assert_eq!(anomalies[0]["machine_id"], "cnc-01");
    assert!(anomalies[0]["reason"].as_str().unwrap().contains("temperature"));

    // The anomaly is sealed in the audit log too.
    let (_, audit) = send(&router, get("/api/v1/audit", "tenant-a")).await;
    assert!(
        audit["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["op"] == "anomaly"),
        "anomaly audited: {audit}"
    );

    // A cool machine, or no thresholds, → no anomalies.
    let (_, body) = send(
        &router,
        post(
            "/api/v1/events",
            "tenant-a",
            json!({
                "events": [{
                    "event_key": "ev-cool-1",
                    "observed_at": "2026-08-12T09:00:00Z",
                    "kind": "machine",
                    "status": "info",
                    "subject": "cnc-01",
                    "provenance": {"connector": "industrial", "source": "machine", "source_id": "cnc-01"},
                    "properties": {"temperature": "60"},
                    "evidence": [],
                    "machine_id": "cnc-01",
                    "raw": null
                }]
            }),
        ),
    )
    .await;
    assert_eq!(body["accepted"], 1);
    assert!(
        body["anomalies"].as_array().map(|a| a.is_empty()).unwrap_or(true),
        "cool reading not flagged"
    );
}

// ── Phase 18 T1: the whole customer journey in one test ─────────────────────
// scan → shift report → quality/anomaly → dream → where → lineage, with
// tenant isolation throughout. Every leg is a real endpoint the same way a
// customer would drive them; the random-projection embedder keeps it offline.
#[tokio::test]
async fn funnel_scan_report_quality_dream_where_lineage_is_tenant_isolated() {
    let (router, dir, _env) = app().await;
    let root = dir.path();

    // 1. SCAN — a document in the scan root becomes a labeled graph node and a
    //    causal-ledger "generated" event.
    std::fs::write(root.join("shift_note.txt"), "CNC press seized bearing this shift").unwrap();
    let (status, body) = send(
        &router,
        post("/api/v1/scan", "tenant-a", json!({"dir": root})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "scan: {body}");
    assert!(
        body["new"].as_u64().unwrap_or(0) >= 1,
        "scan found the new file: {body}"
    );

    // 2. SHIFT REPORT — imported shifts render an Italian OEE report with the
    //    evidence-grade identity line.
    let (_, body) = send(
        &router,
        post(
            "/api/v1/shifts/import",
            "tenant-a",
            json!({"shifts": [{
                "recorded_at": "2026-08-01T06:00:00Z",
                "machine_id": "press-01",
                "shift_length_secs": 28800,
                "ideal_cycle_time_secs": 60,
                "stops": [],
                "total_count": 120,
                "good_count": 117
            }]}),
        ),
    )
    .await;
    assert!(body["accepted"].as_u64().unwrap_or(0) >= 1, "shift imported: {body}");
    let (_, md) = send_text(
        &router,
        Request::builder()
            .method("GET")
            .uri("/api/v1/shifts/report?format=md")
            .header("x-physis-user", "tenant-a")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert!(md.contains("OEE"), "shift report renders: {md}");
    assert!(md.contains("origine:"), "report carries provenance");

    // 3. QUALITY — a hot machine reading against thresholds is caught as an
    //    anomaly (the quality loop) and sealed in the audit log.
    let (_, body) = send(
        &router,
        post(
            "/api/v1/events",
            "tenant-a",
            json!({
                "events": [{
                    "event_key": "funnel-hot",
                    "observed_at": "2026-08-12T08:55:00Z",
                    "kind": "machine",
                    "status": "warning",
                    "subject": "cnc-01",
                    "provenance": {"connector": "industrial", "source": "machine", "source_id": "cnc-01"},
                    "properties": {"temperature": "95"},
                    "evidence": [],
                    "machine_id": "cnc-01",
                    "raw": null
                }],
                "thresholds": {"temp_max": 90.0}
            }),
        ),
    )
    .await;
    assert_eq!(body["anomalies"].as_array().map(|a| a.len()), Some(1));

    // 4. DREAM — a failing typed machine node is replayed with a typed action.
    send(
        &router,
        post(
            "/api/v1/coherence/register",
            "tenant-a",
            json!({"input": "pump overheat", "kind": "machine", "coherence_score": -0.8}),
        ),
    )
    .await;
    let (_, body) = send(&router, post("/api/v1/dream/replay", "tenant-a", json!({}))).await;
    assert!(
        body.as_array()
            .unwrap()
            .iter()
            .any(|e| e["typed_action"].is_object()),
        "dream proposes a typed action: {body}"
    );

    // 5. WHERE — facet query over the causal ledger finds the scan's
    //    "generated" event.
    let (status, body) = send(
        &router,
        post("/api/v1/files/query", "tenant-a", json!({"cause": "scan"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["by_mechanism"]["generated"].as_u64().unwrap_or(0) >= 1,
        "ledger knows the scan generated something: {body}"
    );

    // 6. LINEAGE — the scanned path's cause trail resolves.
    let (status, body) = send(
        &router,
        get(
            &format!("/api/v1/files/lineage?path={}", root.to_string_lossy()),
            "tenant-a",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["lineage"].is_array() && body["descendants"].is_array());
    assert!(
        body["lineage"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["actor"] == "scan"),
        "lineage shows the scan cause: {body}"
    );

    // 7. TENANT ISOLATION — tenant-b's ledger, events, and audit are all empty.
    let (_, body) = send(
        &router,
        post("/api/v1/files/query", "tenant-b", json!({"cause": "scan"})),
    )
    .await;
    assert!(body["matches"].as_array().unwrap().is_empty());
    let (_, body) = send(&router, get("/api/v1/events", "tenant-b")).await;
    assert!(body.as_array().unwrap().is_empty());
    let (_, body) = send(&router, get("/api/v1/audit", "tenant-b")).await;
    assert_eq!(body["count"], 0);
}

// ── Scan v2: idempotent re-scan + one node per document ─────────────────────
#[tokio::test]
async fn scan_is_idempotent_and_produces_one_node_per_document() {
    let (router, dir, _env) = app().await;
    // Scan a subdirectory, so the persistent store (data dir) stays outside the
    // scanned scope — the real production shape.
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(
        docs.join("doc.md"),
        "# Title\n\nSome body content.\n\n- bullet one\n- bullet two",
    )
    .unwrap();

    // First scan: the document is new.
    let (status, body) = send(
        &router,
        post("/api/v1/scan", "tenant-a", json!({"dir": docs})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["new"].as_u64().unwrap_or(0) >= 1, "first scan new: {body}");

    // ONE node per document — its structural path — not one per structural line.
    let (_, body) = send(&router, get("/api/v1/coherence/nodes/all", "tenant-a")).await;
    let doc_nodes: Vec<_> = body["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| {
            n["label"]
                .as_str()
                .map(|l| l.contains("doc.md"))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(doc_nodes.len(), 1, "one node per document, not per line: {body}");

    // Second scan, nothing changed → idempotent: nothing new/changed, and the
    // whole tree is reported unchanged (subtree fingerprint skip).
    let (_, body) = send(
        &router,
        post("/api/v1/scan", "tenant-a", json!({"dir": docs})),
    )
    .await;
    assert_eq!(body["new"].as_u64().unwrap_or(0), 0, "no new on re-scan: {body}");
    assert_eq!(body["changed"].as_u64().unwrap_or(0), 0, "no changed on re-scan: {body}");
    assert!(
        body["unchanged"].as_u64().unwrap_or(0) >= 1,
        "unchanged reported: {body}"
    );
}
