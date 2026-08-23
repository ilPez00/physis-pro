//! Agent orchestration surface: the `/agent` dashboard backend.
//!
//! Physis is the *host/orchestrator*. The LLM brain is the provider cascade
//! (`src/ai/provider.rs`); the agent loop is the ReAct implementation in
//! `src/ai/agent.rs`; the "hands" are tools. One tool — `opencode` — delegates
//! real file editing / builds to OpenCode running as a *sandboxed subprocess*,
//! so the agent can plan, then hand concrete coding work to a dedicated coding
//! engine. See `src/bin/ui/agent.html` for the matching dashboard.
//!
//! Endpoints added in `build_app`:
//!   GET  /agent               → the orchestration dashboard (HTML)
//!   GET  /api/v1/agent/caps   → agent capabilities (tools, opencode bin/model)
//!   POST /api/v1/agent/run    → SSE stream of AgentEvents for one run
//!   POST /api/v1/agent/abort  → cancel a running agent (kills OpenCode child)

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::sse::{Event, Sse};
use axum::response::Html;
use axum::Json;
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ai::agent::{AgentConfig, AgentEvent, EventSink};
use crate::ai::tools::ToolRegistry;
use crate::rag::{self, RagCorpus};
use crate::web::{user_key, SharedState};

/// Per-run cancellation + OpenCode child handle, keyed by run id. The child is
/// behind an `Arc<Mutex>` so the run task and the abort handler can both reach it.
struct RunHandle {
    cancelled: Arc<AtomicBool>,
    child: Arc<Mutex<Option<tokio::process::Child>>>,
    /// Tenant that started the run. Abort is keyed on a run id alone, which is
    /// unguessable but not secret — it is echoed to the client and appears in
    /// logs — so the owner is carried here and checked before any kill.
    owner: String,
}

/// Shared registry of live runs, so `/agent/abort` can stop a run.
pub(crate) struct AgentRunRegistry {
    runs: Mutex<HashMap<String, Arc<RunHandle>>>,
}

impl AgentRunRegistry {
    pub(crate) fn new() -> Self {
        Self {
            runs: Mutex::new(HashMap::new()),
        }
    }

    fn insert(&self, id: &str, owner: &str) -> Arc<RunHandle> {
        let h = Arc::new(RunHandle {
            cancelled: Arc::new(AtomicBool::new(false)),
            child: Arc::new(Mutex::new(None)),
            owner: owner.to_string(),
        });
        self.runs.lock().unwrap().insert(id.to_string(), h.clone());
        h
    }

    fn remove(&self, id: &str) {
        self.runs.lock().unwrap().remove(id);
    }

    /// Look up a run *for a given tenant*. Taking the caller here rather than
    /// returning the handle and trusting every call site to compare makes the
    /// authorization impossible to skip: an unknown id and someone else's id
    /// are both simply `None`, so the handler cannot distinguish them and no
    /// probe learns whether a given run exists.
    fn get_owned(&self, id: &str, owner: &str) -> Option<Arc<RunHandle>> {
        self.runs
            .lock()
            .unwrap()
            .get(id)
            .filter(|h| h.owner == owner)
            .cloned()
    }
}

// ── Request / response types ────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AgentRunRequest {
    /// The user's prompt / goal for the agent.
    prompt: String,
    /// Which tools the agent may use. Empty = all available.
    #[serde(default)]
    tools: Vec<String>,
    /// Reasoning mode hint passed to the agent loop.
    #[serde(default)]
    mode: String,
    /// Optional override model for the orchestrator LLM.
    #[serde(default)]
    model: Option<String>,
    /// Base64 image frame (optional, for vision-capable providers).
    #[serde(default)]
    frame_b64: Option<String>,
}

#[derive(Serialize)]
pub struct AgentToolInfo {
    name: String,
    description: String,
    enabled: bool,
}

