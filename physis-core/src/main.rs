//! physis-core CLI.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use physis_core::classify::{CellClassifier, CellScore};
use physis_core::core::PhysisCore;
use physis_core::embed::{RandomProjectionEmbedder, VectorEmbed};
use physis_core::ontology::OntologyLoader;
use physis_core::quality::QualityTracker;
use physis_core::store;

/// physis-core — embed, classify, cohere, learn from feedback.
#[derive(Parser)]
#[command(name = "physis-core", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Classify text against the semiotic grid.
    Classify { text: String },
    /// Show ontology stats.
    Ontology,
    /// Scan a directory, registering each text file as a coherence node.
    Scan {
        /// Directory to scan recursively.
        dir: PathBuf,
    },
    /// Search recalled coherence nodes by similarity to a query.
    Search {
        /// Query text.
        query: String,
        /// Max results.
        #[arg(long, default_value_t = 10)]
        max: usize,
    },
    /// Show a coherence snapshot of the persisted node graph.
    Snapshot,
    /// Report an asserted verdict on a node by label.
    Assert {
        /// Node label (exact).
        label: String,
        /// Verdict: success|inert|failure.
        verdict: String,
    },
    /// Dream over low-coherence / failed nodes.
    Dream,
    /// Quality feedback loop.
    Quality {
        #[command(subcommand)]
        cmd: QualityCmd,
    },
    /// Run the ontology studio web GUI.
    #[cfg(feature = "studio")]
    Studio {
        /// Port to listen on.
        #[arg(long, default_value_t = 3000)]
        port: u16,
        /// ONNX model directory (model.onnx + tokenizer.json). Optional — falls
        /// back to deterministic random projection when absent or unloadable.
        #[arg(long)]
        model: Option<String>,
    },
}

#[derive(Subcommand)]
enum QualityCmd {
    /// Show quality tracker summary.
    Summary,
    /// Report a failure with optional correct domain.
    Fail { feedback: String },
    /// Report a success for a cell (DOMAIN\x00MODE).
    Pass { cell: String },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    store::ensure_data_dir()?;
    match cli.command {
        Command::Classify { text } => run_classify(&text),
        Command::Ontology => run_ontology(),
        Command::Scan { dir } => run_scan(&dir),
        Command::Search { query, max } => run_search(&query, max),
        Command::Snapshot => run_snapshot(),
        Command::Assert { label, verdict } => run_assert(&label, &verdict),
        Command::Dream => run_dream(),
        Command::Quality { cmd } => run_quality(cmd),
        #[cfg(feature = "studio")]
        Command::Studio { port, model } => run_studio(port, model),
    }
}

fn load_embedder() -> RandomProjectionEmbedder {
    RandomProjectionEmbedder::new(384)
}

fn load_quality() -> QualityTracker {
    QualityTracker::load_or_new(&store::quality_path())
}

fn run_classify(text: &str) -> anyhow::Result<()> {
    let ontology = OntologyLoader::load_all();
    let embedder = load_embedder();
    let classifier = CellClassifier::build(&ontology, &embedder);
    let quality = load_quality();

    let results = classifier.classify_text(text, &embedder);
    let adjusted: Vec<CellScore> = results
        .iter()
        .map(|r| {
            let key = format!("{}\x00{}", r.domain, r.mode);
            let mut r2 = r.clone();
            r2.score = quality.adjust_score(&key, r.score);
            r2
        })
        .collect();

    println!("Query: {text}");
    println!("Cells populated: {}", classifier.cell_count());
    for r in adjusted.iter().take(8) {
        let bar = (r.score * 20.0).round() as usize;
        println!(
            "  {:<11} × {:<10} {:5.3} {}{}",
            r.domain,
            r.mode,
            r.score,
            "█".repeat(bar),
            "░".repeat(20 - bar)
        );
    }
    if let Some((sim, domain, mode)) = classifier.best_entry_sim(&embedder.embed(text)) {
        println!("Best entry cosine: {sim:.3} ({domain}×{mode})");
    }
    Ok(())
}

fn run_ontology() -> anyhow::Result<()> {
    let ontology = OntologyLoader::load_all();
    println!("Ontology: {} classification entries", ontology.entry_count());
    println!("  human (grid) domains: {}", ontology.human_domains.len());
    println!("  custom (extra) domains: {}", ontology.custom_domains.len());
    println!("Categories:");
    for cat in ontology.categories() {
        let n = ontology
            .all_domains()
            .values()
            .filter(|d| d.category.as_deref() == Some(cat.as_str()))
            .count();
        println!("  {cat}: {n}");
    }
    Ok(())
}

fn run_scan(dir: &std::path::Path) -> anyhow::Result<()> {
    let embedder = load_embedder();
    let mut core = load_core();
    let mut count = 0usize;
    for entry in walk(dir)? {
        let Ok(text) = std::fs::read_to_string(&entry) else { continue };
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed.chars().count() > 5000 {
            continue;
        }
        core.register_node_from_text(&format!("{}: {}", entry.display(), trimmed), &embedder);
        count += 1;
    }
    core.persist()?;
    let snap = core.snapshot();
    println!("Registered {count} files. Nodes: {}", snap.total_nodes);
    println!("Coherence index: {:.3}", snap.coherence_index);
    Ok(())
}

