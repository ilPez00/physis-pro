use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::{
    extract::{Query, Request, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use base64::Engine;

use physis_pro::actor::PDCActor;
use physis_pro::config::{EmbedderKindConfig, OntologyLoader, PhysisConfig};
use physis_pro::core::PhysisCore;
use physis_pro::dream::DreamEngine;
use physis_pro::embed::VectorEmbed;
#[cfg(feature = "embed-onnx")]
use physis_pro::embed::clip::ClipEmbedder;
#[cfg(feature = "embed-onnx")]
use physis_pro::embed::jina::JinaEmbedder;
#[cfg(feature = "embed-onnx")]
use physis_pro::embed::onnx::OnnxEmbedder;
use physis_pro::embed::RandomProjectionEmbedder;
use physis_pro::linguistic::{LinguisticLense, LinguisticRouter};
use physis_pro::mapper::OntologyMapper;
use physis_pro::models::{Goal, Score, HumanDomain, HumanMode, SemioticGrid};
use physis_pro::CoherenceSnapshot;

use physis_pro::classify::{CellClassifier, CellScore};
use physis_pro::graph::{IngestRing};
use physis_pro::ai::onnx_worker::OnnxHolon;
use physis_pro::ai::provider::{ProviderCascade, ProviderConfig, Message};
use physis_pro::rachmaninov::RachmaninovHolon;

/// Read-only context built once at startup, shared by every request WITHOUT
/// locking. The hot paths (embed, classify, translate, ontology/semiotic reads)
/// touch only this, so a CPU-heavy ONNX embed never blocks unrelated requests
/// or the healthcheck. See PREMORTEM.md P1.
struct SharedCtx {
    config: PhysisConfig,
    cascade: Arc<ProviderCascade>,
    ontology: OntologyLoader,
    embedder: Box<dyn VectorEmbed>,
    /// Pre-computed per-cell centroids + classification logic.
    classifier: CellClassifier,
    embed_dim: usize,
    /// Which embedder kind actually loaded (provenance of every vector).
    embedder_kind: &'static str,
    /// True when no real ONNX model loaded and vectors are random-projection noise.
    degraded: bool,
    /// Whether the embedder passed the semantic self-test at startup (P4).
    self_test: bool,
    /// Holarchy ingest ring (the onnx/rachmaninov holons are driven by the
    /// background tasks via their own Arc clones, not through this struct).
    ingest: Arc<IngestRing>,
    /// Bearer token required on protected routes. `None` only when bound to
    /// loopback (dev) — a non-loopback bind without a token aborts at startup.
    /// See PREMORTEM.md P6.
    api_token: Option<String>,
}

/// Mutable runtime state behind a single mutex. Kept small and cheap to mutate so
/// the lock is never held across embedding/inference — only across state writes.
struct RuntimeState {
    core: PhysisCore,
    mapper: OntologyMapper,
    actor: PDCActor,
    dreams: DreamEngine,
    goals: Vec<Goal>,
}

impl RuntimeState {
    /// Fresh per-tenant session state. Shares nothing mutable with other users;
    /// the embedder/classifier/cascade/ontology live in SharedCtx and stay global.
    fn new(shared: &SharedCtx) -> Self {
        let mut core = PhysisCore::new();
        core.set_embedder_id(shared.embedder_kind); // vector provenance
        RuntimeState {
            core,
            mapper: OntologyMapper::new(shared.ontology.clone(), shared.embed_dim),
            actor: PDCActor::new(
                shared.config.pdca_stagnant_threshold,
                shared.config.pdca_stagnant_window,
            ),
            dreams: DreamEngine::new(),
            goals: Vec::new(),
        }
    }
}

struct AppCtx {
    shared: Arc<SharedCtx>,
    /// The "_default" tenant. Holarchy background threads drive this one, and
    /// any request without an X-Physis-User header falls here (back-compat).
    state: Mutex<RuntimeState>,
    /// Per-user session state (goals / PDCA / dreams / coherence), created on
    /// first use and keyed by the X-Physis-User header. The holarchy stays
    /// global (single ingest/sensory pipeline), so a tenant is an isolated
    /// reasoning session, not a separate knowledge graph.
    tenants: Mutex<HashMap<String, RuntimeState>>,
    /// Live-reloadable scan-root guard (the only `/scan` path confinement).
    /// Behind an RwLock so `/config/reload` can repoint it without a restart —
    /// reads are a cheap shared lock and `/scan` isn't a hot path. (The embedder
    /// stays fixed at startup: changing embed_dim live would corrupt every stored
    /// vector, and it's read lock-free on the hot path — see PREMORTEM P1.)
    scan_root: std::sync::RwLock<PathBuf>,
    #[cfg(feature = "mcp")]
    mcp_sessions: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, futures::channel::mpsc::Sender<rmcp::model::ClientJsonRpcMessage>>>>,
}

impl AppCtx {
    /// Current scan root (clone out of the lock; callers compare/canonicalize).
    fn scan_root(&self) -> PathBuf {
        self.scan_root.read().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl AppCtx {
    /// Poison-tolerant lock: a panic while the lock is held no longer permanently
    /// 500s every subsequent request — we recover the inner guard. PREMORTEM.md P1.
    fn lock(&self) -> std::sync::MutexGuard<'_, RuntimeState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Run `f` against the runtime state for `uid`. Empty or "_default" routes to
    /// the global default tenant (holarchy-connected); any other id gets its own
    /// isolated session, created on demand. The lock is held across `f`, matching
    /// the single-tenant behavior.
    fn with_user<R>(&self, uid: &str, f: impl FnOnce(&mut RuntimeState) -> R) -> R {
        if uid.is_empty() || uid == "_default" {
            let mut g = self.lock();
            f(&mut g)
        } else {
            let mut map = self.tenants.lock().unwrap_or_else(|e| e.into_inner());
            let rt = map
                .entry(uid.to_string())
                .or_insert_with(|| RuntimeState::new(&self.shared));
            f(rt)
        }
    }
}

/// Tenant key from the `X-Physis-User` header; "_default" when absent/blank.
fn user_key(headers: &HeaderMap) -> String {
    headers
        .get("x-physis-user")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "_default".to_string())
}

type SharedState = Arc<AppCtx>;

// ── Off-thread embedding ─────────────────────────────────────────────────────
// Embedding is CPU-heavy (and, for ONNX, serialized behind a session mutex), so
// run it via spawn_blocking — a slow embedder then never blocks a tokio worker
// (the healthcheck + non-embed requests stay responsive). The embedder is
// Send+Sync behind Arc<SharedCtx>.

async fn embed_text(shared: Arc<SharedCtx>, text: String) -> Vec<f32> {
    tokio::task::spawn_blocking(move || shared.embedder.embed(&text))
        .await
        .unwrap_or_default()
}

/// Image (base64) → `embed_image`, else text — off-thread.
async fn embed_input(shared: Arc<SharedCtx>, text: String, image: Option<String>) -> Vec<f32> {
    tokio::task::spawn_blocking(move || {
        if let Some(b64) = image {
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&b64) {
                if let Some(v) = shared.embedder.embed_image(&bytes) {
                    return v;
                }
                log::warn!("image embedding unavailable; falling back to text");
            } else {
                log::warn!("base64 decode failed; falling back to text");
            }
        }
        shared.embedder.embed(&text)
    })
    .await
    .unwrap_or_default()
}

/// Batch-embed off-thread via `embed_batch` (one hop; embedders that truly batch
/// — e.g. a single ONNX run — get the throughput).
async fn embed_batch_off_thread(shared: Arc<SharedCtx>, texts: Vec<String>) -> Vec<Vec<f32>> {
    tokio::task::spawn_blocking(move || {
        let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        shared.embedder.embed_batch(&refs)
    })
    .await
    .unwrap_or_default()
}

#[derive(Deserialize)]
struct QueryParams {
    q: String,
    #[serde(default = "default_max")]
    max: usize,
}
fn default_max() -> usize { 10 }

#[derive(Deserialize)]
struct ScanRequest {
    dir: String,
}

#[derive(Deserialize)]
struct DreamGenerateRequest {
    #[serde(default = "default_dream_count")]
    count: usize,
    #[serde(default)]
    force: bool,
}
fn default_dream_count() -> usize { 5 }

#[derive(Deserialize)]
struct DreamEvaluateRequest {
    id: String,
    grade: Score,
}

#[derive(Deserialize)]
struct CoherenceRegisterRequest {
    input: String,
}

#[derive(Deserialize)]
struct ContextFilterRequest {
    input: String,
}

#[derive(Deserialize)]
struct CompressLogsRequest {
    logs: Vec<String>,
}

#[derive(Deserialize)]
struct PDCARequest {
    goal_id: String,
    state_vector: Vec<f32>,
}

#[derive(Serialize)]
struct StatsResponse {
    mapper: HashMap<String, usize>,
    core: CoherenceSnapshot,
}

#[derive(Serialize)]
struct ScanResponse {
    goals: Vec<Goal>,
    count: usize,
}

#[derive(Serialize)]
struct QueryResponse {
    results: Vec<Vec<String>>,
    count: usize,
}

#[derive(Serialize)]
struct CoherenceRegisterResponse {
    node_id: String,
}

#[derive(Serialize)]
struct CoherenceNodeMatch {
    id: String,
    label: String,
    score: Score,
}

#[derive(Serialize)]
struct CoherenceNodesResponse {
    results: Vec<CoherenceNodeMatch>,
    count: usize,
}

#[derive(Serialize)]
struct FilterResponse {
    embedding: Vec<f32>,
    valid: bool,
    token_estimate: usize,
}

#[derive(Serialize)]
struct CompressResponse {
    compressed: String,
    input_count: usize,
    output_chars: usize,
}

#[derive(Deserialize)]
struct TranslateRequest {
    text: String,
    lense: Option<String>,
}

#[derive(Serialize)]
struct TranslateResponse {
    results: std::collections::HashMap<String, String>,
}

#[derive(Serialize)]
struct IngestResponse {
    goal: Goal,
    message: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn err_response(status: StatusCode, msg: &str) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error: msg.to_string() }))
}