#[derive(Serialize)]
pub struct AgentCapabilities {
    tools: Vec<AgentToolInfo>,
    opencode_bin: Option<String>,
    opencode_model: String,
    max_rounds: u32,
    provider_count: usize,
    semantic: bool,
    /// Whether answers are judged by the coherence harness before display, and
    /// the fitness floor used. The dashboard needs this to tell "gate is off"
    /// apart from "gate ran and said nothing", which look identical otherwise.
    harness_gate: bool,
    harness_threshold: f32,
}

// ── SSE event helper ─────────────────────────────────────────────────────────

fn sse_event(ev: &AgentEvent) -> Event {
    let payload = serde_json::json!({
        "round": ev.round,
        "kind": ev.kind,
        "text": ev.text,
    });
    Event::default()
        .event(ev.kind.clone())
        .data(serde_json::to_string(&payload).unwrap_or_default())
}

// ── GET /agent ───────────────────────────────────────────────────────────────

pub async fn agent_ui_handler() -> Html<&'static str> {
    Html(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bin/ui/agent.html")))
}

// ── GET /api/v1/agent/caps ──────────────────────────────────────────────────

pub async fn agent_capabilities_handler(
    State(state): State<SharedState>,
) -> Json<AgentCapabilities> {
    let cfg = &state.shared.config.agent;
    let opencode_enabled = cfg.opencode_bin.as_ref().map(|p| p.exists()).unwrap_or(false);
    let tools = vec![
        AgentToolInfo {
            name: "graph_search".into(),
            description: "Search the coherence + goal knowledge graph by text.".into(),
            enabled: true,
        },
        AgentToolInfo {
            name: "node_search".into(),
            description: "Token-bounded similarity search over graph nodes.".into(),
            enabled: true,
        },
        AgentToolInfo {
            name: "classify".into(),
            description: "Classify text onto the semiotic grid (domain × mode).".into(),
            enabled: true,
        },
        AgentToolInfo {
            name: "coherence_register".into(),
            description: "Register a new labeled node in the knowledge graph.".into(),
            enabled: true,
        },
        AgentToolInfo {
            name: "coherence_edit".into(),
            description: "Edit an existing graph node's content (keeps structure).".into(),
            enabled: true,
        },
        AgentToolInfo {
            name: "dream".into(),
            description: "Run a coherence/dream cycle for the active goals.".into(),
            enabled: true,
        },
        AgentToolInfo {
            name: "opencode".into(),
            description: "Delegate a coding task to OpenCode (sandboxed subprocess).".into(),
            enabled: opencode_enabled,
        },
    ];
    Json(AgentCapabilities {
        tools,
        opencode_bin: cfg.opencode_bin.as_ref().map(|p| p.display().to_string()),
        opencode_model: cfg.opencode_model.clone(),
        max_rounds: cfg.max_rounds,
        provider_count: state.shared.cascade.provider_names().len(),
        semantic: state.shared.semantic,
        harness_gate: cfg.harness_gate,
        harness_threshold: cfg.harness_threshold,
    })
}

// ── POST /api/v1/agent/abort ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AgentAbortRequest {
    run_id: String,
}

pub async fn agent_abort_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<AgentAbortRequest>,
) -> Json<serde_json::Value> {
    // Abort kills a subprocess, so it needs the same tenant identity the run
    // was started under. A run id is unguessable but not a secret — it is
    // returned to the client and lands in logs — so it authenticates nothing
    // on its own.
    let uid = user_key(&headers);
    if let Some(handle) = state.agent_runs.get_owned(&req.run_id, &uid) {
        handle.cancelled.store(true, Ordering::SeqCst);
        // Kill any live OpenCode child so a hung build does not outlive the run.
        if let Ok(mut guard) = handle.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.start_kill();
            }
        }
        Json(serde_json::json!({ "ok": true, "aborted": req.run_id }))
    } else {
        // Deliberately one answer for "no such run" and "not yours", so the
        // response cannot be used to probe for other tenants' run ids.
        Json(serde_json::json!({ "ok": false, "reason": "unknown run id" }))
    }
}

