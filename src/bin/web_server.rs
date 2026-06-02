use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::{
    extract::{Query, State},
    http::StatusCode,
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

use physis_pro::graph::{IngestRing};
use physis_pro::ai::onnx_worker::OnnxHolon;
use physis_pro::ai::provider::{ProviderCascade, Message, Content};
use physis_pro::rachmaninov::RachmaninovHolon;

struct AppState {
    config: PhysisConfig,
    cascade: Arc<ProviderCascade>,
    core: PhysisCore,
    mapper: OntologyMapper,
    actor: PDCActor,
    dreams: DreamEngine,
    ontology: OntologyLoader,
    goals: Vec<Goal>,
    embedder: Box<dyn VectorEmbed>,
    // Holarchy
    onnx: Arc<OnnxHolon>,
    rachmaninov: Arc<Mutex<RachmaninovHolon>>,
    ingest: Arc<IngestRing>,
    /// Pre-computed centroid embeddings for each DOMAIN×MODE cell
    cell_centroids: HashMap<String, Vec<f32>>,
    embed_dim: usize,
}

type SharedState = Arc<Mutex<AppState>>;

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

// ── Handlers ──────────────────────────────────────────────────────────

async fn health() -> Json<&'static str> {
    Json("ok")
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
    let cascade = state.lock().unwrap().cascade.clone();
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
    let cascade = state.lock().unwrap().cascade.clone();
    Json(serde_json::json!({ "providers": cascade.providers_list() }))
}

async fn stats_handler(State(state): State<SharedState>) -> Json<StatsResponse> {
    let s = state.lock().unwrap();
    Json(StatsResponse {
        mapper: s.mapper.stats(),
        core: s.core.snapshot(),
    })
}

async fn query_handler(
    State(state): State<SharedState>,
    Query(params): Query<QueryParams>,
) -> Json<QueryResponse> {
    let s = state.lock().unwrap();
    let results = s.mapper.query(&params.q);
    let limited: Vec<Vec<String>> = results.into_iter().take(params.max).collect();
    Json(QueryResponse { count: limited.len(), results: limited })
}

async fn scan_handler(
    State(state): State<SharedState>,
    Json(req): Json<ScanRequest>,
) -> Result<Json<ScanResponse>, (StatusCode, Json<ErrorResponse>)> {
    let path = PathBuf::from(&req.dir);
    if !path.exists() {
        return Err(err_response(StatusCode::BAD_REQUEST, &format!("directory not found: {}", req.dir)));
    }
    let mut s = state.lock().unwrap();
    let goals = s.mapper.map_filesystem(&path, None);
    s.goals = goals.clone();
    Ok(Json(ScanResponse { count: goals.len(), goals }))
}

async fn dream_generate_handler(
    State(state): State<SharedState>,
    Json(req): Json<DreamGenerateRequest>,
) -> Json<serde_json::Value> {
    let mut s = state.lock().unwrap();

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
}

async fn dream_evaluate_handler(
    State(state): State<SharedState>,
    Json(req): Json<DreamEvaluateRequest>,
) -> Json<serde_json::Value> {
    let mut s = state.lock().unwrap();
    let accepted = s.dreams.evaluate_dream(&req.id, req.grade);
    Json(serde_json::json!({
        "accepted": accepted,
        "dream_id": req.id,
        "grade": req.grade,
    }))
}

async fn coherence_snapshot_handler(
    State(state): State<SharedState>,
) -> Json<CoherenceSnapshot> {
    let s = state.lock().unwrap();
    Json(s.core.snapshot())
}

async fn coherence_register_handler(
    State(state): State<SharedState>,
    Json(req): Json<CoherenceRegisterRequest>,
) -> Json<CoherenceRegisterResponse> {
    let mut s = state.lock().unwrap();
    let embedding = s.embedder.embed(&req.input);
    let node_id = s.core.register_node_vec(embedding);
    Json(CoherenceRegisterResponse { node_id })
}

