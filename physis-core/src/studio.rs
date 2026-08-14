//! Ontology Studio — a self-contained web GUI (axum + embedded single-page app)
//! for browsing, editing, classifying against, and feeding back on the semiotic
//! grid. Run with `physis-core studio --port 3000`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};

use crate::classify::CellClassifier;
use crate::discovery::{self, DiscoveryConfig, DiscoveryReport};
use crate::embed::{RandomProjectionEmbedder, VectorEmbed};use crate::models::*;
use crate::ontology::OntologyLoader;
use crate::quality::QualityTracker;
use crate::store;

const APP_HTML: &str = include_str!("studio/app.html");

/// Shared, mutable studio state.
pub struct StudioState {
    pub embedder: Box<dyn VectorEmbed>,
    /// "random-projection" or "onnx"; surfaced so the UI can label vectors.
    pub embedder_kind: String,
    /// True when a real model is loaded (semantic), vs deterministic fallback.
    pub semantic: bool,
    pub ontology: OntologyLoader,
    /// Custom entries edited in the UI, persisted to custom_ontology.json.
    pub custom_entries: HashMap<String, DomainDef>,
    pub classifier: CellClassifier,
    pub quality: QualityTracker,
    pub data_dir: PathBuf,
}

impl StudioState {
    pub fn new(data_dir: PathBuf) -> Self {
        Self::with_embedder(data_dir, None)
    }

    /// Build the studio with an embedder. When `model_dir` is Some and the
    /// `embed-onnx` feature is enabled, an ONNX MiniLM embedder is attempted;
    /// otherwise (or on failure) it falls back to deterministic random projection.
    pub fn with_embedder(data_dir: PathBuf, model_dir: Option<String>) -> Self {
        let (embedder, embedder_kind, semantic) = build_embedder(model_dir);
        let mut ontology = OntologyLoader::load_all();

        // Load persisted custom entries (from previous studio sessions).
        let custom_entries = load_custom_entries(&data_dir);
        if !custom_entries.is_empty() {
            ontology.merge_custom("studio", custom_entries.clone());
        }

        let classifier = CellClassifier::build(&ontology, embedder.as_ref());
        let quality = QualityTracker::load_or_new(&quality_path(&data_dir));
        Self { embedder, embedder_kind, semantic, ontology, classifier, custom_entries, quality, data_dir }
    }

    /// Rebuild the classifier (call after ontology edits) and save custom entries.
    fn rebuild(&mut self) {
        let mut ontology = OntologyLoader::load_all();
        ontology.merge_custom("studio", self.custom_entries.clone());
        self.ontology = ontology;
        self.classifier = CellClassifier::build(&self.ontology, self.embedder.as_ref());
        let _ = save_custom_entries(&self.data_dir, &self.custom_entries);
    }

    fn save_quality(&self) {
        let _ = self.quality.save(&quality_path(&self.data_dir));
    }
}

/// Resolve the active embedder: ONNX when requested + available, else RP.
#[cfg_attr(not(feature = "embed-onnx"), allow(unused_variables))]
fn build_embedder(model_dir: Option<String>) -> (Box<dyn VectorEmbed>, String, bool) {
    #[cfg(feature = "embed-onnx")]
    if let Some(dir) = model_dir.as_deref() {
        let onnx = crate::embed_onnx::OnnxEmbedder::new(dir);
        if onnx.is_available() {
            eprintln!("Ontology Studio embedder: ONNX MiniLM ({dir}, {} dims)", onnx.dimension());
            return (Box::new(onnx), "onnx".to_string(), true);
        }
        eprintln!("warning: no ONNX model at {dir}; falling back to random projection");
    }
    let rp = RandomProjectionEmbedder::new(384);
    (Box::new(rp), "random-projection".to_string(), false)
}

type Shared = Arc<RwLock<StudioState>>;

pub async fn run(port: u16) -> anyhow::Result<()> {
    run_with_model(port, None).await
}

