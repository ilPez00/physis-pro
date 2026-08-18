use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{info_span, Instrument};

use axum::{
    extract::{ConnectInfo, Path, Request, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use tower_http::trace::TraceLayer;
use serde::{Deserialize, Serialize};

use base64::Engine;
use uuid::Uuid;

use crate::actor::PDCActor;
use crate::admin::{review_queue, ReviewDocument, ReviewQueue};
use crate::config::{EmbedderKindConfig, OntologyLoader, PhysisConfig};
use crate::core::PhysisCore;
use crate::dream::DreamEngine;
#[cfg(feature = "embed-onnx")]
use crate::embed::clip::ClipEmbedder;
#[cfg(feature = "embed-onnx")]
use crate::embed::jina::JinaEmbedder;
#[cfg(feature = "embed-onnx")]
use crate::embed::onnx::OnnxEmbedder;
use crate::embed::RandomProjectionEmbedder;
use crate::embed::VectorEmbed;
use crate::linguistic::{LinguisticLense, LinguisticRouter};
use crate::mapper::OntologyMapper;
use crate::models::{CoherenceNode, Goal, HumanDomain, HumanMode, NodeView, Score, SemioticGrid};
use crate::rag::{self, RagCorpus};
use crate::structsim::{self, StructNode};
use crate::CoherenceSnapshot;

use crate::ai::onnx_worker::OnnxHolon;
use crate::ai::provider::{Message, ProviderCascade, ProviderConfig};
use crate::classify::{CellClassifier, CellScore, ClassificationAssessment};
use crate::gantt::{
    compute_schedule, compute_trajectory, AbstractDomain, GanttScheduler, ScheduleResult, TaskInput,
};
use crate::graph::IngestRing;
use crate::industrial::{MachineReading, MachineStatus};
use crate::life_os::tracker::{Charge, DriftReport, LifeOSGoal, LifeOSTracker, TodayPlan};
use crate::maintenance::{
    project_time_to_limit, AnomalyScore, HealthBaseline, MaintenanceMonitor, RulEstimate, Sample,
    DEFAULT_SIGMA_THRESHOLD,
};
use crate::oee::{plant_oee, OeeBoard, OeeReport, OeeShift, PlantOee, StopReason};
use crate::operational::OperationalEvent;
use crate::operational_report::OperationalTimeline;
use crate::planner::{self, CellOutcome, PlannerItem, PlannerSimulation};
use crate::quality::QualityTracker;
use crate::rachmaninov::RachmaninovHolon;

/// Read-only context built once at startup, shared by every request WITHOUT
/// locking. The hot paths (embed, classify, translate, ontology/semiotic reads)
/// touch only this, so a CPU-heavy ONNX embed never blocks unrelated requests
/// or the healthcheck. See PREMORTEM.md P1.
mod coherence;
mod dream;
mod pdca;
mod holarchy;
mod industrial;
mod ingest;
mod search;
mod admin_study;
mod axes;
mod machines;
#[cfg(feature = "spanner")]
mod spanner;
#[cfg(feature = "mcp")]
mod mcp;
mod wiki_semiotic;
mod context;
mod ui_chat;

/// Read-only context built once at startup, shared by every request WITHOUT
/// locking. The hot paths (embed, classify, translate, ontology/semiotic reads)
/// touch only this, so a CPU-heavy ONNX embed never blocks unrelated requests
/// or the healthcheck. See PREMORTEM.md P1.
pub struct SharedCtx {
    pub(crate) config: PhysisConfig,
    pub(crate) cascade: Arc<ProviderCascade>,
    pub(crate) ontology: OntologyLoader,
    pub(crate) embedder: Box<dyn VectorEmbed>,
    /// Pre-computed per-cell centroids + classification logic.
    pub(crate) classifier: CellClassifier,
    pub(crate) embed_dim: usize,
    /// Which embedder kind actually loaded (provenance of every vector).
    pub(crate) embedder_kind: &'static str,
    /// True when no real ONNX model loaded and vectors are random-projection noise.
    pub(crate) degraded: bool,
    /// Whether the embedder passed the semantic self-test at startup (P4).
    pub(crate) self_test: bool,
    /// Holarchy ingest ring (the onnx/rachmaninov holons are driven by the
    /// background tasks via their own Arc clones, not through this struct).
    pub(crate) ingest: Arc<IngestRing>,
    /// Explicit opt-in for loopback auth bypass. When false, the server refuses
    /// to start without tokens even on loopback, so an unauthenticated API can
    /// never be exposed accidentally. See PREMORTEM.md P6.
    pub(crate) dev_mode: bool,
    /// Bearer tokens + their roles. Empty table is only allowed when `dev_mode`
    /// is true and the bind is loopback. See src/rbac.rs for the role model
    /// (viewer/editor/admin) enforced by `require_auth`.
    pub(crate) tokens: crate::rbac::TokenTable,
}

/// Mutable runtime state behind a single mutex. Kept small and cheap to mutate so
/// the lock is never held across embedding/inference — only across state writes.
pub struct RuntimeState {
    pub(crate) core: PhysisCore,
    pub(crate) mapper: OntologyMapper,
    pub(crate) actor: PDCActor,
    pub(crate) dreams: DreamEngine,
    pub(crate) goals: Vec<Goal>,
    pub(crate) scheduler: GanttScheduler,
    pub(crate) lifeos: LifeOSTracker,
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

pub struct AppCtx {
    pub(crate) shared: Arc<SharedCtx>,
    /// The "_default" tenant. Holarchy background threads drive this one, and
    /// any request without an X-Physis-User header falls here (back-compat).
    pub(crate) state: Mutex<RuntimeState>,
    /// Per-user session state (goals / PDCA / dreams / coherence), created on
    /// first use and keyed by the X-Physis-User header. The holarchy stays
    /// global (single ingest/sensory pipeline), so a tenant is an isolated
    /// reasoning session, not a separate knowledge graph.
    pub(crate) tenants: Mutex<HashMap<String, RuntimeState>>,
    /// Live-reloadable scan-root guard (the only `/scan` path confinement).
    /// Behind an RwLock so `/config/reload` can repoint it without a restart —
    /// reads are a cheap shared lock and `/scan` isn't a hot path. (The embedder
    /// stays fixed at startup: changing embed_dim live would corrupt every stored
    /// vector, and it's read lock-free on the hot path — see PREMORTEM P1.)
    pub(crate) scan_root: std::sync::RwLock<PathBuf>,
    /// Accumulated interpretant evidence, owned rather than re-read per request.
    ///
    /// It was loaded from disk, mutated and written back inside each handler,
    /// which lost observations whenever two sweeps overlapped: both read the
    /// same state and the last writer won. One owned copy behind one mutex
    /// makes read-modify-write atomic within the process, and also stops
    /// `/classify` re-reading the whole file on every call. Entries are keyed
    /// by tenant, so this single store still isolates tenants.
    pub(crate) salience: Mutex<crate::salience::SalienceStore>,
    /// Plant-wide OEE board. Shop-floor equipment is shared infra, not per-tenant
    /// reasoning state, so it lives here behind one mutex.
    pub(crate) oee: Mutex<OeeBoard>,
    /// Plant-wide predictive-maintenance monitor (health baselines + trends).
    pub(crate) maintenance: Mutex<MaintenanceMonitor>,
    /// Durable state store (sled on the data-dir volume). `None` when it can't be
    /// opened (e.g. no writable volume in local dev) → state is in-memory only.
    /// Cheap to clone (sled::Db is an Arc internally).
    pub(crate) db: Option<sled::Db>,
    /// Dynamically-promoted ontology axes (Phase 6). Persisted in `db`; global
    /// across tenants (operator-installed). Merged as anchors for the axis layer.
    pub(crate) axes: Mutex<crate::axes::AxisStore>,
    /// Quality feedback tracker — auto-loaded from `quality_failures.json` if
    /// it exists on the data dir. Mutex because `report_failure` mutates
    /// penalties and appends a failure record.
    pub(crate) quality: Mutex<QualityTracker>,
    /// Sliding-window per-IP rate limiter guarding every route (PREMORTEM.md P6).
    pub(crate) limiter: RateLimiter,
    /// Server start time for uptime reporting.
    pub(crate) started_at: std::time::Instant,
    #[cfg(feature = "mcp")]
    pub(crate) mcp_sessions: std::sync::Arc<
        tokio::sync::Mutex<
            std::collections::HashMap<
                String,
                futures::channel::mpsc::Sender<rmcp::model::ClientJsonRpcMessage>,
            >,
        >,
    >,
}

impl AppCtx {
    /// Current scan root (clone out of the lock; callers compare/canonicalize).
    pub(crate) fn scan_root(&self) -> PathBuf {
        self.scan_root
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl AppCtx {
    /// Poison-tolerant lock: a panic while the lock is held no longer permanently
    /// 500s every subsequent request — we recover the inner guard. PREMORTEM.md P1.
    pub(crate) fn lock(&self) -> std::sync::MutexGuard<'_, RuntimeState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Run `f` against the runtime state for `uid`. Empty or "_default" routes to
    /// the global default tenant (holarchy-connected); any other id gets its own
    /// isolated session, created on demand. The lock is held across `f`, matching
    /// the single-tenant behavior.
    pub(crate) fn with_user<R>(&self, uid: &str, f: impl FnOnce(&mut RuntimeState) -> R) -> R {
        if uid.is_empty() || uid == "_default" {
            let mut g = self.lock();
            f(&mut g)
        } else {
            let mut map = self.tenants.lock().unwrap_or_else(|e| e.into_inner());
            let rt = map.entry(uid.to_string()).or_insert_with(|| {
                let mut rt = RuntimeState::new(&self.shared);
                // Rehydrate this tenant's persisted LifeOS goals + coherence
                // nodes on first access.
                if let Some(db) = &self.db {
                    rt.lifeos = LifeOSTracker::load(db, uid);
                    rt.goals = load_goals(db, uid);
                    let (loaded, skipped) = rt.core.load_nodes(db, uid);
                    if loaded > 0 || skipped > 0 {
                        log::info!(
                            "tenant {uid}: {loaded} coherence nodes restored, \
                                    {skipped} skipped (embedder mismatch)"
                        );
                    }
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
pub(crate) fn user_key(headers: &HeaderMap) -> String {
    headers
        .get("x-physis-user")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "_default".to_string())
}

pub type SharedState = Arc<AppCtx>;

// ── Off-thread embedding ─────────────────────────────────────────────────────
// Embedding is CPU-heavy (and, for ONNX, serialized behind a session mutex), so
// run it via spawn_blocking — a slow embedder then never blocks a tokio worker
// (the healthcheck + non-embed requests stay responsive). The embedder is
// Send+Sync behind Arc<SharedCtx>.




/// Build a `QualityTracker` from the shared embedder + ontology, replaying any
/// persisted failures so cell penalties survive restarts.
pub(crate) fn build_quality_tracker(shared: &SharedCtx) -> QualityTracker {
    let embedder: Box<dyn VectorEmbed> = Box::new(RandomProjectionEmbedder::new(shared.embed_dim));
    let mut tracker = QualityTracker::new(embedder, shared.ontology.clone());
    let path = shared.config.data_dir.join("quality_failures.json");
    if path.exists() {
        if let Ok(failures) = QualityTracker::load(&path) {
            for f in &failures {
                let penalty = tracker
                    .cell_penalties
                    .entry(f.top_cell.clone())
                    .or_insert(0.0);
                *penalty = (*penalty + f.severity * 0.2).min(1.0);
                if let Some(ref _correct) = f.correct_domain {
                    let boost = tracker.cell_boosts.entry(f.top_cell.clone()).or_insert(0.0);
                    *boost = (*boost + 0.1).min(1.0);
                }
            }
            tracker.failures = failures;
        }
    }
    tracker
}

/// Fold file events into per-entry salience.
///
/// Attribution is to the single nearest ontology entry, not to the whole cell:
/// salience accrues to a sign, and spreading one file's evidence across a
/// cell's members — up to 87 of them — would record nothing about anything.
///
/// `unchanged` files are deliberately **not** recorded as `Retained`. Doing so
/// would make an entry's salience a function of how often the operator happens
/// to poll `/watch` rather than of anything that happened in the world. Files
/// surviving is already captured by the absence of a `Deleted` observation.
///
/// Returns how many observations were folded in.
async fn record_salience(
    state: &SharedState,
    uid: &str,
    diffs: &[crate::scanner::ScanDiff],
    root: &std::path::Path,
) -> usize {
    use crate::salience::Signal;

    // Deletions are scoped by `watch_cache_path` giving each directory its own
    // cache; without that, absence from this sweep is indistinguishable from
    // deletion and every other directory's files would read as refuted.
    let mut pending: Vec<(String, Signal)> = Vec::new();
    for d in diffs {
        for f in &d.changed {
            pending.push((f.structural_path(), Signal::Edited));
        }
        for p in &d.deleted {
            // Cache keys are basenames, so resolve against the swept root to
            // get the label the classifier was given at registration time.
            let full = root.join(p);
            pending.push((full.to_string_lossy().replace('/', " → "), Signal::Deleted));
        }
    }
    if pending.is_empty() {
        return 0;
    }

    let texts: Vec<String> = pending.iter().map(|(t, _)| t.clone()).collect();
    let embeddings = embed_batch_off_thread(state.shared.clone(), texts).await;

    let ctx = crate::context::Context::from_env();
    let overlays = crate::context::Overlays::load(&state.shared.config.data_dir);
    let interp = overlays.interpretant(&ctx, state.shared.ontology.interpretant("human"));

    // One lock spans read-modify-write, so overlapping sweeps cannot clobber
    // each other's observations.
    let mut store = state.salience.lock().unwrap_or_else(|e| e.into_inner());
    let mut n = 0usize;
    for (emb, (_, signal)) in embeddings.iter().zip(pending.iter()) {
        if let Some((entry, _sim)) = state.shared.classifier.best_entry(emb) {
            store.record(uid, &ctx, &entry, *signal, &interp);
            n += 1;
        }
    }
    store.save(&state.shared.config.data_dir);
    n
}

#[derive(Deserialize)]
pub struct QueryParams {
    q: String,
    #[serde(default = "default_max")]
    max: usize,
}
pub(crate) fn default_max() -> usize {
    10
}

#[derive(Deserialize)]
pub struct ScanRequest {
    dir: String,
}

#[derive(Deserialize)]
pub struct WatchRequest {
    dir: String,
}

#[derive(Deserialize)]
pub struct VaultImportRequest {
    dir: String,
    /// Also import the last N commits of the enclosing git repo (0 = skip).
    #[serde(default = "default_vault_git")]
    git: usize,
}

pub(crate) fn default_vault_git() -> usize {
    50
}

#[derive(Deserialize)]
pub struct HistoryImportRequest {
    /// Absolute path to the export file (bookmarks .html, history .json,
    /// feeds .opml/.xml, chat .jsonl/.txt). Confined to scan_root.
    path: String,
}

#[derive(Deserialize)]
pub struct PraxisBackfillRequest {
    /// Absolute path to a Praxis export JSON (notebook entries + trackers,
    /// or a bare array). Confined to scan_root.
    path: String,
}

#[derive(Deserialize)]
pub struct PlannerTextInput {
    id: String,
    source: String,
}

#[derive(Deserialize)]
pub struct PlannerSimulationRequest {
    /// Stable scenario name supplied by the caller.
    scenario_id: String,
    /// JSON ontology document in the same `{interpretant?, domains: [...]}`
    /// shape accepted by `OntologyLoader::load_str`.
    candidate_ontology: String,
    texts: Vec<PlannerTextInput>,
    #[serde(default = "default_planner_confidence")]
    confidence_threshold: f32,
}

pub(crate) fn default_planner_confidence() -> f32 { 0.60 }

#[derive(Serialize)]
pub struct PraxisBackfillResponse {
    items: usize,
    successes: usize,
    inert: usize,
    failures: usize,
    registered: usize,
    deduped: usize,
}

#[derive(Serialize)]
pub struct HistoryImportResponse {
    /// Imported file name.
    file: String,
    items: usize,
    registered: usize,
    deduped: usize,
}

#[derive(Serialize)]
pub struct VaultImportResponse {
    docs: usize,
    notes: usize,
    commits: usize,
    registered: usize,
    deduped: usize,
}

#[derive(Serialize)]
pub struct WatchResponse {
    /// New files found by the sweep.
    new: usize,
    /// Files whose content changed since the last sweep.
    changed: usize,
    /// Files removed since the last sweep.
    deleted: usize,
    /// Labeled core nodes registered (new + changed, deduped by label).
    registered: usize,
    /// Nodes dropped for removed files.
    removed: usize,
    /// Total labeled nodes persisted for this dir after the sweep.
    total_nodes: usize,
    /// Interpretant observations folded into salience by this sweep.
    /// Edited files confirm the entry they classify to; deleted ones refute it.
    observed: usize,
}

#[derive(Deserialize)]
pub struct DreamGenerateRequest {
    #[serde(default = "default_dream_count")]
    count: usize,
    #[serde(default)]
    force: bool,
    /// Include the raw source/result vectors. Off by default: two 768-d arrays
    /// per dream buried the readable proposal and made the response unusable
    /// in a UI.
    #[serde(default)]
    embeddings: bool,
}
pub(crate) fn default_dream_count() -> usize {
    5
}

#[derive(Deserialize)]
pub struct DreamEvaluateRequest {
    id: String,
    grade: Score,
}

#[derive(Deserialize)]
pub struct CoherenceRegisterRequest {
    input: String,
    /// Optional asserted outcome in [-1, 1]: did this actually work?
    /// Praxis has been sending this field since before the axis existed (it was
    /// accepted and dropped); aion sends it too. It is now applied.
    #[serde(default)]
    coherence_score: Option<f32>,
}

#[derive(Deserialize)]
pub struct ContextFilterRequest {
    input: String,
}

#[derive(Deserialize)]
pub struct CompressLogsRequest {
    logs: Vec<String>,
}

#[derive(Deserialize)]
pub struct PDCARequest {
    goal_id: String,
    state_vector: Vec<f32>,
}

#[derive(Serialize)]
pub struct StatsResponse {
    mapper: HashMap<String, usize>,
    core: CoherenceSnapshot,
}

#[derive(Serialize)]
pub struct ScanResponse {
    goals: Vec<Goal>,
    count: usize,
    new: usize,
    changed: usize,
    deleted: usize,
    unchanged: usize,
    reused_labels: usize,
    embedding_cache_hits: usize,
    embedding_cache_misses: usize,
}

#[derive(Deserialize)]
pub struct SqlScanRequest {
    #[serde(alias = "db")]
    database: String,
    table: String,
    key_columns: Vec<String>,
    #[serde(default)]
    text_columns: Option<Vec<String>>,
    #[serde(default)]
    updated_column: Option<String>,
    #[serde(default)]
    reconcile: bool,
}

#[derive(Serialize)]
pub struct SqlScanResponse {
    database: String,
    table: String,
    total: usize,
    new: usize,
    changed: usize,
    deleted: usize,
    unchanged: usize,
    embedding_cache_hits: usize,
    embedding_cache_misses: usize,
}

#[derive(Deserialize)]
pub struct SqlSchemaRequest {
    #[serde(alias = "db")]
    database: String,
}

#[derive(Serialize)]
pub struct SqlSchemaResponse {
    database: String,
    tables: Vec<crate::sql::SqlTable>,
}

#[derive(Deserialize)]
pub struct PostgresScanRequest {
    dsn: String,
    source: String,
    table: String,
    key_columns: Vec<String>,
    #[serde(default)]
    text_columns: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct PostgresSchemaRequest {
    dsn: String,
}

#[derive(Serialize)]
pub struct QueryResponse {
    results: Vec<Vec<String>>,
    count: usize,
}

#[derive(Serialize)]
pub struct CoherenceRegisterResponse {
    node_id: String,
    /// Echoes the stored verdict so a caller can tell an applied assertion from
    /// a silently ignored one. `None` = nobody has judged this node.
    #[serde(skip_serializing_if = "Option::is_none")]
    asserted: Option<f32>,
}

// A node's wire shape is `models::NodeView`, produced by `CoherenceNode::
// observable()`. This module deliberately no longer defines its own node DTO:
// a hand-written one is a second place to forget a field, which is how
// `cell_pin` ended up stored, persisted, and reported by nothing.

#[derive(Serialize)]
pub struct CoherenceNodesResponse {
    results: Vec<NodeView>,
    count: usize,
}

#[derive(Deserialize)]
pub struct AllNodesParams {
    #[serde(default = "default_all_limit")]
    limit: usize,
}
pub(crate) fn default_all_limit() -> usize {
    500
}

#[derive(Serialize)]
pub struct FilterResponse {
    embedding: Vec<f32>,
    valid: bool,
    token_estimate: usize,
}

#[derive(Serialize)]
pub struct CompressResponse {
    compressed: String,
    input_count: usize,
    output_chars: usize,
}

#[derive(Deserialize)]
pub struct TranslateRequest {
    text: String,
    lense: Option<String>,
}

#[derive(Serialize)]
pub struct TranslateResponse {
    results: std::collections::HashMap<String, String>,
}

#[derive(Serialize)]
pub struct IngestResponse {
    goal: Goal,
    message: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    error: String,
}

pub(crate) fn err_response(status: StatusCode, msg: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: msg.to_string(),
        }),
    )
}

// ── Embedder self-test ──────────────────────────────────────────────────

/// Probe whether an embedder is actually semantic: a related pair must score
/// clearly higher cosine than an unrelated pair, and all vectors must be finite
/// and non-empty. Random-projection and broken-inference fallbacks fail this.
pub(crate) fn embedder_self_test(e: &dyn VectorEmbed) -> bool {
    crate::embed::semantic_self_test(e)
}

// ── Auth ───────────────────────────────────────────────────────────────

/// Per-IP sliding-window rate limiter (PREMORTEM.md P6: throttle token guessing
/// and accidental request floods). Lock is held only for a cheap HashMap touch.
pub struct RateLimiter {
    max: usize,
    window: Duration,
    buckets: Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    /// Fresh per-tenant session state. Shares nothing mutable with other users;
    /// the embedder/classifier/cascade/ontology live in SharedCtx and stay global.
    fn new(max: usize, window: Duration) -> Self {
        RateLimiter {
            max,
            window,
            buckets: Mutex::new(HashMap::new()),
        }
    }
    /// Returns true if `key` is still within its quota; false if throttled.
    /// Rejected requests are not counted, so a burst of 429s can't extend the block.
    fn allow(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut map = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let bucket = map.entry(key.to_string()).or_default();
        bucket.retain(|t| now.duration_since(*t) < self.window);
        if bucket.len() < self.max {
            bucket.push(now);
            if map.len() > 4096 {
                map.retain(|_, v| !v.is_empty()); // drop idle buckets
            }
            true
        } else {
            false
        }
    }
}

/// Axum middleware: throttle every request by client IP before auth runs.
async fn rate_limit(
    State(ctx): State<SharedState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if ctx.limiter.allow(&addr.ip().to_string()) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::TOO_MANY_REQUESTS)
    }
}

/// Generate or reuse a request ID, attach it to the tracing span, and echo it
/// back as `X-Request-ID` so clients can correlate logs.
async fn request_id(
    mut headers: HeaderMap,
    req: Request,
    next: Next,
) -> Response {
    let id = headers
        .get("X-Request-ID")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    headers.insert("X-Request-ID", id.parse().unwrap());
    let span = info_span!("http.request", request_id = %id, method = %req.method(), uri = %req.uri());
    async move {
        let mut res = next.run(req).await;
        res.headers_mut()
            .insert("X-Request-ID", id.parse().unwrap());
        res
    }
    .instrument(span)
    .await
}

/// Attach tenant (`X-Physis-User`) to the tracing span for every request.
async fn trace_tenant(
    headers: HeaderMap,
    req: Request,
    next: Next,
) -> Response {
    let uid = headers
        .get("X-Physis-User")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("_default");
    let span = info_span!("http.request", tenant = %uid, method = %req.method(), uri = %req.uri());
    async move { next.run(req).await }
        .instrument(span)
        .await
}

/// Constant-time byte comparison — avoids leaking the token via timing.
pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Require `Authorization: Bearer <secret>` on protected routes, and enforce the
/// minimum role for the request's method + path (see `rbac::min_role_for`):
/// GET/HEAD -> viewer, POST/PUT/DELETE -> editor, config/axis-promote -> admin.
///
/// When no tokens are configured and `dev_mode` is enabled (PHYSIS_DEV),
/// requests pass through. Otherwise auth is mandatory.
async fn require_auth(
    State(state): State<SharedState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    use crate::rbac::min_role_for;
    if state.shared.tokens.is_enforced() {
        let presented = req
            .headers()
            .get(AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "));
        match presented {
            Some(tok) => {
                let min = min_role_for(req.method().as_str(), req.uri().path());
                match state.shared.tokens.authenticate(tok) {
                    Some(role) if role >= min => Ok(next.run(req).await),
                    Some(role) => {
                        log::warn!(
                            "RBAC: role {:?} insufficient for {} {} (needs {:?})",
                            role,
                            req.method(),
                            req.uri().path(),
                            min
                        );
                        Err(StatusCode::FORBIDDEN)
                    }
                    None if constant_time_eq(tok.as_bytes(), b"") => Err(StatusCode::UNAUTHORIZED),
                    None => Err(StatusCode::UNAUTHORIZED),
                }
            }
            None => Err(StatusCode::UNAUTHORIZED),
        }
    } else if state.shared.dev_mode {
        Ok(next.run(req).await)
    } else {
        log::error!("Auth required but no tokens configured and PHYSIS_DEV not set");
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

// ── Handlers ──────────────────────────────────────────────────────────

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
    uptime_secs: u64,
    features: Vec<&'static str>,
    embedder: &'static str,
    embed_dim: usize,
    degraded: bool,
    self_test: bool,
    tenants: usize,
    db: bool,
}

#[allow(clippy::vec_init_then_push)]
async fn health(State(state): State<SharedState>) -> Json<HealthResponse> {
    let tenant_count = state.tenants.lock().unwrap().len();
    let db_ok = state.db.is_some();
    let mut features = vec![];
    #[cfg(feature = "web")] features.push("web");
    #[cfg(feature = "industrial")] features.push("industrial");
    #[cfg(feature = "voice")] features.push("voice");
    #[cfg(feature = "mcp")] features.push("mcp");
    #[cfg(feature = "spanner")] features.push("spanner");
    #[cfg(feature = "wiki-embedded")] features.push("wiki-embedded");

    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        uptime_secs: state.started_at.elapsed().as_secs(),
        features,
        embedder: state.shared.embedder_kind,
        embed_dim: state.shared.embed_dim,
        degraded: state.shared.degraded,
        self_test: state.shared.self_test,
        tenants: tenant_count,
        db: db_ok,
    })
}

// ── /api/v1/chat ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ChatApiRequest {
    messages: Vec<Message>,
    #[serde(default)]
    task_type: Option<String>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    has_image: bool,
    /// Optional token-fixed RAG grounding: retrieve from this sandboxed dir and
    /// prepend the retrieved context (capped to `rag_budget` tokens) as a system
    /// message. Makes the LLM input-context cost deterministic regardless of
    /// source document/object size.
    #[serde(default)]
    rag_dir: Option<String>,
    #[serde(default = "default_rag_tokens")]
    rag_budget: usize,
    #[serde(default = "default_rag_topk")]
    rag_top_k: usize,
}

#[derive(Serialize)]
pub struct ChatApiResponse {
    content: String,
    provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<serde_json::Value>,
    /// Tokens of retrieved context actually injected (≤ rag_budget). `None` when
    /// RAG was not requested — surfaced so the caller can see the bounded cost.
    #[serde(skip_serializing_if = "Option::is_none")]
    rag_tokens: Option<usize>,
}




/// Best-effort extraction of the last user message text, used as the RAG query.
pub(crate) fn last_user_text(messages: &[Message]) -> String {
    for m in messages.iter().rev() {
        if m.role == "user" {
            if let crate::ai::provider::Content::Text(t) = &m.content {
                return t.clone();
            }
        }
    }
    String::new()
}

// ── /api/v1/providers ────────────────────────────────────────────────────────













// ── Spanner Graph sink (Phase 7C) ───────────────────────────────────────────
// Durable-memory mirror: push a tenant's coherence graph up, or pull durable
// rows back and re-embed them under the live embedder. Off the hot path — call
// from a cron (or by hand) rather than per mutation. Gated behind `spanner`.

#[cfg(feature = "spanner")]
#[derive(Deserialize)]
pub struct SpannerSyncRequest {
    /// Durable rows that no longer exist locally are hard-deleted (mirror).
    /// Default true; set false to only upsert.
    #[serde(default = "default_spanner_mirror")]
    mirror: bool,
}

#[cfg(feature = "spanner")]
pub(crate) fn default_spanner_mirror() -> bool {
    true
}

#[cfg(feature = "spanner")]
#[derive(Serialize)]
pub struct SpannerSyncResponse {
    nodes_upserted: usize,
    edges_upserted: usize,
    nodes_deleted: usize,
    edges_deleted: usize,
}

#[cfg(feature = "spanner")]
#[derive(Serialize)]
pub struct SpannerHydrateResponse {
    pulled: usize,
    re_embedded: usize,
}

// ── Graph file manager (backs the /ui page) ─────────────────────────────────
// fs/graph: scan a sandboxed dir → theme↔file semantic graph (FileThemeGraph),
// embeddings stripped. fs/file: sandboxed text preview. fs/move: sandboxed
// rename within scan_root. Every path is confined to scan_root (PREMORTEM P6).

/// Resolve an existing user-supplied path and confine it to scan_root.
pub fn sandbox_existing(
    state: &SharedState,
    raw: &str,
) -> Result<PathBuf, (StatusCode, Json<ErrorResponse>)> {
    let p = PathBuf::from(raw);
    if !p.exists() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            &format!("path not found: {raw}"),
        ));
    }
    let canon = p.canonicalize().map_err(|e| {
        err_response(
            StatusCode::BAD_REQUEST,
            &format!("cannot resolve path: {e}"),
        )
    })?;
    if !canon.starts_with(state.scan_root()) {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            &format!("path outside scan root ({})", state.scan_root().display()),
        ));
    }
    Ok(canon)
}