async fn context_filter_handler(
    State(state): State<SharedState>,
    Json(req): Json<ContextFilterRequest>,
) -> Json<FilterResponse> {
    let s = state.lock().unwrap();
    let result = s.core.filtra_contesto(&req.input, &s.embedder);
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
    let s = state.lock().unwrap();
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
    let s = state.lock().unwrap();
    let router = LinguisticRouter::with_config(&s.config.linguistic);
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
    Json(req): Json<serde_json::Value>,
) -> Json<IngestResponse> {
    let prompt = req.get("prompt").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let mut s = state.lock().unwrap();
    let embedding = s.embedder.embed(&prompt);
    let mut goal = Goal::new_vec(embedding);
    goal.progress = 0.0;
    s.goals.push(goal.clone());
    Json(IngestResponse {
        message: format!("Goal created from prompt ({} chars)", prompt.len()),
        goal,
    })
}

async fn pdca_plan_handler(
    State(state): State<SharedState>,
) -> Json<serde_json::Value> {
    let s = state.lock().unwrap();
    let planned = s.actor.plan(&s.goals);
    let goals: Vec<serde_json::Value> = planned.iter().map(|g| {
        serde_json::json!({
            "id": g.id,
            "embedding": g.embedding,
            "progress": g.progress,
        })
    }).collect();
    Json(serde_json::json!({ "planned": goals, "count": goals.len() }))
}

async fn pdca_act_handler(
    State(state): State<SharedState>,
    Json(req): Json<PDCARequest>,
) -> Json<serde_json::Value> {
    let (before, after) = {
        let s = state.lock().unwrap();
        let goal = s.goals.iter().find(|g| g.id == req.goal_id);
        match goal {
            Some(g) => (g.embedding.clone(), req.state_vector),
            None => return Json(serde_json::json!({"error": "goal not found"})),
        }
    };

    let exp_id = {
        let mut s = state.lock().unwrap();
        s.actor.do_action(&req.goal_id, before, after.clone()).id
    };

    let exps = {
        let s = state.lock().unwrap();
        s.actor.experiences.clone()
    };
    {
        let mut s = state.lock().unwrap();
        let mut goals = std::mem::take(&mut s.goals);
        s.actor.act(&exps, &mut goals);
        s.goals = goals;
    }

    Json(serde_json::json!({
        "experience_id": exp_id,
    }))
}

async fn pdca_stats_handler(
    State(state): State<SharedState>,
) -> Json<physis_pro::actor::PDCAStats> {
    let s = state.lock().unwrap();
    Json(s.actor.stats(&s.goals))
}

async fn reconstruct_handler(
    State(state): State<SharedState>,
    Json(req): Json<ContextFilterRequest>,
) -> Json<physis_pro::Reconstruction> {
    let s = state.lock().unwrap();
    let rec = physis_pro::reconstruct(&req.input, &s.embedder, &s.core, 5);
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
    let s = state.lock().unwrap();
    s.ingest.push(3, req.data.as_bytes().to_vec()); // Default: ThoughtCapture
    Json(serde_json::json!({ "status": "pushed_to_ring" }))
}