// ── POST /api/v1/agent/run (SSE) ──────────────────────────────────────────────

pub async fn agent_run_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<AgentRunRequest>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let uid = user_key(&headers);
    let run_id = format!("run-{}", uuid::Uuid::new_v4());
    let handle = state.agent_runs.insert(&run_id, &uid);
    let cancelled = handle.cancelled.clone();
    let child_for_run = handle.child.clone();

    // Restrict to the requested tool whitelist, if any.
    let allowed: Option<Vec<String>> = if req.tools.is_empty() {
        None
    } else {
        Some(req.tools.iter().map(|s| s.to_lowercase()).collect())
    };

    // Build the tool registry (native Physis tools + OpenCode subprocess tool).
    let tools = build_tool_registry(state.clone(), child_for_run, cancelled.clone(), &uid, &allowed);

    let cfg = state.shared.config.agent.clone();
    let agent_config = AgentConfig {
        system_prompt: cfg.system_prompt.clone(),
        max_tokens: cfg.max_tokens,
        detail: "low".into(),
        pdca_rails: true,
    };
    let cascade = state.shared.cascade.clone();
    let query = req.prompt.clone();
    let mode = req.mode.clone();
    let model = req.model.clone();
    let frame = req.frame_b64.clone();
    let registry = state.clone();

    // Channel the agent loop writes events into; the SSE body reads from it.
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<AgentEvent>();

    // Spawn the run. The loop pushes AgentEvents to `tx`; on completion we push
    // a final `done` marker and drop the run handle.
    let run_tx = tx.clone();
    let run_id2 = run_id.clone();
    let gate_enabled = cfg.harness_gate;
    let gate_threshold = cfg.harness_threshold;
    let gate_state = state.clone();
    let gate_uid = uid.clone();

    tokio::spawn(async move {
        // The gate needs the answer the loop settled on. Capture it as it goes
        // past rather than changing `run_agent_stream`'s return path: the loop
        // lives in `src/ai/agent.rs` and is deliberately free of any dependency
        // on graph state, which the harness requires.
        let final_text: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let capture = final_text.clone();
        let sink_tx = run_tx.clone();
        let sink: EventSink = Arc::new(move |ev| {
            if ev.kind == "final" {
                if let Ok(mut slot) = capture.lock() {
                    *slot = Some(ev.text.clone());
                }
            }
            let _ = sink_tx.send(ev);
        });

        if cancelled.load(Ordering::SeqCst) {
            let _ = run_tx.send(AgentEvent {
                round: 0,
                kind: "error".into(),
                text: "run cancelled before start".into(),
            });
        } else {
            let history: Vec<crate::ai::provider::Message> = Vec::new();
            if let Err(e) = crate::ai::agent::run_agent_stream(
                &cascade,
                &tools,
                &agent_config,
                &history,
                &query,
                frame.as_deref(),
                &mode,
                model.as_deref(),
                &sink,
            )
            .await
            {
                sink(AgentEvent {
                    round: 0,
                    kind: "error".into(),
                    text: format!("{e}"),
                });
            }
        }

        // Judge the answer with the same arbiter that judges every other claim
        // in Physis, and emit the verdict as its own event so the dashboard can
        // render it beside the answer.
        if gate_enabled {
            let answer = final_text.lock().ok().and_then(|s| s.clone()).unwrap_or_default();
            if !answer.trim().is_empty() {
                let ev = evaluate_answer(&gate_state, &gate_uid, &answer, &query, gate_threshold);
                let _ = run_tx.send(AgentEvent {
                    round: 0,
                    kind: "verdict".into(),
                    // `AgentEvent.text` is a plain string shared with the core
                    // loop, so structured payloads ride as JSON inside it rather
                    // than widening a type the loop also depends on.
                    text: serde_json::to_string(&ev).unwrap_or_default(),
                });
            }
        }

        let _ = run_tx.send(AgentEvent {
            round: 0,
            kind: "done".into(),
            text: run_id2.clone(),
        });
        registry.agent_runs.remove(&run_id2);
    });

    // Adapt the unbounded receiver into an SSE stream.
    let stream = stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Some(ev) => Some((Ok::<_, axum::Error>(sse_event(&ev)), rx)),
            None => None,
        }
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