#[derive(Deserialize)]
pub struct FsGraphQuery {
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
pub struct FsThemeDto {
    id: usize,
    name: String,
    category: Option<String>,
    domain: String,
    mode: String,
}

#[derive(Serialize)]
pub struct FsFileDto {
    id: usize,
    path: String,
    title: String,
    kind: String,
}

#[derive(Serialize)]
pub struct FsEdgeDto {
    theme_id: usize,
    file_id: usize,
    score: f32,
}

/// Semantic similarity edge between two files (undirected; ids index `files`).
#[derive(Serialize)]
pub struct FsSimEdge {
    source: usize,
    target: usize,
    score: f32,
}

#[derive(Serialize)]
pub struct FsGraphResponse {
    root: String,
    categories: Vec<String>,
    themes: Vec<FsThemeDto>,
    files: Vec<FsFileDto>,
    edges: Vec<FsEdgeDto>,
    /// file↔file edges by cosine of the BGE vectors — the classified-item network.
    file_edges: Vec<FsSimEdge>,
}

// ── structsim: compare two data structures node-by-node by vector similarity ──

#[derive(Deserialize)]
pub struct StructSimRequest {
    /// Path A (confined to scan_root). JSON or indentation-structured text.
    a: String,
    /// Path B (confined to scan_root).
    b: String,
    /// Minimum cosine for a field pair to count as a match (default 0.5).
    #[serde(default = "default_struct_threshold")]
    threshold: f32,
}

pub(crate) fn default_struct_threshold() -> f32 {
    0.5
}

/// Flatten arbitrary text into nodes: JSON if it parses, else struct text.
pub(crate) fn flatten_any(text: &str) -> Vec<StructNode> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        structsim::flatten_json(&v)
    } else {
        structsim::flatten_struct_text(text)
    }
}