fn run_search(query: &str, max: usize) -> anyhow::Result<()> {
    let embedder = load_embedder();
    let core = load_core();
    let hits = core.search_text(query, &embedder, max);
    if hits.is_empty() {
        println!("No recalled nodes. Try `physis-core scan <dir>` first.");
        return Ok(());
    }
    println!("Top {max} matches for: {query}");
    for (i, (_, label, score)) in hits.iter().enumerate() {
        println!("  {}. {:.3}  {}", i + 1, score, label.chars().take(100).collect::<String>());
    }
    Ok(())
}

fn run_snapshot() -> anyhow::Result<()> {
    let core = load_core();
    let snap = core.snapshot();
    println!("Coherence snapshot:");
    println!("  nodes:            {}", snap.total_nodes);
    println!("  coherence index:  {:.3}", snap.coherence_index);
    println!("  high/mid/low:     {}/{}/{}", snap.high_coherence, snap.mid_coherence, snap.low_coherence);
    println!("  certified branches: {}", snap.certified_branches_count);
    println!("  isolated branches:  {}", snap.isolated_branches_count);
    println!("  asserted s/i/f:   {}/{}/{}", snap.asserted_success, snap.asserted_inert, snap.asserted_failure);
    println!("  asserted index:   {:?}", snap.asserted_index);
    Ok(())
}

fn run_assert(label: &str, verdict: &str) -> anyhow::Result<()> {
    let mut core = load_core();
    let id = core
        .nodes
        .values()
        .find(|n| n.label.as_deref() == Some(label))
        .map(|n| n.id.clone())
        .ok_or_else(|| anyhow::anyhow!("no node with label '{label}'"))?;
    let score = match verdict.to_lowercase().as_str() {
        "success" => 1.0,
        "inert" => 0.0,
        "failure" => -1.0,
        other => anyhow::bail!("verdict must be success|inert|failure, got '{other}'"),
    };
    core.assert_coherence(&id, score);
    core.persist()?;
    println!("Asserted '{label}' as {verdict}");
    Ok(())
}

fn run_dream() -> anyhow::Result<()> {
    let mut core = load_core();
    let results = core.dream();
    if results.is_empty() {
        println!("No nodes need replaying right now.");
        return Ok(());
    }
    for d in &results {
        println!(
            "Dream {} tested {} node(s): outcome {:.1} prevented_failure={}",
            &d.dream_id[..8],
            d.nodes_tested.len(),
            d.outcome,
            d.prevented_failure,
        );
    }
    core.persist()?;
    Ok(())
}

fn run_quality(cmd: QualityCmd) -> anyhow::Result<()> {
    let mut quality = load_quality();
    match cmd {
        QualityCmd::Summary => {
            println!("Quality tracker: {} failures", quality.failures.len());
            println!("  cells penalized: {}", quality.cell_penalties.len());
            println!("  cells boosted:   {}", quality.cell_boosts.len());
            let mut penalties: Vec<_> = quality.cell_penalties.iter().collect();
            penalties.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
            for (cell, p) in penalties.iter().take(10) {
                println!("  {} penalty={:.2}", cell.replace('\x00', " × "), p);
            }
            for f in quality.failures.iter().rev().take(5) {
                println!(
                    "  [{}] {} score={:.3} sev={:.1} feedback={}",
                    &f.id[..8],
                    f.top_cell.replace('\x00', " × "),
                    f.top_score,
                    f.severity,
                    f.feedback.chars().take(50).collect::<String>(),
                );
            }
        }
        QualityCmd::Fail { feedback } => {
            let ontology = OntologyLoader::load_all();
            let embedder = load_embedder();
            let classifier = CellClassifier::build(&ontology, &embedder);
            let centroids = classifier.cell_centroids();
            let f = quality.report_failure(&feedback, &centroids, None);
            println!(
                "Recorded failure → {} (score {:.3})",
                f.top_cell.replace('\x00', " × "),
                f.top_score
            );
            quality.save(&store::quality_path())?;
        }
        QualityCmd::Pass { cell } => {
            quality.report_success(&cell);
            quality.save(&store::quality_path())?;
            println!("Boosted cell '{}'", cell.replace('\x00', " × "));
        }
    }
    Ok(())
}

#[cfg(feature = "studio")]
fn run_studio(port: u16, model: Option<String>) -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(physis_core::studio::run_with_model(port, model))
}

/// Load the persisted core (or a fresh one).
fn load_core() -> PhysisCore {
    let json = store::read_optional(&store::nodes_path());
    if json.trim().is_empty() {
        PhysisCore::new()
    } else {
        PhysisCore::from_json(&json).unwrap_or_else(|_| PhysisCore::new())
    }
}

fn walk(dir: &std::path::Path) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if is_text(&path) {
                out.push(path);
            }
        }
    }
    Ok(out)
}

fn is_text(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("txt" | "md" | "rs" | "py" | "json" | "toml" | "csv" | "log" | "xml" | "yml" | "yaml")
    )
}

trait PersistCore {
    fn persist(&self) -> anyhow::Result<()>;
}

impl PersistCore for PhysisCore {
    fn persist(&self) -> anyhow::Result<()> {
        let json = self.to_json()?;
        std::fs::write(store::nodes_path(), json)?;
        Ok(())
    }
}