// ── Tool registry construction ────────────────────────────────────────────────

/// Build the agent's tool registry. Every native tool captures `SharedState`,
/// the request `uid`, and (for opencode) the run's child handle + cancel flag.
fn build_tool_registry(
    state: SharedState,
    child: Arc<Mutex<Option<tokio::process::Child>>>,
    cancelled: Arc<AtomicBool>,
    uid: &str,
    allowed: &Option<Vec<String>>,
) -> ToolRegistry {
    let mut reg = ToolRegistry::new();
    // `ToolRegistry::new()` pre-registers builtins that touch the network/fs
    // outside our sandbox. Clear them — the agent only gets curated Physis tools.
    reg.clear();

    let uid = uid.to_string();

    if allow(allowed, "graph_search") {
        let state = state.clone();
        let uid = uid.clone();
        reg.register(
            "graph_search",
            "Search the knowledge graph (coherence nodes + goals) for text matching a query. Returns the most relevant nodes with their scores.",
            serde_json::json!({
                "type": "object",
                "properties": { "query": {"type": "string", "description": "search text"} },
                "required": ["query"]
            }),
            Box::new(move |args: Value| {
                let state = state.clone();
                let uid = uid.clone();
                Box::pin(async move {
                    let q = args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if q.trim().is_empty() {
                        return Ok("error: 'query' is required".to_string());
                    }
                    let q_emb = embed(&state, &q).await;
                    let cands = collect_candidates(&state, &uid, &q_emb);
                    if cands.is_empty() {
                        return Ok("no nodes in the graph yet".to_string());
                    }
                    let corpus = rag_corpus_from(&cands);
                    let retriever = rag::TokenFixedRetriever::new(1200, 8);
                    let result = retriever.retrieve(&q_emb, &corpus);
                    Ok(format_candidates(&cands, &result.chunks))
                })
            }),
        );
    }

    if allow(allowed, "node_search") {
        let state = state.clone();
        let uid = uid.clone();
        reg.register(
            "node_search",
            "Token-budgeted similarity search over the knowledge graph. Returns packed node/goal labels ranked by cosine to the query.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "token_budget": {"type": "integer", "description": "hard token cap (default 600)"},
                    "top_k": {"type": "integer", "description": "max results (default 12)"}
                },
                "required": ["query"]
            }),
            Box::new(move |args: Value| {
                let state = state.clone();
                let uid = uid.clone();
                Box::pin(async move {
                    let q = args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let budget = args.get("token_budget").and_then(|v| v.as_u64()).unwrap_or(600) as usize;
                    let top_k = args.get("top_k").and_then(|v| v.as_u64()).unwrap_or(12) as usize;
                    if q.trim().is_empty() {
                        return Ok("error: 'query' is required".to_string());
                    }
                    let q_emb = embed(&state, &q).await;
                    let cands = collect_candidates(&state, &uid, &q_emb);
                    if cands.is_empty() {
                        return Ok("no nodes in the graph yet".to_string());
                    }
                    let corpus = rag_corpus_from(&cands);
                    let retriever = rag::TokenFixedRetriever::new(budget, top_k);
                    let result = retriever.retrieve(&q_emb, &corpus);
                    Ok(format_candidates(&cands, &result.chunks))
                })
            }),
        );
    }

    if allow(allowed, "classify") {
        let state = state.clone();
        reg.register(
            "classify",
            "Classify free text onto the semiotic grid (HUMAN domain × mode). Returns the top cells with scores.",
            serde_json::json!({
                "type": "object",
                "properties": { "text": {"type": "string"} },
                "required": ["text"]
            }),
            Box::new(move |args: Value| {
                let state = state.clone();
                Box::pin(async move {
                    let t = args.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if t.trim().is_empty() {
                        return Ok("error: 'text' is required".to_string());
                    }
                    let emb = embed(&state, &t).await;
                    let scores = state.shared.classifier.classify(&emb);
                    let top: Vec<String> = scores
                        .iter()
                        .take(5)
                        .map(|c| format!("{} × {} : {:.3}", c.domain, c.mode, c.score))
                        .collect();
                    Ok(top.join("\n"))
                })
            }),
        );
    }

    if allow(allowed, "coherence_register") {
        let state = state.clone();
        let uid = uid.clone();
        reg.register(
            "coherence_register",
            "Register a new labeled node in the knowledge graph from source text. Returns the new node id.",
            serde_json::json!({
                "type": "object",
                "properties": { "input": {"type": "string", "description": "source text / label"} },
                "required": ["input"]
            }),
            Box::new(move |args: Value| {
                let state = state.clone();
                let uid = uid.clone();
                Box::pin(async move {
                    let inp = args.get("input").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if inp.trim().is_empty() {
                        return Ok("error: 'input' is required".to_string());
                    }
                    let emb = embed(&state, &inp).await;
                    let (node_id, _) = state.with_user(&uid, |s| {
                        let id = s.core.register_node_vec_labeled(emb, Some(inp.clone()));
                        if let Some(db) = state.db.as_ref() {
                            if let Err(e) = s.core.persist_node(db, &uid, &id) {
                                log::warn!("agent node {id} not persisted: {e}");
                            }
                        }
                        (id, ())
                    });
                    Ok(format!("registered node {node_id}"))
                })
            }),
        );
    }

    if allow(allowed, "coherence_edit") {
        let state = state.clone();
        let uid = uid.clone();
        reg.register(
            "coherence_edit",
            "Edit an existing graph node's content while preserving its structure (id / edges / provenance). Returns the outcome.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string"},
                    "label": {"type": "string", "description": "new content"}
                },
                "required": ["id", "label"]
            }),
            Box::new(move |args: Value| {
                let state = state.clone();
                let uid = uid.clone();
                Box::pin(async move {
                    let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let label = args.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if id.trim().is_empty() || label.trim().is_empty() {
                        return Ok("error: 'id' and 'label' are required".to_string());
                    }
                    let emb = embed(&state, &label).await;
                    let out = state.with_user(&uid, |s| {
                        match s.core.edit_node(&id, &label, emb, crate::core::PinEdit::Keep, Some(state.shared.embedder_kind)) {
                            Ok(o) => format!("edited {} (coherence {:.3})", o.node_id, o.coherence_score),
                            Err(e) => format!("error: {e}"),
                        }
                    });
                    Ok(out)
                })
            }),
        );
    }

    if allow(allowed, "dream") {
        let state = state.clone();
        let uid = uid.clone();
        reg.register(
            "dream",
            "Run a coherence/dream cycle over the tenant's active goals. Returns generated dream proposals or a status note.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "count": {"type": "integer", "description": "dreams to generate (default 3)"},
                    "force": {"type": "boolean", "description": "override active-PDCA suppression"}
                }
            }),
            Box::new(move |args: Value| {
                let state = state.clone();
                let uid = uid.clone();
                Box::pin(async move {
                    let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
                    let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
                    state.with_user(&uid, |s| {
                        if s.goals.is_empty() {
                            return Ok("no goals; nothing to dream about".to_string());
                        }
                        let dreams = s.dreams.generate_dreams_with_context(
                            &s.goals, count, Some(&s.scheduler), None,
                        );
                        let lines: Vec<String> = dreams
                            .iter()
                            .map(|d| format!("- {} (grade {:.2})", d.description, d.grade.unwrap_or(0.0)))
                            .collect();
                        Ok(format!("dreamed {} proposals:\n{}", dreams.len(), lines.join("\n")))
                    })
                })
            }),
        );
    }

    if allow(allowed, "opencode") && state.shared.config.agent.opencode_bin.is_some() {
        let opencode_bin = state.shared.config.agent.opencode_bin.clone();
        let opencode_model = state.shared.config.agent.opencode_model.clone();
        let scan_root = state.scan_root();
        reg.register(
            "opencode",
            "Delegate a coding task to OpenCode (runs as a subprocess, sandboxed to the scan root). Use for writing/editing/building real files. Returns OpenCode's output.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "task": {"type": "string", "description": "the coding task description"},
                    "workdir": {"type": "string", "description": "directory within scan root (default: scan root)"}
                },
                "required": ["task"]
            }),
            Box::new(move |args: Value| {
                let bin = opencode_bin.clone();
                let model = opencode_model.clone();
                let scan_root = scan_root.clone();
                let child_slot = child.clone();
                let cancelled = cancelled.clone();
                Box::pin(async move {
                    run_opencode(bin, model, scan_root, child_slot, cancelled, args).await
                })
            }),
        );
    }

    reg
}

