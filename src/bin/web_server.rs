use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::{
    extract::{Path, Query, Request, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
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

use physis_pro::oee::{plant_oee, OeeBoard, OeeReport, OeeShift, PlantOee, StopReason};
use physis_pro::maintenance::{
    project_time_to_limit, AnomalyScore, HealthBaseline, MaintenanceMonitor, RulEstimate, Sample,
    DEFAULT_SIGMA_THRESHOLD,
};
use physis_pro::industrial::{MachineReading, MachineStatus};
use physis_pro::classify::{CellClassifier, CellScore};
use physis_pro::graph::{IngestRing};
use physis_pro::ai::onnx_worker::OnnxHolon;
use physis_pro::ai::provider::{ProviderCascade, ProviderConfig, Message};
use physis_pro::gantt::{compute_schedule, compute_trajectory, AbstractDomain, GanttScheduler, ScheduleResult, TaskInput};
use physis_pro::rachmaninov::RachmaninovHolon;
use physis_pro::life_os::tracker::{Charge, DriftReport, LifeOSGoal, LifeOSTracker, TodayPlan};

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
    scheduler: GanttScheduler,
    lifeos: LifeOSTracker,
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
            scheduler: GanttScheduler::new(Box::new(AbstractDomain)),
            lifeos: LifeOSTracker::new(),
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
    /// Plant-wide OEE board. Shop-floor equipment is shared infra, not per-tenant
    /// reasoning state, so it lives here behind one mutex.
    oee: Mutex<OeeBoard>,
    /// Plant-wide predictive-maintenance monitor (health baselines + trends).
    maintenance: Mutex<MaintenanceMonitor>,
    /// Durable state store (sled on the data-dir volume). `None` when it can't be
    /// opened (e.g. no writable volume in local dev) → state is in-memory only.
    /// Cheap to clone (sled::Db is an Arc internally).
    db: Option<sled::Db>,
    /// Dynamically-promoted ontology axes (Phase 6). Persisted in `db`; global
    /// across tenants (operator-installed). Merged as anchors for the axis layer.
    axes: Mutex<physis_pro::axes::AxisStore>,
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
            let rt = map.entry(uid.to_string()).or_insert_with(|| {
                let mut rt = RuntimeState::new(&self.shared);
                // Rehydrate this tenant's persisted LifeOS goals on first access.
                if let Some(db) = &self.db {
                    rt.lifeos = LifeOSTracker::load(db, uid);
                }
                rt
            });
            f(rt)
        }
    }

    /// Borrow the durable store, if persistence is enabled.
    fn db(&self) -> Option<&sled::Db> {
        self.db.as_ref()
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

#[derive(Deserialize)]
struct AllNodesParams {
    #[serde(default = "default_all_limit")]
    limit: usize,
}
fn default_all_limit() -> usize { 500 }

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
    physis_pro::embed::semantic_self_test(e)
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

// ── Graph file manager (backs the /ui page) ─────────────────────────────────
// fs/graph: scan a sandboxed dir → theme↔file semantic graph (FileThemeGraph),
// embeddings stripped. fs/file: sandboxed text preview. fs/move: sandboxed
// rename within scan_root. Every path is confined to scan_root (PREMORTEM P6).

/// Resolve an existing user-supplied path and confine it to scan_root.
fn sandbox_existing(state: &SharedState, raw: &str) -> Result<PathBuf, (StatusCode, Json<ErrorResponse>)> {
    let p = PathBuf::from(raw);
    if !p.exists() {
        return Err(err_response(StatusCode::BAD_REQUEST, &format!("path not found: {raw}")));
    }
    let canon = p
        .canonicalize()
        .map_err(|e| err_response(StatusCode::BAD_REQUEST, &format!("cannot resolve path: {e}")))?;
    if !canon.starts_with(state.scan_root()) {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            &format!("path outside scan root ({})", state.scan_root().display()),
        ));
    }
    Ok(canon)
}

#[derive(Deserialize)]
struct FsGraphQuery {
    dir: String,
    /// "cells" (default) = fixed 70-cell semiotic grid as hubs;
    /// "discovered" = emergent clusters mined from the scanned files (content map).
    #[serde(default)]
    grid: Option<String>,
    /// Discovered-mode cluster count (k-means). Omitted → auto ≈ √(files), 2..16.
    #[serde(default)]
    k: Option<usize>,
}

#[derive(Serialize)]
struct FsThemeDto {
    id: usize,
    name: String,
    category: Option<String>,
    domain: String,
    mode: String,
}

#[derive(Serialize)]
struct FsFileDto {
    id: usize,
    path: String,
    title: String,
    kind: String,
}

#[derive(Serialize)]
struct FsEdgeDto {
    theme_id: usize,
    file_id: usize,
    score: f32,
}

/// Semantic similarity edge between two files (undirected; ids index `files`).
#[derive(Serialize)]
struct FsSimEdge {
    source: usize,
    target: usize,
    score: f32,
}

#[derive(Serialize)]
struct FsGraphResponse {
    root: String,
    categories: Vec<String>,
    themes: Vec<FsThemeDto>,
    files: Vec<FsFileDto>,
    edges: Vec<FsEdgeDto>,
    /// file↔file edges by cosine of the BGE vectors — the classified-item network.
    file_edges: Vec<FsSimEdge>,
}

/// GET /api/v1/fs/graph?dir=… — scan a directory into a theme↔file graph.
async fn fs_graph_handler(
    State(state): State<SharedState>,
    Query(q): Query<FsGraphQuery>,
) -> Result<Json<FsGraphResponse>, (StatusCode, Json<ErrorResponse>)> {
    let canonical = sandbox_existing(&state, &q.dir)?;
    let root_str = canonical.display().to_string();

    // Theme hubs are the semiotic cells (≤70) and reuse the classifier's
    // *precomputed* centroids — so a scan only embeds the files themselves (a
    // handful), never the ~600 ontology themes. (Re-embedding the ontology per
    // request made BGE scans take minutes; centroids are built once at startup.)
    const TOP_K: usize = 5;
    const MIN_SCORE: f32 = 0.20;
    // file↔file network: each file linked to its top-K nearest neighbours above τ.
    const SIM_TOP_K: usize = 4;
    const SIM_TAU: f32 = 0.55;
    // discovered-mode clustering: corpus carved into emergent groups via spherical
    // k-means (auto-k ≈ √files, or ?k=). Balanced topics + TF-IDF labels.
    const DISCOVER_MIN: usize = 2;
    let discovered = matches!(q.grid.as_deref(), Some("discovered"));
    let want_k = q.k;
    let shared = state.shared.clone();
    let (themes, files, edges, file_edges) = tokio::task::spawn_blocking(move || {
        let docs = physis_pro::ingest::ingest_all(&[canonical]);
        // Embed in BATCHES — one ORT session run per chunk amortizes per-call
        // overhead and lets the batch matmul use multiple cores, far cheaper than
        // N separate embed() runs (which serialize on the session Mutex). Order is
        // preserved so file ids stay stable.
        let bodies: Vec<&str> = docs.iter().map(|d| d.body.as_str()).collect();
        let embs: Vec<Vec<f32>> = bodies
            .chunks(32)
            .flat_map(|c| shared.embedder.embed_batch(c))
            .collect();
        let files: Vec<FsFileDto> = docs
            .iter()
            .enumerate()
            .map(|(fid, doc)| FsFileDto {
                id: fid,
                path: doc.path.display().to_string(),
                title: doc.title.clone(),
                kind: format!("{:?}", doc.kind),
            })
            .collect();
        let texts: Vec<String> = docs.iter().map(|d| d.body.clone()).collect();

        let mut themes: Vec<FsThemeDto> = Vec::new();
        let mut edges: Vec<FsEdgeDto> = Vec::new();

        if discovered {
            // Hubs are the corpus's OWN emergent clusters, labelled by top terms.
            // Clusters below DISCOVER_MIN are lumped into one "Uncategorized" hub so
            // every file appears on the content map.
            let k = want_k.unwrap_or_else(|| ((files.len() as f64).sqrt().round() as usize).clamp(2, 16));
            let clusters = physis_pro::axes::cluster_corpus_kmeans(&texts, &embs, k, 8);
            let mut uncategorized: Vec<usize> = Vec::new();
            for c in &clusters {
                if c.members.len() < DISCOVER_MIN {
                    uncategorized.extend(c.members.iter().copied());
                    continue;
                }
                let tid = themes.len();
                themes.push(FsThemeDto {
                    id: tid,
                    name: format!("{} ({})", c.label, c.members.len()),
                    category: Some("discovered".into()),
                    domain: "discovered".into(),
                    mode: c.label.clone(),
                });
                for &fid in &c.members {
                    let score = physis_pro::models::cosine_sim(&embs[fid], &c.centroid);
                    edges.push(FsEdgeDto { theme_id: tid, file_id: fid, score });
                }
            }
            if !uncategorized.is_empty() {
                let tid = themes.len();
                themes.push(FsThemeDto {
                    id: tid,
                    name: format!("Uncategorized ({})", uncategorized.len()),
                    category: Some("discovered".into()),
                    domain: "discovered".into(),
                    mode: "uncategorized".into(),
                });
                for fid in uncategorized {
                    edges.push(FsEdgeDto { theme_id: tid, file_id: fid, score: 0.5 });
                }
            }
        } else {
            // Fixed 70-cell semiotic grid via precomputed centroids.
            let mut theme_ids: std::collections::HashMap<(String, String), usize> = std::collections::HashMap::new();
            for (fid, emb) in embs.iter().enumerate() {
                for cs in shared.classifier.classify(emb).iter().take(TOP_K).filter(|c| c.score > MIN_SCORE) {
                    let key = (cs.domain.clone(), cs.mode.clone());
                    let tid = *theme_ids.entry(key).or_insert_with(|| {
                        let id = themes.len();
                        themes.push(FsThemeDto {
                            id,
                            name: format!("{} × {}", cs.domain, cs.mode),
                            category: Some(cs.domain.clone()),
                            domain: cs.domain.clone(),
                            mode: cs.mode.clone(),
                        });
                        id
                    });
                    edges.push(FsEdgeDto { theme_id: tid, file_id: fid, score: cs.score });
                }
            }
        }

        // file↔file similarity edges: top-K neighbours per file (cosine > τ),
        // deduped to one undirected edge per pair.
        let mut file_edges: Vec<FsSimEdge> = Vec::new();
        let mut seen: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for i in 0..embs.len() {
            let mut sims: Vec<(usize, f32)> = (0..embs.len())
                .filter(|&j| j != i)
                .map(|j| (j, physis_pro::models::cosine_sim(&embs[i], &embs[j])))
                .filter(|(_, s)| *s > SIM_TAU)
                .collect();
            sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            for (j, s) in sims.into_iter().take(SIM_TOP_K) {
                let key = if i < j { (i, j) } else { (j, i) };
                if seen.insert(key) {
                    file_edges.push(FsSimEdge { source: key.0, target: key.1, score: s });
                }
            }
        }
        (themes, files, edges, file_edges)
    })
    .await
    .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("graph build failed: {e}")))?;

    // Domains present among the cells that attracted a file.
    let mut categories: Vec<String> = themes
        .iter()
        .map(|t| t.domain.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    categories.sort();

    Ok(Json(FsGraphResponse {
        root: root_str,
        categories,
        themes,
        files,
        edges,
        file_edges,
    }))
}