// ── rag: token-fixed retrieval (hard token-budget context assembly) ──

#[derive(Deserialize)]
pub struct RagRequest {
    /// Directory to retrieve from (confined to scan_root).
    dir: String,
    /// The retrieval query.
    query: String,
    /// Hard token budget for the assembled context (default 1500).
    #[serde(default = "default_rag_tokens")]
    tokens: usize,
    /// Maximum chunks to select (default 8).
    #[serde(default = "default_rag_topk")]
    top_k: usize,
}

pub(crate) fn default_rag_tokens() -> usize {
    1500
}
pub(crate) fn default_rag_topk() -> usize {
    8
}

#[derive(Deserialize)]
pub struct FsFileQuery {
    path: String,
}

#[derive(Serialize)]
pub struct FsFileResponse {
    path: String,
    size: u64,
    truncated: bool,
    content: String,
}

#[derive(Deserialize)]
pub struct FsMoveRequest {
    from: String,
    to: String,
}

#[derive(Serialize)]
pub struct FsMoveResponse {
    from: String,
    to: String,
    moved: bool,
}

#[derive(Deserialize)]
pub struct FsListQuery {
    /// Directory to list. Omitted or empty → the scan root.
    #[serde(default)]
    dir: Option<String>,
}

#[derive(Serialize)]
pub struct FsEntryDto {
    name: String,
    path: String,
    kind: String, // "dir" | "file"
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
}

#[derive(Serialize)]
pub struct FsListResponse {
    /// Engine confinement root — every path the UI can browse lives here.
    root: String,
    dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<String>,
    entries: Vec<FsEntryDto>,
    truncated: bool,
}

#[derive(Deserialize)]
pub struct FsResolveQuery {
    /// Basename to look for under the scan root (from an OS drop or folder picker).
    name: String,
}

#[derive(Serialize)]
pub struct FsResolveHit {
    path: String,
    kind: String,
}

#[derive(Serialize)]
pub struct FsResolveResponse {
    name: String,
    root: String,
    matches: Vec<FsResolveHit>,
}

#[derive(Serialize)]
pub struct DreamReplayEntry {
    node_id: String,
    label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    asserted: Option<Score>,
    density: Score,
    outcome: f32,
    prevented_failure: bool,
}

#[derive(Serialize)]
pub struct QualityResponse {
    /// Per-cell learned penalty, keyed `domain\0mode`. `adjust_score` already
    /// applies these to live classification.
    penalties: std::collections::HashMap<String, f32>,
    /// Per-cell learned boost, same keying.
    boosts: std::collections::HashMap<String, f32>,
    /// How many failures have been reported, of any kind.
    failures: usize,
}

/// How hard one pin disagreement pushes against the cell the classifier chose.
///
/// A pin is **locally authoritative but globally advisory**: it always wins for
/// its own node, and counts as a single low-weight sample everywhere else. At
/// `severity * 0.2` that is +0.05 penalty per dissent, so it takes roughly six
/// independent disagreements to match one typed complaint — a stray click
/// cannot reshape classification for everyone, and repetition still lands.
const PIN_DISSENT_SEVERITY: f32 = 0.25;