/// True when no whitelist is set, or `name` is in it.
fn allow(allowed: &Option<Vec<String>>, name: &str) -> bool {
    match allowed {
        None => true,
        Some(names) => names.iter().any(|n| n == name),
    }
}

// ── Engine helpers that mirror the existing HTTP handlers ─────────────────────

/// Embed text off the tokio worker (the embedder can be CPU-heavy / ONNX-locked).
async fn embed(state: &SharedState, text: &str) -> Vec<f32> {
    let shared = state.shared.clone();
    let text = text.to_string();
    tokio::task::spawn_blocking(move || shared.embedder.embed(&text))
        .await
        .unwrap_or_default()
}

/// Collect (kind, id, label, embedding) from the tenant's graph, mirroring
/// `search_nodes_handler`.
fn collect_candidates(
    state: &SharedState,
    uid: &str,
    _q_emb: &[f32],
) -> Vec<(String, String, String, Vec<f32>)> {
    state.with_user(uid, |s| {
        let mut out: Vec<(String, String, String, Vec<f32>)> = Vec::new();
        for (id, node) in &s.core.nodes {
            out.push((
                "node".to_string(),
                id.clone(),
                node.label.clone().unwrap_or_else(|| id.clone()),
                node.embedding.clone(),
            ));
        }
        for g in &s.goals {
            out.push((
                "goal".to_string(),
                g.id.clone(),
                g.label.clone().unwrap_or_default(),
                g.embedding.clone(),
            ));
        }
        out
    })
}