#[derive(Deserialize)]
struct FsFileQuery {
    path: String,
}

#[derive(Serialize)]
struct FsFileResponse {
    path: String,
    size: u64,
    truncated: bool,
    content: String,
}

/// GET /api/v1/fs/file?path=… — sandboxed text preview (first 16 KB).
async fn fs_file_handler(
    State(state): State<SharedState>,
    Query(q): Query<FsFileQuery>,
) -> Result<Json<FsFileResponse>, (StatusCode, Json<ErrorResponse>)> {
    let canonical = sandbox_existing(&state, &q.path)?;
    if canonical.is_dir() {
        return Err(err_response(StatusCode::BAD_REQUEST, "path is a directory"));
    }
    const CAP: usize = 16 * 1024;
    let bytes = std::fs::read(&canonical)
        .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("read failed: {e}")))?;
    let size = bytes.len() as u64;
    let truncated = bytes.len() > CAP;
    let slice = &bytes[..bytes.len().min(CAP)];
    let content = String::from_utf8_lossy(slice).to_string();
    Ok(Json(FsFileResponse {
        path: canonical.display().to_string(),
        size,
        truncated,
        content,
    }))
}

#[derive(Deserialize)]
struct FsMoveRequest {
    from: String,
    to: String,
}

#[derive(Serialize)]
struct FsMoveResponse {
    from: String,
    to: String,
    moved: bool,
}

/// POST /api/v1/fs/move {from,to} — sandboxed rename within scan_root. Refuses
/// to escape the root, overwrite an existing target, or move a directory.
async fn fs_move_handler(
    State(state): State<SharedState>,
    Json(req): Json<FsMoveRequest>,
) -> Result<Json<FsMoveResponse>, (StatusCode, Json<ErrorResponse>)> {
    let from = sandbox_existing(&state, &req.from)?;
    if from.is_dir() {
        return Err(err_response(StatusCode::BAD_REQUEST, "moving directories is not allowed"));
    }
    // The destination does not exist yet, so confine its *parent* to scan_root.
    let to = PathBuf::from(&req.to);
    let parent = to
        .parent()
        .ok_or_else(|| err_response(StatusCode::BAD_REQUEST, "destination has no parent"))?;
    let parent_canon = parent
        .canonicalize()
        .map_err(|e| err_response(StatusCode::BAD_REQUEST, &format!("cannot resolve destination dir: {e}")))?;
    if !parent_canon.starts_with(state.scan_root()) {
        return Err(err_response(StatusCode::FORBIDDEN, "destination outside scan root"));
    }
    let file_name = to
        .file_name()
        .ok_or_else(|| err_response(StatusCode::BAD_REQUEST, "destination has no file name"))?;
    let dest = parent_canon.join(file_name);
    if dest.exists() {
        return Err(err_response(StatusCode::CONFLICT, "destination already exists"));
    }
    std::fs::rename(&from, &dest)
        .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("move failed: {e}")))?;
    Ok(Json(FsMoveResponse {
        from: from.display().to_string(),
        to: dest.display().to_string(),
        moved: true,
    }))
}

/// GET /ui — the graph file-manager single-page app (public; calls the
/// authenticated fs/* endpoints with a token the user pastes in the page).
async fn graph_fm_ui_handler() -> Html<&'static str> {
    Html(include_str!("ui/console.html"))
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