// ── Embedder self-test ──────────────────────────────────────────────────

/// Probe whether an embedder is actually semantic: a related pair must score
/// clearly higher cosine than an unrelated pair, and all vectors must be finite
/// and non-empty. Random-projection and broken-inference fallbacks fail this.
fn embedder_self_test(e: &dyn VectorEmbed) -> bool {
    let a = e.embed("a photograph of a dog");
    let related = e.embed("a photograph of a puppy");
    let unrelated = e.embed("quarterly corporate tax accounting spreadsheet");
    let finite = |v: &[f32]| !v.is_empty() && v.iter().all(|x| x.is_finite());
    if !(finite(&a) && finite(&related) && finite(&unrelated)) {
        return false;
    }
    let sim_related = physis_pro::models::cosine_sim(&a, &related);
    let sim_unrelated = physis_pro::models::cosine_sim(&a, &unrelated);
    sim_related > sim_unrelated + 0.05
}

// ── Auth ───────────────────────────────────────────────────────────────

/// Constant-time byte comparison — avoids leaking the token via timing.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Require `Authorization: Bearer <PHYSIS_API_TOKEN>` on protected routes.
/// When no token is configured (loopback-only dev), requests pass through.
async fn require_auth(
    State(state): State<SharedState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    match &state.shared.api_token {
        None => Ok(next.run(req).await),
        Some(expected) => {
            let presented = req
                .headers()
                .get(AUTHORIZATION)
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer "));
            match presented {
                Some(tok) if constant_time_eq(tok.as_bytes(), expected.as_bytes()) => {
                    Ok(next.run(req).await)
                }
                _ => Err(StatusCode::UNAUTHORIZED),
            }
        }
    }
}

// ── Handlers ──────────────────────────────────────────────────────────

async fn health() -> Json<&'static str> {
    Json("ok")
}

/// Reports the live embedder provenance so degraded (random-projection) mode is
/// observable instead of silent. See PREMORTEM.md P0.
async fn embedder_info_handler(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = &state.shared;
    Json(serde_json::json!({
        "kind": s.embedder_kind,
        "dim": s.embed_dim,
        "degraded": s.degraded,
        "semantic": !s.degraded,
        "self_test": s.self_test,
    }))
}

// ── /api/v1/chat ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ChatApiRequest {
    messages: Vec<Message>,
    #[serde(default)]
    task_type: Option<String>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    has_image: bool,
}

#[derive(Serialize)]
struct ChatApiResponse {
    content: String,
    provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<serde_json::Value>,
}

async fn chat_handler(
    State(state): State<SharedState>,
    Json(req): Json<ChatApiRequest>,
) -> Result<Json<ChatApiResponse>, StatusCode> {
    let cascade = state.shared.cascade.clone();
    let hint = {
        let task = req.task_type.as_deref().unwrap_or("smart");
        physis_pro::ai::provider::RouteHint {
            task_type: task.to_string(),
            has_image: req.has_image,
        }
    };
    match cascade.complete(&req.messages, None, &hint, req.max_tokens).await {
        Ok((msg, provider)) => Ok(Json(ChatApiResponse {
            content: msg.content.unwrap_or_default(),
            provider,
            tool_calls: msg.tool_calls.as_ref().map(|tc| serde_json::to_value(tc).unwrap_or_default()),
        })),
        Err(e) => {
            log::warn!("[chat] all providers failed: {}", e);
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

// ── /api/v1/providers ────────────────────────────────────────────────────────

async fn providers_handler(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let cascade = state.shared.cascade.clone();
    Json(serde_json::json!({ "providers": cascade.providers_list() }))
}

async fn stats_handler(State(state): State<SharedState>, headers: HeaderMap) -> Json<StatsResponse> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        Json(StatsResponse {
            mapper: s.mapper.stats(),
            core: s.core.snapshot(),
        })
    })
}

async fn query_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<QueryParams>,
) -> Json<QueryResponse> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let results = s.mapper.query(&params.q);
        let limited: Vec<Vec<String>> = results.into_iter().take(params.max).collect();
        Json(QueryResponse { count: limited.len(), results: limited })
    })
}

async fn scan_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<ScanRequest>,
) -> Result<Json<ScanResponse>, (StatusCode, Json<ErrorResponse>)> {
    let path = PathBuf::from(&req.dir);
    if !path.exists() {
        return Err(err_response(StatusCode::BAD_REQUEST, &format!("directory not found: {}", req.dir)));
    }
    // Path-traversal guard: resolve symlinks/`..` and confine the scan to
    // scan_root. Without this, `{"dir":"/etc"}` would expose the host fs.
    let canonical = path.canonicalize().map_err(|e| {
        err_response(StatusCode::BAD_REQUEST, &format!("cannot resolve path: {e}"))
    })?;
    if !canonical.starts_with(state.scan_root()) {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            &format!("path outside scan root ({})", state.scan_root().display()),
        ));
    }
    let uid = user_key(&headers);
    let (goals, labels) = state.with_user(&uid, |s| {
        let (goals, labels) = s.mapper.map_filesystem_labeled(&canonical, None);
        s.goals = goals.clone();
        (goals, labels)
    });

    // Register each scanned file as a labeled core node so it's recallable by
    // content in the wiki. Embed with the SHARED embedder (not mapper's internal
    // RandomProjection one) so these vectors are comparable to query vectors.
    // Batched + off-thread: one spawn_blocking hop, never blocks a tokio worker.
    let embeddings = embed_batch_off_thread(state.shared.clone(), labels.clone()).await;
    state.with_user(&uid, |s| {
        for (emb, label) in embeddings.into_iter().zip(labels) {
            s.core.register_node_vec_labeled(emb, Some(label));
        }
    });

    Ok(Json(ScanResponse { count: goals.len(), goals }))
}

/// Browsable content labels for one scanned file: one per structural line
/// (`path → fn/struct`) plus the file's structural path. Mirrors the format in
/// `OntologyMapper::map_filesystem_labeled` so watch-mode and /scan agree.
fn file_labels(f: &physis_pro::scanner::FileInfo) -> Vec<String> {
    let p = f.structural_path();
    let mut labels: Vec<String> = f
        .structural_lines
        .iter()
        .map(|line| format!("{} → {}", p, line))
        .collect();
    labels.push(p);
    labels
}