/// Build a RagCorpus from candidate (kind,id,label,emb) tuples (labels only).
fn rag_corpus_from(cands: &[(String, String, String, Vec<f32>)]) -> RagCorpus {
    RagCorpus {
        chunks: cands
            .iter()
            .enumerate()
            .map(|(i, (_, _, label, emb))| rag::RagChunk {
                id: i,
                text: label.clone(),
                tokens: rag::count_tokens(label),
                embedding: emb.clone(),
            })
            .collect(),
    }
}

/// Format retrieved chunks back into candidate (kind,id,label) labels.
fn format_candidates(
    cands: &[(String, String, String, Vec<f32>)],
    chunks: &[rag::RetrievedChunk],
) -> String {
    let out: Vec<String> = chunks
        .iter()
        .filter_map(|c| {
            let (kind, id, label, _) = cands.get(c.id)?;
            Some(format!("[{}] {} ({}): {:.3}", kind, id, label, c.score))
        })
        .collect();
    if out.is_empty() {
        "no matches".to_string()
    } else {
        out.join("\n")
    }
}

/// Spawn OpenCode as a sandboxed subprocess for a coding task.
async fn run_opencode(
    bin: Option<PathBuf>,
    model: String,
    scan_root: PathBuf,
    child_slot: Arc<Mutex<Option<tokio::process::Child>>>,
    cancelled: Arc<AtomicBool>,
    args: Value,
) -> crate::ai::AiResult<String> {
    let bin = match bin {
        Some(b) if b.exists() => b,
        _ => return Ok("opencode tool disabled: no opencode_bin configured/present".to_string()),
    };
    let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if task.trim().is_empty() {
        return Ok("error: 'task' is required".to_string());
    }
    let workdir_raw = args
        .get("workdir")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    // Confine workdir to scan_root (PREMORTEM P6): default to scan root itself.
    let workdir = match resolve_workdir(&scan_root, &workdir_raw) {
        Ok(d) => d,
        Err(msg) => return Ok(msg),
    };

    let mut cmd = tokio::process::Command::new(&bin);
    // `task` is caller-supplied. Passed as a bare positional it is indistinguishable
    // from a flag, so a task beginning with `-` is parsed by opencode as one —
    // letting a caller set options this endpoint never meant to expose. Put every
    // flag we control first, then `--`, then the untrusted value.
    cmd.arg("run")
        .arg("--model")
        .arg(&model)
        .arg("--")
        .arg(&task)
        .current_dir(&workdir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return Ok(format!("error: failed to spawn opencode: {e}")),
    };
    // Publish the child so /agent/abort can kill it. Take stdout FIRST (owning the
    // `Child` here, then moving it into the slot, would drop the pipe otherwise).
    let out = child.stdout.take();
    child_slot.lock().unwrap().replace(child);

    // Stream stdout lines back as the tool result (capped).
    let mut output = String::new();
    if let Some(mut out) = out {
        use tokio::io::AsyncReadExt;
        let mut buf = [0u8; 1024];
        loop {
            if cancelled.load(Ordering::SeqCst) {
                if let Ok(mut guard) = child_slot.lock() {
                    if let Some(mut c) = guard.take() {
                        let _ = c.start_kill();
                    }
                }
                output.push_str("\n[cancelled]");
                break;
            }
            match out.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    output.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if output.len() > 16_000 {
                        output.truncate(16_000);
                        output.push_str("\n[truncated]");
                        break;
                    }
                }
                Err(e) => {
                    output.push_str(&format!("\n[read error: {e}]"));
                    break;
                }
            }
        }
    }
    // Reap the child (it lives in the slot so abort can kill it). Take it out of
    // the slot BEFORE awaiting so the MutexGuard is not held across the await.
    let finished = child_slot.lock().unwrap().take();
    if let Some(mut c) = finished {
        let _ = c.wait().await;
    }
    if output.trim().is_empty() {
        Ok("(opencode returned no output)".to_string())
    } else {
        Ok(output)
    }
}


