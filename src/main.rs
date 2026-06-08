use std::collections::HashMap;
use std::path::{Path, PathBuf};
use physis_pro::vfs::orchestrator::VfsOrchestrator;
use clap::{Parser, Subcommand};

use physis_pro::cli::PhysisApp;
use physis_pro::config::{OntologyLoader, PhysisProConfig};
use physis_pro::embed::{RandomProjectionEmbedder, VectorEmbed};
use physis_pro::graph::FileThemeGraph;
use physis_pro::ingest;
use physis_pro::models::{cosine_sim, DomainDef, Score};
use physis_pro::quality::QualityTracker;
use physis_pro::wiki;

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
    /// Search the wiki (llmwiki + file-theme graph)
    Wiki {
        query: String,
    },
    /// Browse the file-theme graph interactively
    BrowseGraph {
        /// Path to a specific directory to browse (optional, uses ingest dirs otherwise)
        #[arg(short, long)]
        dir: Option<PathBuf>,
    },
    /// Regenerate the theme graph from ingested files
    RebuildGraph,
    /// Discover new ontology domains from a directory
    Discover {
        /// Directory to discover
        dir: PathBuf,
    },
    /// Export the file-theme graph as uberwiki markdown pages
    WikiExport,
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
                let config = PhysisProConfig::default();
                physis_pro::mcp::start_mcp_server(config.physis).await
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
            run_ingest_pipeline(&config, "Ingested", true)?;
        }
        Commands::Process => {
            #[cfg(feature = "industrial")]
            {
                if config.industrial_endpoints.is_empty() {
                    println!("Process monitoring: no industrial endpoints configured.");
                    println!("  Add MQTT/Modbus endpoints to config.industrial_endpoints.");
                } else {
                    println!("Process monitoring — checking {} endpoint(s):", config.industrial_endpoints.len());
                    let rt = tokio::runtime::Runtime::new()?;
                    rt.block_on(async {
                        for ep in &config.industrial_endpoints {
                            let clean = ep
                                .trim_start_matches("mqtt://")
                                .trim_start_matches("tcp://")
                                .trim_start_matches("modbus://");
                            let host = clean.split(':').next().unwrap_or(clean);
                            let port: u16 = clean.split(':').nth(1).and_then(|p| p.parse().ok()).unwrap_or(1883);
                            let addr = format!("{host}:{port}");
                            match tokio::time::timeout(
                                std::time::Duration::from_secs(3),
                                tokio::net::TcpStream::connect(&addr),
                            )
                            .await
                            {
                                Ok(Ok(_)) => println!("  ✅ {ep} — reachable"),
                                Ok(Err(e)) => println!("  ❌ {ep} — {e}"),
                                Err(_) => println!("  ❌ {ep} — timeout (3s)"),
                            }
                        }
                    });
                }
            }
            #[cfg(not(feature = "industrial"))]
            {
                println!("Process monitoring requires the 'industrial' feature.");
                if !config.industrial_endpoints.is_empty() {
                    println!("  Configured endpoints: {}", config.industrial_endpoints.len());
                    for ep in &config.industrial_endpoints {
                        println!("    - {ep}");
                    }
                }
                println!("  Rebuild with: cargo build --features industrial");
            }
        }
        Commands::Inventory => {
            match sled::open(&config.inventory_db) {
                Ok(db) => {
                    let count = db.iter().count();
                    println!("Inventory database: {count} item(s)");
                    println!("  Path: {}", config.inventory_db.display());
                    println!("  Order mode: {}", config.order_mode);
                    println!("  Order cooldown: {} hours", config.order_cooldown_hours);
                    if count > 0 {
                        println!("\n  Items (first 20):");
                        for result in db.iter().take(20) {
                            match result {
                                Ok((key, value)) => {
                                    let k = String::from_utf8_lossy(&key);
                                    let v = String::from_utf8_lossy(&value);
                                    println!("    {k}: {v}");
                                }
                                Err(e) => eprintln!("    Error reading item: {e}"),
                            }
                        }
                    }
                    println!("\n  Use 'physis-pro order <item> <qty>' to suggest a restock.");
                }
                Err(e) => {
                    println!("Could not open inventory database at {}.", config.inventory_db.display());
                    println!("  Error: {e}");
                    println!("  Run 'physis-pro ingest' to initialize it.");
                }
            }
        }
        Commands::Order { item, qty } => {
            println!(
                "Order suggestion: {} x {} — requires manual approval (order_mode={})",
                item, qty, config.order_mode
            );
        }
        Commands::Wiki { query } => {
            let wiki_path = config.wiki_path.unwrap_or_else(|| config.physis.data_dir.join("wiki"));
            let graph_path = config.physis.data_dir.join("theme_graph.json");
            let results = wiki::search_all(&query, &wiki_path, &graph_path);
            if results.is_empty() {
                println!("No results found for: {query}");
                println!("  Tip: Run 'physis-pro ingest' to build the theme graph, then wiki pages with 'physis-pro wiki-export'.");
            } else {
                println!("=== Results for '{query}' ===\n");
                for r in &results {
                    println!("{r}");
                }
                println!("\n{} results total", results.len());
            }
        }
        Commands::BrowseGraph { dir } => {
            let graph_path = config.physis.data_dir.join("theme_graph.json");
            let graph = match FileThemeGraph::load(&graph_path) {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("No theme graph found at {}. Run 'physis-pro ingest' first.", graph_path.display());
                    eprintln!("Error: {e}");
                    return Ok(());
                }
            };
            browse_graph_interactive(&graph, dir.as_deref())?;
        }
        Commands::RebuildGraph => {
            run_ingest_pipeline(&config, "Rebuilding graph from", false)?;
        }
        Commands::Discover { dir } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                println!("Starting Universal Discovery on {}...", dir.display());
                
                // 1. Scan the project
                let files = physis_pro::scanner::scan_project(&dir, None);
                println!("Found {} files to analyze.", files.len());
                
                // 2. Synthesize using LLM Agent
                println!("Synthesizing ontology proposals via AI...");
                
                use physis_pro::ai::provider::ProviderCascade;
                use physis_pro::ai::agent::{run_agent, AgentConfig};
                use physis_pro::ai::tools::ToolRegistry;

                let cascade = ProviderCascade::from_env();
                let tools = ToolRegistry::new();
                let mut summary = String::new();
                for file in files.iter().take(20) { 
                     summary.push_str(&format!("File: {}\nContent snippet: {}\n\n", file.path, 
                        std::fs::read_to_string(&file.abs_path).unwrap_or_default().chars().take(200).collect::<String>()));
                }

                let config = AgentConfig {
                    system_prompt: r#"You are a Physis Ontology Engineer. 
    Analyze the provided file list and their content snippets. 
    Synthesize a JSON list of proposed ontology domains. 
    Each entry must have: "name", "category", "domain", "mode", "axis_kind", "axis_name", "unit", "hints" (list).
    Output ONLY valid JSON."#.into(),
                    ..Default::default()
                };

                let output = run_agent(&cascade, &tools, &config, &[], &format!("Propose ontology domains based on these files:\n{}", summary), None, "DATA", None).await?;
                
                if let (Some(s), Some(e)) = (output.text.find('['), output.text.rfind(']')) {
                    let json_data = &output.text[s..=e];
                    let proposals: Vec<physis_pro::models::DomainDef> = serde_json::from_str(json_data)?;
                    
                    let json = serde_json::to_string_pretty(&proposals)?;
                    std::fs::write(dir.join("proposed_ontology.json"), json)?;
                    println!("Proposals written to {}/proposed_ontology.json. Review and apply.", dir.display());
                } else {
                    println!("Failed to parse LLM output: {}", output.text);
                }
                anyhow::Ok(())
            })?;
        }
        Commands::WikiExport => {
            let graph_path = config.physis.data_dir.join("theme_graph.json");
            let wiki_dir = config.wiki_path.unwrap_or_else(|| config.physis.data_dir.join("wiki"));
            let graph = match FileThemeGraph::load(&graph_path) {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("No theme graph found at {}. Run 'physis-pro ingest' first.", graph_path.display());
                    eprintln!("Error: {e}");
                    return Ok(());
                }
            };
            graph.export_to_wiki(&wiki_dir)?;
            println!("Wiki pages exported to {}", wiki_dir.display());
            println!("  {} categories, {} themes, {} files", graph.list_categories().len(), graph.themes.len(), graph.files.len());
            println!("  Index: {}/index.md", wiki_dir.display());

            // Also generate an uberwiki index file listing all themes and files
            let uber_path = config.physis.data_dir.join("uberwiki_index.md");
            let mut uber = String::new();
            uber.push_str("# Physis Uberwiki Index\n\n");
            uber.push_str("Generated from the File Theme Graph.\n\n");
            uber.push_str("## Quick Links\n\n");
            uber.push_str("- [[theme_graph_index|File Theme Graph]]\n");
            uber.push_str(&format!("- [[wiki|LLM Wiki]] — {}/index.md\n\n", wiki_dir.display()));
            uber.push_str("## By Theme\n\n");
            for theme in &graph.themes {
                let safe = theme.name.to_lowercase().replace(' ', "_");
                uber.push_str(&format!("- [[{}|{}]]  [{}×{}]\n", safe, theme.name, theme.domain, theme.mode));
            }
            uber.push_str("\n## By File\n\n");
            for file in &graph.files {
                let safe = file.title.to_lowercase().replace(' ', "_");
                uber.push_str(&format!("- [[{}|{}]] — {}\n", safe, file.title, file.path.display()));
            }
            std::fs::write(&uber_path, &uber)?;
            println!("  Uberwiki index: {}", uber_path.display());
        }
    }

    Ok(())
}