async fn dream_generate_handler(
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
        let dreams = s.dreams.generate_dreams(&goals, req.count);
        Json(serde_json::json!({
            "dreams": dreams.iter().map(|d| serde_json::json!({
                "id": d.id,
                "source": d.source,
                "embedding": d.embedding,
                "grade": d.grade,
            })).collect::<Vec<_>>(),
            "count": dreams.len(),
            "active": true,
        }))
    })
}

async fn dream_evaluate_handler(
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

async fn coherence_snapshot_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<CoherenceSnapshot> {
    let uid = user_key(&headers);
    Json(state.with_user(&uid, |s| s.core.snapshot()))
}

/// Recall registered coherence nodes by content similarity — across every
/// machine that wrote into the shared graph. Unlike `/query` (mapper/ontology
/// surface), this reads the coherence nodes that `/coherence/register` creates.
async fn coherence_nodes_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<QueryParams>,
) -> Json<CoherenceNodesResponse> {
    let emb = embed_text(state.shared.clone(), params.q.clone()).await;
    let uid = user_key(&headers);
    let matches = state.with_user(&uid, |s| s.core.search_nodes(&emb, params.max));
    let results: Vec<CoherenceNodeMatch> = matches
        .into_iter()
        .map(|(id, label, score)| CoherenceNodeMatch { id, label, score })
        .collect();
    Json(CoherenceNodesResponse { count: results.len(), results })
}

async fn coherence_register_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CoherenceRegisterRequest>,
) -> Json<CoherenceRegisterResponse> {
    let embedding = embed_text(state.shared.clone(), req.input.clone()).await;
    let uid = user_key(&headers);
    // Retain the source text so the node can be recalled/browsed by content.
    let label = Some(req.input.clone());
    let node_id = state.with_user(&uid, |s| s.core.register_node_vec_labeled(embedding, label));
    Json(CoherenceRegisterResponse { node_id })
}

async fn context_filter_handler(
    State(state): State<SharedState>,
    Json(req): Json<ContextFilterRequest>,
) -> Json<FilterResponse> {
    let s = state.lock();
    let result = s.core.filter_context(&req.input, &state.shared.embedder);
    Json(FilterResponse {
        embedding: result.embedding,
        valid: result.valid,
        token_estimate: result.token_estimate,
    })
}

async fn compress_logs_handler(
    State(state): State<SharedState>,
    Json(req): Json<CompressLogsRequest>,
) -> Json<CompressResponse> {
    let s = state.lock();
    let compressed = s.core.compress_logs(&req.logs);
    Json(CompressResponse {
        input_count: req.logs.len(),
        output_chars: compressed.len(),
        compressed,
    })
}

async fn translate_handler(
    State(state): State<SharedState>,
    Json(req): Json<TranslateRequest>,
) -> Json<TranslateResponse> {
    let router = LinguisticRouter::with_config(&state.shared.config.linguistic);
    let mut results = std::collections::HashMap::new();
    match req.lense {
        Some(ref l) => {
            let lense = match l.to_lowercase().as_str() {
                "wenyan" => LinguisticLense::Wenyan,
                "piraha" => LinguisticLense::Piraha,
                "sanskrit" => LinguisticLense::Sanskrit,
                _ => {
                    results.insert("error".to_string(), format!("Unknown lense: {l}. Use: wenyan, piraha, sanskrit"));
                    return Json(TranslateResponse { results });
                }
            };
            results.insert(lense.as_str().to_string(), router.route(&req.text, lense));
        }
        None => {
            for (lense, text) in router.route_all(&req.text) {
                results.insert(lense.as_str().to_string(), text);
            }
        }
    }
    Json(TranslateResponse { results })
}

async fn ingest_prompt_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<serde_json::Value>,
) -> Json<IngestResponse> {
    let prompt = req.get("prompt").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let embedding = embed_text(state.shared.clone(), prompt.clone()).await;
    let mut goal = Goal::new_vec(embedding);
    goal.progress = 0.0;
    let uid = user_key(&headers);
    state.with_user(&uid, |s| s.goals.push(goal.clone()));
    Json(IngestResponse {
        message: format!("Goal created from prompt ({} chars)", prompt.len()),
        goal,
    })
}

async fn pdca_plan_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let planned = s.actor.plan(&s.goals);
        let goals: Vec<serde_json::Value> = planned.iter().map(|g| {
            serde_json::json!({
                "id": g.id,
                "embedding": g.embedding,
                "progress": g.progress,
            })
        }).collect();
        Json(serde_json::json!({ "planned": goals, "count": goals.len() }))
    })
}

async fn pdca_act_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<PDCARequest>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let before = match s.goals.iter().find(|g| g.id == req.goal_id) {
            Some(g) => g.embedding.clone(),
            None => return Json(serde_json::json!({"error": "goal not found"})),
        };
        let after = req.state_vector.clone();
        let exp_id = s.actor.do_action(&req.goal_id, before, after).id;
        let exps = s.actor.experiences.clone();
        let mut goals = std::mem::take(&mut s.goals);
        s.actor.act(&exps, &mut goals);
        s.goals = goals;
        Json(serde_json::json!({ "experience_id": exp_id }))
    })
}

async fn pdca_stats_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<physis_pro::actor::PDCAStats> {
    let uid = user_key(&headers);
    Json(state.with_user(&uid, |s| s.actor.stats(&s.goals)))
}

#[derive(Serialize)]
struct ConfigReloadResponse {
    reloaded: bool,
    pdca_threshold: f32,
    pdca_window: usize,
    note: String,
}

/// Re-read `<data_dir>/config.json` and apply the live-tunable knobs to the
/// tenant's PDCA actor (stagnation threshold + window). Structural params
/// (embedder, dim, scan_root) are fixed at startup and need a restart — say so.
async fn config_reload_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<ConfigReloadResponse> {
    let path = state.shared.config.data_dir.join("config.json");
    let cfg = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<PhysisConfig>(&s).ok());
    match cfg {
        Some(c) => {
            let uid = user_key(&headers);
            state.with_user(&uid, |s| {
                s.actor = PDCActor::new(c.pdca_stagnant_threshold, c.pdca_stagnant_window);
            });
            // scan_root reloads live (behind its RwLock): PHYSIS_SCAN_ROOT env, else
            // the reloaded config's data_dir. Canonicalized like at startup.
            let new_root = std::env::var("PHYSIS_SCAN_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|_| c.data_dir.clone());
            let new_root = new_root.canonicalize().unwrap_or(new_root);
            *state.scan_root.write().unwrap_or_else(|e| e.into_inner()) = new_root.clone();
            Json(ConfigReloadResponse {
                reloaded: true,
                pdca_threshold: c.pdca_stagnant_threshold,
                pdca_window: c.pdca_stagnant_window,
                note: format!("applied live: PDCA knobs + scan_root={}. embedder/embed_dim need a restart (dim change corrupts vectors)", new_root.display()),
            })
        }
        None => Json(ConfigReloadResponse {
            reloaded: false,
            pdca_threshold: state.shared.config.pdca_stagnant_threshold,
            pdca_window: state.shared.config.pdca_stagnant_window,
            note: format!("no readable config.json at {}", path.display()),
        }),
    }
}

async fn reconstruct_handler(
    State(state): State<SharedState>,
    Json(req): Json<ContextFilterRequest>,
) -> Json<physis_pro::Reconstruction> {
    let s = state.lock();
    let rec = physis_pro::reconstruct(&req.input, &state.shared.embedder, &s.core, 5);
    Json(rec)
}

// ── Holarchy Handlers ───────────────────────────────────────────────

#[derive(Deserialize)]
struct HolarchyIngestRequest {
    data: String,
}

async fn holarchy_ingest_handler(
    State(state): State<SharedState>,
    Json(req): Json<HolarchyIngestRequest>,
) -> Json<serde_json::Value> {
    state.shared.ingest.push(3, req.data.as_bytes().to_vec()); // Default: ThoughtCapture
    Json(serde_json::json!({ "status": "pushed_to_ring" }))
}

