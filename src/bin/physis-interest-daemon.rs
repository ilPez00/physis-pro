//! physis-interest-daemon — local observer that watches what you work on and
//! distills an interest profile.
//!
//! Three feeds, one sled store, one tiny read API:
//! 1. **Clipboard** — polls every `--interval-secs`, dedupes by content hash,
//!    persists only a preview + hash (never the full text);
//! 2. **Watched dirs** — file events via notify; new/changed analyzable
//!    documents (.md/.txt/.csv/.log/.pptx/.docx) get text+metadata extracted;
//! 3. **Interest profile** — computed on request from stored observations,
//!    classified onto the ontological grid with time decay.
//!
//! Read surface (loopback by default):
//!   GET /health
//!   GET /api/v1/interest/profile?limit=10
//!   GET /api/v1/interest/events?limit=50
//!
//! Run: cargo run --features daemon --bin physis-interest-daemon -- \
//!        --dirs ~/notes ~/reports --port 19878

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use sha2::{Digest, Sha256};

use physis_pro::classify::CellClassifier;
use physis_pro::config::{OntologyLoader, PhysisConfig};
use physis_pro::embed::RandomProjectionEmbedder;
use physis_pro::interest::{
    self, ClipboardCapture, InterestProfile, ObservationKind, ObservedItem,
};

#[derive(Parser)]
#[command(
    name = "physis-interest-daemon",
    about = "Local workflow observer → interest profile"
)]
struct Args {
    /// Comma-separated directories to watch for document activity.
    #[arg(long, value_delimiter = ',')]
    dirs: Vec<PathBuf>,
    /// HTTP port for the read API.
    #[arg(long, default_value_t = 19878)]
    port: u16,
    /// Bind address (loopback unless you really mean it).
    #[arg(long, default_value = "127.0.0.1")]
    bind: String,
    /// Sled database path.
    #[arg(long, default_value = "~/.physis/interest.db")]
    db: PathBuf,
    /// Tenant id under which observations are stored.
    #[arg(long, default_value = "local")]
    user: String,
    /// Poll interval in seconds (clipboard + watcher drain).
    #[arg(long, default_value_t = 2.0)]
    interval_secs: f64,
    /// Disable clipboard capture entirely.
    #[arg(long, default_value_t = false)]
    no_clipboard: bool,
}