/// `QualityTracker` keys a cell as `domain\0mode` (see `cell_centroids`).
pub(crate) fn cell_key(domain: &str, mode: &str) -> String {
    format!("{domain}\x00{mode}")
}

/// The classifier's top cell for a node's vector — the *auto* half of the
/// effective cell. A pin, when the editor set one, outranks this; the choice
/// lives in `CoherenceNode::observable` so every surface resolves it the same
/// way instead of each deciding for itself.
pub(crate) fn auto_cell(shared: &SharedCtx, node: &CoherenceNode) -> Option<(String, String)> {
    shared
        .classifier
        .classify(&node.embedding)
        .first()
        .map(|c| (c.domain.clone(), c.mode.clone()))
}

// ── Coherence node editing ("exchange parts") ───────────────────────────────
// Semiotic editing of a *single* data node: swap the content (label → re-embed)
// while preserving the structure (id, asserted verdict, holarchy edges, embedder
// provenance). The grid cell is re-derived from the new content; an editor may
// pin a desired cell, which overrides the auto result and survives restarts.

#[derive(Deserialize, Serialize)]
pub struct CellPin {
    domain: String,
    mode: String,
}

#[derive(Deserialize)]
pub struct CoherenceEditRequest {
    id: String,
    /// New content. Re-embedded with the shared embedder; structure preserved.
    label: String,
    /// Semiotic cell pin. Three distinguishable states, which is why this is a
    /// double option: the field **absent** keeps whatever pin the node has
    /// (`PinEdit::Keep` — editing the text is not a retraction), an explicit
    /// `"cell": null` retracts it, and an object re-pins.
    ///
    /// `default` alone is not enough: serde maps an explicit `null` onto the
    /// *outer* `None`, making it indistinguishable from an absent field. The
    /// custom deserializer only runs when the key is present, so it can wrap
    /// whatever it finds — including `null` — in `Some`.
    #[serde(default, deserialize_with = "present_or_null")]
    cell: Option<Option<CellPin>>,
}

/// Distinguish "key absent" from "key present and null". Called only when the
/// key exists, so its result is always `Some(..)`; the `default` supplies
/// `None` for the absent case.
pub(crate) fn present_or_null<'de, D>(de: D) -> Result<Option<Option<CellPin>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<CellPin>::deserialize(de).map(Some)
}

#[derive(Serialize)]
pub struct CoherenceEditResponse {
    ok: bool,
    node_id: String,
    label: String,
    /// Cell the classifier would place the new content in (auto result).
    #[serde(skip_serializing_if = "Option::is_none")]
    auto_cell: Option<CellPin>,
    /// Cell the editor pinned, if any (persisted on the node).
    #[serde(skip_serializing_if = "Option::is_none")]
    pinned_cell: Option<CellPin>,
    /// Cell the classifier placed the OLD content in, before the swap.
    #[serde(skip_serializing_if = "Option::is_none")]
    before_cell: Option<CellPin>,
    /// Caller's verdict, preserved verbatim across the swap.
    #[serde(skip_serializing_if = "Option::is_none")]
    asserted: Option<Score>,
    /// Derived density after the new content was scored.
    coherence_score: Score,
}

#[derive(Deserialize)]
pub struct CoherenceDeleteRequest {
    id: String,
}

// ── Fixed-token node search ──────────────────────────────────────────────────
// Search the tenant's coherence + goal nodes under a hard token budget: results
// are ranked by cosine, then packed greedily so `total_tokens <= budget` no
// matter how large the store grows. Same contract as `/api/v1/rag/retrieve`,
// applied to the live node store instead of on-disk files — the "fixed" token
// cost of a bounded response.

#[derive(Deserialize)]
pub struct SearchNodesRequest {
    query: String,
    /// Hard token budget for the packed result labels (default 600).
    #[serde(default = "default_search_budget")]
    token_budget: usize,
    /// Maximum number of results (default 12).
    #[serde(default = "default_search_topk")]
    top_k: usize,
}

pub(crate) fn default_search_budget() -> usize {
    600
}
pub(crate) fn default_search_topk() -> usize {
    12
}

#[derive(Serialize)]
pub struct SearchNodeHit {
    /// "node" (coherence) or "goal".
    kind: String,
    id: String,
    label: String,
    score: f32,
    tokens: usize,
}

#[derive(Serialize)]
pub struct SearchNodesResponse {
    query: String,
    budget: usize,
    total_tokens: usize,
    truncated: bool,
    results: Vec<SearchNodeHit>,
}

#[derive(Deserialize)]
pub struct OeeMachineRequest {
    machine_id: String,
    shift_length_secs: f32,
    ideal_cycle_time_secs: f32,
}

#[derive(Serialize)]
pub struct OkResponse {
    pub ok: bool,
    pub detail: String,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OeeEvent {
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

// ── Predictive maintenance ───────────────────────────────────────────────────
// Learn a per-machine health baseline from readings the operator marks healthy,
// then score live readings for multivariate drift threshold rules miss. An
// anomaly is registered as a coherence node so the dream loop can propose a
// countermeasure — detect → cohere → dream, the auto-kaizen seam.

/// A telemetry reading on the wire. Every metric optional; `status` is a string
/// (`online`/`offline`/`warning`/`critical`). `timestamp` is optional and
/// defaults to now — supplying it lets callers backfill a metric history so RUL
/// projection is demoable/realistic (the monitor otherwise stamps arrival time).
#[derive(Deserialize)]
pub struct ReadingRequest {
    machine_id: String,
    temperature: Option<f32>,
    pressure: Option<f32>,
    vibration: Option<f32>,
    rpm: Option<f32>,
    status: Option<String>,
    #[serde(default)]
    timestamp: Option<chrono::DateTime<chrono::Utc>>,
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
        let status = match self
            .status
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("offline") => MachineStatus::Offline,
            Some("warning") => MachineStatus::Warning,
            Some("critical") => MachineStatus::Critical,
            _ => MachineStatus::Online,
        };
        MachineReading {
            machine_id: self.machine_id.clone(),
            timestamp: self.timestamp.unwrap_or_else(chrono::Utc::now),
            temperature: self.temperature,
            pressure: self.pressure,
            vibration: self.vibration,
            rpm: self.rpm,
            status,
            raw: self.raw.clone(),
        }
    }
}

#[derive(Serialize)]
pub struct MaintenanceCheckResponse {
    pub score: AnomalyScore,
    pub rul: Option<RulEstimate>,
    pub coherence_node_id: Option<String>,
}

// ── Stateless compute (replica-safe; works online + locally identically) ─────────
// The in-memory board/monitor above holds state in ONE process — fine locally, but
// in a multi-replica cloud deploy each request may hit a different replica, so the
// state shards. These endpoints take ALL inputs in the request and hold nothing:
// the caller (e.g. Praxis/Postgres) persists the shift / baseline / history and
// passes it in. Same compute as the stateful path (shared `oee`/`maintenance`
// modules), just no server state.

#[derive(Deserialize)]
pub struct ReportFormatQuery {
    /// "html" → self-contained HTML document (email-able, browser print-to-PDF);
    /// anything else → JSON `{markdown}`.
    #[serde(default)]
    format: Option<String>,
}

pub(crate) fn report_response(input: &crate::report::ShiftReportInput, format: Option<&str>) -> Response {
    if format == Some("html") {
        Html(crate::report::render_shift_report_html(input)).into_response()
    } else {
        Json(serde_json::json!({
            "markdown": crate::report::render_shift_report_it(input)
        }))
        .into_response()
    }
}

#[derive(Deserialize)]
pub struct DiscoverRequest {
    texts: Vec<String>,
    #[serde(default)]
    coverage_threshold: Option<f32>,
    /// `kind` for the draft ontology JSON (default "discovered").
    #[serde(default)]
    kind: Option<String>,
}

#[derive(Deserialize)]
pub struct OperationalTimelineRequest {
    events: Vec<OperationalEvent>,
}

const OPERATIONAL_TIMELINE_DEMO: &str =
    include_str!("../../tests/fixtures/operational_timeline_demo.json");










#[derive(Deserialize)]
pub struct FoldRequest {
    /// Prior baseline (omit/null to start fresh). Persist the returned one.
    baseline: Option<HealthBaseline>,
    reading: ReadingRequest,
}

#[derive(Deserialize)]
pub struct ScoreRequest {
    baseline: HealthBaseline,
    reading: ReadingRequest,
    sigma_threshold: Option<f32>,
}

#[derive(Deserialize)]
pub struct RulRequest {
    samples: Vec<Sample>,
    limit: f64,
}

/// sled key for a tenant's goals. One blob per tenant rather than one key per
/// goal: goals are a bounded working set (the PDCA plan), not an unbounded
/// graph, so the whole-list write that would be wrong for nodes is right here.
pub(crate) fn goals_key(uid: &str) -> String {
    format!("goals:{uid}")
}

pub(crate) fn save_goals(db: &sled::Db, uid: &str, goals: &[Goal]) -> anyhow::Result<()> {
    db.insert(goals_key(uid), serde_json::to_vec(goals)?)?;
    Ok(())
}

pub(crate) fn load_goals(db: &sled::Db, uid: &str) -> Vec<Goal> {
    db.get(goals_key(uid))
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_slice::<Vec<Goal>>(&v).ok())
        .unwrap_or_default()
}

#[derive(Deserialize)]
pub struct AddTaskRequest {
    task_id: String,
    duration_secs: f64,
    #[serde(default)]
    dependencies: Vec<String>,
}













#[derive(Deserialize)]
pub struct ScheduleComputeRequest {
    tasks: Vec<TaskInput>,
    #[serde(default)]
    goal_progress: Vec<(String, f32)>,
    #[serde(default)]
    domain: Option<String>,
}

// ── LifeOS (native goal/habit tracking) ──────────────────────────────────────
// Goals live in-engine (per tenant) — physis owns them rather than pulling from
// an external Praxis/Agora service. Push goals in, poll status/drift out.

#[derive(Deserialize)]
pub struct LifeOsProgressRequest {
    id: String,
    progress: f32,
}

#[derive(Serialize)]
pub struct LifeOsStatus {
    goals: Vec<LifeOSGoal>,
    drift: DriftReport,
}

#[derive(Deserialize)]
pub struct ChargeRequest {
    charge: Charge,
}

#[derive(Serialize)]
pub struct ConfigReloadResponse {
    reloaded: bool,
    pdca_threshold: f32,
    pdca_window: usize,
    note: String,
}

// ── Holarchy Handlers ───────────────────────────────────────────────

#[derive(Deserialize)]
pub struct HolarchyIngestRequest {
    data: String,
}




#[derive(Deserialize)]
pub struct EdgeWriteRequest {
    /// Coherence node ids (not RawNodeKeys). Both are bridged automatically.
    source: String,
    target: String,
    #[serde(default = "default_edge_weight")]
    weight: f32,
    #[serde(default)]
    semantic_type: u32,
}
pub(crate) fn default_edge_weight() -> f32 {
    1.0
}

#[derive(Serialize)]
pub struct GraphResponse {
    nodes: Vec<(crate::graph::RawNodeKey, crate::graph::NodePayload)>,
    edges: Vec<crate::graph::Edge>,
}

#[derive(Deserialize)]
pub struct NeighborsParams {
    /// Coherence node id to walk from.
    id: String,
}

#[derive(Serialize)]
pub struct NeighborEdge {
    id: String,
    label: String,
    weight: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    asserted: Option<Score>,
}

// ── Semiotic / Ontology Handlers ──────────────────────────────────

#[derive(Serialize)]
pub struct OntologyListing {
    kind: String,
    count: usize,
}