#[derive(Deserialize)]
struct EdgeWriteRequest {
    /// Coherence node ids (not RawNodeKeys). Both are bridged automatically.
    source: String,
    target: String,
    #[serde(default = "default_edge_weight")]
    weight: f32,
    #[serde(default)]
    semantic_type: u32,
}
fn default_edge_weight() -> f32 { 1.0 }

/// Assert a relation between two coherence nodes (machines compose graphs, not
/// just nodes). Bridges both endpoints, then adds a holarchy edge. 404 if either
/// id is unknown in the tenant.
async fn holarchy_edge_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<EdgeWriteRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let uid = user_key(&headers);
    let ok = state.with_user(&uid, |s| {
        s.core.add_edge_between(&req.source, &req.target, req.weight, req.semantic_type)
    });
    if ok {
        Ok(Json(serde_json::json!({ "status": "edge_added" })))
    } else {
        Err(err_response(StatusCode::NOT_FOUND, "source or target node id not found"))
    }
}

async fn holarchy_snapshot_handler(
    State(state): State<SharedState>,
) -> Json<serde_json::Value> {
    let s = state.lock();
    let nodes_count = s.core.holarchy.nodes.len();
    let edges_count = s.core.holarchy.edges.len();
    Json(serde_json::json!({
        "nodes": nodes_count,
        "edges": edges_count,
    }))
}

async fn holarchy_trajectory_handler(
    State(state): State<SharedState>,
) -> Json<physis_pro::gantt::Trajectory> {
    let s = state.lock();
    let trajectory = physis_pro::gantt::GanttHolon::compute_trajectory(&s.core.holarchy);
    Json(trajectory)
}

#[derive(Serialize)]
struct GraphResponse {
    nodes: Vec<(physis_pro::graph::RawNodeKey, physis_pro::graph::NodePayload)>,
    edges: Vec<physis_pro::graph::Edge>,
}

async fn holarchy_graph_handler(
    State(state): State<SharedState>,
) -> Json<GraphResponse> {
    let s = state.lock();
    let nodes = s.core.holarchy.nodes.iter()
        .map(|(k, p)| (physis_pro::graph::RawNodeKey::from(k), *p))
        .collect();
    let edges = s.core.holarchy.edges.clone();
    Json(GraphResponse { nodes, edges })
}

// ── Semiotic / Ontology Handlers ──────────────────────────────────

#[derive(Serialize)]
struct OntologyListing {
    kind: String,
    count: usize,
}

async fn ontology_list_handler(
    State(state): State<SharedState>,
) -> Json<Vec<OntologyListing>> {
    let s = &state.shared;
    Json(vec![
        OntologyListing { kind: "human".into(), count: s.ontology.human_domains.len() },
        OntologyListing { kind: "machine".into(), count: s.ontology.machine_domains.len() },
        OntologyListing { kind: "semiotic".into(), count: s.ontology.semiotic_domains.len() },
        OntologyListing { kind: "category".into(), count: s.ontology.category_domains.len() },
        OntologyListing { kind: "agent".into(), count: s.ontology.agent_domains.len() },
        OntologyListing { kind: "natural".into(), count: s.ontology.natural_domains.len() },
        OntologyListing { kind: "social".into(), count: s.ontology.social_domains.len() },
        OntologyListing { kind: "abstract".into(), count: s.ontology.abstract_domains.len() },
        OntologyListing { kind: "engineering".into(), count: s.ontology.engineering_domains.len() },
    ])
}

#[derive(Serialize)]
struct SemioticGridResponse {
    cells: Vec<serde_json::Value>,
}

async fn semiotic_grid_handler(
    State(state): State<SharedState>,
) -> Json<SemioticGridResponse> {
    let s = &state.shared;
    let mut grid = SemioticGrid::new();
    // Classify all ontology entries into the grid
    for def in s.ontology.human_domains.values() {
        if let (Some(d), Some(m)) = (def.domain.as_ref().and_then(|d| HumanDomain::from_str(d)),
                                       def.mode.as_ref().and_then(|m| HumanMode::from_str(m))) {
            grid.classify(&def.name, d, m);
        }
    }
    for def in s.ontology.semiotic_domains.values() {
        if let (Some(d), Some(m)) = (def.domain.as_ref().and_then(|d| HumanDomain::from_str(d)),
                                       def.mode.as_ref().and_then(|m| HumanMode::from_str(m))) {
            grid.classify(&def.name, d, m);
        }
    }
    for def in s.ontology.category_domains.values() {
        if let (Some(d), Some(m)) = (def.domain.as_ref().and_then(|d| HumanDomain::from_str(d)),
                                       def.mode.as_ref().and_then(|m| HumanMode::from_str(m))) {
            grid.classify(&def.name, d, m);
        }
    }
    for def in s.ontology.agent_domains.values() {
        if let (Some(d), Some(m)) = (def.domain.as_ref().and_then(|d| HumanDomain::from_str(d)),
                                       def.mode.as_ref().and_then(|m| HumanMode::from_str(m))) {
            grid.classify(&def.name, d, m);
        }
    }
    for def in s.ontology.natural_domains.values() {
        if let (Some(d), Some(m)) = (def.domain.as_ref().and_then(|d| HumanDomain::from_str(d)),
                                       def.mode.as_ref().and_then(|m| HumanMode::from_str(m))) {
            grid.classify(&def.name, d, m);
        }
    }
    for def in s.ontology.social_domains.values() {
        if let (Some(d), Some(m)) = (def.domain.as_ref().and_then(|d| HumanDomain::from_str(d)),
                                       def.mode.as_ref().and_then(|m| HumanMode::from_str(m))) {
            grid.classify(&def.name, d, m);
        }
    }
    for def in s.ontology.abstract_domains.values() {
        if let (Some(d), Some(m)) = (def.domain.as_ref().and_then(|d| HumanDomain::from_str(d)),
                                       def.mode.as_ref().and_then(|m| HumanMode::from_str(m))) {
            grid.classify(&def.name, d, m);
        }
    }
    for def in s.ontology.engineering_domains.values() {
        if let (Some(d), Some(m)) = (def.domain.as_ref().and_then(|d| HumanDomain::from_str(d)),
                                       def.mode.as_ref().and_then(|m| HumanMode::from_str(m))) {
            grid.classify(&def.name, d, m);
        }
    }
    let cells: Vec<serde_json::Value> = grid.cells.iter().map(|c| serde_json::json!({
        "domain": c.domain.as_str(),
        "mode": c.mode.as_str(),
        "entries": c.entries,
        "activation": c.activation,
    })).collect();
    Json(SemioticGridResponse { cells })
}

#[derive(Serialize)]
struct SemioticTriangleResponse {
    mermaid: String,
}

async fn semiotic_triangle_handler() -> Json<SemioticTriangleResponse> {
    let mermaid = physis_pro::output::format_semiotic_triangle(
        "Peircean",
        "Sign\n(Quality/Feeling)",
        "Object\n(Fact/Reality)",
        "Interpretant\n(Thought/Meaning)",
    );
    Json(SemioticTriangleResponse { mermaid })
}

#[derive(Serialize)]
struct GreimasSquareResponse {
    mermaid: String,
}

async fn greimas_square_handler() -> Json<GreimasSquareResponse> {
    let mermaid = physis_pro::output::format_greimas_square(
        "Physis", "HEAL", "FABRICATE", "CONSTRUCT", "STUDY",
    );
    Json(GreimasSquareResponse { mermaid })
}

#[derive(Deserialize)]
struct CategoryDiagramRequest {
    objects: Vec<CategoryObject>,
    morphisms: Vec<CategoryMorphism>,
}

#[derive(Deserialize)]
struct CategoryObject {
    id: String,
    label: String,
}

#[derive(Deserialize)]
struct CategoryMorphism {
    from: String,
    to: String,
    label: String,
}

#[derive(Serialize)]
struct CategoryDiagramResponse {
    mermaid: String,
}

async fn category_diagram_handler(
    Json(req): Json<CategoryDiagramRequest>,
) -> Json<CategoryDiagramResponse> {
    let objects: Vec<(&str, &str)> = req.objects.iter().map(|o| (o.id.as_str(), o.label.as_str())).collect();
    let morphisms: Vec<(&str, &str, &str)> = req.morphisms.iter().map(|m| (m.from.as_str(), m.to.as_str(), m.label.as_str())).collect();
    let mermaid = physis_pro::output::format_category_diagram(&objects, &morphisms);
    Json(CategoryDiagramResponse { mermaid })
}

