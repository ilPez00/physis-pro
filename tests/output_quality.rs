//! Output-quality probe (Phase 10c) — qualitative inspection of the new
//! surfaces under a REAL embedder. `#[ignore]`d like `accuracy_floor` because
//! it needs the ONNX model on disk; run with:
//!
//! ```bash
//! cargo test --features full --test output_quality -- --ignored --nocapture
//! ```
//!
//! This is not an assertion suite: it prints what a user would actually see —
//! top cells with entry names, recall rankings, ledger facets, lineage chains,
//! purpose links, mail timestamps — so quality regressions are visible to a
//! human, not just shape-checked. The HTTP surfaces run under the same
//! in-process router the http_surface tests use.
//!
//! Gated behind `embed-onnx`: this probe imports `embed::onnx`, which only
//! exists with that feature, so compiling it into a plain `--features web`
//! test run failed the whole suite. It is a real-embedder probe, not a shape
//! check — it should only ever build where the model path is available.

#![cfg(feature = "embed-onnx")]
#![allow(dead_code)]

use serde_json::Value;
use tokio::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

fn real_embedder() -> Option<(Box<dyn physis_pro::embed::VectorEmbed>, &'static str)> {
    use physis_pro::config::{OnnxConfig, PoolingStrategy};
    use physis_pro::embed::onnx::OnnxEmbedder;
    let bge = OnnxEmbedder::with_config(&OnnxConfig {
        enabled: true,
        model_dir: Some("models/bge-base-en-v1.5".to_string()),
        dim: 768,
        max_length: 512,
        pooling: PoolingStrategy::Mean,
    });
    if bge.is_available() {
        return Some((Box::new(bge), "bge-base-en-v1.5"));
    }
    None
}

async fn router_on(dir: &std::path::Path) -> axum::Router {
    let _env = ENV_LOCK.lock().await;
    // No PHYSIS_EMBEDDER override: build_app runs the same ONNX cascade as
    // production, so recall/ask scores below are semantic, not RP artifacts.
    std::env::remove_var("PHYSIS_EMBEDDER");
    std::env::remove_var("PHYSIS_API_TOKEN");
    std::env::set_var("PHYSIS_SCAN_ROOT", dir);
    std::env::set_var("PHYSIS_DEV", "1");
    std::env::set_var("PHYSIS_DATA_DIR", dir);
    let (_state, router) = physis_pro::web::build_app().await.expect("build_app");
    router
}