/// Run the studio with an optional ONNX model directory (None = random projection).
pub async fn run_with_model(port: u16, model_dir: Option<String>) -> anyhow::Result<()> {
    let state: Shared = Arc::new(RwLock::new(StudioState::with_embedder(store::data_dir(), model_dir)));

    let app = Router::new()
        .route("/", get(index))
        .route("/api/ontology", get(ontology_json))
        .route("/api/grid", get(grid_json))
        .route("/api/classify", post(classify))
        .route("/api/embedder", get(embedder_json))
        .route("/api/ontology/upsert", post(upsert_entry))
        .route("/api/ontology/delete", post(delete_entry))
        .route("/api/ingest", post(ingest_dir))
        .route("/api/ingest/promote", post(promote_proposal))
        .route("/api/quality", get(quality_json))
        .route("/api/quality/fail", post(quality_fail))
        .route("/api/quality/pass", post(quality_pass))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    eprintln!("Ontology Studio → http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(APP_HTML)
}

#[derive(serde::Serialize)]
struct EmbedderJson {
    kind: String,
    semantic: bool,
    dimension: usize,
}

async fn embedder_json(State(state): State<Shared>) -> Response {
    let s = state.read().unwrap();
    Json(EmbedderJson {
        kind: s.embedder_kind.clone(),
        semantic: s.semantic,
        dimension: s.embedder.dimension(),
    })
    .into_response()
}

// ── Serialization DTOs ───────────────────────────────────────────────

#[derive(serde::Serialize)]
struct OntologyJson {
    categories: Vec<CategoryJson>,
    entry_count: usize,
    custom_count: usize,
}

#[derive(serde::Serialize)]
struct CategoryJson {
    name: String,
    entries: Vec<EntryJson>,
}

#[derive(serde::Serialize)]
struct EntryJson {
    name: String,
    category: Option<String>,
    domain: String,
    mode: String,
    axis_kind: String,
    axis_name: String,
    unit: String,
    hints: Vec<String>,
    /// True when this entry was edited in the studio (persisted custom).
    custom: bool,
}

fn entry_json(name: &str, d: &DomainDef, custom: bool) -> EntryJson {
    EntryJson {
        name: name.to_string(),
        category: d.category.clone(),
        domain: d.domain.clone().unwrap_or_default(),
        mode: d.mode.clone().unwrap_or_default(),
        axis_kind: d.axis_kind.clone().unwrap_or_default(),
        axis_name: d.axis_name.clone().unwrap_or_default(),
        unit: d.unit.clone(),
        hints: d.hints.clone(),
        custom,
    }
}

async fn ontology_json(State(state): State<Shared>) -> Response {
    let s = state.read().unwrap();
    let all = s.ontology.all_domains();
    let mut cats: HashMap<String, Vec<EntryJson>> = HashMap::new();
    let mut uncat = Vec::new();
    for (k, d) in &all {
        let ej = entry_json(k, d, s.custom_entries.contains_key(k));
        match d.category.clone() {
            Some(c) => cats.entry(c).or_default().push(ej),
            None => uncat.push(ej),
        }
    }
    let mut categories: Vec<CategoryJson> = cats
        .into_iter()
        .map(|(name, mut entries)| {
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            CategoryJson { name, entries }
        })
        .collect();
    categories.sort_by(|a, b| a.name.cmp(&b.name));
    if !uncat.is_empty() {
        categories.insert(0, CategoryJson { name: "(uncategorized)".into(), entries: uncat });
    }
    Json(OntologyJson {
        categories,
        entry_count: all.len(),
        custom_count: s.custom_entries.len(),
    })
    .into_response()
}

#[derive(serde::Serialize)]
struct GridJson {
    domains: Vec<String>,
    modes: Vec<String>,
    cells: Vec<GridCellJson>,
    /// Heatmap matrix [domain][mode].
    heatmap: Vec<Vec<u32>>,
}

#[derive(serde::Serialize)]
struct GridCellJson {
    domain: String,
    mode: String,
    entries: Vec<String>,
    count: u32,
}

async fn grid_json(State(state): State<Shared>) -> Response {
    let s = state.read().unwrap();
    let domains: Vec<String> = HumanDomain::all().iter().map(|d| d.as_str().to_string()).collect();
    let modes: Vec<String> = HumanMode::all().iter().map(|m| m.as_str().to_string()).collect();

    // Build a HashMap<(domain, mode), Vec<name>> from classification ontology.
    let mut cell_map: HashMap<(String, String), Vec<String>> = HashMap::new();
    for def in s.ontology.classification_domains() {
        if let (Some(d), Some(m)) = (def.domain.as_deref(), def.mode.as_deref()) {
            cell_map.entry((d.to_string(), m.to_string())).or_default().push(def.name.clone());
        }
    }

    let mut heatmap = vec![vec![0u32; modes.len()]; domains.len()];
    let mut cells = Vec::new();
    for (di, d) in domains.iter().enumerate() {
        for (mi, m) in modes.iter().enumerate() {
            let mut entries = cell_map.get(&(d.clone(), m.clone())).cloned().unwrap_or_default();
            entries.sort();
            let count = entries.len() as u32;
            cells.push(GridCellJson {
                domain: d.clone(),
                mode: m.clone(),
                entries,
                count,
            });
            heatmap[di][mi] = count;
        }
    }
    Json(GridJson { domains, modes, cells, heatmap }).into_response()
}

#[derive(serde::Deserialize)]
struct ClassifyReq {
    text: String,
}

#[derive(serde::Serialize)]
struct ClassifyResp {
    query_embedding_dim: usize,
    results: Vec<CellScoreJson>,
    quality_penalties: HashMap<String, f32>,
}

#[derive(serde::Serialize)]
struct CellScoreJson {
    domain: String,
    mode: String,
    raw_score: f32,
    adjusted_score: f32,
    entries: Vec<String>,
}

async fn classify(State(state): State<Shared>, Json(req): Json<ClassifyReq>) -> Response {
    let s = state.read().unwrap();
    let results = s.classifier.classify_text(&req.text, s.embedder.as_ref());
    let resp = ClassifyResp {
        query_embedding_dim: s.embedder.dimension(),
        results: results
            .iter()
            .take(10)
            .map(|r| {
                let key = format!("{}\x00{}", r.domain, r.mode);
                CellScoreJson {
                    domain: r.domain.clone(),
                    mode: r.mode.clone(),
                    raw_score: r.score,
                    adjusted_score: s.quality.adjust_score(&key, r.score),
                    entries: r.entries.iter().take(12).cloned().collect(),
                }
            })
            .collect(),
        quality_penalties: s.quality.cell_penalties.clone(),
    };
    Json(resp).into_response()
}

#[derive(serde::Deserialize)]
struct EntryReq {
    name: String,
    category: Option<String>,
    domain: String,
    mode: String,
    axis_kind: String,
    axis_name: String,
    unit: String,
    hints: String,
}

async fn upsert_entry(State(state): State<Shared>, Json(req): Json<EntryReq>) -> Response {
    if req.name.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "name required").into_response();
    }
    let hints: Vec<String> = req
        .hints
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let def = DomainDef {
        name: req.name.clone(),
        category: req.category.clone().filter(|c| !c.trim().is_empty()),
        domain: Some(req.domain.clone()),
        mode: Some(req.mode.clone()),
        axis_kind: Some(req.axis_kind.clone()),
        axis_name: Some(req.axis_name.clone()),
        unit: req.unit.clone(),
        hints,
    };
    {
        let mut s = state.write().unwrap();
        s.custom_entries.insert(req.name.clone(), def);
        s.rebuild();
    }
    Json(serde_json::json!({ "ok": true, "upserted": req.name })).into_response()
}