fn expand_tilde(p: &std::path::Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    p.to_path_buf()
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();

    let db_path = expand_tilde(&args.db);
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let db = sled::Config::new()
        .path(&db_path)
        .open()
        .with_context(|| format!("open interest db at {}", db_path.display()))?;
    let db = Arc::new(db);

    // Same classification stack as the web server: prefer the trained BGE
    // embedder when the models are present (`--features embed-onnx`), fall back
    // to the deterministic random-projection otherwise. The kind actually
    // loaded is logged — it is the provenance of every interest score this
    // process produces. `PHYSIS_EMBEDDER=random-projection` forces offline mode.
    let t0 = std::time::Instant::now();
    let config = PhysisConfig::default();
    let ontology = OntologyLoader::load_all(&config);

    #[cfg_attr(not(feature = "embed-onnx"), allow(unused_variables))]
    let rp_override = matches!(
        std::env::var("PHYSIS_EMBEDDER").as_deref(),
        Ok("random-projection") | Ok("rp")
    );
    #[cfg(feature = "embed-onnx")]
    let onnx: Option<Box<dyn physis_pro::VectorEmbed>> = if rp_override {
        None
    } else {
        let cfg = physis_pro::config::OnnxConfig {
            enabled: true,
            model_dir: Some("models/bge-base-en-v1.5".to_string()),
            dim: 768,
            max_length: 512,
            pooling: physis_pro::config::PoolingStrategy::Mean,
        };
        let b = physis_pro::embed::onnx::OnnxEmbedder::with_config(&cfg);
        b.is_available()
            .then(|| Box::new(b) as Box<dyn physis_pro::VectorEmbed>)
    };
    #[cfg(not(feature = "embed-onnx"))]
    let onnx: Option<Box<dyn physis_pro::VectorEmbed>> = None;

    let (embedder, embedder_kind): (Arc<dyn physis_pro::VectorEmbed>, &'static str) = match onnx {
        Some(e) if physis_pro::embed::semantic_self_test(e.as_ref()) => (Arc::from(e), "bge-base"),
        Some(_) => {
            log::warn!("BGE loaded but FAILED semantic self-test — using random-projection.");
            (
                Arc::new(RandomProjectionEmbedder::new(config.embed_dim)),
                "random-projection",
            )
        }
        None => (
            Arc::new(RandomProjectionEmbedder::new(config.embed_dim)),
            "random-projection",
        ),
    };
    if embedder_kind == "random-projection" {
        eprintln!(
            "[daemon] offline embedder: random-projection (structural domains only). \
             Run with --features embed-onnx and models/ present for semantic interests."
        );
    }

    // Seeds are fixed by the ontology → serve them from the shared embedding
    // cache so a restart doesn't re-embed ~650 entries (~78s under ONNX).
    let mut seed_cache = physis_pro::pack::EmbedCache::load(&config.data_dir);
    let classifier = Arc::new(CellClassifier::build_cached(
        &ontology,
        embedder.as_ref(),
        embedder_kind,
        &mut seed_cache,
    ));
    seed_cache.save(&config.data_dir);
    log::info!(
        "classifier ready: {} cells, embedder={embedder_kind}, in {:?}",
        classifier.cell_count(),
        t0.elapsed()
    );

    let watched: Vec<PathBuf> = args.dirs.iter().map(|p| expand_tilde(p)).collect();
    let obs_args = ObserverConfig {
        interval: std::time::Duration::from_millis((args.interval_secs * 1000.0) as u64),
        clipboard: !args.no_clipboard,
        user: args.user.clone(),
    };

    {
        let db = db.clone();
        let classifier = classifier.clone();
        let embedder = embedder.clone();
        std::thread::spawn(move || {
            if let Err(e) = observe_loop(obs_args, watched, db, classifier, embedder) {
                log::error!("observer stopped: {e:#}");
            }
        });
    }

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(serve(
        args.bind,
        args.port,
        db,
        classifier,
        embedder,
        args.user.clone(),
    ))
}

// ── Observation loop ─────────────────────────────────────────────────────────

struct ObserverConfig {
    interval: std::time::Duration,
    clipboard: bool,
    user: String,
}

/// Hash of raw bytes, hex head — used to skip unchanged files.
fn content_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))[..24].to_string()
}