/// Judge one agent answer with the coherence harness.
///
/// The agent's prompt is passed as the harness `context` so the arbiter scores
/// the answer as a response to that question, not as a free-floating claim.
///
/// `register_as_hypothesis` is off here. The standalone harness registers
/// candidates because a human asked for that claim to enter the graph; an agent
/// answers many times per session, and writing each one in would let the agent
/// grade itself against its own prior output on the next run.
///
/// Returns the evaluation for the caller to emit; it never alters the answer.
fn evaluate_answer(
    state: &SharedState,
    uid: &str,
    answer: &str,
    prompt: &str,
    threshold: f32,
) -> crate::ai::harness::CoherenceEvaluation {
    let cfg = crate::ai::harness::HarnessConfig {
        coherence_threshold: threshold,
        // The gate reports; it does not re-run the model. Revision belongs to
        // the agent loop, which owns the provider and the tool budget.
        auto_revise: false,
        register_as_hypothesis: false,
        ..Default::default()
    };
    state.with_user(uid, |s| {
        let quality = state.quality.lock().unwrap_or_else(|e| e.into_inner());
        crate::ai::harness::CoherenceHarness::evaluate(
            answer,
            Some(prompt),
            &cfg,
            &mut s.core,
            &state.shared.classifier,
            state.shared.embedder.as_ref(),
            &quality,
        )
    })
}