fn run_ingest_pipeline(config: &PhysisProConfig, label: &str, sync_vfs: bool) -> anyhow::Result<()> {
    let graph_path = config.physis.data_dir.join("theme_graph.json");
    let watch_dirs: Vec<PathBuf> = if config.ingest_dirs.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        config.ingest_dirs.clone()
    };
    let docs = ingest::ingest_all(&watch_dirs);
    println!("{} {} files from {:?}", label, docs.len(), watch_dirs);

    let ontology = OntologyLoader::load_all(&config.physis);
    let embedder: Box<dyn VectorEmbed> =
        Box::new(RandomProjectionEmbedder::new(config.physis.embed_dim));

    let mut graph = FileThemeGraph::new();
    let all_domains: Vec<(&str, &HashMap<String, DomainDef>)> = vec![
        ("office", &ontology.office_domains),
        ("human", &ontology.human_domains),
        ("machine", &ontology.machine_domains),
        ("agent", &ontology.agent_domains),
        ("engineering", &ontology.engineering_domains),
    ];

    for (_kind, domains) in &all_domains {
        for def in domains.values() {
            let mut text = def.name.clone();
            for hint in &def.hints {
                text.push(' ');
                text.push_str(hint);
            }
            let embedding = embedder.embed(&text);
            graph.add_theme(
                &def.name,
                def.category.as_deref(),
                def.domain.as_deref().unwrap_or(""),
                def.mode.as_deref().unwrap_or(""),
                def.axis_kind.as_deref().unwrap_or(""),
                embedding,
            );
        }
    }

    for doc in &docs {
        let file_embedding = embedder.embed(&doc.body);
        let file_id = graph.add_file(doc.path.clone(), &doc.title, &format!("{:?}", doc.kind), file_embedding.clone());

        let mut theme_scores: Vec<(usize, f32)> = graph.themes.iter()
            .map(|t| {
                let sim = cosine_sim(&file_embedding, &t.embedding);
                (t.id, sim)
            })
            .filter(|(_, sim)| *sim > 0.25)
            .collect();
        theme_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        for (theme_id, score) in theme_scores.iter().take(5) {
            graph.add_edge(*theme_id, file_id, *score);
        }

        let top_themes: Vec<String> = theme_scores.iter().take(3)
            .filter_map(|(tid, _)| graph.get_theme(*tid))
            .map(|t| t.name.clone())
            .collect();
        if sync_vfs {
            println!("  {} → matches: {}", doc.title, top_themes.join(", "));
        }
    }

    if sync_vfs {
        let mut vfs = VfsOrchestrator::new(config.physis.data_dir.clone());
        vfs.sync_graph(&graph)?;
        println!("Virtual File System synchronized in {}", config.physis.data_dir.join("virtual").display());
    }

    graph.save(&graph_path)?;
    println!("\nTheme graph saved to {}", graph_path.display());
    println!("  {} themes, {} files, {} edges", graph.themes.len(), graph.files.len(), graph.edges.len());
    Ok(())
}