/// List ALL coherence nodes for the tenant (id + label + score), capped + sorted
/// by score desc. Unlike `/coherence/nodes` (similarity search, needs a query),
/// this enumerates the graph so a client can render/browse it without a query.
/// Per-tenant — NOT exposed via the public proxy.
async fn coherence_nodes_all_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<AllNodesParams>,
) -> Json<CoherenceNodesResponse> {
    let uid = user_key(&headers);
    let mut results: Vec<CoherenceNodeMatch> = state.with_user(&uid, |s| {
        s.core.nodes.values()
            .map(|n| CoherenceNodeMatch {
                id: n.id.clone(),
                label: n.label.clone().unwrap_or_default(),
                score: n.coherence_score,
            })
            .collect()
    });
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(params.limit);
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

// ── OEE / shop-floor ─────────────────────────────────────────────────────────
// Plant-wide Overall Equipment Effectiveness. The board is shared infra, so all
// of these touch `state.oee` (one mutex) and ignore the tenant header.

#[derive(Deserialize)]
struct OeeMachineRequest {
    machine_id: String,
    shift_length_secs: f32,
    ideal_cycle_time_secs: f32,
}

#[derive(Serialize)]
struct OkResponse {
    ok: bool,
    detail: String,
}

/// Register a machine / set its shift parameters.
async fn oee_machine_handler(
    State(state): State<SharedState>,
    Json(req): Json<OeeMachineRequest>,
) -> Json<OkResponse> {
    {
        let mut board = state.oee.lock().unwrap_or_else(|e| e.into_inner());
        board.upsert_machine(&req.machine_id, req.shift_length_secs, req.ideal_cycle_time_secs);
        if let Some(db) = state.db() { let _ = board.save(db); }
    }
    Json(OkResponse { ok: true, detail: format!("machine {} registered", req.machine_id) })
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum OeeEvent {
    /// A stoppage: `reason` is one of the six big losses
    /// (`breakdown`, `setup_adjustment`, `minor_stop`, `reduced_speed`, `planned_stop`).
    Stop {
        machine_id: String,
        reason: StopReason,
        duration_secs: f32,
        #[serde(default)]
        note: String,
    },
    /// Produced parts: `good` pass quality, `scrap` are defects/rework.
    Production {
        machine_id: String,
        good: u64,
        #[serde(default)]
        scrap: u64,
    },
    /// Start a fresh shift (zero counts + stops, keep config).
    StartShift { machine_id: String },
}

/// Record a shop-floor event (stop / production / shift start).
async fn oee_event_handler(
    State(state): State<SharedState>,
    Json(ev): Json<OeeEvent>,
) -> (StatusCode, Json<OkResponse>) {
    let mut board = state.oee.lock().unwrap_or_else(|e| e.into_inner());
    let (ok, detail) = match ev {
        OeeEvent::Stop { machine_id, reason, duration_secs, note } => {
            let ok = board.record_stop(&machine_id, reason, duration_secs, note);
            (ok, format!("stop on {machine_id}"))
        }
        OeeEvent::Production { machine_id, good, scrap } => {
            let ok = board.record_production(&machine_id, good, scrap);
            (ok, format!("+{good} good, +{scrap} scrap on {machine_id}"))
        }
        OeeEvent::StartShift { machine_id } => {
            let ok = board.start_shift(&machine_id);
            (ok, format!("shift started on {machine_id}"))
        }
    };
    if ok {
        if let Some(db) = state.db() { let _ = board.save(db); }
    }
    let code = if ok { StatusCode::OK } else { StatusCode::NOT_FOUND };
    let detail = if ok { detail } else { format!("unknown machine ({detail})") };
    (code, Json(OkResponse { ok, detail }))
}

/// OEE report for one machine.
async fn oee_machine_report_handler(
    State(state): State<SharedState>,
    Path(machine_id): Path<String>,
) -> Result<Json<OeeReport>, StatusCode> {
    state
        .oee
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .report(&machine_id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// Plant-wide OEE rollup + merged six-big-losses Pareto.
async fn oee_plant_handler(State(state): State<SharedState>) -> Json<PlantOee> {
    Json(state.oee.lock().unwrap_or_else(|e| e.into_inner()).plant())
}

// ── Predictive maintenance ───────────────────────────────────────────────────
// Learn a per-machine health baseline from readings the operator marks healthy,
// then score live readings for multivariate drift threshold rules miss. An
// anomaly is registered as a coherence node so the dream loop can propose a
// countermeasure — detect → cohere → dream, the auto-kaizen seam.

/// A telemetry reading on the wire. Every metric optional; `status` is a string
/// (`online`/`offline`/`warning`/`critical`).
#[derive(Deserialize)]
struct ReadingRequest {
    machine_id: String,
    temperature: Option<f32>,
    pressure: Option<f32>,
    vibration: Option<f32>,
    rpm: Option<f32>,
    status: Option<String>,
    #[serde(default)]
    raw: HashMap<String, f32>,
    /// Optional anomaly cutoff in σ (default 3.0).
    sigma_threshold: Option<f32>,
    /// Optional RUL projection: forecast time until `rul_metric` hits `rul_limit`.
    rul_metric: Option<String>,
    rul_limit: Option<f64>,
}

impl ReadingRequest {
    fn to_reading(&self) -> MachineReading {
        let status = match self.status.as_deref().map(str::to_ascii_lowercase).as_deref() {
            Some("offline") => MachineStatus::Offline,
            Some("warning") => MachineStatus::Warning,
            Some("critical") => MachineStatus::Critical,
            _ => MachineStatus::Online,
        };
        MachineReading {
            machine_id: self.machine_id.clone(),
            timestamp: chrono::Utc::now(),
            temperature: self.temperature,
            pressure: self.pressure,
            vibration: self.vibration,
            rpm: self.rpm,
            status,
            raw: self.raw.clone(),
        }
    }
}

/// Fold a healthy reading into the machine's baseline (training).
async fn maintenance_observe_handler(
    State(state): State<SharedState>,
    Json(req): Json<ReadingRequest>,
) -> Json<OkResponse> {
    let reading = req.to_reading();
    let n = {
        let mut mon = state.maintenance.lock().unwrap_or_else(|e| e.into_inner());
        mon.observe_healthy(&reading);
        if let Some(db) = state.db() { let _ = mon.save(db); }
        mon.baseline(&req.machine_id).map(|b| b.min_count()).unwrap_or(0)
    };
    Json(OkResponse { ok: true, detail: format!("baseline {} now {} samples", req.machine_id, n) })
}

#[derive(Serialize)]
struct MaintenanceCheckResponse {
    score: AnomalyScore,
    /// RUL projection, present only when `rul_metric` + `rul_limit` were supplied
    /// and the metric is trending toward the limit.
    rul: Option<RulEstimate>,
    /// Coherence node id, present when an anomaly was registered for the dream
    /// loop to reason over (the auto-kaizen seam).
    coherence_node_id: Option<String>,
}

/// Score a live reading; on anomaly, register it as a coherence node.
async fn maintenance_check_handler(
    State(state): State<SharedState>,
    Json(req): Json<ReadingRequest>,
) -> Result<Json<MaintenanceCheckResponse>, StatusCode> {
    let reading = req.to_reading();
    let threshold = req.sigma_threshold.unwrap_or(DEFAULT_SIGMA_THRESHOLD);

    let (score, rul) = {
        let mut mon = state.maintenance.lock().unwrap_or_else(|e| e.into_inner());
        let score = mon.check(&reading, threshold).ok_or(StatusCode::NOT_FOUND)?;
        let rul = match (&req.rul_metric, req.rul_limit) {
            (Some(m), Some(limit)) => mon.rul(&req.machine_id, m, limit),
            _ => None,
        };
        if let Some(db) = state.db() { let _ = mon.save(db); } // check() appended history
        (score, rul)
    };

    // Auto-kaizen seam: an anomaly becomes a coherence node so the dream loop
    // already sees the defect and can propose a countermeasure.
    let coherence_node_id = if score.is_anomaly {
        let text = score.describe();
        let embedding = embed_text(state.shared.clone(), text.clone()).await;
        let mut s = state.lock();
        Some(s.core.register_node_vec_labeled(embedding, Some(text)))
    } else {
        None
    };

    Ok(Json(MaintenanceCheckResponse { score, rul, coherence_node_id }))
}

/// Health baseline summary for one machine.
async fn maintenance_baseline_handler(
    State(state): State<SharedState>,
    Path(machine_id): Path<String>,
) -> Result<Json<HealthBaseline>, StatusCode> {
    state
        .maintenance
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .baseline(&machine_id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

// ── Stateless compute (replica-safe; works online + locally identically) ─────────
// The in-memory board/monitor above holds state in ONE process — fine locally, but
// in a multi-replica cloud deploy each request may hit a different replica, so the
// state shards. These endpoints take ALL inputs in the request and hold nothing:
// the caller (e.g. Praxis/Postgres) persists the shift / baseline / history and
// passes it in. Same compute as the stateful path (shared `oee`/`maintenance`
// modules), just no server state.

/// Compute one machine's OEE from a full shift supplied in the body.
/// Body = an `OeeShift` (`machine_id`, `shift_length_secs`, `ideal_cycle_time_secs`,
/// `stops[]`, `total_count`, `good_count`).
async fn oee_compute_handler(Json(shift): Json<OeeShift>) -> Json<OeeReport> {
    Json(shift.compute())
}

#[derive(Deserialize)]
struct ReportFormatQuery {
    /// "html" → self-contained HTML document (email-able, browser print-to-PDF);
    /// anything else → JSON `{markdown}`.
    #[serde(default)]
    format: Option<String>,
}

fn report_response(input: &physis_pro::report::ShiftReportInput, format: Option<&str>) -> Response {
    if format == Some("html") {
        Html(physis_pro::report::render_shift_report_html(input)).into_response()
    } else {
        Json(serde_json::json!({
            "markdown": physis_pro::report::render_shift_report_it(input)
        }))
        .into_response()
    }
}

#[derive(Deserialize)]
struct DiscoverRequest {
    texts: Vec<String>,
    #[serde(default)]
    coverage_threshold: Option<f32>,
    /// `kind` for the draft ontology JSON (default "discovered").
    #[serde(default)]
    kind: Option<String>,
}

/// Ontology discovery (PLAN 1D): which of the supplied texts does the current
/// grid cover poorly, and what draft entries would cover them? Stateless —
/// embeds off-thread against the shared classifier; returns the report plus a
/// ready-to-edit ontology JSON draft.
async fn discover_handler(
    State(state): State<SharedState>,
    Json(req): Json<DiscoverRequest>,
) -> Json<serde_json::Value> {
    let shared = state.shared.clone();
    let kind = req.kind.unwrap_or_else(|| "discovered".to_string());
    let mut cfg = physis_pro::discovery::DiscoveryConfig::default();
    if let Some(t) = req.coverage_threshold {
        cfg.coverage_threshold = t;
    }
    let (report, draft) = tokio::task::spawn_blocking(move || {
        let report =
            physis_pro::discovery::discover(&req.texts, &shared.classifier, &shared.embedder, &cfg);
        let draft = physis_pro::discovery::to_ontology_json(&report, &kind);
        (report, draft)
    })
    .await
    .expect("discovery task completes");
    Json(serde_json::json!({ "report": report, "ontology_draft": draft }))
}

/// Stateless shift report: caller supplies all data (`ShiftReportInput`),
/// gets Italian Markdown (default) or HTML (`?format=html`) back.
/// Replica-safe like the other compute routes.
async fn report_shift_compute_handler(
    Query(q): Query<ReportFormatQuery>,
    Json(input): Json<physis_pro::report::ShiftReportInput>,
) -> Response {
    report_response(&input, q.format.as_deref())
}

/// Stateful shift report from the live engine: every machine on the OEE board,
/// plus accepted/ungraded dream descriptions from the default tenant as
/// proposed countermeasures. Anomaly/RUL sections come via the stateless POST
/// (the monitor scores readings as they arrive; it doesn't keep a shift log).
async fn report_shift_live_handler(
    State(state): State<SharedState>,
    Query(q): Query<ReportFormatQuery>,
) -> Response {
    let oee: Vec<OeeReport> = {
        let board = state.oee.lock().unwrap_or_else(|e| e.into_inner());
        board
            .machine_ids()
            .iter()
            .filter_map(|id| board.report(id))
            .collect()
    };
    let countermeasures: Vec<String> = {
        let s = state.lock();
        s.dreams
            .dreams
            .iter()
            .filter(|d| d.grade.unwrap_or(1.0) > 0.5)
            .map(|d| d.description.clone())
            .take(10)
            .collect()
    };
    let input = physis_pro::report::ShiftReportInput {
        oee,
        countermeasures,
        ..Default::default()
    };
    report_response(&input, q.format.as_deref())
}

/// Roll up a plant OEE from a list of per-machine reports supplied in the body.
async fn oee_rollup_handler(Json(reports): Json<Vec<OeeReport>>) -> Json<PlantOee> {
    Json(plant_oee(&reports))
}

#[derive(Deserialize)]
struct FoldRequest {
    /// Prior baseline (omit/null to start fresh). Persist the returned one.
    baseline: Option<HealthBaseline>,
    reading: ReadingRequest,
}

/// Fold a healthy reading into a baseline and return the updated baseline. The
/// caller stores the result (replica-safe training).
async fn maintenance_fold_handler(Json(req): Json<FoldRequest>) -> Json<HealthBaseline> {
    let mut baseline = req
        .baseline
        .unwrap_or_else(|| HealthBaseline::new(&req.reading.machine_id));
    baseline.observe(&req.reading.to_reading());
    Json(baseline)
}

#[derive(Deserialize)]
struct ScoreRequest {
    baseline: HealthBaseline,
    reading: ReadingRequest,
    sigma_threshold: Option<f32>,
}

/// Score a reading against a caller-supplied baseline (no server state).
async fn maintenance_score_handler(Json(req): Json<ScoreRequest>) -> Json<AnomalyScore> {
    let threshold = req.sigma_threshold.unwrap_or(DEFAULT_SIGMA_THRESHOLD);
    Json(req.baseline.score(&req.reading.to_reading(), threshold))
}

#[derive(Deserialize)]
struct RulRequest {
    samples: Vec<Sample>,
    limit: f64,
}

/// Project time-to-limit from a caller-supplied metric history.
async fn maintenance_rul_handler(Json(req): Json<RulRequest>) -> Json<Option<RulEstimate>> {
    Json(project_time_to_limit(&req.samples, req.limit))
}

async fn context_filter_handler(
    State(state): State<SharedState>,
    Json(req): Json<ContextFilterRequest>,
) -> Json<FilterResponse> {
    // Embed off-thread before taking the lock — the ONNX embed is the slow part
    // and must not serialize other handlers behind the global state mutex.
    let token_estimate = req.input.split_whitespace().count();
    let embedding = embed_text(state.shared.clone(), req.input.clone()).await;
    let s = state.lock();
    let result = s.core.filter_context_embedded(embedding, token_estimate);
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

#[derive(Deserialize)]
struct AddTaskRequest {
    task_id: String,
    duration_secs: f64,
    #[serde(default)]
    dependencies: Vec<String>,
}

async fn schedule_status_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let health = s.scheduler.schedule_health();
        let tasks: Vec<serde_json::Value> = s.scheduler.tasks.iter().map(|t| {
            serde_json::json!({
                "id": t.id,
                "name": t.name,
                "duration": t.planned_duration_secs,
                "early_start": t.early_start,
                "early_finish": t.early_finish,
                "late_start": t.late_start,
                "late_finish": t.late_finish,
                "float": t.float,
                "critical": t.on_critical_path,
                "progress": t.progress,
            })
        }).collect();
        let critical_path: Vec<&str> = s.scheduler.tasks.iter()
            .filter(|t| t.on_critical_path).map(|t| t.id.as_str()).collect();
        let at_risk: Vec<&str> = s.scheduler.tasks_approaching_late_finish(3600)
            .iter().map(|t| t.id.as_str()).collect();
        Json(serde_json::json!({
            "schedule_health": health,
            "task_count": tasks.len(),
            "tasks": tasks,
            "critical_path": critical_path,
            "at_risk_tasks": at_risk,
        }))
    })
}

async fn schedule_cross_correlate_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let progress_pairs: Vec<(String, f32)> = s.goals.iter()
            .map(|g| (g.id.clone(), g.progress)).collect();
        let r = s.scheduler.cross_correlate(&progress_pairs);
        Json(serde_json::json!({
            "pearson_r": r,
            "interpretation": if r.abs() > 0.7 {
                "strong correlation"
            } else if r.abs() > 0.4 {
                "moderate correlation"
            } else {
                "weak or no correlation"
            }
        }))
    })
}

async fn schedule_add_task_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<AddTaskRequest>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        req.task_id.hash(&mut hasher);
        let node_key = physis_pro::graph::RawNodeKey { data: hasher.finish() };
        let deps: Vec<physis_pro::graph::RawNodeKey> = req.dependencies.iter().map(|d| {
            let mut h = DefaultHasher::new();
            d.hash(&mut h);
            physis_pro::graph::RawNodeKey { data: h.finish() }
        }).collect();
        s.scheduler.add_task_with_duration(node_key, req.task_id.clone(), req.duration_secs.max(0.0) as u64, 0, deps);
        Json(serde_json::json!({ "status": "ok", "task_id": req.task_id }))
    })
}