fn observe_loop(
    cfg: ObserverConfig,
    watch_dirs: Vec<PathBuf>,
    db: Arc<sled::Db>,
    classifier: Arc<CellClassifier>,
    embedder: Arc<dyn physis_pro::VectorEmbed>,
) -> Result<()> {
    use notify::Watcher as _;
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        })?;
    for dir in &watch_dirs {
        match watcher.watch(dir, notify::RecursiveMode::Recursive) {
            Ok(()) => log::info!("watching {}", dir.display()),
            Err(e) => log::warn!("cannot watch {}: {e}", dir.display()),
        }
    }

    let mut clipboard = if cfg.clipboard {
        arboard::Clipboard::new().ok()
    } else {
        None
    };
    let mut last_clip_hash = String::new();
    // path → content hash, so an editor's save storm analyzes once.
    let mut seen_files: HashMap<String, String> = HashMap::new();
    let mut pending: Vec<ObservedItem> = Vec::new();

    loop {
        // 1. Clipboard.
        if let Some(clip) = clipboard.as_mut() {
            if let Ok(text) = clip.get_text() {
                if !text.trim().is_empty() {
                    let cap = ClipboardCapture::capture(chrono::Utc::now(), &text);
                    if cap.hash != last_clip_hash {
                        last_clip_hash = cap.hash.clone();
                        pending.push(ObservedItem::from_clipboard(&cap));
                    }
                }
            }
        }

        // 2. File events.
        while let Ok(event) = rx.try_recv() {
            for path in event.paths {
                if !path.is_file() || !interest::analyzable(&path) {
                    continue;
                }
                let key = path.display().to_string();
                let Ok(bytes) = std::fs::read(&path) else {
                    continue;
                };
                let hash = content_hash(&bytes);
                if seen_files.get(&key) == Some(&hash) {
                    continue; // unchanged since last analysis
                }
                seen_files.insert(key, hash);
                match interest::analyze_document(&path) {
                    Ok(doc) => {
                        log::info!("document: {} ({})", doc.title, doc.kind);
                        let mut item = ObservedItem::from_document(&doc);
                        item.kind = ObservationKind::Document;
                        pending.push(item);
                    }
                    Err(e) => log::debug!("skip {}: {e}", path.display()),
                }
            }
        }

        // 3. Persist + report profile drift occasionally.
        if !pending.is_empty() {
            let batch = std::mem::take(&mut pending);
            let n = batch.len();
            match interest::append_items(&db, &cfg.user, batch) {
                Ok(total) => {
                    log::info!("stored {n} observation(s), total {total}");
                    if total % 25 < n as usize {
                        let items = interest::load_items(&db, &cfg.user);
                        let mut profile = InterestProfile::default();
                        interest::update_profile(&mut profile, &items, &|text| {
                            top_domains(&classifier, embedder.as_ref(), text)
                        });
                        let heads: Vec<String> = profile
                            .entries
                            .iter()
                            .take(3)
                            .map(|e| format!("{} {:.0}%", e.domain, e.score * 100.0))
                            .collect();
                        log::info!("profile: [{}]", heads.join(", "));
                    }
                }
                Err(e) => log::error!("persist failed: {e}"),
            }
        }

        std::thread::sleep(cfg.interval);
    }
}

/// Classify a text excerpt onto the grid, keeping the strongest few domains.
fn top_domains(
    classifier: &CellClassifier,
    embedder: &dyn physis_pro::VectorEmbed,
    text: &str,
) -> Vec<(String, f32)> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    classifier
        .classify_text(text, embedder)
        .into_iter()
        .take(5)
        .map(|c| (c.domain, c.score))
        .collect()
}

// ── Read API ─────────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct ListQuery {
    limit: Option<usize>,
}

async fn health() -> &'static str {
    "ok"
}

async fn profile(
    axum::extract::Query(q): axum::extract::Query<ListQuery>,
    axum::extract::State((db, classifier, embedder, user)): axum::extract::State<ReadCtx>,
) -> axum::Json<serde_json::Value> {
    let items = interest::load_items(&db, &user);
    let mut p = InterestProfile::default();
    interest::update_profile(&mut p, &items, &|text| {
        top_domains(&classifier, embedder.as_ref(), text)
    });
    p.entries.truncate(q.limit.unwrap_or(10));
    axum::Json(serde_json::to_value(&p).unwrap_or(serde_json::json!({"error": "encode"})))
}

async fn events(
    axum::extract::Query(q): axum::extract::Query<ListQuery>,
    axum::extract::State((db, _, _, user)): axum::extract::State<ReadCtx>,
) -> axum::Json<serde_json::Value> {
    let mut items = interest::load_items(&db, &user);
    items.reverse(); // newest first
    items.truncate(q.limit.unwrap_or(50));
    axum::Json(serde_json::json!({ "count": items.len(), "items": items }))
}

type ReadCtx = (
    Arc<sled::Db>,
    Arc<CellClassifier>,
    Arc<dyn physis_pro::VectorEmbed>,
    String,
);

async fn serve(
    bind: String,
    port: u16,
    db: Arc<sled::Db>,
    classifier: Arc<CellClassifier>,
    embedder: Arc<dyn physis_pro::VectorEmbed>,
    user: String,
) -> Result<()> {
    let app = axum::Router::new()
        .route("/health", axum::routing::get(health))
        .route("/api/v1/interest/profile", axum::routing::get(profile))
        .route("/api/v1/interest/events", axum::routing::get(events))
        .with_state((db, classifier, embedder, user));

    let addr = format!("{bind}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    log::info!("interest read API on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