// ── Wiki (in-process Tantivy BM25; replaces the llmwiki subprocess) ──────────

#[cfg(feature = "wiki-embedded")]
#[derive(Deserialize)]
struct WikiSearchRequest {
    query: String,
    wiki_root: String,
    #[serde(default)]
    top_k: Option<usize>,
}

#[cfg(feature = "wiki-embedded")]
async fn wiki_search_handler(Json(req): Json<WikiSearchRequest>) -> Json<serde_json::Value> {
    let root = PathBuf::from(&req.wiki_root);
    match llmwiki_physis::search_bm25(&root, &req.query, req.top_k.unwrap_or(10)) {
        Ok(results) => Json(serde_json::json!({
            "count": results.len(),
            "results": llmwiki_physis::results_to_json(&results),
        })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string(), "results": [] })),
    }
}

#[cfg(feature = "wiki-embedded")]
#[derive(Deserialize)]
struct WikiIndexRequest {
    wiki_root: String,
    #[serde(default)]
    incremental: bool,
}

#[cfg(feature = "wiki-embedded")]
async fn wiki_index_handler(Json(req): Json<WikiIndexRequest>) -> Json<serde_json::Value> {
    let root = PathBuf::from(&req.wiki_root);
    match llmwiki_physis::index(&root, req.incremental) {
        Ok(s) => Json(serde_json::json!({
            "added": s.added, "updated": s.updated,
            "skipped": s.skipped, "removed": s.removed,
        })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

#[derive(Serialize)]
struct HeatmapResponse {
    table: String,
    matrix: Vec<Vec<f32>>,
}

async fn heatmap_handler(
    State(state): State<SharedState>,
) -> Json<HeatmapResponse> {
    let s = &state.shared;
    let mut grid = SemioticGrid::new();
    for def in s.ontology.human_domains.values() {
        if let (Some(d), Some(m)) = (def.domain.as_ref().and_then(|d| HumanDomain::from_str(d)),
                                       def.mode.as_ref().and_then(|m| HumanMode::from_str(m))) {
            grid.classify(&def.name, d, m);
        }
    }
    let table = physis_pro::output::format_heatmap_table(&grid);
    let matrix = grid.heatmap_matrix();
    Json(HeatmapResponse { table, matrix })
}

// ── Classify Handler ──────────────────────────────────────────────

#[derive(Deserialize)]
struct ClassifyRequest {
    text: String,
    /// Optional base64-encoded image bytes (PNG/JPEG). If present, image
    /// embedding is used instead of text embedding.
    #[serde(default)]
    image: Option<String>,
    /// BYOK re-ranker: a caller-supplied LLM key activates the Gemma re-ranker
    /// for this request using the caller's own provider (no server key needed).
    /// Defaults to an OpenAI-compatible Gemini endpoint + `gemini-2.5-flash`.
    #[serde(default)]
    llm_key: Option<String>,
    #[serde(default)]
    llm_base_url: Option<String>,
    #[serde(default)]
    llm_model: Option<String>,
}

#[derive(Serialize)]
struct ClassifyResponse {
    results: Vec<CellScore>,
    top: CellScore,
}

/// Gemma re-ranker (Phase 6G): when the top-2 cells are close (vector classify is
/// uncertain), ask the LLM cascade to pick among the top candidates. Returns the
/// chosen (domain, mode). Best-effort — any failure leaves the vector result.
/// Text-only (the prompt is textual). Gated by `PHYSIS_RERANK`; margin via
/// `PHYSIS_RERANK_MARGIN` (default 0.05). Most queries skip the LLM entirely.
async fn rerank_cell(
    cascade: &ProviderCascade,
    text: &str,
    candidates: &[CellScore],
    task_hint: &str,
) -> Option<(String, String)> {
    use physis_pro::ai::provider::{Content, RouteHint};
    let options = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let ex = c.entries.iter().take(4).cloned().collect::<Vec<_>>().join(", ");
            format!("{}. {}×{} (e.g. {ex})", i + 1, c.domain, c.mode)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let user = format!(
        "Activity: \"{text}\"\nWhich cell fits best?\n{options}\n\nAnswer with ONLY the option number."
    );
    let messages = vec![
        Message {
            role: "system".into(),
            content: Content::Text(
                "You classify a human activity into one DOMAIN×MODE cell. \
                 DOMAIN = what (HEAL/CONSTRUCT/FABRICATE/BOND/STUDY); \
                 MODE = how (LIFT=intense, REST=recover, WALK=steady, WORK=execute, \
                 CREATE=make, LEARN=study). Reply with ONLY a number.".into(),
            ),
            tool_calls: None, tool_call_id: None, name: None,
        },
        Message { role: "user".into(), content: Content::Text(user), tool_calls: None, tool_call_id: None, name: None },
    ];
    let hint = RouteHint { task_type: task_hint.into(), has_image: false };
    let (msg, _provider) = cascade.complete(&messages, None, &hint, Some(8)).await.ok()?;
    let txt = msg.content.unwrap_or_default();
    let n: usize = txt.trim().chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()?;
    candidates.get(n.checked_sub(1)?).map(|c| (c.domain.clone(), c.mode.clone()))
}

async fn classify_handler(
    State(state): State<SharedState>,
    Json(req): Json<ClassifyRequest>,
) -> Json<ClassifyResponse> {
    let embedding = embed_input(state.shared.clone(), req.text.clone(), req.image.clone()).await;
    let mut results = state.shared.classifier.classify(&embedding);

    // Re-rank ambiguous, text-only cases with the LLM. Activated by the server
    // (PHYSIS_RERANK) OR by a caller-supplied key (BYOK) — the latter lets users
    // bring their own provider without a server key (and pay for their own calls).
    let byok = req.llm_key.as_ref().filter(|k| !k.is_empty()).map(|key| {
        let mut c = ProviderCascade::new();
        c.add(ProviderConfig {
            name: "byok".into(),
            base_url: req.llm_base_url.clone()
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta/openai".into()),
            model: req.llm_model.clone().unwrap_or_else(|| "gemini-2.5-flash".into()),
            api_key: key.clone(),
            supports_vision: false,
            supports_tools: false,
            max_tokens: 64,
            timeout_secs: 15.0,
        });
        c
    });
    let rerank_on = byok.is_some() || std::env::var("PHYSIS_RERANK").is_ok();
    if rerank_on && req.image.is_none() && results.len() >= 2 {
        let margin_thresh: f32 = std::env::var("PHYSIS_RERANK_MARGIN")
            .ok().and_then(|s| s.parse().ok()).unwrap_or(0.05);
        if results[0].score - results[1].score < margin_thresh {
            let topk = results[..results.len().min(4)].to_vec();
            // BYOK uses the caller's provider with the "smart" hint (so their chosen
            // model isn't dropped by the fast-path gpt-4 filter); else the server
            // cascade on the "fast" path.
            let (cascade, hint): (&ProviderCascade, &str) = match &byok {
                Some(c) => (c, "smart"),
                None => (state.shared.cascade.as_ref(), "fast"),
            };
            if let Some((d, m)) = rerank_cell(cascade, &req.text, &topk, hint).await {
                if let Some(pos) = results.iter().position(|c| c.domain == d && c.mode == m) {
                    let chosen = results.remove(pos);
                    results.insert(0, chosen);
                }
            }
        }
    }

    let top = results.first().cloned().unwrap_or(CellScore {
        domain: "unknown".into(),
        mode: "unknown".into(),
        score: 0.0,
        entries: vec![],
    });
    Json(ClassifyResponse { results, top })
}

// ── Embed Handler ─────────────────────────────────────────────────
// Raw vector embedding — for pgvector storage and semantic search.

#[derive(Deserialize)]
struct EmbedRequest {
    text: String,
    /// Optional base64-encoded image bytes (PNG/JPEG). If present, image
    /// embedding is used instead of text embedding.
    #[serde(default)]
    image: Option<String>,
}

#[derive(Serialize)]
struct EmbedResponse {
    embedding: Vec<f32>,
    dim: usize,
}

#[derive(Deserialize)]
struct VideoEmbedRequest {
    /// Server-side path (confined to scan_root), OR base64 below.
    #[serde(default)]
    path: Option<String>,
    /// Base64-encoded video bytes (alternative to `path`).
    #[serde(default)]
    video_base64: Option<String>,
    /// Container hint for base64 input (e.g. "mp4", "webm"). Default "mp4".
    #[serde(default)]
    ext: Option<String>,
    /// Frames sampled per second. Default 1.0.
    #[serde(default)]
    fps: Option<f32>,
    /// Max frames embedded. Default 32.
    #[serde(default)]
    max_frames: Option<usize>,
}

#[derive(Serialize)]
struct VideoEmbedResponse {
    embedding: Vec<f32>,
    dim: usize,
    /// Frames that contributed to the pooled vector.
    frames: usize,
}

async fn embed_handler(
    State(state): State<SharedState>,
    Json(req): Json<EmbedRequest>,
) -> Json<EmbedResponse> {
    let embedding = embed_input(state.shared.clone(), req.text.clone(), req.image.clone()).await;
    let dim = embedding.len();
    Json(EmbedResponse { embedding, dim })
}

/// Embed a video into the shared space: ffmpeg frame-sample → image-embed each →
/// temporal mean-pool. Runs on a blocking thread (ffmpeg + N inferences).
async fn embed_video_handler(
    State(state): State<SharedState>,
    Json(req): Json<VideoEmbedRequest>,
) -> Result<Json<VideoEmbedResponse>, (StatusCode, Json<ErrorResponse>)> {
    let opts = physis_pro::video::VideoEmbedOptions {
        fps: req.fps.unwrap_or(1.0).clamp(0.1, 30.0),
        max_frames: req.max_frames.unwrap_or(32).clamp(1, 128),
    };

    enum Src {
        Path(PathBuf),
        Bytes(Vec<u8>, String),
    }

    let src = if let Some(p) = req.path {
        // Same path-traversal guard as /scan: confine to scan_root.
        let canonical = PathBuf::from(&p)
            .canonicalize()
            .map_err(|e| err_response(StatusCode::BAD_REQUEST, &format!("cannot resolve path: {e}")))?;
        if !canonical.starts_with(state.scan_root()) {
            return Err(err_response(
                StatusCode::FORBIDDEN,
                &format!("path outside scan root ({})", state.scan_root().display()),
            ));
        }
        Src::Path(canonical)
    } else if let Some(b64) = req.video_base64 {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| err_response(StatusCode::BAD_REQUEST, &format!("base64 decode failed: {e}")))?;
        Src::Bytes(bytes, req.ext.unwrap_or_else(|| "mp4".to_string()))
    } else {
        return Err(err_response(StatusCode::BAD_REQUEST, "provide `path` or `video_base64`"));
    };

    let shared = state.shared.clone();
    let result = tokio::task::spawn_blocking(move || {
        let emb = shared.embedder.as_ref();
        match src {
            Src::Path(p) => physis_pro::video::embed_video_file(&p, emb, &opts),
            Src::Bytes(b, ext) => physis_pro::video::embed_video_bytes(&b, &ext, emb, &opts),
        }
    })
    .await
    .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("video task failed: {e}")))?;

    match result {
        Some(r) => Ok(Json(VideoEmbedResponse {
            dim: r.embedding.len(),
            embedding: r.embedding,
            frames: r.frames,
        })),
        None => Err(err_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "video embedding unavailable: ffmpeg missing, embedder has no image path, or no frames decoded",
        )),
    }
}