#[derive(serde::Deserialize)]
struct DelReq {
    name: String,
}

async fn delete_entry(State(state): State<Shared>, Json(req): Json<DelReq>) -> Response {
    let mut s = state.write().unwrap();
    let removed = s.custom_entries.remove(&req.name).is_some();
    s.rebuild();
    Json(serde_json::json!({ "ok": removed, "removed": req.name })).into_response()
}

/// Text extensions ingested by the studio discovery pass.
const TEXT_EXTS: &[&str] = &["txt", "md", "rs", "py", "json", "toml", "csv", "log", "xml", "yml", "yaml"];

#[derive(serde::Deserialize)]
struct IngestReq {
    dir: String,
}

#[derive(serde::Serialize)]
struct IngestResp {
    files: usize,
    lines: usize,
    report: DiscoveryReport,
}

async fn ingest_dir(State(state): State<Shared>, Json(req): Json<IngestReq>) -> Response {
    let dir = std::path::PathBuf::from(&req.dir);
    if !dir.is_dir() {
        return (StatusCode::BAD_REQUEST, format!("not a directory: {}", dir.display())).into_response();
    }
    let s = state.read().unwrap();

    // Walk the dir, read text files, split into corpus lines.
    let mut lines: Vec<String> = Vec::new();
    let mut files = 0usize;
    let mut stack = vec![dir.clone()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for entry in rd {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !TEXT_EXTS.contains(&ext) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            files += 1;
            for line in text.lines() {
                let line = line.trim();
                if line.len() > 12 && line.chars().count() < 300 {
                    lines.push(line.to_string());
                }
            }
        }
    }

    let cfg = DiscoveryConfig::default();
    let report = discovery::discover(&lines, &s.classifier, s.embedder.as_ref(), &cfg);
    Json(IngestResp { files, lines: lines.len(), report }).into_response()
}