#[derive(Serialize)]
pub struct StudioEntry {
    name: String,
    kind: String,
    domain: Option<String>,
    mode: Option<String>,
    category: Option<String>,
    hints: Vec<String>,
    facets: crate::models::Facets,
    /// Whether the classifier can ever return this entry.
    ///
    /// `classification_domains()` covers only `human` + `custom`, while the
    /// grid renders `all_domains()`. Everything else is authored, loaded, and
    /// displayed, but unreachable by classification. Surfacing the flag per
    /// entry is the point of the studio: the gap is invisible otherwise.
    classified: bool,
    /// True when any facet is set. Facets are the designed expansion beyond
    /// the DOMAIN×MODE grid, and are currently unpopulated everywhere.
    faceted: bool,
}

#[derive(Serialize)]
pub struct StudioResponse {
    entries: Vec<StudioEntry>,
    /// Per-kind totals, split by whether the classifier can reach them.
    kinds: Vec<serde_json::Value>,
    total: usize,
    classified: usize,
    faceted: usize,
}

#[derive(Deserialize)]
pub struct MeaningParams {
    /// Entry name to look up. Omitted, the response lists contested names only.
    #[serde(default)]
    name: Option<String>,
}

#[derive(Serialize)]
pub struct SemioticGridResponse {
    cells: Vec<serde_json::Value>,
}

#[derive(Serialize)]
pub struct SemioticTriangleResponse {
    mermaid: String,
}




#[derive(Serialize)]
pub struct GreimasSquareResponse {
    mermaid: String,
}




#[derive(Deserialize)]
pub struct CategoryDiagramRequest {
    objects: Vec<CategoryObject>,
    morphisms: Vec<CategoryMorphism>,
}

#[derive(Deserialize)]
pub struct CategoryObject {
    id: String,
    label: String,
}

#[derive(Deserialize)]
pub struct CategoryMorphism {
    from: String,
    to: String,
    label: String,
}

#[derive(Serialize)]
pub struct CategoryDiagramResponse {
    mermaid: String,
}




// ── Wiki (in-process Tantivy BM25; replaces the llmwiki subprocess) ──────────
//
// Only `wiki_root` is read without the `wiki-embedded` feature, but the bodies
// are still parsed so a request is rejected on its shape rather than on the
// build, and the response can say which of the two went wrong.

#[derive(Deserialize)]
pub struct WikiSearchRequest {
    #[cfg_attr(not(feature = "wiki-embedded"), allow(dead_code))]
    query: String,
    wiki_root: String,
    #[serde(default)]
    #[cfg_attr(not(feature = "wiki-embedded"), allow(dead_code))]
    top_k: Option<usize>,
}

#[derive(Deserialize)]
pub struct WikiIndexRequest {
    wiki_root: String,
    #[serde(default)]
    #[cfg_attr(not(feature = "wiki-embedded"), allow(dead_code))]
    incremental: bool,
}


#[derive(Serialize)]
pub struct HeatmapResponse {
    table: String,
    matrix: Vec<Vec<f32>>,
}

// ── Classify Handler ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ClassifyRequest {
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
pub struct ClassifyResponse {
    results: Vec<CellScore>,
    top: CellScore,
    assessment: ClassificationAssessment,
    #[serde(skip_serializing_if = "Option::is_none")]
    discovery: Option<crate::discovery::DiscoverySuggestion>,
    context: String,
    overlay_count: usize,
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
    use crate::ai::provider::{Content, RouteHint};
    let options = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let ex = c
                .entries
                .iter()
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
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
                 CREATE=make, LEARN=study). Reply with ONLY a number."
                    .into(),
            ),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
        Message {
            role: "user".into(),
            content: Content::Text(user),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
    ];
    let hint = RouteHint {
        task_type: task_hint.into(),
        has_image: false,
    };
    let (msg, _provider) = cascade
        .complete(&messages, None, &hint, Some(8))
        .await
        .ok()?;
    let txt = msg.content.unwrap_or_default();
    let n: usize = txt
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()?;
    candidates
        .get(n.checked_sub(1)?)
        .map(|c| (c.domain.clone(), c.mode.clone()))
}




// ── Embed Handler ─────────────────────────────────────────────────
// Raw vector embedding — for pgvector storage and semantic search.

#[derive(Deserialize)]
pub struct EmbedRequest {
    text: String,
    /// Optional base64-encoded image bytes (PNG/JPEG). If present, image
    /// embedding is used instead of text embedding.
    #[serde(default)]
    image: Option<String>,
}

#[derive(Serialize)]
pub struct EmbedResponse {
    embedding: Vec<f32>,
    dim: usize,
}