/// Interactive graph browser — navigate themes by category, see files per theme.
fn browse_graph_interactive(graph: &FileThemeGraph, _filter_dir: Option<&Path>) -> anyhow::Result<()> {
    use std::io::{self, BufRead, Write};

    let categories = graph.list_categories();

    println!("\n━━━ Physis File Theme Graph ━━━");
    println!("{} themes, {} files, {} edges\n", graph.themes.len(), graph.files.len(), graph.edges.len());

    'outer: loop {
        println!("\n── Categories ──");
        for (i, cat) in categories.iter().enumerate() {
            let count = graph.themes_by_category(cat).len();
            println!("  {}. {} ({} themes)", i + 1, cat, count);
        }
        println!("  s. Search files by name");
        println!("  q. Quit");
        print!("\n> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        let input = input.trim();

        if input == "q" {
            break 'outer;
        }

        if input == "s" {
            print!("Search query: ");
            io::stdout().flush()?;
            let mut q = String::new();
            io::stdin().lock().read_line(&mut q)?;
            let results = graph.search_files(q.trim());
            if results.is_empty() {
                println!("No files match '{}'", q.trim());
            } else {
                println!("\n── Search Results ──");
                for file in results {
                    let themes: Vec<String> = graph.edges.iter()
                        .filter(|e| e.file_id == file.id)
                        .filter_map(|e| graph.get_theme(e.theme_id))
                        .map(|t| t.name.clone())
                        .collect();
                    println!("  {} ({})", file.title, file.path.display());
                    if !themes.is_empty() {
                        println!("    themes: {}", themes.join(", "));
                    }
                }
            }
            continue;
        }

        if let Ok(idx) = input.parse::<usize>() {
            if idx == 0 || idx > categories.len() {
                println!("Invalid choice");
                continue;
            }
            let category = &categories[idx - 1];
            let themes = graph.themes_by_category(category);

            'category: loop {
                println!("\n── {} ──", category);
                for (i, theme) in themes.iter().enumerate() {
                    let file_count = graph.edges.iter().filter(|e| e.theme_id == theme.id).count();
                    let domain_mode = format!("{}×{}", theme.domain, theme.mode);
                    println!("  {}. {}  [{}]  ({} files)", i + 1, theme.name, domain_mode, file_count);
                }
                println!("  b. Back to categories");
                println!("  q. Quit");
                print!("\n> ");
                io::stdout().flush()?;

                let mut input2 = String::new();
                io::stdin().lock().read_line(&mut input2)?;
                let input2 = input2.trim();

                if input2 == "q" {
                    break 'outer;
                }
                if input2 == "b" {
                    break 'category;
                }

                if let Ok(idx2) = input2.parse::<usize>() {
                    if idx2 == 0 || idx2 > themes.len() {
                        println!("Invalid choice");
                        continue;
                    }
                    let theme = themes[idx2 - 1];

                    'theme: loop {
                        println!("\n── {} ──", theme.name);
                        println!("  Domain: {} × {}", theme.domain, theme.mode);
                        println!("  Axis: {} ({})", theme.axis_kind, theme.category.as_deref().unwrap_or(""));

                        // Show files
                        let files = graph.files_for_theme(theme.id);
                        if files.is_empty() {
                            println!("  No files linked to this theme.");
                        } else {
                            println!("\n  Files:");
                            for (fi, file) in files.iter().enumerate() {
                                println!("    {}. {}  ({})", fi + 1, file.title, file.path.display());
                            }
                        }

                        // Show related themes
                        let related = graph.find_related_themes(theme.id, 0.5);
                        if !related.is_empty() {
                            println!("\n  Related themes:");
                            for (ri, (rid, sim)) in related.iter().enumerate().take(8) {
                                if let Some(rt) = graph.get_theme(*rid) {
                                    let pct = (sim * 100.0) as u32;
                                    println!("    {}. {}  ({}% similar)", ri + 1, rt.name, pct);
                                }
                            }
                        }

                        println!("\n  r. Choose related theme");
                        println!("  b. Back to category");
                        println!("  q. Quit");
                        print!("\n> ");
                        io::stdout().flush()?;

                        let mut input3 = String::new();
                        io::stdin().lock().read_line(&mut input3)?;
                        let input3 = input3.trim();

                        if input3 == "q" {
                            break 'outer;
                        }
                        if input3 == "b" {
                            break 'theme;
                        }
                        if input3 == "r" && !related.is_empty() {
                            print!("  Related theme # (1-{}): ", related.len().min(8));
                            io::stdout().flush()?;
                            let mut r_input = String::new();
                            io::stdin().lock().read_line(&mut r_input)?;
                            if let Ok(ri) = r_input.trim().parse::<usize>() {
                                if ri > 0 && ri <= related.len().min(8) {
                                    let (rid, _) = related[ri - 1];
                                    // Navigate to this theme's detail
                                    // Re-enter the theme loop for this theme
                                    // We'll locate the theme in the themes list
                                    if let Some(r_theme) = graph.get_theme(rid) {
                                        // Create a single-element view for the related theme
                                        println!("\n── {} (related) ──", r_theme.name);
                                        println!("  Domain: {} × {}", r_theme.domain, r_theme.mode);
                                        let r_files = graph.files_for_theme(rid);
                                        if r_files.is_empty() {
                                            println!("  No files linked to this theme.");
                                        } else {
                                            println!("\n  Files:");
                                            for (fi, rf) in r_files.iter().enumerate() {
                                                println!("    {}. {}  ({})", fi + 1, rf.title, rf.path.display());
                                            }
                                        }
                                        // Just show once and return to related theme menu
                                        println!("\n  Press Enter to go back...");
                                        let mut _pause = String::new();
                                        io::stdin().lock().read_line(&mut _pause)?;
                                    }
                                }
                            }
                        }
                    }
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use physis_pro::graph::FileThemeGraph;

    #[test]
    fn test_browse_graph_empty() {
        let graph = FileThemeGraph::new();
        let cats = graph.list_categories();
        assert!(cats.is_empty());
        assert_eq!(graph.themes.len(), 0);
    }

    #[test]
    fn test_browse_graph_with_themes() {
        let mut graph = FileThemeGraph::new();
        graph.add_theme("Email", Some("Communication"), "BOND", "WORK", "office", vec![0.9, 0.1]);
        graph.add_theme("Meeting", Some("Communication"), "BOND", "CREATE", "office", vec![0.8, 0.2]);
        graph.add_theme("Budget", Some("Finance"), "CONSTRUCT", "CREATE", "office", vec![0.1, 0.9]);
        graph.add_file(Path::new("msg.eml").to_path_buf(), "Q4 report", "Email", vec![0.85, 0.15]);
        graph.add_edge(0, 0, 0.95);

        let cats = graph.list_categories();
        assert_eq!(cats.len(), 2);
        assert!(cats.contains(&"Communication".to_string()));

        let comm_themes = graph.themes_by_category("Communication");
        assert_eq!(comm_themes.len(), 2);

        let files = graph.files_for_theme(0);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].title, "Q4 report");
    }

    #[test]
    fn test_graph_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_graph.json");

        let mut graph = FileThemeGraph::new();
        graph.add_theme("Test", None, "STUDY", "WORK", "human", vec![0.5; 4]);
        graph.save(&path).unwrap();

        let loaded = FileThemeGraph::load(&path).unwrap();
        assert_eq!(loaded.themes.len(), 1);
        assert_eq!(loaded.themes[0].name, "Test");
    }

    #[test]
    fn test_search_files() {
        let mut graph = FileThemeGraph::new();
        graph.add_file(Path::new("invoices/2024-01.pdf").to_path_buf(), "Invoice Jan", "Invoice", vec![0.5; 4]);
        graph.add_file(Path::new("reports/q4.md").to_path_buf(), "Q4 Report", "Report", vec![0.5; 4]);

        let results = graph.search_files("invoice");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Invoice Jan");
    }

    #[test]
    fn test_related_themes_similarity() {
        let mut graph = FileThemeGraph::new();
        graph.add_theme("A", None, "STUDY", "WORK", "human", vec![1.0, 0.0, 0.0]);
        graph.add_theme("B", None, "STUDY", "WORK", "human", vec![0.95, 0.1, 0.0]);
        graph.add_theme("C", None, "BOND", "WALK", "human", vec![0.0, 0.0, 1.0]);

        let related = graph.find_related_themes(0, 0.5);
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].0, 1);
    }
}