async fn send(
    router: &axum::Router,
    method: &str,
    path: &str,
    user: &str,
    body: Option<Value>,
) -> (axum::http::StatusCode, Value) {
    use axum::body::Body;
    use axum::http::Request;
    let mut b = Request::builder().method(method).uri(path);
    if let Some(_json) = &body {
        b = b.header("content-type", "application/json");
    }
    let req = b
        .header("x-physis-user", user)
        .body(Body::from(body.map(|j| j.to_string()).unwrap_or_default()))
        .unwrap();
    let req = {
        let mut r = req;
        r.extensions_mut()
            .insert(axum::extract::ConnectInfo::<std::net::SocketAddr>(
                std::net::SocketAddr::from(([127, 0, 0, 1], 4242)),
            ));
        r
    };
    let resp = router.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap_or_default();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
#[ignore = "needs a real embedder model on disk; qualitative probe"]
async fn classification_reach_quality() {
    let Some((embedder, kind)) = real_embedder() else {
        eprintln!("SKIP: no real model on disk");
        return;
    };
    eprintln!("\n═══ 1. classification reach ({kind}) ═══");
    let ontology =
        physis_pro::config::OntologyLoader::load_all(&physis_pro::config::PhysisConfig::default());
    let clf = physis_pro::classify::CellClassifier::build(&ontology, embedder.as_ref());
    eprintln!(
        "seeds scoreable: {}",
        ontology.classification_domains().count()
    );

    let queries = [
        (
            "OEE dropped due to unplanned downtime on the CNC press",
            "machine",
        ),
        ("invoice approval workflow overdue in procurement", "office"),
        (
            "agent tool invocation failed, retrying with exponential backoff",
            "agent",
        ),
        (
            "hydraulic press tonnage drifted beyond tolerance",
            "machine",
        ),
        (
            "a serene landscape painting with soft brush strokes",
            "human/arts control",
        ),
    ];
    for (q, expect) in queries {
        let emb = embedder.embed(q);
        let top3 = clf.classify(&emb);
        let lines: Vec<String> = top3
            .iter()
            .take(3)
            .map(|c| {
                let names: Vec<&str> = c.entries.iter().take(2).map(|e| e.as_str()).collect();
                format!(
                    "{}×{} {:.3} [{}]",
                    c.domain,
                    c.mode,
                    c.score,
                    names.join(", ")
                )
            })
            .collect();
        eprintln!("{q}\n   expects {expect}\n   → {}", lines.join("\n   → "));
    }
}

#[tokio::test]
#[ignore = "needs a real embedder model on disk; qualitative probe"]
async fn memory_and_ledger_quality() {
    let Some((_embedder, kind)) = real_embedder() else {
        eprintln!("SKIP: no real model on disk");
        return;
    };
    eprintln!("\n═══ 2. remember → recall → why → serve → causes ({kind}) ═══");
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let vault = root.join("vault");
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::write(
        vault.join("press-log.md"),
        "# Press log\n\ntonage drift on line 2",
    )
    .unwrap();

    let router = router_on(root).await;

    // Remember three facts with distinct topics.
    for fact in [
        "the hydraulic press on line 2 drifts above nominal tonnage when cold",
        "quarterly supplier audit for bearings is scheduled in September",
        "the espresso machine descaling routine runs every 300 shots",
    ] {
        let (s, j) = send(
            &router,
            "POST",
            "/api/v1/coherence/register",
            "qz",
            Some(serde_json::json!({"input": fact})),
        )
        .await;
        assert_eq!(s, axum::http::StatusCode::OK, "{j}");
    }

    // Recall: does the right fact rank first?
    for q in [
        "why does the press lose tonnage?",
        "when is the bearing audit?",
    ] {
        let (_, j) = send(
            &router,
            "POST",
            "/api/v1/search/nodes",
            "qz",
            Some(serde_json::json!({"query": q, "top_k": 3})),
        )
        .await;
        let hits: Vec<String> = j["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                format!(
                    "{:.3} {}",
                    r["score"].as_f64().unwrap_or(0.0),
                    r["label"].as_str().unwrap_or("")
                )
            })
            .collect();
        eprintln!("recall {q:?}\n   → {}", hits.join("\n   → "));
    }

    // Ask without a provider: degradation note + matches.
    let (_, ask) = send(
        &router,
        "POST",
        "/api/v1/ask",
        "qz",
        Some(serde_json::json!({"question": "what do I know about the hydraulic press?"})),
    )
    .await;
    match ask["answer"].as_str() {
        Some(a) => eprintln!(
            "ask via {}: {:?}",
            ask["provider"].as_str().unwrap_or("?"),
            a
        ),
        None => eprintln!(
            "ask degraded: note={:?} results={}",
            ask["note"].as_str().unwrap_or(""),
            ask["results"].as_array().map(|a| a.len()).unwrap_or(0)
        ),
    }

    // Import the vault → causal event; then lineage + serve + causes.
    let vault_str = vault.display().to_string();
    let (_, v) = send(
        &router,
        "POST",
        "/api/v1/vault/import",
        "qz",
        Some(serde_json::json!({"dir": vault_str, "git": 0})),
    )
    .await;
    eprintln!(
        "vault import: registered={} deduped={}",
        v["registered"], v["deduped"]
    );

    let (_, g) = send(
        &router,
        "POST",
        "/api/v1/goals",
        "qz",
        Some(serde_json::json!({"prompt": "keep the presses running"})),
    )
    .await;
    let goal_id = g["goal"]["id"].as_str().unwrap().to_string();
    let (_, s) = send(
        &router,
        "POST",
        "/api/v1/files/serve",
        "qz",
        Some(serde_json::json!({"object": vault_str, "goal_id": goal_id})),
    )
    .await;
    eprintln!("serve: recorded.mechanism={}", s["recorded"]["mechanism"]);

    let enc = urlencode(&vault_str);
    let (_, lin) = send(
        &router,
        "GET",
        &format!("/api/v1/files/lineage?path={enc}"),
        "qz",
        None,
    )
    .await;
    eprintln!(
        "lineage of {}: {} upstream, {} purposes",
        lin["path"]
            .as_str()
            .unwrap_or("")
            .split('/')
            .next_back()
            .unwrap_or("?"),
        lin["lineage"].as_array().map(|a| a.len()).unwrap_or(0),
        lin["purposes"].as_array().map(|a| a.len()).unwrap_or(0)
    );

    let (_, q) = send(
        &router,
        "POST",
        "/api/v1/files/query",
        "qz",
        Some(serde_json::json!({})),
    )
    .await;
    eprintln!("ledger rollups: by_mechanism={:?}", q["by_mechanism"]);
    for m in q["matches"].as_array().unwrap().iter().take(5) {
        eprintln!(
            "   {} {} {} ← {} ({})",
            m["mechanism"].as_str().unwrap_or(""),
            m["at"].as_str().unwrap_or("").get(0..16).unwrap_or(""),
            m["object"]
                .as_str()
                .unwrap_or("")
                .split('/')
                .next_back()
                .unwrap_or(""),
            m["input"].as_str().unwrap_or("—"),
            m["actor"].as_str().unwrap_or("")
        );
    }
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

use tower::util::ServiceExt;