async fn schedule_recompute_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        s.scheduler.recompute();
        Json(serde_json::json!({ "schedule_health": s.scheduler.schedule_health() }))
    })
}

#[derive(Deserialize)]
struct ScheduleComputeRequest {
    tasks: Vec<TaskInput>,
    #[serde(default)]
    goal_progress: Vec<(String, f32)>,
    #[serde(default)]
    domain: Option<String>,
}

/// Stateless CPM: caller supplies the full task list, gets the computed schedule
/// back. Holds no server state, so it behaves identically across replicas and on
/// a redeploy (unlike the in-memory `/schedule/*` routes above, which are
/// single-instance/local only).
async fn schedule_compute_handler(Json(req): Json<ScheduleComputeRequest>) -> Json<ScheduleResult> {
    Json(compute_schedule(&req.tasks, &req.goal_progress, req.domain.as_deref().unwrap_or("abstract")))
}

// ── LifeOS (native goal/habit tracking) ──────────────────────────────────────
// Goals live in-engine (per tenant) — physis owns them rather than pulling from
// an external Praxis/Agora service. Push goals in, poll status/drift out.

/// Insert or replace a LifeOS goal in the caller's tenant.
async fn lifeos_goal_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(goal): Json<LifeOSGoal>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    let id = goal.id.clone();
    let db = state.db.clone();
    let replaced = state.with_user(&uid, |s| {
        let r = s.lifeos.upsert_goal(goal);
        if let Some(db) = &db { let _ = s.lifeos.save(db, &uid); }
        r
    });
    Json(serde_json::json!({ "status": "ok", "id": id, "replaced": replaced }))
}

#[derive(Deserialize)]
struct LifeOsProgressRequest {
    id: String,
    progress: f32,
}

/// Update a goal's progress.
async fn lifeos_progress_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<LifeOsProgressRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let uid = user_key(&headers);
    let db = state.db.clone();
    let ok = state.with_user(&uid, |s| {
        let r = s.lifeos.update_progress(&req.id, req.progress);
        if r {
            if let Some(db) = &db { let _ = s.lifeos.save(db, &uid); }
        }
        r
    });
    let code = if ok { StatusCode::OK } else { StatusCode::NOT_FOUND };
    (code, Json(serde_json::json!({ "ok": ok, "id": req.id })))
}

#[derive(Serialize)]
struct LifeOsStatus {
    goals: Vec<LifeOSGoal>,
    drift: DriftReport,
}

/// Current goals + schedule-drift snapshot (replaces the old outbound push).
async fn lifeos_status_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<LifeOsStatus> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        // Reflect current goals in the schedule before reporting drift.
        let RuntimeState { lifeos, scheduler, .. } = s;
        lifeos.apply_to_scheduler(scheduler);
        scheduler.recompute();
        Json(LifeOsStatus {
            goals: lifeos.goals.clone(),
            drift: lifeos.drift_report(scheduler),
        })
    })
}

#[derive(Deserialize)]
struct ChargeRequest {
    charge: Charge,
}

/// Set the current bioenergetic charge (Praxis derives it from check-ins).
async fn lifeos_charge_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<ChargeRequest>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    let db = state.db.clone();
    state.with_user(&uid, |s| {
        s.lifeos.set_charge(req.charge);
        if let Some(db) = &db { let _ = s.lifeos.save(db, &uid); }
    });
    Json(serde_json::json!({ "ok": true }))
}