/// Resolve a caller-supplied `workdir` to a real directory inside `scan_root`.
///
/// Returns `Err(message)` with a caller-facing string when the request must be
/// refused; the message is surfaced as the tool result.
///
/// The subtlety this exists for: `Path::starts_with` compares whole components
/// and performs no normalization, so a lexical prefix test is not containment.
/// `scan_root.join("../../etc")` is `<root>/../../etc`, whose components really
/// do begin with `<root>` — it passes — and then `current_dir` resolves the
/// `..` at the OS level and the agent runs outside the sandbox. So: reject `..`
/// outright, then compare *canonical* paths, which also stops a symlink inside
/// the root from pointing out of it.
fn resolve_workdir(scan_root: &std::path::Path, workdir_raw: &str) -> Result<PathBuf, String> {
    use std::path::Component;

    if workdir_raw.trim().is_empty() {
        return Ok(scan_root.to_path_buf());
    }
    if std::path::Path::new(workdir_raw)
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err("error: workdir must not contain '..'".to_string());
    }
    let candidate = scan_root.join(workdir_raw.trim_start_matches('/'));
    let root = std::fs::canonicalize(scan_root)
        .map_err(|e| format!("error: scan root unreadable: {e}"))?;
    // A workdir that does not resolve is refused rather than created: spawning
    // into a path we could not verify is the case this check exists to stop.
    let resolved = std::fs::canonicalize(&candidate)
        .map_err(|_| "error: workdir does not exist under scan root".to_string())?;
    if !resolved.starts_with(&root) {
        return Err("error: workdir outside scan root".to_string());
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lexical check this replaced accepted every one of these.
    #[test]
    fn workdir_cannot_escape_the_scan_root_with_dotdot() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        std::fs::create_dir_all(root.join("sub")).unwrap();

        for attack in ["..", "../..", "../../etc", "/../../etc", "sub/../../..", "./../out"] {
            let err = resolve_workdir(&root, attack)
                .expect_err("{attack:?} must be refused");
            assert!(
                err.contains("must not contain '..'") || err.contains("outside scan root"),
                "{attack:?} refused for the wrong reason: {err}",
            );
        }
    }

    #[test]
    fn a_symlink_pointing_out_of_the_root_is_refused() {
        // Rejecting ".." is not enough on its own: a link inside the root
        // resolves outward without any parent component in the request.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();
            let err = resolve_workdir(&root, "escape").expect_err("symlink must be refused");
            assert!(err.contains("outside scan root"), "got: {err}");
        }
    }

    #[test]
    fn a_real_directory_inside_the_root_is_accepted() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        std::fs::create_dir_all(root.join("pkg/src")).unwrap();

        let got = resolve_workdir(&root, "pkg/src").unwrap();
        assert!(got.starts_with(std::fs::canonicalize(&root).unwrap()));
        assert!(got.ends_with("pkg/src"));

        // Empty means "the root itself", not "reject".
        assert_eq!(resolve_workdir(&root, "  ").unwrap(), root);
        // A leading slash is treated as root-relative, not absolute.
        assert_eq!(resolve_workdir(&root, "/pkg").unwrap(), std::fs::canonicalize(root.join("pkg")).unwrap());
    }

    #[test]
    fn a_workdir_that_does_not_exist_is_refused_not_created() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        std::fs::create_dir_all(&root).unwrap();

        let err = resolve_workdir(&root, "nope").expect_err("missing dir must be refused");
        assert!(err.contains("does not exist"), "got: {err}");
        assert!(!root.join("nope").exists(), "must not have been created");
    }

    /// A run id is unguessable but not secret — it is returned to the client
    /// and appears in logs — so it must not by itself authorize a kill.
    #[test]
    fn abort_only_reaches_the_tenant_that_started_the_run() {
        let reg = AgentRunRegistry::new();
        reg.insert("run-1", "alice");

        assert!(reg.get_owned("run-1", "alice").is_some(), "owner may abort");
        assert!(reg.get_owned("run-1", "bob").is_none(), "another tenant may not");
        // Unknown id and someone else's id are indistinguishable, so the
        // response cannot be used to probe for live run ids.
        assert!(reg.get_owned("run-does-not-exist", "bob").is_none());
    }
}