async fn holarchy_snapshot_handler(
    State(state): State<SharedState>,
) -> Json<serde_json::Value> {
    let s = state.lock().unwrap();
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
    let s = state.lock().unwrap();
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
    let s = state.lock().unwrap();
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
    let s = state.lock().unwrap();
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
    let s = state.lock().unwrap();
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

#[derive(Serialize)]
struct HeatmapResponse {
    table: String,
    matrix: Vec<Vec<f32>>,
}

async fn heatmap_handler(
    State(state): State<SharedState>,
) -> Json<HeatmapResponse> {
    let s = state.lock().unwrap();
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
}

#[derive(Serialize, Clone)]
struct ClassifyResult {
    domain: String,
    mode: String,
    score: f32,
    entries: Vec<String>,
}

#[derive(Serialize)]
struct ClassifyResponse {
    results: Vec<ClassifyResult>,
    top: ClassifyResult,
}

async fn classify_handler(
    State(state): State<SharedState>,
    Json(req): Json<ClassifyRequest>,
) -> Json<ClassifyResponse> {
    let s = state.lock().unwrap();

    // Embed: use image if provided, otherwise text
    let embedding = if let Some(ref b64) = req.image {
        match base64::engine::general_purpose::STANDARD.decode(b64) {
            Ok(bytes) => {
                s.embedder.embed_image(&bytes).unwrap_or_else(|| {
                    log::warn!("image embedding not available; falling back to text");
                    s.embedder.embed(&req.text)
                })
            }
            Err(e) => {
                log::warn!("base64 decode failed: {e}; falling back to text");
                s.embedder.embed(&req.text)
            }
        }
    } else {
        s.embedder.embed(&req.text)
    };

    let mut results: Vec<ClassifyResult> = Vec::new();
    for (key, centroid) in &s.cell_centroids {
        let score = physis_pro::models::cosine_sim(&embedding, centroid);
        let parts: Vec<&str> = key.splitn(2, '\x00').collect();
        let (domain, mode) = if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            continue;
        };
        let mut entries: Vec<String> = Vec::new();
        for def in s.ontology.classification_domains() {
            if def.domain.as_deref() == Some(&domain) && def.mode.as_deref() == Some(&mode) {
                entries.push(def.name.clone());
            }
        }
        results.push(ClassifyResult { domain, mode, score, entries });
    }
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let top = results.first().cloned().unwrap_or(ClassifyResult {
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

async fn embed_handler(
    State(state): State<SharedState>,
    Json(req): Json<EmbedRequest>,
) -> Json<EmbedResponse> {
    let s = state.lock().unwrap();
    let embedding = if let Some(ref b64) = req.image {
        match base64::engine::general_purpose::STANDARD.decode(b64) {
            Ok(bytes) => s.embedder.embed_image(&bytes).unwrap_or_else(|| {
                log::warn!("image embedding not available; falling back to text");
                s.embedder.embed(&req.text)
            }),
            Err(e) => {
                log::warn!("base64 decode failed: {e}; falling back to text");
                s.embedder.embed(&req.text)
            }
        }
    } else {
        s.embedder.embed(&req.text)
    };
    let dim = embedding.len();
    Json(EmbedResponse { embedding, dim })
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
    let core = PhysisCore::new();
    
    // Initialize Holarchy
    let onnx = Arc::new(OnnxHolon::spawn());
    let (graph_tx, graph_rx) = crossbeam_channel::unbounded();
    let ingest = Arc::new(IngestRing::spawn(onnx.get_tx(), graph_tx));
    let rachmaninov = Arc::new(Mutex::new(RachmaninovHolon::new()));

    let embedder: Box<dyn VectorEmbed> = {
        #[cfg(feature = "embed-onnx")]
        {
            // Try the configured primary embedder, then fall through Jina → CLIP → MiniLM → RP
            let try_primary = || -> Option<Box<dyn VectorEmbed>> {
                let ec = config.embedders.primary.as_ref()?;
                if !ec.enabled { return None; }
                match ec.kind {
                    EmbedderKindConfig::JinaV2 => {
                        let j = JinaEmbedder::with_model_dir(&ec.model_dir);
                        j.is_available().then(|| Box::new(j) as Box<dyn VectorEmbed>)
                    }
                    EmbedderKindConfig::Clip => {
                        let c = ClipEmbedder::with_config(&config.onnx);
                        c.is_available().then(|| Box::new(c) as Box<dyn VectorEmbed>)
                    }
                    EmbedderKindConfig::MiniLM => {
                        let m = OnnxEmbedder::with_config(&config.onnx);
                        m.is_available().then(|| Box::new(m) as Box<dyn VectorEmbed>)
                    }
                    EmbedderKindConfig::RandomProjection => {
                        Some(Box::new(RandomProjectionEmbedder::new(config.embed_dim)))
                    }
                }
            };
            try_primary()
                .or_else(|| {
                    let j = JinaEmbedder::with_model_dir("models/jina-clip-v2");
                    j.is_available().then(|| Box::new(j) as Box<dyn VectorEmbed>)
                })
                .or_else(|| {
                    let c = ClipEmbedder::with_config(&config.onnx);
                    c.is_available().then(|| Box::new(c) as Box<dyn VectorEmbed>)
                })
                .or_else(|| {
                    let m = OnnxEmbedder::with_config(&config.onnx);
                    m.is_available().then(|| Box::new(m) as Box<dyn VectorEmbed>)
                })
                .unwrap_or_else(|| Box::new(RandomProjectionEmbedder::new(config.embed_dim)))
        }
        #[cfg(not(feature = "embed-onnx"))]
        Box::new(RandomProjectionEmbedder::new(config.embed_dim))
    };
    let embed_dim = embedder.dimension();

    // Pre-compute centroid embeddings per DOMAIN×MODE cell from ontology hints.
    // Uses the human grid plus every extra domain-specific ontology so the
    // centroids are enriched across software, fitness, finance, etc.
    let mut cell_centroids: HashMap<String, (Vec<f32>, usize)> = HashMap::new();
    for def in ontology.classification_domains() {
        let domain = match def.domain.as_deref() {
            Some(d) => d,
            None => continue,
        };
        let mode = match def.mode.as_deref() {
            Some(m) => m,
            None => continue,
        };
        let mut text = def.name.clone();
        for hint in &def.hints {
            text.push(' ');
            text.push_str(hint);
        }
        let emb = embedder.embed(&text);
        let key = format!("{domain}\x00{mode}");
        let entry = cell_centroids.entry(key).or_insert((vec![0.0f32; emb.len()], 0));
        for (i, v) in emb.iter().enumerate() {
            entry.0[i] += v;
        }
        entry.1 += 1;
    }
    // Average
    let cell_centroids: HashMap<String, Vec<f32>> = cell_centroids.into_iter()
        .map(|(k, (sum, count))| {
            let n = count as f32;
            (k, sum.into_iter().map(|v| v / n).collect())
        })
        .collect();

    let cascade = Arc::new(ProviderCascade::from_env());
    log::info!("AI cascade: {} providers loaded", cascade.provider_names().len());

    let state = Arc::new(Mutex::new(AppState {
        config,
        cascade: cascade.clone(),
        core,
        mapper,
        actor,
        dreams,
        ontology,
        goals: Vec::new(),
        embedder,
        onnx: onnx.clone(),
        rachmaninov: rachmaninov.clone(),
        ingest: ingest.clone(),
        cell_centroids,
        embed_dim,
    }));

    let state_clone = state.clone();
    // 1. Holarchy Graph Updater
    std::thread::spawn(move || {
        while let Ok((payload, embedding)) = graph_rx.recv() {
            let mut s = state_clone.lock().unwrap();
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
            let mut s = state_clone_2.lock().unwrap();
            let mut rach = rachmaninov_clone.lock().unwrap();
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

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/v1/chat", post(chat_handler))
        .route("/api/v1/providers", get(providers_handler))
        .route("/api/v1/stats", get(stats_handler))
        .route("/api/v1/query", get(query_handler))
        .route("/api/v1/scan", post(scan_handler))
        .route("/api/v1/dream/generate", post(dream_generate_handler))
        .route("/api/v1/dream/evaluate", post(dream_evaluate_handler))
        .route("/api/v1/coherence/snapshot", get(coherence_snapshot_handler))
        .route("/api/v1/coherence/register", post(coherence_register_handler))
        .route("/api/v1/context/filter", post(context_filter_handler))
        .route("/api/v1/compress/logs", post(compress_logs_handler))
        .route("/api/v1/goals", post(ingest_prompt_handler))
        .route("/api/v1/pdca/plan", get(pdca_plan_handler))
        .route("/api/v1/pdca/act", post(pdca_act_handler))
        .route("/api/v1/pdca/stats", get(pdca_stats_handler))
        .route("/api/v1/reconstruct", post(reconstruct_handler))
        .route("/api/v1/translate", post(translate_handler))
        .route("/api/v1/holarchy/ingest", post(holarchy_ingest_handler))
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
        .route("/api/v1/category/diagram", post(category_diagram_handler))
        .with_state(state);

    let port = std::env::var("PHYSIS_PORT").unwrap_or_else(|_| "19876".to_string());
    let host = std::env::var("PHYSIS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let addr = format!("{}:{}", host, port);
    println!("Physis Web API listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}