/// Charge-governed "today": goals matched to current charge + a bioenergetic
/// countermeasure when depleted/drifting. The keystone of the bioenergetics↔
/// lifeos merge.
async fn lifeos_today_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<TodayPlan> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let RuntimeState { lifeos, scheduler, .. } = s;
        lifeos.apply_to_scheduler(scheduler);
        scheduler.recompute();
        Json(lifeos.today(scheduler))
    })
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
    headers: HeaderMap,
) -> Json<physis_pro::gantt::Trajectory> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let goals_progress: Vec<(String, f32)> = s.goals.iter()
            .map(|g| (g.id.clone(), g.progress))
            .collect();
        Json(compute_trajectory(&s.scheduler, &goals_progress))
    })
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
                    pooling: physis_pro::config::PoolingStrategy::Mean,
                };
                let b = OnnxEmbedder::with_config(&cfg);
                b.is_available().then(|| (Box::new(b) as Box<dyn VectorEmbed>, "bge-base"))
            };
            // BGE-M3 (1024-d, XLM-R, 100+ languages). Preferred over BGE-base when
            // its weights are present: Italian/multilingual industrial input is why.
            let make_bge_m3 = || -> Option<(Box<dyn VectorEmbed>, &'static str)> {
                let cfg = physis_pro::config::OnnxConfig {
                    enabled: true,
                    model_dir: Some("models/bge-m3".to_string()),
                    dim: 1024,
                    max_length: 512,
                    pooling: physis_pro::config::PoolingStrategy::Cls,
                };
                let b = OnnxEmbedder::with_config(&cfg);
                b.is_available().then(|| (Box::new(b) as Box<dyn VectorEmbed>, "bge-m3"))
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
            // Try the configured primary embedder, then fall through Jina → BGE → BGE-M3 → CLIP → MiniLM → RP
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
                    EmbedderKindConfig::BgeM3 => make_bge_m3(),
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
                // BGE-M3 sits BELOW bge-base: measured 2026-06-12, M3 CORE
                // 0.923/0.769 vs bge-base 1.000/0.949; Italian only ties (0.733
                // domain). Explicit `embedders.primary = bge-m3` still selects it.
                .or_else(|| gate(make_bge_m3()))
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

    // Durable state store on the data-dir volume. Best-effort: if it can't open
    // (no writable volume / local dev), run in-memory only.
    let db: Option<sled::Db> = {
        let path = shared.config.data_dir.join("state.sled");
        match sled::open(&path) {
            Ok(d) => { log::info!("state persistence: sled at {}", path.display()); Some(d) }
            Err(e) => { log::warn!("state persistence disabled ({e}); in-memory only"); None }
        }
    };

    let state = Arc::new(AppCtx {
        shared: shared.clone(),
        state: Mutex::new(RuntimeState {
            core,
            mapper,
            actor,
            dreams,
            goals: Vec::new(),
            scheduler: GanttScheduler::new(Box::new(AbstractDomain)),
            // Rehydrate the default tenant's persisted goals.
            lifeos: db.as_ref().map(|d| LifeOSTracker::load(d, "_default")).unwrap_or_default(),
        }),
        tenants: Mutex::new(HashMap::new()),
        scan_root: std::sync::RwLock::new(scan_root),
        oee: Mutex::new(db.as_ref().map(OeeBoard::load).unwrap_or_default()),
        maintenance: Mutex::new(
            db.as_ref().map(|d| MaintenanceMonitor::load(d, 256)).unwrap_or_else(|| MaintenanceMonitor::new(256)),
        ),
        db: db.clone(),
        axes: Mutex::new(db.as_ref().map(physis_pro::axes::AxisStore::load).unwrap_or_default()),
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
    // 2. PDCA/Rachmaninov Tick Cycle (every 10s). Fully self-contained now —
    // LifeOS goals live in-engine, so a tick is pure-sync: mirror the native
    // goals into the schedule, recompute CPM, and log Gantt-aware dreams. No
    // outbound HTTP to Praxis/Agora.
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            interval.tick().await;

            let mut s = state_clone_2.lock();
            let mut rach = rachmaninov_clone.lock().unwrap_or_else(|e| e.into_inner());
            s.core.tick_holarchy(&onnx_clone, &mut rach);
            // Mirror native LifeOS goals into the schedule, then recompute.
            // (split borrow: take lifeos out by ref, scheduler by mut)
            let RuntimeState { lifeos, scheduler, .. } = &mut *s;
            lifeos.apply_to_scheduler(scheduler);
            scheduler.recompute();
            if scheduler.schedule_health() < 0.7 {
                for prop in scheduler.dream_proposals() {
                    log::info!("Gantt dream: {}", prop.description);
                }
            }
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
        .route("/api/v1/embedder", get(embedder_info_handler))
        // Graph file-manager SPA. The page is public; its data calls hit the
        // authenticated fs/* routes with a token the user pastes in-page.
        .route("/ui", get(graph_fm_ui_handler));

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
        .route("/api/v1/fs/graph", get(fs_graph_handler))
        .route("/api/v1/fs/file", get(fs_file_handler))
        .route("/api/v1/fs/move", post(fs_move_handler))
        .route("/api/v1/dream/generate", post(dream_generate_handler))
        .route("/api/v1/dream/evaluate", post(dream_evaluate_handler))
        .route("/api/v1/coherence/snapshot", get(coherence_snapshot_handler))
        .route("/api/v1/coherence/nodes", get(coherence_nodes_handler))
        .route("/api/v1/coherence/nodes/all", get(coherence_nodes_all_handler))
        .route("/api/v1/coherence/register", post(coherence_register_handler))
        .route("/api/v1/oee/machine", post(oee_machine_handler))
        .route("/api/v1/oee/event", post(oee_event_handler))
        .route("/api/v1/oee/plant", get(oee_plant_handler))
        .route("/api/v1/oee/:machine", get(oee_machine_report_handler))
        .route("/api/v1/maintenance/observe", post(maintenance_observe_handler))
        .route("/api/v1/maintenance/check", post(maintenance_check_handler))
        .route("/api/v1/oee/compute", post(oee_compute_handler))
        .route("/api/v1/oee/rollup", post(oee_rollup_handler))
        .route("/api/v1/report/shift", get(report_shift_live_handler).post(report_shift_compute_handler))
        .route("/api/v1/discover", post(discover_handler))
        .route("/api/v1/maintenance/fold", post(maintenance_fold_handler))
        .route("/api/v1/maintenance/score", post(maintenance_score_handler))
        .route("/api/v1/maintenance/rul", post(maintenance_rul_handler))
        .route("/api/v1/maintenance/:machine", get(maintenance_baseline_handler))
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
        .route("/api/v1/schedule/status", get(schedule_status_handler))
        .route("/api/v1/schedule/cross-correlate", get(schedule_cross_correlate_handler))
        .route("/api/v1/schedule/add-task", post(schedule_add_task_handler))
        .route("/api/v1/schedule/recompute", post(schedule_recompute_handler))
        .route("/api/v1/schedule/compute", post(schedule_compute_handler))
        .route("/api/v1/lifeos/goal", post(lifeos_goal_handler))
        .route("/api/v1/lifeos/progress", post(lifeos_progress_handler))
        .route("/api/v1/lifeos/status", get(lifeos_status_handler))
        .route("/api/v1/lifeos/charge", post(lifeos_charge_handler))
        .route("/api/v1/lifeos/today", get(lifeos_today_handler))
        // Semiotic / Ontology endpoints
        .route("/api/v1/ontology/list", get(ontology_list_handler))
        .route("/api/v1/semiotic/grid", get(semiotic_grid_handler))
        .route("/api/v1/semiotic/triangle", get(semiotic_triangle_handler))
        .route("/api/v1/semiotic/square", get(greimas_square_handler))
        .route("/api/v1/semiotic/heatmap", get(heatmap_handler))
        .route("/api/v1/classify", post(classify_handler))
        .route("/api/v1/embed", post(embed_handler))
        .route("/api/v1/embed/video", post(embed_video_handler))
        .route("/api/v1/category/diagram", post(category_diagram_handler))
        // Philosophical machines: registry + stateless reducers.
        .route("/api/v1/machines", get(machines_list_handler))
        .route("/api/v1/machines/chronos", post(machines_chronos_handler))
        .route("/api/v1/machines/topos", post(machines_topos_handler))
        .route("/api/v1/machines/harmony", post(machines_harmony_handler))
        .route("/api/v1/machines/polis", post(machines_polis_handler))
        .route("/api/v1/machines/soma", post(machines_soma_handler))
        .route("/api/v1/machines/elenchus", post(machines_elenchus_handler))
        .route("/api/v1/machines/lex", post(machines_lex_handler))
        .route("/api/v1/machines/empedocles", post(machines_empedocles_handler))
        .route("/api/v1/machines/erosp", post(machines_erosp_handler))
        .route("/api/v1/machines/erosa", post(machines_erosa_handler))
        .route("/api/v1/machines/techne", post(machines_techne_handler))
        .route("/api/v1/machines/euclid", post(machines_euclid_handler))
        .route("/api/v1/machines/plutos", post(machines_plutos_handler))
        .route("/api/v1/machines/krisis", post(machines_krisis_handler))
        .route("/api/v1/machines/pathos", post(machines_pathos_handler))
        .route("/api/v1/machines/chroma", post(machines_chroma_handler))
        .route("/api/v1/machines/kinesis", post(machines_kinesis_handler))
        // Dynamic axis expansion (Phase 6): read + propose + promote/demote/lineage.
        .route("/api/v1/axes", get(axes_list_handler))
        .route("/api/v1/axes/discover", post(axes_discover_handler))
        .route("/api/v1/axes/promote", post(axes_promote_handler))
        .route("/api/v1/axes/demote", post(axes_demote_handler))
        .route("/api/v1/axes/lineage", get(axes_lineage_handler));

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

// ─────────────────────────────────────────────────────────────────────────────
// Philosophical machines — HTTP surface.
// Each machine reduces a modality to *primitives* (the smallest elaborable unit).
// These handlers expose the stateless reducers (no core/embedder mutation); the
// node-registering variants (register_*) are a planned follow-up. See src/machines/.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct MachineInfo {
    name: &'static str,
    modality: &'static str,
    primitive: &'static str,
    /// "live" = HTTP reducer wired; "reducer-only" = Rust reducer exists, endpoint pending.
    status: &'static str,
}

/// GET /api/v1/machines — registry of the philosophical machines.
async fn machines_list_handler() -> Json<serde_json::Value> {
    let machines = [
        MachineInfo { name: "chronos",    modality: "timed events",        primitive: "rhythm (recurrence)",            status: "live" },
        MachineInfo { name: "topos",      modality: "geotagged waypoints",  primitive: "place / region (recurring)",     status: "live" },
        MachineInfo { name: "harmony",    modality: "notes (pitches)",     primitive: "interval / motif",               status: "live" },
        MachineInfo { name: "elenchus",   modality: "dialogue statements", primitive: "claim / warrant / rebuttal",     status: "live" },
        MachineInfo { name: "lex",        modality: "contract clauses",    primitive: "obligation / right / condition", status: "live" },
        MachineInfo { name: "soma",       modality: "training sessions",   primitive: "effort / recovery / drift",      status: "live" },
        MachineInfo { name: "polis",      modality: "social interactions", primitive: "affinity / reciprocity tie",     status: "live" },
        MachineInfo { name: "empedocles", modality: "physical readings",   primitive: "element (earth/air/fire/water)", status: "live" },
        MachineInfo { name: "erosp",      modality: "directed reaches",    primitive: "longing (Penia/Poros)",          status: "live" },
        MachineInfo { name: "erosa",      modality: "directed reaches",    primitive: "conquest (Ares/Aphrodite)",      status: "live" },
        MachineInfo { name: "techne",     modality: "source files",        primitive: "capability / dependency",        status: "live" },
        MachineInfo { name: "euclid",     modality: "visual primitives",   primitive: "polygon (vertices)",             status: "live" },
        MachineInfo { name: "plutos",     modality: "transactions",        primitive: "value flow (recurring)",         status: "live" },
        MachineInfo { name: "krisis",     modality: "pairwise choices",    primitive: "preference (A > B)",             status: "live" },
        MachineInfo { name: "pathos",     modality: "affect episodes",     primitive: "affect (valence x arousal)",     status: "live" },
        MachineInfo { name: "chroma",     modality: "colors",              primitive: "color relation (wheel)",         status: "live" },
        MachineInfo { name: "kinesis",    modality: "trajectories",        primitive: "motion gestalt (reach/gait)",    status: "live" },
    ];
    Json(serde_json::json!({ "machines": machines }))
}

#[derive(Deserialize)]
struct ChronosEvent {
    label: String,
    /// RFC3339 timestamp, e.g. "2026-06-14T08:00:00Z".
    at: String,
}

#[derive(Deserialize)]
struct ChronosRequest {
    events: Vec<ChronosEvent>,
    #[serde(default)]
    horizon_days: Option<i64>,
    #[serde(default)]
    tolerance: Option<f64>,
}

/// POST /api/v1/machines/chronos — reduce timed events to recurrence rhythms,
/// and partition them into upcoming (due within horizon) and slipping (overdue).
/// Stateless: nothing is written to the core.
async fn machines_chronos_handler(Json(req): Json<ChronosRequest>) -> Json<serde_json::Value> {
    use physis_pro::machines::chronos;

    let events: Vec<chronos::TimedEvent> = req
        .events
        .iter()
        .filter_map(|e| {
            chrono::DateTime::parse_from_rfc3339(&e.at).ok().map(|dt| chronos::TimedEvent {
                label: e.label.clone(),
                at: dt.with_timezone(&chrono::Utc),
            })
        })
        .collect();

    let rhythms = chronos::detect_rhythms(&events);
    let now = chrono::Utc::now();
    let horizon = req.horizon_days.unwrap_or(7);
    let tol = req.tolerance.unwrap_or(0.5);

    let upcoming: Vec<String> =
        chronos::upcoming(&rhythms, now, horizon).iter().map(|r| r.label.clone()).collect();
    let slipping: Vec<String> =
        chronos::slipping(&rhythms, now, tol).iter().map(|r| r.label.clone()).collect();

    let rhythms_json: Vec<serde_json::Value> = rhythms
        .iter()
        .map(|r| {
            serde_json::json!({
                "label": r.label,
                "period_days": r.period_days,
                "count": r.count,
                "last": r.last.to_rfc3339(),
                "next_due": r.next_due.to_rfc3339(),
                "confidence": r.confidence,
            })
        })
        .collect();

    Json(serde_json::json!({
        "rhythms": rhythms_json,
        "upcoming": upcoming,
        "slipping": slipping,
    }))
}


#[derive(Deserialize)]
struct HarmonyRequest {
    notes: Vec<i32>,
    #[serde(default)]
    motif_len: Option<usize>,
}

/// POST /api/v1/machines/harmony — reduce a melody to key-invariant intervals
/// and recurring motifs. Stateless.
async fn machines_harmony_handler(Json(req): Json<HarmonyRequest>) -> Json<serde_json::Value> {
    use physis_pro::machines::harmony;
    let len = req.motif_len.unwrap_or(3);
    let intervals = harmony::intervals(&req.notes);
    let motifs: Vec<serde_json::Value> = harmony::motifs(&req.notes, len)
        .into_iter()
        .map(|(m, c)| serde_json::json!({ "intervals": m, "count": c }))
        .collect();
    Json(serde_json::json!({ "intervals": intervals, "motifs": motifs }))
}

#[derive(Deserialize)]
struct PolisInteraction {
    from: String,
    to: String,
    weight: f32,
}

#[derive(Deserialize)]
struct PolisRequest {
    interactions: Vec<PolisInteraction>,
    #[serde(default)]
    min_strength: Option<f32>,
    #[serde(default)]
    min_reciprocity: Option<f32>,
    #[serde(default)]
    max_reciprocity: Option<f32>,
}

/// POST /api/v1/machines/polis — reduce directed social interactions to
/// reciprocity-weighted ties; flag strengthening vs fading. Stateless.
async fn machines_polis_handler(Json(req): Json<PolisRequest>) -> Json<serde_json::Value> {
    use physis_pro::machines::polis;
    let interactions: Vec<polis::Interaction> = req
        .interactions
        .iter()
        .map(|i| polis::Interaction { from: i.from.clone(), to: i.to.clone(), weight: i.weight })
        .collect();
    let ties = polis::ties(&interactions);
    let min_s = req.min_strength.unwrap_or(0.0);
    let min_r = req.min_reciprocity.unwrap_or(0.5);
    let max_r = req.max_reciprocity.unwrap_or(0.5);
    let tie_json = |t: &polis::Tie| {
        serde_json::json!({ "a": t.a, "b": t.b, "strength": t.strength, "reciprocity": t.reciprocity })
    };
    let all: Vec<serde_json::Value> = ties.iter().map(tie_json).collect();
    let strengthening: Vec<serde_json::Value> =
        polis::strengthening(&ties, min_s, min_r).into_iter().map(tie_json).collect();
    let fading: Vec<serde_json::Value> =
        polis::fading(&ties, max_r).into_iter().map(tie_json).collect();
    Json(serde_json::json!({ "ties": all, "strengthening": strengthening, "fading": fading }))
}

/// A directed reaching — the shared perception of both eros machines.
#[derive(Deserialize)]
struct ErosReach {
    subject: String,
    object: String,
    weight: f32,
}

#[derive(Deserialize)]
struct ErospRequest {
    reaches: Vec<ErosReach>,
    #[serde(default)]
    min_intensity: Option<f32>,
    #[serde(default)]
    min_ascent: Option<f32>,
    #[serde(default)]
    max_ascent: Option<f32>,
}

#[derive(Deserialize)]
struct ErosaRequest {
    reaches: Vec<ErosReach>,
    #[serde(default)]
    min_balance: Option<f32>,
}

fn to_eros_reaches(reaches: &[ErosReach]) -> Vec<physis_pro::machines::erosp::Reach> {
    reaches
        .iter()
        .map(|r| physis_pro::machines::erosp::Reach {
            subject: r.subject.clone(),
            object: r.object.clone(),
            weight: r.weight,
        })
        .collect()
}

/// POST /api/v1/machines/erosp — Platonic eros (Penia × Poros): reduce directed
/// reaches to ascending longings; flag ascending (drive + resource) vs hopeless
/// (drive, no resource). Stateless.
async fn machines_erosp_handler(Json(req): Json<ErospRequest>) -> Json<serde_json::Value> {
    use physis_pro::machines::erosp;
    let reaches = to_eros_reaches(&req.reaches);
    let longings = erosp::detect_longings(&reaches);
    let min_i = req.min_intensity.unwrap_or(0.0);
    let min_a = req.min_ascent.unwrap_or(0.1);
    let max_a = req.max_ascent.unwrap_or(0.05);
    let lj = |l: &erosp::Longing| {
        serde_json::json!({
            "subject": l.subject, "object": l.object,
            "penia": l.penia, "poros": l.poros,
            "intensity": l.intensity, "ascent": l.ascent,
        })
    };
    let all: Vec<serde_json::Value> = longings.iter().map(lj).collect();
    let ascending: Vec<serde_json::Value> =
        erosp::ascending(&longings, min_i, min_a).into_iter().map(lj).collect();
    let hopeless: Vec<serde_json::Value> =
        erosp::hopeless(&longings, max_a).into_iter().map(lj).collect();
    Json(serde_json::json!({ "longings": all, "ascending": ascending, "hopeless": hopeless }))
}

/// POST /api/v1/machines/erosa — mythic eros (Ares × Aphrodite): reduce directed
/// reaches to conquests (valid only as child of love AND strife); flag passionate
/// (balanced) vs jealous (strife overruns love). Stateless.
async fn machines_erosa_handler(Json(req): Json<ErosaRequest>) -> Json<serde_json::Value> {
    use physis_pro::machines::erosa;
    let reaches = to_eros_reaches(&req.reaches);
    let conquests = erosa::detect_conquests(&reaches);
    let min_b = req.min_balance.unwrap_or(0.5);
    let cj = |c: &erosa::Conquest| {
        serde_json::json!({
            "subject": c.subject, "object": c.object,
            "aphrodite": c.aphrodite, "ares": c.ares,
            "rivals": c.rivals, "balance": c.balance,
        })
    };
    let all: Vec<serde_json::Value> = conquests.iter().map(cj).collect();
    let passionate: Vec<serde_json::Value> =
        erosa::passionate(&conquests, min_b).into_iter().map(cj).collect();
    let jealous: Vec<serde_json::Value> =
        erosa::jealous(&conquests).into_iter().map(cj).collect();
    Json(serde_json::json!({ "conquests": all, "passionate": passionate, "jealous": jealous }))
}

#[derive(Deserialize)]
struct TechneFile {
    path: String,
    #[serde(default)]
    lines: Vec<String>,
}

#[derive(Deserialize)]
struct TechneRequest {
    files: Vec<TechneFile>,
    /// Optional: list the modules transitively impacted if this one changes.
    #[serde(default)]
    impact_target: Option<String>,
}

/// POST /api/v1/machines/techne — reduce a set of source files to capabilities
/// and module dependency edges; optionally compute the blast radius of a module.
/// Stateless.
async fn machines_techne_handler(Json(req): Json<TechneRequest>) -> Json<serde_json::Value> {
    use physis_pro::machines::techne;
    let files: Vec<techne::SourceFile> = req
        .files
        .iter()
        .map(|f| techne::SourceFile { path: f.path.clone(), lines: f.lines.clone() })
        .collect();
    let caps: Vec<serde_json::Value> = techne::classify_capabilities(&files)
        .iter()
        .map(|c| serde_json::json!({ "name": c.name, "files": c.files }))
        .collect();
    let edges = techne::dependency_edges(&files);
    let deps: Vec<serde_json::Value> =
        edges.iter().map(|(a, b)| serde_json::json!([a, b])).collect();
    let impact = req.impact_target.as_ref().map(|t| techne::impact_of(&edges, t));
    Json(serde_json::json!({ "capabilities": caps, "dependencies": deps, "impact": impact }))
}

#[derive(Deserialize)]
struct EuclidPrimitive {
    vertices: u32,
    circularity: f32,
    #[serde(default)]
    aspect_ratio: f32,
    #[serde(default)]
    color: [u8; 3],
    #[serde(default)]
    texture: f32,
    /// (x, y, w, h)
    #[serde(default)]
    bbox: [f32; 4],
}

#[derive(Deserialize)]
struct EuclidRequest {
    primitives: Vec<EuclidPrimitive>,
    #[serde(default)]
    circularity_limit: Option<f32>,
    #[serde(default)]
    near: Option<f32>,
}

/// POST /api/v1/machines/euclid — reduce closed forms to geometric primitives
/// (Form vs Noise by circularity), and relate them (contained / near). Stateless.
async fn machines_euclid_handler(Json(req): Json<EuclidRequest>) -> Json<serde_json::Value> {
    use physis_pro::machines::euclid;
    let limit = req.circularity_limit.unwrap_or(0.85);
    let near = req.near.unwrap_or(50.0);
    let prims: Vec<euclid::Primitive> = req
        .primitives
        .iter()
        .map(|p| euclid::Primitive {
            vertices: p.vertices,
            circularity: p.circularity,
            aspect_ratio: p.aspect_ratio,
            color: (p.color[0], p.color[1], p.color[2]),
            texture: p.texture,
            bbox: (p.bbox[0], p.bbox[1], p.bbox[2], p.bbox[3]),
        })
        .collect();
    let reduced: Vec<serde_json::Value> = prims
        .iter()
        .map(|p| {
            let st = euclid::classify(p, limit);
            serde_json::json!({
                "shape": euclid::classify_by_vertices(p.vertices),
                "status": if matches!(st, euclid::Status::Form) { "form" } else { "noise" },
                "coherence": st.coherence(),
                "hash": euclid::compute_hash(p),
            })
        })
        .collect();
    let mut relations = Vec::new();
    for i in 0..prims.len() {
        for j in (i + 1)..prims.len() {
            let rel = euclid::relation(&prims[i], &prims[j], near);
            let name = match rel {
                euclid::Relation::Contained => "contained",
                euclid::Relation::Near => "near",
                euclid::Relation::Disjoint => continue,
            };
            relations.push(serde_json::json!({ "a": i, "b": j, "relation": name }));
        }
    }
    Json(serde_json::json!({ "primitives": reduced, "relations": relations }))
}

#[derive(Deserialize)]
struct PlutosTransaction {
    from: String,
    to: String,
    amount: f32,
}

#[derive(Deserialize)]
struct PlutosRequest {
    transactions: Vec<PlutosTransaction>,
    #[serde(default)]
    min_count: Option<usize>,
    #[serde(default)]
    min_total: Option<f32>,
}

/// POST /api/v1/machines/plutos — reduce transactions to recurring value flows;
/// compose per-party balances; surface the largest drains and the breaching
/// (net-negative) parties. Stateless.
async fn machines_plutos_handler(Json(req): Json<PlutosRequest>) -> Json<serde_json::Value> {
    use physis_pro::machines::plutos;
    let txns: Vec<plutos::Transaction> = req
        .transactions
        .iter()
        .map(|t| plutos::Transaction { from: t.from.clone(), to: t.to.clone(), amount: t.amount })
        .collect();
    let flows = plutos::detect_flows(&txns, req.min_count.unwrap_or(2));
    let bals = plutos::balances(&flows);
    let fj = |f: &plutos::Flow| {
        serde_json::json!({
            "from": f.from, "to": f.to, "total": f.total, "count": f.count, "mean": f.mean,
        })
    };
    let bj = |b: &plutos::Balance| {
        serde_json::json!({
            "party": b.party, "inflow": b.inflow, "outflow": b.outflow, "net": b.net,
        })
    };
    let flows_json: Vec<serde_json::Value> = flows.iter().map(fj).collect();
    let drains: Vec<serde_json::Value> =
        plutos::drains(&flows, req.min_total.unwrap_or(0.0)).into_iter().map(fj).collect();
    let balances_json: Vec<serde_json::Value> = bals.iter().map(bj).collect();
    let breaches: Vec<serde_json::Value> =
        plutos::breaches(&bals).into_iter().map(bj).collect();
    Json(serde_json::json!({
        "flows": flows_json,
        "drains": drains,
        "balances": balances_json,
        "breaches": breaches,
    }))
}

#[derive(Deserialize)]
struct KrisisChoice {
    over: String,
    under: String,
}

#[derive(Deserialize)]
struct KrisisRequest {
    choices: Vec<KrisisChoice>,
}

/// POST /api/v1/machines/krisis — reduce pairwise choices to net preferences,
/// rank them (Copeland), and flag intransitive cycles (the incoherent taste).
/// Stateless.
async fn machines_krisis_handler(Json(req): Json<KrisisRequest>) -> Json<serde_json::Value> {
    use physis_pro::machines::krisis;
    let choices: Vec<krisis::Choice> = req
        .choices
        .iter()
        .map(|c| krisis::Choice { over: c.over.clone(), under: c.under.clone() })
        .collect();
    let prefs = krisis::detect_preferences(&choices);
    let prefs_json: Vec<serde_json::Value> = prefs
        .iter()
        .map(|p| serde_json::json!({ "over": p.over, "under": p.under, "margin": p.margin }))
        .collect();
    let ranking: Vec<serde_json::Value> = krisis::ranking(&prefs)
        .iter()
        .map(|r| serde_json::json!({ "option": r.option, "score": r.score }))
        .collect();
    let cycles: Vec<serde_json::Value> =
        krisis::cycles(&prefs).into_iter().map(|c| serde_json::json!(c)).collect();
    Json(serde_json::json!({ "preferences": prefs_json, "ranking": ranking, "cycles": cycles }))
}

#[derive(Deserialize)]
struct PathosEpisode {
    trigger: String,
    valence: f32,
    arousal: f32,
}

#[derive(Deserialize)]
struct PathosRequest {
    episodes: Vec<PathosEpisode>,
    #[serde(default)]
    min_count: Option<usize>,
    #[serde(default)]
    flat: Option<f32>,
}

/// POST /api/v1/machines/pathos — reduce affect episodes to recurring patterns
/// (flat episodes rejected); mark amplifiers and soothers. Stateless.
async fn machines_pathos_handler(Json(req): Json<PathosRequest>) -> Json<serde_json::Value> {
    use physis_pro::machines::pathos;
    let episodes: Vec<pathos::Episode> = req
        .episodes
        .iter()
        .map(|e| pathos::Episode { trigger: e.trigger.clone(), valence: e.valence, arousal: e.arousal })
        .collect();
    let affects = pathos::detect_affects(&episodes, req.min_count.unwrap_or(2), req.flat.unwrap_or(0.1));
    let aj = |a: &pathos::Affect| {
        serde_json::json!({ "trigger": a.trigger, "valence": a.valence, "arousal": a.arousal, "count": a.count })
    };
    let all: Vec<serde_json::Value> = affects.iter().map(aj).collect();
    let amplifiers: Vec<serde_json::Value> =
        pathos::amplifiers(&affects, 0.5).into_iter().map(aj).collect();
    let soothers: Vec<serde_json::Value> =
        pathos::soothers(&affects, 0.5).into_iter().map(aj).collect();
    Json(serde_json::json!({ "affects": all, "amplifiers": amplifiers, "soothers": soothers }))
}

#[derive(Deserialize)]
struct ChromaColor {
    r: u8,
    g: u8,
    b: u8,
}

#[derive(Deserialize)]
struct ChromaRequest {
    colors: Vec<ChromaColor>,
    #[serde(default)]
    min_chroma: Option<f32>,
    #[serde(default)]
    tol: Option<f32>,
}

/// POST /api/v1/machines/chroma — reduce colors to saturated hues (muddy grey
/// rejected) and relate them on the wheel (complement/analogous/triad/clash).
/// Stateless.
async fn machines_chroma_handler(Json(req): Json<ChromaRequest>) -> Json<serde_json::Value> {
    use physis_pro::machines::chroma;
    let colors: Vec<chroma::Color> =
        req.colors.iter().map(|c| chroma::Color { r: c.r, g: c.g, b: c.b }).collect();
    let min_chroma = req.min_chroma.unwrap_or(0.2);
    let tol = req.tol.unwrap_or(20.0);
    let rels = chroma::detect_relations(&colors, min_chroma, tol);
    let rj = |r: &chroma::Relation| {
        let kind = match r.kind {
            chroma::Kind::Complement => "complement",
            chroma::Kind::Analogous => "analogous",
            chroma::Kind::Triad => "triad",
            chroma::Kind::Clash => "clash",
        };
        serde_json::json!({ "a": r.a, "b": r.b, "kind": kind })
    };
    let all: Vec<serde_json::Value> = rels.iter().map(rj).collect();
    let harmonies: Vec<serde_json::Value> = chroma::harmonies(&rels).into_iter().map(rj).collect();
    let clashes: Vec<serde_json::Value> = chroma::clashes(&rels).into_iter().map(rj).collect();
    Json(serde_json::json!({ "relations": all, "harmonies": harmonies, "clashes": clashes }))
}

#[derive(Deserialize)]
struct KinesisPoint {
    x: f32,
    y: f32,
}

#[derive(Deserialize)]
struct KinesisGesture {
    label: String,
    points: Vec<KinesisPoint>,
}

#[derive(Deserialize)]
struct KinesisRequest {
    gestures: Vec<KinesisGesture>,
    #[serde(default)]
    min_move: Option<f32>,
    #[serde(default)]
    min_net: Option<f32>,
    #[serde(default)]
    drop: Option<f32>,
}

/// POST /api/v1/machines/kinesis — reduce gesture trajectories to motion gestalts
/// (sub-threshold jitter rejected); mark reaches and degrading gaits. Stateless.
async fn machines_kinesis_handler(Json(req): Json<KinesisRequest>) -> Json<serde_json::Value> {
    use physis_pro::machines::kinesis;
    let gestures: Vec<kinesis::Gesture> = req
        .gestures
        .iter()
        .map(|g| kinesis::Gesture {
            label: g.label.clone(),
            points: g.points.iter().map(|p| kinesis::Point { x: p.x, y: p.y }).collect(),
        })
        .collect();
    let motions = kinesis::detect_motions(&gestures, req.min_move.unwrap_or(0.5));
    let mj = |m: &kinesis::Motion| {
        serde_json::json!({
            "label": m.label, "path": m.path, "net": m.net,
            "straightness": m.straightness, "moves": m.moves,
        })
    };
    let all: Vec<serde_json::Value> = motions.iter().map(mj).collect();
    let reaches: Vec<serde_json::Value> =
        kinesis::reaches(&motions, req.min_net.unwrap_or(1.0)).into_iter().map(mj).collect();
    let degrading = kinesis::degrading(&motions, req.drop.unwrap_or(0.2));
    Json(serde_json::json!({ "motions": all, "reaches": reaches, "degrading": degrading }))
}

#[derive(Deserialize)]
struct SomaSession {
    day: u32,
    activity: String,
    load: f32,
    recovery: f32,
}

#[derive(Deserialize)]
struct SomaRequest {
    sessions: Vec<SomaSession>,
    #[serde(default)]
    window: Option<usize>,
}

/// POST /api/v1/machines/soma — classify training sessions as effort/recovery/
/// overreaching and compute load drift over a window. Stateless.
async fn machines_soma_handler(Json(req): Json<SomaRequest>) -> Json<serde_json::Value> {
    use physis_pro::machines::soma;
    let sessions: Vec<soma::Session> = req
        .sessions
        .iter()
        .map(|s| soma::Session { day: s.day, activity: s.activity.clone(), load: s.load, recovery: s.recovery })
        .collect();
    let classified: Vec<serde_json::Value> = sessions
        .iter()
        .map(|s| {
            let label = match soma::classify(s) {
                soma::Status::Effort => "effort",
                soma::Status::Recovery => "recovery",
                soma::Status::Overreaching => "overreaching",
            };
            serde_json::json!({ "day": s.day, "activity": s.activity, "status": label })
        })
        .collect();
    let window = req.window.unwrap_or(7);
    let drift = soma::drift(&sessions, window);
    Json(serde_json::json!({ "sessions": classified, "drift": drift }))
}


#[derive(Deserialize)]
struct ElenchusStatement {
    id: String,
    /// "claim" | "warrant" | "rebuttal"
    kind: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    target: Option<String>,
}

#[derive(Deserialize)]
struct ElenchusRequest {
    statements: Vec<ElenchusStatement>,
}

/// POST /api/v1/machines/elenchus — reduce a dialogue to Toulmin claim/warrant/
/// rebuttal primitives; return each claim's standing + the unsupported/contested
/// claims to press. Statements with an unknown kind are ignored. Stateless.
async fn machines_elenchus_handler(Json(req): Json<ElenchusRequest>) -> Json<serde_json::Value> {
    use physis_pro::machines::elenchus;
    let statements: Vec<elenchus::Statement> = req
        .statements
        .iter()
        .filter_map(|s| {
            let kind = match s.kind.to_ascii_lowercase().as_str() {
                "claim" => elenchus::Kind::Claim,
                "warrant" => elenchus::Kind::Warrant,
                "rebuttal" => elenchus::Kind::Rebuttal,
                _ => return None,
            };
            Some(elenchus::Statement { id: s.id.clone(), kind, text: s.text.clone(), target: s.target.clone() })
        })
        .collect();
    let standings: Vec<serde_json::Value> = elenchus::evaluate(&statements)
        .into_iter()
        .map(|(id, st)| {
            let label = match st {
                elenchus::Standing::Supported => "supported",
                elenchus::Standing::Contested => "contested",
                elenchus::Standing::Unsupported => "unsupported",
            };
            serde_json::json!({ "claim": id, "standing": label })
        })
        .collect();
    Json(serde_json::json!({
        "standings": standings,
        "unsupported": elenchus::unsupported_claims(&statements),
        "contested": elenchus::contested_claims(&statements),
    }))
}

#[derive(Deserialize)]
struct LexClause {
    id: String,
    /// "obligation" | "right" | "condition"
    kind: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    requires: Vec<String>,
}

#[derive(Deserialize)]
struct LexRequest {
    clauses: Vec<LexClause>,
    /// Optional set of failed condition ids to compute breach paths for.
    #[serde(default)]
    failed: Vec<String>,
}

/// POST /api/v1/machines/lex — reduce contract clauses to obligation/right/
/// condition primitives; return unconditional obligations, breach paths for the
/// given failed conditions, and dangling (undefined) conditions. Stateless.
async fn machines_lex_handler(Json(req): Json<LexRequest>) -> Json<serde_json::Value> {
    use physis_pro::machines::lex;
    let clauses: Vec<lex::Clause> = req
        .clauses
        .iter()
        .filter_map(|c| {
            let kind = match c.kind.to_ascii_lowercase().as_str() {
                "obligation" => lex::Kind::Obligation,
                "right" => lex::Kind::Right,
                "condition" => lex::Kind::Condition,
                _ => return None,
            };
            Some(lex::Clause { id: c.id.clone(), kind, text: c.text.clone(), requires: c.requires.clone() })
        })
        .collect();
    Json(serde_json::json!({
        "unconditional_obligations": lex::unconditional_obligations(&clauses),
        "breach_paths": lex::breach_paths(&clauses, &req.failed),
        "dangling_conditions": lex::dangling_conditions(&clauses),
    }))
}

#[derive(Deserialize)]
struct EmpedoclesReading {
    metric: String,
    value: f32,
    baseline: f32,
}

#[derive(Deserialize)]
struct EmpedoclesRequest {
    readings: Vec<EmpedoclesReading>,
    #[serde(default)]
    threshold: Option<f32>,
    /// Optional single time-series to compute a trend slope over.
    #[serde(default)]
    series: Option<Vec<f32>>,
}

/// POST /api/v1/machines/empedocles — classify telemetry readings into the four
/// elements (earth/air/fire/water) and flag stable vs anomaly past threshold;
/// optional trend slope over a series. Stateless.
async fn machines_empedocles_handler(Json(req): Json<EmpedoclesRequest>) -> Json<serde_json::Value> {
    use physis_pro::machines::empedocles;
    let threshold = req.threshold.unwrap_or(1.0);
    let readings: Vec<serde_json::Value> = req
        .readings
        .iter()
        .map(|r| {
            let reading = empedocles::Reading { metric: r.metric.clone(), value: r.value, baseline: r.baseline };
            let element = match reading.element() {
                empedocles::Element::Earth => "earth",
                empedocles::Element::Air => "air",
                empedocles::Element::Fire => "fire",
                empedocles::Element::Water => "water",
            };
            let status = match empedocles::classify(&reading, threshold) {
                empedocles::Status::Stable => "stable",
                empedocles::Status::Anomaly => "anomaly",
            };
            serde_json::json!({ "metric": r.metric, "element": element, "deviation": reading.deviation(), "status": status })
        })
        .collect();
    let trend = req.series.as_ref().map(|s| empedocles::trend(s));
    Json(serde_json::json!({ "readings": readings, "trend": trend }))
}


#[derive(Deserialize)]
struct ToposWaypoint {
    lat: f64,
    lng: f64,
    #[serde(default)]
    label: Option<String>,
    /// Optional RFC3339 timestamp (enables fading detection).
    #[serde(default)]
    at: Option<String>,
}

#[derive(Deserialize)]
struct ToposRequest {
    waypoints: Vec<ToposWaypoint>,
    #[serde(default)]
    radius_km: Option<f64>,
    #[serde(default)]
    min_visits: Option<usize>,
    #[serde(default)]
    stale_days: Option<i64>,
}

/// POST /api/v1/machines/topos — reduce geotagged waypoints to recurring places
/// (clustered, visited >= min_visits) and flag fading ones (not seen within
/// stale_days). Stateless.
async fn machines_topos_handler(Json(req): Json<ToposRequest>) -> Json<serde_json::Value> {
    use physis_pro::machines::topos;
    let waypoints: Vec<topos::Waypoint> = req
        .waypoints
        .iter()
        .map(|w| topos::Waypoint {
            lat: w.lat,
            lng: w.lng,
            label: w.label.clone(),
            at: w.at.as_ref().and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.with_timezone(&chrono::Utc))
            }),
        })
        .collect();

    let radius = req.radius_km.unwrap_or(0.5);
    let min_visits = req.min_visits.unwrap_or(2);
    let stale = req.stale_days.unwrap_or(30);

    let places = topos::detect_places(&waypoints, radius, min_visits);
    let now = chrono::Utc::now();
    let place_json = |p: &topos::Place| {
        serde_json::json!({
            "lat": p.lat,
            "lng": p.lng,
            "label": p.label,
            "visits": p.visits,
            "last": p.last.map(|d| d.to_rfc3339()),
            "radius_km": p.radius_km,
        })
    };
    let all: Vec<serde_json::Value> = places.iter().map(place_json).collect();
    let fading: Vec<serde_json::Value> = topos::fading(&places, now, stale).into_iter().map(place_json).collect();

    Json(serde_json::json!({ "places": all, "fading": fading }))
}