// ── Main ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    env_logger::init();
    dotenvy::dotenv().ok();

    let config = PhysisConfig::default();
    let ontology = OntologyLoader::load_all(&config);
    let mapper = OntologyMapper::new(ontology.clone(), config.embed_dim);
    let actor = PDCActor::new(config.pdca_stagnant_threshold, config.pdca_stagnant_window);
    let dreams = DreamEngine::new();
    let mut core = PhysisCore::new();

    // Initialize Holarchy
    let onnx = Arc::new(OnnxHolon::spawn());
    let (graph_tx, graph_rx) = crossbeam_channel::unbounded();
    let ingest = Arc::new(IngestRing::spawn(onnx.get_tx(), graph_tx));
    let rachmaninov = Arc::new(Mutex::new(RachmaninovHolon::new()));

    // Select the embedder, tracking which kind actually loaded. `kind` is the
    // real provenance of every vector this process produces — surfaced at startup
    // and on /api/v1/embedder so a silent fall-through to random projection
    // (P0 in PREMORTEM.md) can never masquerade as a working model.
    let (embedder, embedder_kind): (Box<dyn VectorEmbed>, &'static str) =
    if matches!(std::env::var("PHYSIS_EMBEDDER").as_deref(), Ok("random-projection") | Ok("rp")) {
        // Fast-start override: skip the ONNX embedder cascade entirely. Comparable
        // vectors are enough for coherence/machine recall, and loading ONNX models
        // off slow storage stalls cold starts. Default path (below) is unchanged:
        // the real embedder cascade with random-projection only as last resort.
        log::info!("PHYSIS_EMBEDDER=random-projection: fast-start, ONNX cascade skipped.");
        (Box::new(RandomProjectionEmbedder::new(config.embed_dim)), "random-projection")
    } else {
        #[cfg(feature = "embed-onnx")]
        {
            // BGE-base-en-v1.5 (768-d). Loads `onnx/model.onnx` + tokenizer from
            // models/bge-base-en-v1.5. Strong text-only fallback when Jina's quant
            // won't load under the current ort.
            let make_bge = || -> Option<(Box<dyn VectorEmbed>, &'static str)> {
                let cfg = physis_pro::config::OnnxConfig {
                    enabled: true,
                    model_dir: Some("models/bge-base-en-v1.5".to_string()),
                    dim: 768,
                    max_length: 512,
                };
                let b = OnnxEmbedder::with_config(&cfg);
                b.is_available().then(|| (Box::new(b) as Box<dyn VectorEmbed>, "bge-base"))
            };
            // Self-test gate: accept a candidate only if it loads AND produces
            // semantic vectors. A model that loads but emits garbage (e.g. Jina
            // fp16, whose float16 outputs the f32 extractor can't read → random
            // fallback) is skipped so the next working candidate wins instead of
            // the engine silently running on noise. PREMORTEM.md P4.
            let gate = |opt: Option<(Box<dyn VectorEmbed>, &'static str)>| {
                opt.and_then(|(e, k)| {
                    if embedder_self_test(e.as_ref()) {
                        Some((e, k))
                    } else {
                        log::warn!("embedder '{k}' loaded but FAILED the semantic self-test — skipping to next candidate.");
                        None
                    }
                })
            };
            // Try the configured primary embedder, then fall through Jina → BGE → CLIP → MiniLM → RP
            let try_primary = || -> Option<(Box<dyn VectorEmbed>, &'static str)> {
                let ec = config.embedders.primary.as_ref()?;
                if !ec.enabled { return None; }
                match ec.kind {
                    EmbedderKindConfig::JinaV2 => {
                        let j = JinaEmbedder::with_model_dir(&ec.model_dir);
                        j.is_available().then(|| (Box::new(j) as Box<dyn VectorEmbed>, "jina-v2"))
                    }
                    EmbedderKindConfig::Clip => {
                        let c = ClipEmbedder::with_config(&config.onnx);
                        c.is_available().then(|| (Box::new(c) as Box<dyn VectorEmbed>, "clip"))
                    }
                    EmbedderKindConfig::MiniLM => {
                        let m = OnnxEmbedder::with_config(&config.onnx);
                        m.is_available().then(|| (Box::new(m) as Box<dyn VectorEmbed>, "minilm"))
                    }
                    EmbedderKindConfig::BgeBase => make_bge(),
                    EmbedderKindConfig::RandomProjection => {
                        Some((Box::new(RandomProjectionEmbedder::new(config.embed_dim)), "random-projection"))
                    }
                }
            };
            gate(try_primary())
                .or_else(|| gate({
                    let j = JinaEmbedder::with_model_dir("models/jina-clip-v2");
                    j.is_available().then(|| (Box::new(j) as Box<dyn VectorEmbed>, "jina-v2"))
                }))
                .or_else(|| gate(make_bge()))
                .or_else(|| gate({
                    let c = ClipEmbedder::with_config(&config.onnx);
                    c.is_available().then(|| (Box::new(c) as Box<dyn VectorEmbed>, "clip"))
                }))
                .or_else(|| gate({
                    let m = OnnxEmbedder::with_config(&config.onnx);
                    m.is_available().then(|| (Box::new(m) as Box<dyn VectorEmbed>, "minilm"))
                }))
                .unwrap_or_else(|| (Box::new(RandomProjectionEmbedder::new(config.embed_dim)), "random-projection"))
        }
        #[cfg(not(feature = "embed-onnx"))]
        { (Box::new(RandomProjectionEmbedder::new(config.embed_dim)), "random-projection") }
    };
    let embed_dim = embedder.dimension();

    // Semantic self-test (PREMORTEM.md P4): a model can load yet silently emit
    // garbage (opset/quant mismatch → inference falls back to random vectors).
    // Verify dog≈puppy lands clearly closer than dog≈tax-spreadsheet; if not,
    // the embedder is non-semantic regardless of which kind "loaded".
    let self_test_ok = embedder_self_test(embedder.as_ref());
    let mut degraded = embedder_kind == "random-projection";
    if !degraded && !self_test_ok {
        log::error!("embedder '{embedder_kind}' loaded but FAILED the semantic self-test — output is non-semantic (PREMORTEM.md P4).");
        eprintln!("[physis] WARNING: embedder '{embedder_kind}' failed semantic self-test — treating as DEGRADED.");
        degraded = true;
    }

    if degraded {
        log::error!("════════════════════════════════════════════════════════════════");
        log::error!("  DEGRADED: no ONNX model loaded — using RANDOM-PROJECTION embedder.");
        log::error!("  Classification output is NOT semantic; cosine scores are noise.");
        log::error!("  Set MODEL_DIR / MODEL_BASE_URL so real weights load. See PREMORTEM.md P0.");
        log::error!("════════════════════════════════════════════════════════════════");
        eprintln!("[physis] WARNING: DEGRADED random-projection embedder active — classification is non-semantic.");
    } else {
        log::info!("Embedder loaded: kind={embedder_kind} dim={embed_dim}");
    }
    core.set_embedder_id(embedder_kind); // vector provenance on the default tenant

    // Pre-compute per-cell centroids from the human grid + every extra
    // domain-specific ontology (software, fitness, finance, …). The classify
    // path lives in the library so it is unit-testable (tests/classification.rs).
    let classifier = CellClassifier::build(&ontology, embedder.as_ref());
    log::info!("classifier ready: {} populated cells", classifier.cell_count());

    let cascade = Arc::new(ProviderCascade::from_env());
    log::info!("AI cascade: {} providers loaded", cascade.provider_names().len());

    let port = std::env::var("PHYSIS_PORT").unwrap_or_else(|_| "19876".to_string());
    let host = std::env::var("PHYSIS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

    // ── Security gate (PREMORTEM.md P6) ──────────────────────────────────
    // A non-loopback bind with no token would publish an unauthenticated API
    // that also proxies paid LLM providers (/api/v1/chat) — an open relay.
    // Refuse to start in that configuration.
    let api_token = std::env::var("PHYSIS_API_TOKEN").ok().filter(|t| !t.is_empty());
    let is_loopback = matches!(host.as_str(), "127.0.0.1" | "::1" | "localhost");
    if !is_loopback && api_token.is_none() {
        log::error!("Refusing to bind {host} without PHYSIS_API_TOKEN — would expose an unauthenticated API + open LLM relay.");
        eprintln!("[physis] FATAL: non-loopback bind ({host}) requires PHYSIS_API_TOKEN. Set it, or bind 127.0.0.1.");
        std::process::exit(1);
    }
    if api_token.is_some() {
        log::info!("API auth enabled: Bearer token required on protected routes.");
    } else {
        log::warn!("API auth DISABLED (loopback dev bind, PHYSIS_API_TOKEN unset).");
    }

    // Confine /api/v1/scan to a root (path-traversal guard). Defaults to the
    // data dir; override with PHYSIS_SCAN_ROOT. Canonicalized for real-path compare.
    let scan_root = std::env::var("PHYSIS_SCAN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| config.data_dir.clone());
    let scan_root = scan_root.canonicalize().unwrap_or(scan_root);
    log::info!("scan root: {}", scan_root.display());

    // Seed the wiki with default physis documentation pages and build the
    // Tantivy BM25 index. Safe to call on every startup (incremental index).
    // Must happen before config is moved into SharedCtx below.
    #[cfg(feature = "wiki-embedded")]
    {
        let wiki_path = config.data_dir.join("wiki");
        physis_pro::wiki::ensure_wiki_indexed(&wiki_path);
    }

    let shared = Arc::new(SharedCtx {
        config,
        cascade: cascade.clone(),
        ontology,
        embedder,
        classifier,
        embed_dim,
        embedder_kind,
        degraded,
        self_test: self_test_ok,
        ingest: ingest.clone(),
        api_token,
    });
    let state = Arc::new(AppCtx {
        shared: shared.clone(),
        state: Mutex::new(RuntimeState {
            core,
            mapper,
            actor,
            dreams,
            goals: Vec::new(),
        }),
        tenants: Mutex::new(HashMap::new()),
        scan_root: std::sync::RwLock::new(scan_root),
        #[cfg(feature = "mcp")]
        mcp_sessions: std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
    });

    let state_clone = state.clone();
    // 1. Holarchy Graph Updater
    std::thread::spawn(move || {
        while let Ok((payload, embedding)) = graph_rx.recv() {
            let mut s = state_clone.lock();
            s.core.holarchy.add_node(payload, embedding);
        }
    });

    let onnx_clone = onnx.clone();
    let rachmaninov_clone = rachmaninov.clone();
    let state_clone_2 = state.clone();
    // 2. PDCA/Rachmaninov Tick Cycle
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            let mut s = state_clone_2.lock();
            let mut rach = rachmaninov_clone.lock().unwrap_or_else(|e| e.into_inner());
            s.core.tick_holarchy(&onnx_clone, &mut rach);
        }
    });

    let ingest_sensory = ingest.clone();
    // 3. Sensory Server
    tokio::spawn(async move {
        let socket_path = "/tmp/physis_sensory.sock";
        if let Err(e) = physis_pro::sensory::listener::start_sensory_server(socket_path, ingest_sensory).await {
            log::error!("Sensory server error: {}", e);
        }
    });

    // 4. Watch-mode auto-ingest (Phase 7B): poll PHYSIS_WATCH_DIR and register
    //    new/changed files as labeled core nodes, so the wiki stays fed without a
    //    manual /scan. Confined to scan_root; dedup makes repeated polls cheap.
    //    Runs on a std::thread (blocking FS + hashing) off the async runtime.
    if let Ok(watch_dir) = std::env::var("PHYSIS_WATCH_DIR") {
        match PathBuf::from(&watch_dir).canonicalize() {
            Ok(canon) if canon.starts_with(state.scan_root()) => {
                let interval_secs: u64 = std::env::var("PHYSIS_WATCH_INTERVAL_SECS")
                    .ok().and_then(|s| s.parse().ok()).unwrap_or(30);
                let cache_path = state.shared.config.data_dir.join("watch_hash_cache.json");
                let scanner = physis_pro::network::NetworkScanner::new(
                    vec![canon.clone()], cache_path, interval_secs);
                let watch_state = state.clone();
                log::info!("Watch-mode: polling {} every {}s", canon.display(), interval_secs);
                std::thread::spawn(move || loop {
                    let diffs = scanner.scan_all();
                    let mut labels: Vec<String> = Vec::new();
                    for d in &diffs {
                        for f in d.new.iter().chain(d.changed.iter()) {
                            labels.extend(file_labels(f));
                        }
                    }
                    if !labels.is_empty() {
                        // Batch-embed outside the lock (CPU-heavy); register under it.
                        // This thread is already off the async runtime.
                        let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
                        let embs = watch_state.shared.embedder.embed_batch(&refs);
                        let mut s = watch_state.lock();
                        for (emb, label) in embs.into_iter().zip(labels) {
                            s.core.register_node_vec_labeled(emb, Some(label));
                        }
                    }
                    scanner.update_cache(&diffs);
                    std::thread::sleep(std::time::Duration::from_secs(interval_secs));
                });
            }
            Ok(canon) => log::error!(
                "PHYSIS_WATCH_DIR {} outside scan root {} — watch disabled",
                canon.display(), state.scan_root().display()),
            Err(e) => log::error!("PHYSIS_WATCH_DIR invalid ({watch_dir}): {e} — watch disabled"),
        }
    }

    // Public routes: liveness + embedder provenance (no secrets, needed by the
    // Railway healthcheck and ops).
    let public = Router::new()
        .route("/health", get(health))
        .route("/api/v1/embedder", get(embedder_info_handler));

    // MCP SSE transport — exposed without auth (the URL + port are not discoverable;
    // add token-based guard later if needed).
    #[cfg(feature = "mcp")]
    let public = public
        .route("/mcp/sse", get(mcp_sse_handler))
        .route("/mcp/message", post(mcp_message_handler));

    // Protected routes: everything that mutates state, reads data, or proxies
    // to paid LLM providers. Gated by the Bearer-token middleware.
    let protected = Router::new()
        .route("/api/v1/chat", post(chat_handler))
        .route("/api/v1/providers", get(providers_handler))
        .route("/api/v1/stats", get(stats_handler))
        .route("/api/v1/query", get(query_handler))
        .route("/api/v1/scan", post(scan_handler))
        .route("/api/v1/dream/generate", post(dream_generate_handler))
        .route("/api/v1/dream/evaluate", post(dream_evaluate_handler))
        .route("/api/v1/coherence/snapshot", get(coherence_snapshot_handler))
        .route("/api/v1/coherence/nodes", get(coherence_nodes_handler))
        .route("/api/v1/coherence/register", post(coherence_register_handler))
        .route("/api/v1/context/filter", post(context_filter_handler))
        .route("/api/v1/compress/logs", post(compress_logs_handler))
        .route("/api/v1/goals", post(ingest_prompt_handler))
        .route("/api/v1/pdca/plan", get(pdca_plan_handler))
        .route("/api/v1/pdca/act", post(pdca_act_handler))
        .route("/api/v1/pdca/stats", get(pdca_stats_handler))
        .route("/api/v1/config/reload", post(config_reload_handler))
        .route("/api/v1/reconstruct", post(reconstruct_handler))
        .route("/api/v1/translate", post(translate_handler))
        .route("/api/v1/holarchy/ingest", post(holarchy_ingest_handler))
        .route("/api/v1/holarchy/edge", post(holarchy_edge_handler))
        .route("/api/v1/holarchy/snapshot", get(holarchy_snapshot_handler))
        .route("/api/v1/holarchy/trajectory", get(holarchy_trajectory_handler))
        .route("/api/v1/holarchy/graph", get(holarchy_graph_handler))
        // Semiotic / Ontology endpoints
        .route("/api/v1/ontology/list", get(ontology_list_handler))
        .route("/api/v1/semiotic/grid", get(semiotic_grid_handler))
        .route("/api/v1/semiotic/triangle", get(semiotic_triangle_handler))
        .route("/api/v1/semiotic/square", get(greimas_square_handler))
        .route("/api/v1/semiotic/heatmap", get(heatmap_handler))
        .route("/api/v1/classify", post(classify_handler))
        .route("/api/v1/embed", post(embed_handler))
        .route("/api/v1/embed/video", post(embed_video_handler))
        .route("/api/v1/category/diagram", post(category_diagram_handler));

    // In-process wiki routes (Tantivy BM25) — only when compiled with the feature.
    #[cfg(feature = "wiki-embedded")]
    let protected = protected
        .route("/api/v1/wiki/search", post(wiki_search_handler))
        .route("/api/v1/wiki/index", post(wiki_index_handler));

    let protected =
        protected.route_layer(middleware::from_fn_with_state(state.clone(), require_auth));

    let app = public.merge(protected).with_state(state);

    let addr = format!("{}:{}", host, port);
    println!("Physis Web API listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ── MCP SSE transport ──────────────────────────────────────────────────────
// Model Context Protocol over HTTP/SSE.
//   GET  /mcp/sse?session_id=X  → SSE stream (endpoint + message events)
//   POST /mcp/message           → send JSON-RPC to server
//
// Each SSE session creates a channel-based transport for rmcp::serve_server.

#[cfg(feature = "mcp")]
async fn mcp_sse_handler(
    state: axum::extract::State<Arc<AppCtx>>,
) -> Result<
    axum::response::sse::Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, std::io::Error>>>,
    axum::response::Response<String>,
> {
    use futures::channel::mpsc;
    use futures::{SinkExt, StreamExt};
    use rmcp::{ServiceExt, model::*};

    let session_id = format!("{:016x}", rand::random::<u128>());

    // Server → client channel: rmcp writes responses here
    let (server_tx, server_rx) = mpsc::channel::<ServerJsonRpcMessage>(64);
    // Client → server channel: POST handler writes here
    let (client_tx, client_rx) = mpsc::channel::<ClientJsonRpcMessage>(64);

    // Register client_tx so the POST handler can deliver messages
    state.mcp_sessions.lock().await.insert(session_id.clone(), client_tx);

    // Create fresh PhysisMcpServer using the shared config
    let handler = physis_pro::mcp::PhysisMcpServer::new(state.shared.config.clone());

    // Build the transport with error mapping (mpsc::SendError → io::Error)
    use std::io;
    let transport = (server_tx.sink_map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "mcp session closed")), client_rx);

    // rmcp's serve_inner spawns an internal task — it returns RunningService
    // immediately while the message loop runs in the background. Do NOT clean
    // up the session here. The session (client_tx sender) must stay alive in
    // the map until the SSE connection ends; otherwise the message loop's
    // receiver gets None and terminates prematurely.
    let sid = session_id.clone();
    tokio::spawn(async move {
        if let Err(e) = handler.serve(transport).await {
            eprintln!("[mcp] session {sid} error: {e}");
        }
    });

    // Build the SSE response. When the client disconnects, server_rx ends
    // and we remove the session from the map (dropping client_tx), which
    // eventually causes the serve_inner loop to exit.
    let post_path = format!("/mcp/message?session_id={session_id}");
    let endpoint_event = axum::response::sse::Event::default()
        .event("endpoint")
        .data(post_path);

    let message_stream = server_rx.map(|msg| {
        serde_json::to_string(&msg)
            .map(|json| axum::response::sse::Event::default().event("message").data(json))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    });

    let sid = session_id.clone();
    let mcp = state.mcp_sessions.clone();
    let cleanup = futures::stream::once(async move {
        mcp.lock().await.remove(&sid);
        // The SSE connection is closing; this event will not reach the client.
        Ok(axum::response::sse::Event::default().event("close").data(""))
    });

    let stream = futures::stream::once(futures::future::ok(endpoint_event))
        .chain(message_stream)
        .chain(cleanup);
    Ok(axum::response::sse::Sse::new(stream))
}

#[cfg(feature = "mcp")]
async fn mcp_message_handler(
    state: axum::extract::State<Arc<AppCtx>>,
    axum::extract::Query(query): axum::extract::Query<std::collections::HashMap<String, String>>,
    axum::Json(msg): axum::Json<serde_json::Value>,
) -> axum::http::StatusCode {
    use futures::SinkExt;
    use rmcp::model::ClientJsonRpcMessage;

    let session_id = match query.get("session_id").or_else(|| query.get("sessionId")) {
        Some(id) => id,
        None => return axum::http::StatusCode::BAD_REQUEST,
    };

    let msg: ClientJsonRpcMessage = match serde_json::from_value(msg) {
        Ok(m) => m,
        Err(_) => return axum::http::StatusCode::BAD_REQUEST,
    };

    let mut tx = match state.mcp_sessions.lock().await.get(session_id).cloned() {
        Some(tx) => tx,
        None => return axum::http::StatusCode::NOT_FOUND,
    };

    if tx.send(msg).await.is_err() {
        return axum::http::StatusCode::GONE;
    }
    axum::http::StatusCode::ACCEPTED
}