#[derive(serde::Deserialize)]
struct PromoteReq {
    name: String,
    category: Option<String>,
    domain: String,
    mode: String,
    hints: Vec<String>,
    samples: Vec<String>,
}

async fn promote_proposal(State(state): State<Shared>, Json(req): Json<PromoteReq>) -> Response {
    let mut s = state.write().unwrap();
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return (StatusCode::BAD_REQUEST, "proposal name required").into_response();
    }
    let def = DomainDef {
        name: name.clone(),
        category: req.category.or(Some("Discovered".into())),
        domain: Some(req.domain),
        mode: Some(req.mode),
        axis_kind: Some("discovered".into()),
        axis_name: Some("discovered".into()),
        unit: "items".into(),
        hints: req.hints,
    };
    s.custom_entries.insert(name.clone(), def);
    s.rebuild();
    Json(serde_json::json!({ "ok": true, "promoted": name, "samples": req.samples })).into_response()
}

#[derive(serde::Serialize)]
struct QualityJson {
    failures: Vec<serde_json::Value>,
    penalties: HashMap<String, f32>,
    boosts: HashMap<String, f32>,
}

async fn quality_json(State(state): State<Shared>) -> Response {
    let s = state.read().unwrap();
    let failures: Vec<serde_json::Value> = s
        .quality
        .failures
        .iter()
        .rev()
        .take(20)
        .map(|f| {
            serde_json::json!({
                "id": &f.id[..8.min(f.id.len())],
                "feedback": f.feedback.chars().take(120).collect::<String>(),
                "top_cell": f.top_cell,
                "top_score": f.top_score,
                "severity": f.severity,
                "timestamp": f.timestamp.to_rfc3339(),
            })
        })
        .collect();
    Json(QualityJson {
        failures,
        penalties: s.quality.cell_penalties.clone(),
        boosts: s.quality.cell_boosts.clone(),
    })
    .into_response()
}

#[derive(serde::Deserialize)]
struct FailReq {
    feedback: String,
}

async fn quality_fail(State(state): State<Shared>, Json(req): Json<FailReq>) -> Response {
    let mut s = state.write().unwrap();
    let centroids = s.classifier.cell_centroids();
    let (top_cell, top_score, severity) = {
        let f = s.quality.report_failure(&req.feedback, &centroids, None);
        (f.top_cell.clone(), f.top_score, f.severity)
    };
    s.save_quality();
    Json(serde_json::json!({
        "ok": true,
        "top_cell": top_cell,
        "top_score": top_score,
        "severity": severity,
    }))
    .into_response()
}

#[derive(serde::Deserialize)]
struct PassReq {
    cell: String,
}

async fn quality_pass(State(state): State<Shared>, Json(req): Json<PassReq>) -> Response {
    let mut s = state.write().unwrap();
    s.quality.report_success(&req.cell);
    s.save_quality();
    Json(serde_json::json!({ "ok": true, "boosted": req.cell })).into_response()
}

// ── Persistence helpers ──────────────────────────────────────────────

fn quality_path(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("quality.json")
}

fn custom_ontology_file(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("custom_ontology.json")
}

fn load_custom_entries(data_dir: &std::path::Path) -> HashMap<String, DomainDef> {
    let path = custom_ontology_file(data_dir);
    let contents = std::fs::read_to_string(&path).unwrap_or_default();
    if contents.trim().is_empty() {
        return HashMap::new();
    }
    serde_json::from_str(&contents).unwrap_or_default()
}

fn save_custom_entries(data_dir: &std::path::Path, entries: &HashMap<String, DomainDef>) -> anyhow::Result<()> {
    let _ = std::fs::create_dir_all(data_dir);
    let json = serde_json::to_string_pretty(entries)?;
    std::fs::write(custom_ontology_file(data_dir), json)?;
    Ok(())
}