// ─────────────────────────────────────────────────────────────────────────────
// Dynamic axis expansion (Phase 6). Read + propose + promote/demote/lineage.
// Installed axes are persisted (sled) and merged as anchors for the axis layer;
// they do NOT mutate the cell classifier. See src/axes.rs.
// ─────────────────────────────────────────────────────────────────────────────

fn parse_axis_role(role: &Option<String>) -> physis_pro::axes::AxisRole {
    match role.as_deref() {
        Some("mode") => physis_pro::axes::AxisRole::Mode,
        _ => physis_pro::axes::AxisRole::Domain,
    }
}

/// GET /api/v1/axes — current axes: the builtin values derived from the live
/// ontology entries, plus any installed (promoted) axes with their time-decayed
/// activation and lineage.
async fn axes_list_handler(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let shared = state.shared.clone();
    let derived = tokio::task::spawn_blocking(move || {
        physis_pro::axes::derive_axes(&shared.classifier.cell_centroids())
    })
    .await
    .expect("axes derivation completes");

    let now = chrono::Utc::now();
    let store = state.axes.lock().unwrap_or_else(|e| e.into_inner());
    let installed: Vec<serde_json::Value> = store
        .active()
        .into_iter()
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "role": a.role,
                "name": a.name,
                "operator": a.operator,
                "world_scope": a.world_scope,
                "created_at": a.created_at,
                "activation": physis_pro::axes::decayed_activation(a, now),
            })
        })
        .collect();

    Json(serde_json::json!({ "builtin": derived, "installed": installed }))
}

