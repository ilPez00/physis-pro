use std::path::PathBuf;

use clap::{Parser, Subcommand};

use std::path::Path;

use physis_pro::cli::PhysisApp;
use physis_pro::config::{OntologyLoader, PhysisProConfig};
use physis_pro::embed::{RandomProjectionEmbedder, VectorEmbed};
use physis_pro::models::Score;
use physis_pro::quality::QualityTracker;

#[derive(Parser)]
#[command(name = "physis-pro", version, about = "Industrial backoffice intelligence engine")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Scan a directory and build ontological map
    Scan {
        dir: PathBuf,
        #[arg(short, long, default_value = "wiki")]
        format: String,
    },
    /// Deep AI-powered scan
    DeepScan {
        dir: PathBuf,
    },
    /// Query the ontology trie
    Query {
        query: String,
        #[arg(short, long, default_value = "10")]
        max_results: usize,
    },
    /// Generate stochastic dreams
    Dream {
        #[arg(short, long, default_value = "5")]
        count: usize,
    },
    /// Evaluate a dream
    Evaluate {
        id: String,
        grade: Score,
    },
    /// Start MCP server
    #[cfg(feature = "mcp")]
    Serve,
    /// Watch directories for changes
    Watch {
        dirs: Vec<PathBuf>,
    },
    /// Show stats
    Stats,
    /// Print config
    Config,
    /// Translate text through a linguistic lense
    Translate {
        text: String,
        #[arg(short, long)]
        lense: Option<String>,
    },
    /// Classify a text file against the semiotic grid
    Classify {
        file: PathBuf,
    },

    // ── Business backoffice commands ──
    /// Score the quality of a business output against the semiotic grid
    Score {
        /// Text output to score
        text: String,
    },
    /// Report that an output was wrong (quality failure)
    Fail {
        /// Description of what was wrong
        feedback: String,
        /// Optional correct domain
        #[arg(short, long)]
        correct: Option<String>,
    },
    /// Report that an output was correct (quality success)
    Pass {
        /// Semiotic cell that was correct
        cell: String,
    },
    /// Show quality tracker summary
    Quality,
    /// Ingest documents from ingest directories
    Ingest,
    /// Check industrial process health
    Process,
    /// Show inventory status
    Inventory,
    /// Place an order suggestion (human-in-loop)
    Order {
        /// SKU/item name
        item: String,
        /// Quantity
        qty: u32,
    },
    /// Search the wiki
    Wiki {
        query: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = PhysisProConfig::default();
    let mut app = PhysisApp::new(config.physis.clone());

    match cli.command {
        Commands::Scan { dir, format } => {
            let output = app.run_scan(&dir, &format);
            println!("{output}");
        }
        Commands::DeepScan { dir } => {
            let rt = tokio::runtime::Runtime::new()?;
            let output = rt.block_on(app.run_deep_scan(&dir))?;
            println!("{output}");
        }
        Commands::Query { query, max_results } => {
            let results = app.run_query(&query, max_results);
            if results.is_empty() {
                println!("No results found for: {query}");
            } else {
                for path in results {
                    println!("  {}", path.join(" -> "));
                }
            }
        }
        Commands::Dream { count } => {
            let dreams = app.run_dream(count);
            for dream in &dreams {
                println!(
                    "[{}] source={:?} embedding={:?} grade={:?}",
                    dream.id,
                    &dream.source[..dream.source.len().min(4)],
                    &dream.embedding[..dream.embedding.len().min(4)],
                    dream.grade
                );
            }
        }
        Commands::Evaluate { id, grade } => {
            if app.run_evaluate(&id, grade) {
                println!("Dream {id} evaluated with grade {grade}");
            } else {
                println!("Dream {id} not found");
            }
        }
        #[cfg(feature = "mcp")]
        Commands::Serve => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let config = PhysisConfig::default();
                physis_pro::mcp::start_mcp_server(config).await
            })?;
        }
        Commands::Watch { dirs } => {
            let output = app.run_watch(dirs);
            println!("{output}");
        }
        Commands::Stats => {
            let output = app.run_stats();
            println!("{output}");
        }
        Commands::Config => {
            let config_str = serde_json::to_string_pretty(&PhysisProConfig::default())?;
            println!("{config_str}");
        }
        Commands::Translate { text, lense } => {
            let output = app.run_translate(&text, lense.as_deref());
            println!("{output}");
        }
        Commands::Classify { file } => {
            let output = app.run_classify(&file)?;
            println!("{output}");
        }

        // ── Business commands ──
        Commands::Score { text } => {
            let embedder: Box<dyn VectorEmbed> =
                Box::new(RandomProjectionEmbedder::new(config.physis.embed_dim));
            let embedding = embedder.embed(&text);

            let mut results: Vec<(&str, &str, f32)> = Vec::new();
            for (key, centroid) in &app.cell_centroids {
                let score = physis_pro::models::cosine_sim(&embedding, centroid);
                let mut parts = key.splitn(2, '\x00');
                if let (Some(domain), Some(mode)) = (parts.next(), parts.next()) {
                    results.push((domain, mode, score));
                }
            }
            results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

            println!("=== Quality Score ===");
            for (i, (domain, mode, score)) in results.iter().enumerate() {
                if i == 0 {
                    println!("  TOP: {domain} x {mode} = {score:.3}");
                } else if *score > 0.5 {
                    println!("       {domain} x {mode} = {score:.3}");
                }
            }
        }
        Commands::Fail { feedback, correct } => {
            let fail_path = config.physis.data_dir.join("quality_failures.json");
            let mut tracker = load_tracker(&fail_path, &config);
            let _ = tracker.report_failure(
                &feedback,
                &app.cell_centroids,
                correct.as_deref(),
            );
            if let Err(e) = tracker.save(&fail_path) {
                eprintln!("Warning: could not save quality failures: {e}");
            }
            println!("Failure recorded. Use 'quality' to view summary.");
        }
        Commands::Pass { cell } => {
            let fail_path = config.physis.data_dir.join("quality_failures.json");
            let mut tracker = load_tracker(&fail_path, &config);
            tracker.report_success(&cell);
            if let Err(e) = tracker.save(&fail_path) {
                eprintln!("Warning: could not save quality boosts: {e}");
            }
            println!("Success recorded for cell '{cell}'.");
        }
        Commands::Quality => {
            let fail_path = config.physis.data_dir.join("quality_failures.json");
            let tracker = load_tracker(&fail_path, &config);
            println!("{}", tracker.summary());
        }
        Commands::Ingest => {
            println!("Ingest not yet implemented. Configured dirs: {:?}", config.ingest_dirs);
        }
        Commands::Process => {
            println!("Process monitoring not yet implemented.");
        }
        Commands::Inventory => {
            println!("Inventory tracking not yet implemented.");
        }
        Commands::Order { item, qty } => {
            println!(
                "Order suggestion: {} x {} — requires manual approval (order_mode={})",
                item, qty, config.order_mode
            );
        }
        Commands::Wiki { query } => {
            println!("Wiki search for '{query}' not yet implemented.");
        }
    }

    Ok(())
}

/// Load quality tracker from disk, or create empty if no saved state.
fn load_tracker(path: &Path, config: &PhysisProConfig) -> QualityTracker {
    let embedder: Box<dyn VectorEmbed> =
        Box::new(RandomProjectionEmbedder::new(config.physis.embed_dim));
    let ontology = OntologyLoader::load_all(&config.physis);
    let mut tracker = QualityTracker::new(embedder, ontology);
    if path.exists() {
        let failures = QualityTracker::load(path).unwrap_or_default();
        for f in &failures {
            let penalty = tracker.cell_penalties.entry(f.top_cell.clone()).or_insert(0.0);
            *penalty = (*penalty + f.severity * 0.2).min(1.0);
            if let Some(ref _correct) = f.correct_domain {
                let boost = tracker.cell_boosts.entry(f.top_cell.clone()).or_insert(0.0);
                *boost = (*boost + 0.1).min(1.0);
            }
        }
        tracker.failures = failures;
    }
    tracker
}