#[derive(Deserialize)]
pub struct VideoEmbedRequest {
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
pub struct VideoEmbedResponse {
    embedding: Vec<f32>,
    dim: usize,
    /// Frames that contributed to the pooled vector.
    frames: usize,
}




// ── Main ──────────────────────────────────────────────────────────────

/// Parse `PHYSIS_RATE_LIMIT` ("N/WINDOW_SECONDS", e.g. "60/60" = 60 req/min).
/// Falls back to 60 req / 60 s when unset or malformed.
pub(crate) fn rate_limit_from_env() -> RateLimiter {
    let (max, window) = std::env::var("PHYSIS_RATE_LIMIT")
        .ok()
        .and_then(|v| {
            let mut parts = v.trim().split('/');
            let n = parts.next()?.parse::<usize>().ok()?;
            let secs = parts.next().unwrap_or("60").parse::<u64>().ok()?;
            Some((n, secs))
        })
        .unwrap_or((60, 60));
    RateLimiter::new(max, Duration::from_secs(window))
}

/// Build the fully wired application: shared state plus the router.
///
/// Split out of `main` so `tests/` can exercise the HTTP surface without
/// binding a port. Every wiring bug found in this codebase — the rate limiter
/// never layered in, handlers reading the wrong tenant, a missing persistence
/// call site — lived here and was invisible to the engine tests.
///
/// Set `PHYSIS_EMBEDDER=random-projection` for a fast start when the vectors'
/// semantics do not matter (tests, cold-start smoke checks).
async fn embed_text(shared: Arc<SharedCtx>, text: String) -> Vec<f32> {
    tokio::task::spawn_blocking(move || shared.embedder.embed(&text))
        .await
        .unwrap_or_default()
}

/// Embed scan labels through the persistent content-addressed cache. The
/// cache key includes the active embedder and dimension, so a model change
/// invalidates old vectors automatically.
async fn embed_scan_labels_cached(
    shared: Arc<SharedCtx>,
    texts: Vec<String>,
) -> (Vec<Vec<f32>>, usize, usize) {
    tokio::task::spawn_blocking(move || {
        let mut cache = crate::pack::EmbedCache::load(&shared.config.data_dir);
        let embeddings = cache.embed_texts(&texts, shared.embedder.as_ref(), shared.embedder_kind);
        let (hits, misses) = cache.stats();
        cache.save(&shared.config.data_dir);
        (embeddings, hits, misses)
    })
    .await
    .unwrap_or_default()
}

/// Embed arbitrary source texts through the same persistent content-addressed
/// cache. SQL rows use their content here, not merely their display label.
async fn embed_texts_cached(
    shared: Arc<SharedCtx>,
    texts: Vec<String>,
) -> (Vec<Vec<f32>>, usize, usize) {
    tokio::task::spawn_blocking(move || {
        let mut cache = crate::pack::EmbedCache::load(&shared.config.data_dir);
        let embeddings = cache.embed_texts(&texts, shared.embedder.as_ref(), shared.embedder_kind);
        let (hits, misses) = cache.stats();
        cache.save(&shared.config.data_dir);
        (embeddings, hits, misses)
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

/// Hash-cache path for one watched directory.
///
/// The cache was a single shared file keyed by *basename*, which made every
/// sweep interfere with every other: watching directory B reported all of
/// directory A's files as deleted (they are simply absent from this sweep),
/// which already dropped their coherence nodes and — once the interpretant
/// loop landed — would have recorded a refutation against every entry those
/// files ever matched. Two files sharing a basename across directories
/// collided outright. One cache per directory scopes the diff to what the
/// sweep actually looked at.
fn watch_cache_path(data_dir: &std::path::Path, dir: &std::path::Path) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    dir.hash(&mut h);
    data_dir.join(format!("watch_hash_cache_{:016x}.json", h.finish()))
}

/// Build the fully wired application: shared state plus the router.
///
/// Split out of `main` so `tests/` can exercise the HTTP surface without
/// binding a port. Every wiring bug found in this codebase — the rate limiter
/// never layered in, handlers reading the wrong tenant, a missing persistence
/// call site — lived here and was invisible to the engine tests.
///
/// Set `PHYSIS_EMBEDDER=random-projection` for a fast start when the vectors'
/// semantics do not matter (tests, cold-start smoke checks).
pub async fn build_app() -> Result<(SharedState, Router), String> {
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
    let (embedder, embedder_kind): (Box<dyn VectorEmbed>, &'static str) = if matches!(
        std::env::var("PHYSIS_EMBEDDER").as_deref(),
        Ok("random-projection") | Ok("rp")
    ) {
        // Fast-start override: skip the ONNX embedder cascade entirely. Comparable
        // vectors are enough for coherence/machine recall, and loading ONNX models
        // off slow storage stalls cold starts. Default path (below) is unchanged:
        // the real embedder cascade with random-projection only as last resort.
        log::info!("PHYSIS_EMBEDDER=random-projection: fast-start, ONNX cascade skipped.");
        (
            Box::new(RandomProjectionEmbedder::new(config.embed_dim)),
            "random-projection",
        )
    } else {
        #[cfg(feature = "embed-onnx")]
        {
            // BGE-base-en-v1.5 (768-d). Loads `onnx/model.onnx` + tokenizer from
            // models/bge-base-en-v1.5. Strong text-only fallback when Jina's quant
            // won't load under the current ort.
            let make_bge = || -> Option<(Box<dyn VectorEmbed>, &'static str)> {
                let cfg = crate::config::OnnxConfig {
                    enabled: true,
                    model_dir: Some("models/bge-base-en-v1.5".to_string()),
                    dim: 768,
                    max_length: 512,
                    pooling: crate::config::PoolingStrategy::Mean,
                };
                let b = OnnxEmbedder::with_config(&cfg);
                b.is_available()
                    .then(|| (Box::new(b) as Box<dyn VectorEmbed>, "bge-base"))
            };
            // BGE-M3 (1024-d, XLM-R, 100+ languages). Preferred over BGE-base when
            // its weights are present: Italian/multilingual industrial input is why.
            let make_bge_m3 = || -> Option<(Box<dyn VectorEmbed>, &'static str)> {
                let cfg = crate::config::OnnxConfig {
                    enabled: true,
                    model_dir: Some("models/bge-m3".to_string()),
                    dim: 1024,
                    max_length: 512,
                    pooling: crate::config::PoolingStrategy::Cls,
                };
                let b = OnnxEmbedder::with_config(&cfg);
                b.is_available()
                    .then(|| (Box::new(b) as Box<dyn VectorEmbed>, "bge-m3"))
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
                if !ec.enabled {
                    return None;
                }
                match ec.kind {
                    EmbedderKindConfig::JinaV2 => {
                        let j = JinaEmbedder::with_model_dir(&ec.model_dir);
                        j.is_available()
                            .then(|| (Box::new(j) as Box<dyn VectorEmbed>, "jina-v2"))
                    }
                    EmbedderKindConfig::Clip => {
                        let c = ClipEmbedder::with_config(&config.onnx);
                        c.is_available()
                            .then(|| (Box::new(c) as Box<dyn VectorEmbed>, "clip"))
                    }
                    EmbedderKindConfig::MiniLM => {
                        let m = OnnxEmbedder::with_config(&config.onnx);
                        m.is_available()
                            .then(|| (Box::new(m) as Box<dyn VectorEmbed>, "minilm"))
                    }
                    EmbedderKindConfig::BgeBase => make_bge(),
                    EmbedderKindConfig::BgeM3 => make_bge_m3(),
                    EmbedderKindConfig::RandomProjection => Some((
                        Box::new(RandomProjectionEmbedder::new(config.embed_dim)),
                        "random-projection",
                    )),
                }
            };
            gate(try_primary())
                .or_else(|| {
                    gate({
                        let j = JinaEmbedder::with_model_dir("models/jina-clip-v2");
                        j.is_available()
                            .then(|| (Box::new(j) as Box<dyn VectorEmbed>, "jina-v2"))
                    })
                })
                .or_else(|| gate(make_bge()))
                // BGE-M3 sits BELOW bge-base: measured 2026-06-12, M3 CORE
                // 0.923/0.769 vs bge-base 1.000/0.949; Italian only ties (0.733
                // domain). Explicit `embedders.primary = bge-m3` still selects it.
                .or_else(|| gate(make_bge_m3()))
                .or_else(|| {
                    gate({
                        let c = ClipEmbedder::with_config(&config.onnx);
                        c.is_available()
                            .then(|| (Box::new(c) as Box<dyn VectorEmbed>, "clip"))
                    })
                })
                .or_else(|| {
                    gate({
                        let m = OnnxEmbedder::with_config(&config.onnx);
                        m.is_available()
                            .then(|| (Box::new(m) as Box<dyn VectorEmbed>, "minilm"))
                    })
                })
                .unwrap_or_else(|| {
                    (
                        Box::new(RandomProjectionEmbedder::new(config.embed_dim)),
                        "random-projection",
                    )
                })
        }
        #[cfg(not(feature = "embed-onnx"))]
        {
            (
                Box::new(RandomProjectionEmbedder::new(config.embed_dim)),
                "random-projection",
            )
        }
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
    // Served from the on-disk embedding cache: these seeds are fixed by the
    // ontology, so recomputing them on every boot cost ~78s of startup for a
    // result that never changes. `physis-pro warm` populates the same cache.
    let mut seed_cache = crate::pack::EmbedCache::load(&config.data_dir);
    let classifier =
        CellClassifier::build_cached(&ontology, embedder.as_ref(), embedder_kind, &mut seed_cache);
    seed_cache.save(&config.data_dir);
    log::info!(
        "classifier ready: {} populated cells",
        classifier.cell_count()
    );

    let cascade = Arc::new(ProviderCascade::from_env());
    log::info!(
        "AI cascade: {} providers loaded",
        cascade.provider_names().len()
    );

    // `host` gates the open-relay check below; the bind address itself is
    // serve()'s business, so PHYSIS_PORT is read there.
    let host = std::env::var("PHYSIS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

    // ── Security gate (PREMORTEM.md P6) ──────────────────────────────────
    // A non-loopback bind with no token would publish an unauthenticated API
    // that also proxies paid LLM providers (/api/v1/chat) — an open relay.
    // Refuse to start in that configuration.
    //
    // Tokens: build a role-aware table.
    //   PHYSIS_API_TOKEN (legacy, flat secret)  -> registered as editor:<secret>
    //   PHYSIS_TOKENS     (new, "role:secret" list, space/comma separated)
    //                      -> viewer/editor/admin tokens for least-privilege use
    // See src/rbac.rs for the role model enforced by require_auth.
    let mut tokens = crate::rbac::TokenTable::new();
    if let Some(legacy) = std::env::var("PHYSIS_API_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
    {
        tokens.register(&format!("editor:{}", legacy));
    }
    if let Ok(list) = std::env::var("PHYSIS_TOKENS") {
        for tok in list.split([',', ' ', '\n']).filter(|s| !s.is_empty()) {
            if !tokens.register(tok) {
                log::warn!(
                    "PHYSIS_TOKENS: skipping malformed entry {:?} (want role:secret)",
                    tok
                );
            }
        }
    }
    let is_loopback = matches!(host.as_str(), "127.0.0.1" | "::1" | "localhost");
    let dev_mode = std::env::var("PHYSIS_DEV")
        .ok()
        .filter(|v| !v.is_empty())
        .is_some();
    if !tokens.is_enforced() {
        if !is_loopback {
            log::error!("Refusing to bind {host} without any token — would expose an unauthenticated API + open LLM relay.");
            eprintln!("[physis] FATAL: non-loopback bind ({host}) requires PHYSIS_API_TOKEN or PHYSIS_TOKENS. Set one, or bind 127.0.0.1.");
            return Err(format!("non-loopback bind ({host}) requires PHYSIS_API_TOKEN or PHYSIS_TOKENS"));
        }
        if !dev_mode {
            log::error!("Refusing to start on loopback without any token and without PHYSIS_DEV — set a token or set PHYSIS_DEV=1 for local development.");
            eprintln!("[physis] FATAL: loopback bind without tokens requires PHYSIS_DEV=1 for explicit dev opt-in.");
            return Err("loopback bind without tokens requires PHYSIS_DEV=1".into());
        }
        log::warn!("API auth DISABLED (PHYSIS_DEV + loopback dev bind, no token set).");
    } else {
        log::info!("API auth enabled (RBAC): Bearer tokens required on protected routes (viewer/editor/admin).");
    }

    // Confine /api/v1/scan to a root (path-traversal guard). Defaults to the
    // parent of the data dir (the project root when data_dir is ./.physis);
    // override with PHYSIS_SCAN_ROOT. Canonicalized for real-path compare.
    let scan_root = std::env::var("PHYSIS_SCAN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| config.data_dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))));
    let scan_root = scan_root.canonicalize().unwrap_or(scan_root);
    log::info!("scan root: {}", scan_root.display());

    // Seed the wiki with default physis documentation pages and build the
    // Tantivy BM25 index. Safe to call on every startup (incremental index).
    // Must happen before config is moved into SharedCtx below.
    #[cfg(feature = "wiki-embedded")]
    {
        let wiki_path = config.data_dir.join("wiki");
        crate::wiki::ensure_wiki_indexed(&wiki_path);
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
        dev_mode,
        tokens,
    });

    // Durable state store on the data-dir volume. Best-effort: if it can't open
    // (no writable volume / local dev), run in-memory only.
    let db: Option<sled::Db> = {
        let dir = &shared.config.data_dir;
        // sled::open fails (not creates) when the parent doesn't exist, and a
        // fresh install has no data dir yet — which silently disabled ALL
        // persistence (goals, nodes, OEE, maintenance) until something else
        // happened to create the directory. Make it exist.
        if let Err(e) = std::fs::create_dir_all(dir) {
            log::warn!(
                "cannot create data dir {} ({e}); in-memory only",
                dir.display()
            );
        }
        let path = dir.join("state.sled");
        match sled::open(&path) {
            Ok(d) => {
                log::info!("state persistence: sled at {}", path.display());
                Some(d)
            }
            Err(e) => {
                log::warn!("state persistence disabled ({e}); in-memory only");
                None
            }
        }
    };

    let salience_store = crate::salience::SalienceStore::load(&shared.config.data_dir);

    let state = Arc::new(AppCtx {
        shared: shared.clone(),
        state: Mutex::new(RuntimeState {
            core,
            mapper,
            actor,
            dreams,
            // Rehydrate the default tenant's goals (other tenants load lazily
            // in `with_user`).
            goals: db
                .as_ref()
                .map(|d| load_goals(d, "_default"))
                .unwrap_or_default(),
            scheduler: GanttScheduler::new(Box::new(AbstractDomain)),
            // Rehydrate the default tenant's persisted goals.
            lifeos: db
                .as_ref()
                .map(|d| LifeOSTracker::load(d, "_default"))
                .unwrap_or_default(),
        }),
        tenants: Mutex::new(HashMap::new()),
        salience: Mutex::new(salience_store),
        scan_root: std::sync::RwLock::new(scan_root),
        oee: Mutex::new(db.as_ref().map(OeeBoard::load).unwrap_or_default()),
        maintenance: Mutex::new(
            db.as_ref()
                .map(|d| MaintenanceMonitor::load(d, 256))
                .unwrap_or_else(|| MaintenanceMonitor::new(256)),
        ),
        db: db.clone(),
        axes: Mutex::new(
            db.as_ref()
                .map(crate::axes::AxisStore::load)
                .unwrap_or_default(),
        ),
        quality: Mutex::new(build_quality_tracker(&shared)),
        limiter: rate_limit_from_env(),
        started_at: std::time::Instant::now(),
        #[cfg(feature = "mcp")]
        mcp_sessions: std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
    });

    // Rehydrate the default tenant's coherence nodes (the other tenants load
    // lazily on first access in `with_user`). Done after construction because
    // `core` is moved into the struct above.
    if let Some(db) = state.db().cloned() {
        let (loaded, skipped) = state.lock().core.load_nodes(&db, "_default");
        if loaded > 0 || skipped > 0 {
            log::info!(
                "tenant _default: {loaded} coherence nodes restored, \
                        {skipped} skipped (embedder mismatch)"
            );
        }
    }

    let state_clone = Arc::downgrade(&state);
    // 1. Holarchy Graph Updater. Holds only a Weak ref: when every strong
    // handle to AppCtx is gone (server shutting down, or an embedded test
    // dropping the router), the upgrade fails and this exits instead of keeping
    // the sled Db open forever — which also lets a new app open the same data
    // dir in-process.
    std::thread::spawn(move || {
        while let Ok((payload, embedding)) = graph_rx.recv() {
            let Some(strong) = state_clone.upgrade() else {
                break;
            };
            let mut s = strong.lock();
            s.core.holarchy.add_node(payload, embedding);
        }
    });

    let onnx_clone = onnx.clone();
    let rachmaninov_clone = rachmaninov.clone();
    let state_clone_2 = Arc::downgrade(&state);
    // 2. PDCA/Rachmaninov Tick Cycle (every 10s). Fully self-contained now —
    // LifeOS goals live in-engine, so a tick is pure-sync: mirror the native
    // goals into the schedule, recompute CPM, and log Gantt-aware dreams. No
    // outbound HTTP to Praxis/Agora. Strong refs are taken per-tick; between
    // ticks only the Weak handle is held, so dropping the app state stops it.
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            let Some(strong) = state_clone_2.upgrade() else {
                break;
            };

            let mut s = strong.lock();
            let mut rach = rachmaninov_clone.lock().unwrap_or_else(|e| e.into_inner());
            s.core.tick_holarchy(&onnx_clone, &mut rach);
            // Mirror native LifeOS goals into the schedule, then recompute.
            // (split borrow: take lifeos out by ref, scheduler by mut)
            let RuntimeState {
                lifeos, scheduler, ..
            } = &mut *s;
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
        if let Err(e) =
            crate::sensory::listener::start_sensory_server(socket_path, ingest_sensory).await
        {
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
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(30);
                let cache_path = watch_cache_path(&state.shared.config.data_dir, &canon);
                let scanner = crate::network::NetworkScanner::new(
                    vec![canon.clone()],
                    cache_path,
                    interval_secs,
                );
                let watch_state = state.clone();
                log::info!(
                    "Watch-mode: polling {} every {}s",
                    canon.display(),
                    interval_secs
                );
                std::thread::spawn(move || loop {
                    let diffs = scanner.scan_all();
                    let mut labels: Vec<String> = Vec::new();
                    for d in &diffs {
                        for f in d.new.iter().chain(d.changed.iter()) {
                            labels.extend(f.labels());
                        }
                    }
                    if !labels.is_empty() {
                        // Batch-embed outside the lock (CPU-heavy); register under it.
                        // This thread is already off the async runtime.
                        let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
                        let embs = watch_state.shared.embedder.embed_batch(&refs);
                        let mut s = watch_state.lock();
                        for (emb, label) in embs.into_iter().zip(labels) {
                            let id = s.core.register_node_vec_labeled(emb, Some(label));
                            if let Some(db) = watch_state.db() {
                                let _ = s.core.persist_node(db, "_default", &id);
                            }
                        }
                    }
                    scanner.update_cache(&diffs);
                    std::thread::sleep(std::time::Duration::from_secs(interval_secs));
                });
            }
            Ok(canon) => log::error!(
                "PHYSIS_WATCH_DIR {} outside scan root {} — watch disabled",
                canon.display(),
                state.scan_root().display()
            ),
            Err(e) => log::error!("PHYSIS_WATCH_DIR invalid ({watch_dir}): {e} — watch disabled"),
        }
    }

    // Public routes: liveness + embedder provenance (no secrets, needed by the
    // Public, unauthenticated routes: healthcheck + embedder metadata (ops dashboards
    // poll it without a token). Everything else — including the /ui SPA page and the
    // MCP SSE transport — is gated by the Bearer-token middleware below.
    let public = Router::new()
        .route("/health", get(health))
        .route("/api/v1/embedder", get(ui_chat::embedder_info_handler))
        // Shared UI runtime. Public because a <script src> carries no Bearer
        // token; see ui_chat::ui_shell_js_handler.
        .route("/ui-shell.js", get(ui_chat::ui_shell_js_handler));

    // Protected routes: everything that mutates state, reads data, or proxies
    // to paid LLM providers. Gated by the Bearer-token middleware.
    let protected = Router::new()
        // Graph file-manager SPA. The page is gated; its data calls hit the
        // authenticated fs/* routes with a token the user pastes in-page.
        .route("/ui", get(ui_chat::graph_fm_ui_handler))
        .route("/demo", get(ui_chat::demo_ui_handler))
        .route("/study", get(ui_chat::study_ui_handler))
        .route("/api/v1/chat", post(ui_chat::chat_handler))
        .route("/api/v1/providers", get(ui_chat::providers_handler))
        .route("/api/v1/stats", get(ui_chat::stats_handler))
        .route("/api/v1/query", get(ui_chat::query_handler))
        .route("/api/v1/scan", post(ingest::scan_handler))
        .route("/api/v1/sql/schema", get(ingest::sql_schema_handler))
        .route("/api/v1/sql/scan", post(ingest::sql_scan_handler))
        .route("/api/v1/sql/postgres/schema", post(ingest::postgres_schema_handler))
        .route("/api/v1/sql/postgres/scan", post(ingest::postgres_scan_handler))
        .route("/api/v1/watch", post(ingest::watch_handler))
        .route("/api/v1/vault/import", post(ingest::vault_import_handler))
        .route("/api/v1/history/import", post(ingest::history_import_handler))
        .route("/api/v1/praxis/backfill", post(ingest::praxis_backfill_handler))
        .route("/api/v1/fs/graph", get(ingest::fs_graph_handler))
        .route("/api/v1/fs/list", get(ingest::fs_list_handler))
        .route("/api/v1/fs/resolve", get(ingest::fs_resolve_handler))
        .route("/api/v1/structsim", post(ingest::structsim_handler))
        .route("/api/v1/rag/retrieve", post(search::rag_retrieve_handler))
        .route("/api/v1/fs/file", get(ingest::fs_file_handler))
        .route("/api/v1/fs/move", post(ingest::fs_move_handler))
        .route("/api/v1/dream/generate", post(dream::dream_generate_handler))
        .route("/api/v1/dream/evaluate", post(dream::dream_evaluate_handler))
        .route("/api/v1/dream/replay", post(dream::dream_replay_handler))
        .route(
            "/api/v1/coherence/snapshot",
            get(coherence::coherence_snapshot_handler),
        )
        .route("/api/v1/coherence/nodes", get(coherence::coherence_nodes_handler))
        .route(
            "/api/v1/coherence/nodes/all",
            get(coherence::coherence_nodes_all_handler),
        )
        .route("/api/v1/quality", get(coherence::quality_handler))
        .route(
            "/api/v1/coherence/register",
            post(coherence::coherence_register_handler),
        )
        .route("/api/v1/coherence/edit", post(coherence::coherence_edit_handler))
        .route("/api/v1/coherence/delete", post(coherence::coherence_delete_handler))
        .route("/api/v1/hypotheses", get(coherence::hypotheses_list_handler).post(coherence::hypothesis_create_handler))
        .route("/api/v1/hypotheses/:id", get(coherence::hypothesis_get_handler))
        .route("/api/v1/hypotheses/:id/evidence", post(coherence::hypothesis_evidence_handler))
        .route("/api/v1/hypotheses/:id/prediction", post(coherence::hypothesis_prediction_handler))
        .route("/api/v1/hypotheses/:id/transition", post(coherence::hypothesis_transition_handler))
        .route("/api/v1/explanation/:id", get(coherence::explanation_get_handler))
        .route("/api/v1/coherence/query", post(coherence::coherence_query_handler))
        .route("/api/v1/contradictions", get(coherence::contradictions_list_handler).post(coherence::contradiction_create_handler))
        .route("/api/v1/contradictions/:id/resolve", post(coherence::contradiction_resolve_handler))
        .route("/api/v1/epistemic/audit", get(coherence::epistemic_audit_handler))
        .route("/api/v1/epistemic/replay", get(coherence::epistemic_replay_handler))
        .route("/api/v1/processes", get(coherence::processes_list_handler))
        .route("/api/v1/ontology/discover", post(coherence::ontology_discover_handler))
        .route("/api/v1/fitness/feedback", post(coherence::fitness_feedback_handler))
        .route("/api/v1/harness/config", get(coherence::harness_config_handler))
        .route("/api/v1/harness/evaluate", post(coherence::harness_evaluate_handler))
        .route("/api/v1/harness/generate", post(coherence::harness_generate_handler))
        .route("/api/v1/search/nodes", post(search::search_nodes_handler))
        .route("/api/v1/oee/machine", post(industrial::oee_machine_handler))
        .route("/api/v1/oee/event", post(industrial::oee_event_handler))
        .route("/api/v1/oee/plant", get(industrial::oee_plant_handler))
        .route("/api/v1/oee/:machine", get(industrial::oee_machine_report_handler))
        .route(
            "/api/v1/maintenance/observe",
            post(industrial::maintenance_observe_handler),
        )
        .route("/api/v1/maintenance/check", post(industrial::maintenance_check_handler))
        .route("/api/v1/oee/compute", post(industrial::oee_compute_handler))
        .route("/api/v1/oee/rollup", post(industrial::oee_rollup_handler))
        .route(
            "/api/v1/report/shift",
            get(admin_study::report_shift_live_handler).post(admin_study::report_shift_compute_handler),
        )
        .route(
            "/api/v1/operational/timeline",
            post(context::operational_timeline_handler),
        )
        .route(
            "/api/v1/operational/timeline/demo",
            get(context::operational_timeline_demo_handler),
        )
        .route(
            "/api/v1/operational/timeline/demo/events",
            get(context::operational_timeline_demo_events_handler),
        )
        .route("/api/v1/study/log", post(admin_study::study_log_handler))
        .route("/api/v1/study/session", get(admin_study::study_session_handler))
        .route("/api/v1/study/clear", post(admin_study::study_clear_handler))
        .route("/api/v1/discover", post(context::discover_handler))
        .route("/api/v1/planner/simulate", post(context::planner_simulate_handler))
        .route("/api/v1/admin/review", post(admin_study::admin_review_handler))
        .route("/api/v1/maintenance/fold", post(industrial::maintenance_fold_handler))
        .route("/api/v1/maintenance/score", post(industrial::maintenance_score_handler))
        .route("/api/v1/maintenance/rul", post(industrial::maintenance_rul_handler))
        .route(
            "/api/v1/maintenance/:machine",
            get(industrial::maintenance_baseline_handler),
        )
        .route("/api/v1/context/filter", post(context::context_filter_handler))
        .route("/api/v1/compress/logs", post(context::compress_logs_handler))
        .route("/api/v1/goals", post(pdca::ingest_prompt_handler))
        .route("/api/v1/pdca/plan", get(pdca::pdca_plan_handler))
        .route("/api/v1/pdca/act", post(pdca::pdca_act_handler))
        .route("/api/v1/pdca/stats", get(pdca::pdca_stats_handler))
        .route("/api/v1/config/reload", post(context::config_reload_handler))
        .route("/api/v1/reconstruct", post(context::reconstruct_handler))
        .route("/api/v1/translate", post(context::translate_handler))
        .route("/api/v1/holarchy/ingest", post(holarchy::holarchy_ingest_handler))
        .route("/api/v1/holarchy/edge", post(holarchy::holarchy_edge_handler))
        .route("/api/v1/holarchy/snapshot", get(holarchy::holarchy_snapshot_handler))
        .route(
            "/api/v1/holarchy/trajectory",
            get(holarchy::holarchy_trajectory_handler),
        )
        .route("/api/v1/holarchy/graph", get(holarchy::holarchy_graph_handler))
        .route(
            "/api/v1/holarchy/neighbors",
            get(holarchy::holarchy_neighbors_handler),
        )
        .route("/api/v1/schedule/status", get(pdca::schedule_status_handler))
        .route(
            "/api/v1/schedule/cross-correlate",
            get(pdca::schedule_cross_correlate_handler),
        )
        .route("/api/v1/schedule/add-task", post(pdca::schedule_add_task_handler))
        .route(
            "/api/v1/schedule/recompute",
            post(pdca::schedule_recompute_handler),
        )
        .route("/api/v1/schedule/compute", post(pdca::schedule_compute_handler))
        .route("/api/v1/lifeos/goal", post(pdca::lifeos_goal_handler))
        .route("/api/v1/lifeos/progress", post(pdca::lifeos_progress_handler))
        .route("/api/v1/lifeos/status", get(pdca::lifeos_status_handler))
        .route("/api/v1/lifeos/charge", post(pdca::lifeos_charge_handler))
        .route("/api/v1/lifeos/today", get(pdca::lifeos_today_handler))
        // Semiotic / Ontology endpoints
        .route("/api/v1/ontology/list", get(wiki_semiotic::ontology_list_handler))
        .route("/api/v1/ontology/studio", get(wiki_semiotic::ontology_studio_handler))
        .route("/api/v1/ontology/meaning", get(wiki_semiotic::ontology_meaning_handler))
        .route("/api/v1/semiotic/grid", get(wiki_semiotic::semiotic_grid_handler))
        .route("/api/v1/semiotic/triangle", get(wiki_semiotic::semiotic_triangle_handler))
        .route("/api/v1/semiotic/square", get(wiki_semiotic::greimas_square_handler))
        .route("/api/v1/semiotic/heatmap", get(wiki_semiotic::heatmap_handler))
        .route("/api/v1/classify", post(wiki_semiotic::classify_handler))
        .route("/api/v1/embed", post(wiki_semiotic::embed_handler))
        .route("/api/v1/embed/video", post(wiki_semiotic::embed_video_handler))
        .route("/api/v1/category/diagram", post(wiki_semiotic::category_diagram_handler))
        // Philosophical machines: registry + stateless reducers.
        .route("/api/v1/machines", get(machines::machines_list_handler))
        .route("/api/v1/machines/chronos", post(machines::machines_chronos_handler))
        .route("/api/v1/machines/topos", post(machines::machines_topos_handler))
        .route("/api/v1/machines/harmony", post(machines::machines_harmony_handler))
        .route("/api/v1/machines/polis", post(machines::machines_polis_handler))
        .route("/api/v1/machines/soma", post(machines::machines_soma_handler))
        .route("/api/v1/machines/elenchus", post(machines::machines_elenchus_handler))
        .route("/api/v1/machines/lex", post(machines::machines_lex_handler))
        .route(
            "/api/v1/machines/empedocles",
            post(machines::machines_empedocles_handler),
        )
        .route("/api/v1/machines/erosp", post(machines::machines_erosp_handler))
        .route("/api/v1/machines/erosa", post(machines::machines_erosa_handler))
        .route("/api/v1/machines/techne", post(machines::machines_techne_handler))
        .route("/api/v1/machines/euclid", post(machines::machines_euclid_handler))
        .route("/api/v1/machines/plutos", post(machines::machines_plutos_handler))
        .route("/api/v1/machines/krisis", post(machines::machines_krisis_handler))
        .route("/api/v1/machines/pathos", post(machines::machines_pathos_handler))
        .route("/api/v1/machines/chroma", post(machines::machines_chroma_handler))
        .route("/api/v1/machines/kinesis", post(machines::machines_kinesis_handler))
        .route(
            "/api/v1/machines/apodeixis",
            post(machines::machines_apodeixis_handler),
        )
        // Dynamic axis expansion (Phase 6): read + propose + promote/demote/lineage.
        .route("/api/v1/axes", get(axes::axes_list_handler))
        .route("/api/v1/axes/discover", post(axes::axes_discover_handler))
        .route("/api/v1/axes/promote", post(axes::axes_promote_handler))
        .route("/api/v1/axes/demote", post(axes::axes_demote_handler))
        .route("/api/v1/axes/lineage", get(axes::axes_lineage_handler));

    // In-process wiki routes (Tantivy BM25). Mounted unconditionally: the
    // handlers answer "wiki-embedded feature not enabled" when the feature is
    // off, which tells a caller more than a 404 would.
    let protected = protected
        .route("/api/v1/wiki/search", post(wiki_semiotic::wiki_search_handler))
        .route("/api/v1/wiki/index", post(wiki_semiotic::wiki_index_handler));

    // MCP SSE transport — gated like every other route. The URL + port are not a
    // secret; the token is. (Was previously exposed without auth — see PREMORTEM P6.)
    #[cfg(feature = "mcp")]
    let protected = protected
        .route("/mcp/sse", get(mcp::mcp_sse_handler))
        .route("/mcp/message", post(mcp::mcp_message_handler));

    // Spanner Graph sink (Phase 7C) — durable-memory mirror, off the hot path.
    #[cfg(feature = "spanner")]
    let protected = protected
        .route("/api/v1/spanner/sync", post(spanner::spanner_sync_handler))
        .route("/api/v1/spanner/hydrate", post(spanner::spanner_hydrate_handler));

    let protected =
        protected.route_layer(middleware::from_fn_with_state(state.clone(), require_auth));

    // Per-IP throttle, applied to the merged router so it runs BEFORE auth and
    // covers the public routes too — otherwise a token-guessing loop is bounded
    // only by how fast it can open sockets (PREMORTEM.md P6). The limiter was
    // built and configured from env but never layered in, so until now every
    // route was unthrottled.
    let app = public
        .merge(protected)
        .layer(middleware::from_fn(request_id))
        .layer(middleware::from_fn(trace_tenant))
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit))
        .with_state(state.clone());

    // Hand the state back too: tests assert on engine state after driving the
    // HTTP surface (e.g. that a registration actually reached the tenant).
    Ok((state, app))
}

/// Bind and serve. This is all `main` does.
pub async fn serve() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("PHYSIS_LOG").unwrap_or_else(|_| "info,physis=info,tower_http=info".into()),
        )
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();
    let host = std::env::var("PHYSIS_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port = std::env::var("PHYSIS_PORT").unwrap_or_else(|_| "19876".into());
    let Ok((_state, app)) = build_app().await else {
        std::process::exit(1);
    };

    let addr = format!("{host}:{port}");
    println!("Physis Web API listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    // ConnectInfo is what gives `rate_limit` the peer IP; without this the
    // middleware's extractor fails and every request 500s.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

// ── MCP SSE transport ──────────────────────────────────────────────────────
// Model Context Protocol over HTTP/SSE.
//   GET  /mcp/sse?session_id=X  → SSE stream (endpoint + message events)
//   POST /mcp/message           → send JSON-RPC to server
//
// Each SSE session creates a channel-based transport for rmcp::serve_server.







// ─────────────────────────────────────────────────────────────────────────────
// Philosophical machines — HTTP surface.
// Each machine reduces a modality to *primitives* (the smallest elaborable unit).
// These handlers expose the stateless reducers (no core/embedder mutation); the
// node-registering variants (register_*) are a planned follow-up. See src/machines/.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct MachineInfo {
    name: &'static str,
    modality: &'static str,
    primitive: &'static str,
    /// "live" = HTTP reducer wired; "reducer-only" = Rust reducer exists, endpoint pending.
    status: &'static str,
}

#[derive(Deserialize)]
pub struct ChronosEvent {
    label: String,
    /// RFC3339 timestamp, e.g. "2026-06-14T08:00:00Z".
    at: String,
}

#[derive(Deserialize)]
pub struct ChronosRequest {
    events: Vec<ChronosEvent>,
    #[serde(default)]
    horizon_days: Option<i64>,
    #[serde(default)]
    tolerance: Option<f64>,
}

#[derive(Deserialize)]
pub struct HarmonyRequest {
    notes: Vec<i32>,
    #[serde(default)]
    motif_len: Option<usize>,
}

#[derive(Deserialize)]
pub struct PolisInteraction {
    from: String,
    to: String,
    weight: f32,
}

#[derive(Deserialize)]
pub struct PolisRequest {
    interactions: Vec<PolisInteraction>,
    #[serde(default)]
    min_strength: Option<f32>,
    #[serde(default)]
    min_reciprocity: Option<f32>,
    #[serde(default)]
    max_reciprocity: Option<f32>,
}

/// A directed reaching — the shared perception of both eros machines.
#[derive(Deserialize)]
pub struct ErosReach {
    subject: String,
    object: String,
    weight: f32,
}

#[derive(Deserialize)]
pub struct ErospRequest {
    reaches: Vec<ErosReach>,
    #[serde(default)]
    min_intensity: Option<f32>,
    #[serde(default)]
    min_ascent: Option<f32>,
    #[serde(default)]
    max_ascent: Option<f32>,
}

#[derive(Deserialize)]
pub struct ErosaRequest {
    reaches: Vec<ErosReach>,
    #[serde(default)]
    min_balance: Option<f32>,
}

pub(crate) fn to_eros_reaches(reaches: &[ErosReach]) -> Vec<crate::machines::erosp::Reach> {
    reaches
        .iter()
        .map(|r| crate::machines::erosp::Reach {
            subject: r.subject.clone(),
            object: r.object.clone(),
            weight: r.weight,
        })
        .collect()
}

#[derive(Deserialize)]
pub struct TechneFile {
    path: String,
    #[serde(default)]
    lines: Vec<String>,
}

#[derive(Deserialize)]
pub struct TechneRequest {
    files: Vec<TechneFile>,
    /// Optional: list the modules transitively impacted if this one changes.
    #[serde(default)]
    impact_target: Option<String>,
}

#[derive(Deserialize)]
pub struct EuclidPrimitive {
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
pub struct EuclidRequest {
    primitives: Vec<EuclidPrimitive>,
    #[serde(default)]
    circularity_limit: Option<f32>,
    #[serde(default)]
    near: Option<f32>,
}

#[derive(Deserialize)]
pub struct PlutosTransaction {
    from: String,
    to: String,
    amount: f32,
}

#[derive(Deserialize)]
pub struct PlutosRequest {
    transactions: Vec<PlutosTransaction>,
    #[serde(default)]
    min_count: Option<usize>,
    #[serde(default)]
    min_total: Option<f32>,
}

#[derive(Deserialize)]
pub struct KrisisChoice {
    over: String,
    under: String,
}

#[derive(Deserialize)]
pub struct KrisisRequest {
    choices: Vec<KrisisChoice>,
}

#[derive(Deserialize)]
pub struct PathosEpisode {
    trigger: String,
    valence: f32,
    arousal: f32,
}

#[derive(Deserialize)]
pub struct PathosRequest {
    episodes: Vec<PathosEpisode>,
    #[serde(default)]
    min_count: Option<usize>,
    #[serde(default)]
    flat: Option<f32>,
}

#[derive(Deserialize)]
pub struct ChromaColor {
    r: u8,
    g: u8,
    b: u8,
}

#[derive(Deserialize)]
pub struct ChromaRequest {
    colors: Vec<ChromaColor>,
    #[serde(default)]
    min_chroma: Option<f32>,
    #[serde(default)]
    tol: Option<f32>,
}

#[derive(Deserialize)]
pub struct KinesisPoint {
    x: f32,
    y: f32,
}

#[derive(Deserialize)]
pub struct KinesisGesture {
    label: String,
    points: Vec<KinesisPoint>,
}

#[derive(Deserialize)]
pub struct KinesisRequest {
    gestures: Vec<KinesisGesture>,
    #[serde(default)]
    min_move: Option<f32>,
    #[serde(default)]
    min_net: Option<f32>,
    #[serde(default)]
    drop: Option<f32>,
}

#[derive(Deserialize)]
pub struct SomaSession {
    day: u32,
    activity: String,
    load: f32,
    recovery: f32,
}

#[derive(Deserialize)]
pub struct SomaRequest {
    sessions: Vec<SomaSession>,
    #[serde(default)]
    window: Option<usize>,
}

#[derive(Deserialize)]
pub struct ElenchusStatement {
    id: String,
    /// "claim" | "warrant" | "rebuttal"
    kind: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    target: Option<String>,
}

#[derive(Deserialize)]
pub struct ElenchusRequest {
    statements: Vec<ElenchusStatement>,
}

#[derive(Deserialize)]
pub struct LexClause {
    id: String,
    /// "obligation" | "right" | "condition"
    kind: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    requires: Vec<String>,
}

#[derive(Deserialize)]
pub struct LexRequest {
    clauses: Vec<LexClause>,
    /// Optional set of failed condition ids to compute breach paths for.
    #[serde(default)]
    failed: Vec<String>,
}

#[derive(Deserialize)]
pub struct ApodeixisOb {
    id: String,
    #[serde(default)]
    claim: String,
    snippet: String,
}

#[derive(Deserialize)]
pub struct ApodeixisRequest {
    obligations: Vec<ApodeixisOb>,
    #[serde(default)]
    timeout_secs: Option<u64>,
}

#[derive(Deserialize)]
pub struct EmpedoclesReading {
    metric: String,
    value: f32,
    baseline: f32,
}

#[derive(Deserialize)]
pub struct EmpedoclesRequest {
    readings: Vec<EmpedoclesReading>,
    #[serde(default)]
    threshold: Option<f32>,
    /// Optional single time-series to compute a trend slope over.
    #[serde(default)]
    series: Option<Vec<f32>>,
}

#[derive(Deserialize)]
pub struct ToposWaypoint {
    lat: f64,
    lng: f64,
    #[serde(default)]
    label: Option<String>,
    /// Optional RFC3339 timestamp (enables fading detection).
    #[serde(default)]
    at: Option<String>,
}

#[derive(Deserialize)]
pub struct ToposRequest {
    waypoints: Vec<ToposWaypoint>,
    #[serde(default)]
    radius_km: Option<f64>,
    #[serde(default)]
    min_visits: Option<usize>,
    #[serde(default)]
    stale_days: Option<i64>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Dynamic axis expansion (Phase 6). Read + propose + promote/demote/lineage.
// Installed axes are persisted (sled) and merged as anchors for the axis layer;
// they do NOT mutate the cell classifier. See src/axes.rs.
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn parse_axis_role(role: &Option<String>) -> crate::axes::AxisRole {
    match role.as_deref() {
        Some("mode") => crate::axes::AxisRole::Mode,
        _ => crate::axes::AxisRole::Domain,
    }
}

#[derive(Deserialize)]
pub struct AxesDiscoverRequest {
    texts: Vec<String>,
    /// "domain" (default) | "mode"
    #[serde(default)]
    role: Option<String>,
    /// Best-axis cosine below this = axis-orphan (default 0.5).
    #[serde(default)]
    tau: Option<f32>,
}

#[derive(Deserialize)]
pub struct AxesPromoteRequest {
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

#[derive(Deserialize)]
pub struct AxesDemoteRequest {
    id: String,
}

#[derive(Deserialize)]
pub struct AxesLineageQuery {
    id: String,
}