#[derive(Deserialize)]
struct AxesDiscoverRequest {
    texts: Vec<String>,
    /// "domain" (default) | "mode"
    #[serde(default)]
    role: Option<String>,
    /// Best-axis cosine below this = axis-orphan (default 0.5).
    #[serde(default)]
    tau: Option<f32>,
}

/// POST /api/v1/axes/discover — propose NEW axis values (a new domain or mode)
/// from texts that no existing axis can name (axis-orphans), clustered. Anchors
/// include both the builtin derived axes AND installed ones, so a promoted axis's
/// region is no longer orphaned. Each proposal carries its lineage. No grid mutation.
async fn axes_discover_handler(
    State(state): State<SharedState>,
    Json(req): Json<AxesDiscoverRequest>,
) -> Json<serde_json::Value> {
    let shared = state.shared.clone();
    let role = parse_axis_role(&req.role);
    let tau = req.tau.unwrap_or(0.5);

    // Snapshot installed axes (clone out of the lock — not held across the await).
    let installed: Vec<physis_pro::axes::AxisValue> = {
        let store = state.axes.lock().unwrap_or_else(|e| e.into_inner());
        store.axes.clone()
    };

    let proposals = tokio::task::spawn_blocking(move || {
        let centroids = shared.classifier.cell_centroids();
        let mut anchors = physis_pro::axes::axis_anchors(&centroids, role);
        anchors.extend(physis_pro::axes::installed_anchors(&installed, role));
        let embeddings: Vec<Vec<f32>> = req.texts.iter().map(|t| shared.embedder.embed(t)).collect();
        physis_pro::axes::propose_axes(role, &req.texts, &embeddings, &anchors, tau, 0.70, 2, 8)
    })
    .await
    .expect("axes discovery completes");
    Json(serde_json::json!({ "role": role, "proposals": proposals }))
}

#[derive(Deserialize)]
struct AxesPromoteRequest {
    #[serde(default)]
    role: Option<String>,
    name: String,
    #[serde(default)]
    hints: Vec<String>,
    /// The source texts that earned the axis — the lineage (required).
    samples: Vec<String>,
    /// Who is installing it (review path) — required (the cause).
    operator: String,
    #[serde(default)]
    world_scope: Option<String>,
}

/// POST /api/v1/axes/promote — install a proposed axis. The anchor is the centroid
/// of the (re-embedded) sample texts. Refuses any axis without its cause: samples
/// AND operator are required (the Phase 6 lineage invariant). Persisted to sled.
async fn axes_promote_handler(
    State(state): State<SharedState>,
    Json(req): Json<AxesPromoteRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if req.samples.is_empty() || req.operator.trim().is_empty() || req.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "promotion requires name, operator, and samples — an axis must carry its cause" })),
        );
    }
    let role = parse_axis_role(&req.role);
    let shared = state.shared.clone();
    let samples = req.samples.clone();
    let anchor = tokio::task::spawn_blocking(move || {
        let embs: Vec<Vec<f32>> = samples.iter().map(|t| shared.embedder.embed(t)).collect();
        if embs.is_empty() {
            return Vec::<f32>::new();
        }
        let dim = embs[0].len();
        let mut mean = vec![0.0f32; dim];
        for e in &embs {
            if e.len() != dim {
                continue;
            }
            for (i, v) in e.iter().enumerate() {
                mean[i] += v;
            }
        }
        let n = embs.len() as f32;
        mean.iter_mut().for_each(|v| *v /= n);
        mean
    })
    .await
    .expect("anchor embed completes");

    let now = chrono::Utc::now();
    let id = format!("axis-{}-{}", now.timestamp_millis(), req.name.trim().to_lowercase().replace(|c: char| !c.is_alphanumeric(), "-"));
    let av = physis_pro::axes::AxisValue {
        id: id.clone(),
        role,
        name: req.name.trim().to_string(),
        anchor,
        hints: req.hints,
        operator: req.operator.trim().to_string(),
        samples: req.samples,
        world_scope: req.world_scope.unwrap_or_else(|| "global".to_string()),
        created_at: now.to_rfc3339(),
        last_seen: now.to_rfc3339(),
        activation: 1.0,
        archived: false,
    };
    {
        let mut store = state.axes.lock().unwrap_or_else(|e| e.into_inner());
        store.axes.push(av.clone());
        if let Some(db) = &state.db {
            store.save(db);
        }
    }
    log::info!("[axes] promoted {:?} axis '{}' by {} (id={})", role, av.name, av.operator, id);
    (StatusCode::CREATED, Json(serde_json::json!({ "installed": av })))
}

#[derive(Deserialize)]
struct AxesDemoteRequest {
    id: String,
}

/// POST /api/v1/axes/demote — archive an installed axis (kept for provenance, not
/// deleted — exactly as failure nodes are kept). Persisted.
async fn axes_demote_handler(
    State(state): State<SharedState>,
    Json(req): Json<AxesDemoteRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut store = state.axes.lock().unwrap_or_else(|e| e.into_inner());
    match store.axes.iter_mut().find(|a| a.id == req.id) {
        Some(a) => {
            a.archived = true;
            if let Some(db) = &state.db {
                store.save(db);
            }
            (StatusCode::OK, Json(serde_json::json!({ "archived": req.id })))
        }
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "axis id not found" }))),
    }
}

#[derive(Deserialize)]
struct AxesLineageQuery {
    id: String,
}

/// GET /api/v1/axes/lineage?id=… — the cause of an installed axis: who installed
/// it, the source texts that earned it, and when. An axis with no lineage is
/// inadmissible — this is how you audit one.
async fn axes_lineage_handler(
    State(state): State<SharedState>,
    Query(q): Query<AxesLineageQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let store = state.axes.lock().unwrap_or_else(|e| e.into_inner());
    match store.axes.iter().find(|a| a.id == q.id) {
        Some(a) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "id": a.id, "role": a.role, "name": a.name,
                "operator": a.operator, "samples": a.samples,
                "world_scope": a.world_scope, "created_at": a.created_at,
                "archived": a.archived,
            })),
        ),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "axis id not found" }))),
    }
}
