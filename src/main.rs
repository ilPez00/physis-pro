use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
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
    /// Start MCP server. Use --stdio for Claude Code, Codex, OpenCode and any
    /// other client that spawns the server as a child process; the default TCP
    /// listener exists for the hosted deployment and no mainstream client can
    /// attach to it.
    #[cfg(feature = "mcp")]
    Serve {
        /// Serve MCP over stdin/stdout instead of binding a TCP port.
        #[arg(long)]
        stdio: bool,
    },
    /// Watch directories for changes; register new/changed files as labeled
    /// core nodes (auto-ingest), drop nodes for removed files
    Watch {
        /// Directories to watch
        dirs: Vec<PathBuf>,
        /// Single sweep then exit (cron / smoke test) instead of looping
        #[arg(long)]
        once: bool,
    },
    /// Pack context for a query from a directory into a hard token budget,
    /// reporting the path and line range of every chunk selected. The shell
    /// equivalent of the `physis_pack` MCP tool — use it to benchmark against
    /// a baseline retriever.
    Pack {
        /// Directory to pack from
        dir: PathBuf,
        /// What the context is needed for
        query: String,
        /// Hard token ceiling — never exceeded
        #[arg(long, default_value_t = 4000)]
        budget: usize,
        /// Maximum chunks to consider
        #[arg(long, default_value_t = 50)]
        top_k: usize,
        /// 0 = pure relevance, 1 = maximum spread
        #[arg(long, default_value_t = 0.3)]
        diversity: f32,
        /// Print the assembled context after the summary
        #[arg(long)]
        show_context: bool,
    },
    /// Pre-compute the embeddings the engine needs at startup, so the first
    /// real run is fast. Intended to be run once by the installer: without it
    /// the first launch spends ~78s embedding ontology seeds, which is longer
    /// than an MCP client will wait for a handshake.
    Warm {
        /// Also pre-embed this directory's chunks, so the first `pack` of it
        /// is warm too. Omit to warm only the ontology seeds.
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Show stats
    Stats,
    /// Query the ontology by orthogonal facets (lifecycle/agency/scale/
    /// abstraction/sub-domain/sub-mode), beyond the DOMAIN×MODE grid
    Facet {
        /// Restrict to one ontology kind (office, human, machine, agent, …)
        #[arg(long)]
        kind: Option<String>,
        /// Restrict to a domain (case-insensitive)
        #[arg(long)]
        domain: Option<String>,
        /// Restrict to a mode (case-insensitive)
        #[arg(long)]
        mode: Option<String>,
        /// Lifecycle facet: DESIGN | BUILD | OPERATE | RETIRE
        #[arg(long)]
        lifecycle: Option<String>,
        /// Agency facet: SELF | OTHER | AUTOMATED | COLLECTIVE
        #[arg(long)]
        agency: Option<String>,
        /// Scale facet: PERSONAL | INTERPERSONAL | ORGANIZATIONAL | CIVIL
        #[arg(long)]
        scale: Option<String>,
        /// Abstraction facet: CONCRETE | ABSTRACT
        #[arg(long)]
        abstraction: Option<String>,
        /// Sub-domain facet (string, case-insensitive)
        #[arg(long)]
        sub_domain: Option<String>,
        /// Sub-mode facet (string, case-insensitive)
        #[arg(long)]
        sub_mode: Option<String>,
    },
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
    /// Render a timeline/incident view from normalized operational events
    Timeline {
        /// JSON file containing an array of OperationalEvent records
        input: PathBuf,
        /// Emit JSON instead of markdown
        #[arg(long)]
        json: bool,
    },
    /// Run diagnostics: config, data dir, scan root, embedder, license
    Doctor,
    /// Edit a coherence node's content ("exchange parts"): new label is
    /// re-embedded, structure (id / verdict / edges) preserved. Optional cell
    /// pin overrides the auto-derived semiotic cell.
    NodeEdit {
        /// Node id
        id: String,
        /// New content (label)
        label: String,
        /// Pin domain (e.g. FABRICATE). Requires --mode.
        #[arg(long)]
        domain: Option<String>,
        /// Pin mode (e.g. MAINTAIN). Requires --domain.
        #[arg(long)]
        mode: Option<String>,
    },
    /// Delete a coherence node and every reference to it.
    NodeDelete {
        /// Node id
        id: String,
    },
    /// Fixed-token search over the persisted coherence + goal nodes.
    NodeSearch {
        /// Search query
        query: String,
        /// Hard token budget for the packed result labels
        #[arg(long, default_value_t = 600)]
        budget: usize,
        /// Max results
        #[arg(long, default_value_t = 12)]
        top: usize,
    },
    /// Import a knowledge vault (Obsidian/markdown notes, plain text, git log)
    /// into the shared store as labeled core nodes
    Vault {
        /// Vault or notes directory
        dir: PathBuf,
        /// Also import the last N commits of the enclosing git repo (0 = skip)
        #[arg(long, default_value_t = 50)]
        git: usize,
    },
    /// Import a personal-history export (browser bookmarks/history, OPML feed
    /// list, chat JSONL) into the shared store as labeled core nodes
    History {
        /// Path to the export file (.html/.htm, .json, .opml/.xml, .jsonl/.txt)
        file: PathBuf,
    },
    /// Backfill coherence from a Praxis life-log export (notebook entries +
    /// trackers): register labeled behavioral nodes and assert success/inert/
    /// failure verdicts
    Praxis {
        /// Path to the Praxis export JSON (notebook + trackers, or a bare array)
        file: PathBuf,
    },
    /// Mirror the coherence graph to Spanner (durable memory), or pull durable
    /// rows back and re-embed them locally (new device / model swap). Needs the
    /// `spanner` feature and Spanner env (SPANNER_PROJECT/INSTANCE/DATABASE +
    /// GOOGLE_APPLICATION_CREDENTIALS or GOOGLE_OAUTH_TOKEN).
    #[cfg(feature = "spanner")]
    Spanner {
        #[command(subcommand)]
        action: SpannerAction,
    },
    /// Run a live connector, emitting normalized events as JSON lines
    Connect {
        #[command(subcommand)]
        source: ConnectSource,
    },

    // ── Business backoffice commands ──
    /// Render the Italian shift report from a ShiftReportInput JSON file
    Report {
        /// Path to a JSON file with oee[] (+ optional anomalies/rul/countermeasures)
        #[arg(long)]
        shift: PathBuf,
        /// Emit a self-contained HTML document (email-able / print-to-PDF) instead of Markdown
        #[arg(long)]
        html: bool,
    },
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
    /// Ontology Studio — ingest a directory, map docs onto the live grid, surface
    /// the gaps the current ontology can't name, and emit one self-contained
    /// HTML a prospect can open. The "drop in your docs → see your ontology" wedge.
    Studio {
        /// Directory of documents (.txt/.md/.csv/.log/.xlsx/.eml) to ingest
        dir: PathBuf,
        /// Where to write the studio HTML (default <dir>/studio.html)
        #[arg(long)]
        html_out: Option<PathBuf>,
        /// Seed the directory with a bundled sample corpus first (one-command demo
        /// — no file gathering needed). Creates <dir> if missing.
        #[arg(long)]
        demo: bool,
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
    /// Discover ontology gaps in a corpus and draft new entries
    Discover {
        /// Directory of .txt/.md/.log/.csv files (one corpus text per line)
        dir: PathBuf,
        /// Write the ready-to-edit ontology JSON draft here
        #[arg(long)]
        json_out: Option<PathBuf>,
        /// Coverage threshold: texts with best-entry cosine below this are
        /// "unmapped" (default 0.55). Run once, read the printed sim
        /// distribution, then re-run with a tuned value.
        #[arg(long)]
        threshold: Option<f32>,
        /// Use the LLM agent to synthesize proposals (legacy "Universal
        /// Discovery") instead of the measured embedding-gap pipeline
        #[arg(long)]
        ai: bool,
    },
    /// Export the file-theme graph as uberwiki markdown pages
    WikiExport,
    /// Run a philosophical machine reducer over JSON input — no server, no
    /// embedder, no disk state. Reads --input FILE or stdin; prints JSON.
    /// e.g. `physis-pro machine chronos --input events.json`
    Machine {
        /// Machine name (chronos, topos, harmony, polis, soma, elenchus, lex,
        /// empedocles, euclid, techne, erosp, erosa, plutos, krisis, pathos,
        /// chroma, kinesis, apodeixis).
        name: String,
        /// JSON input file; omit or use '-' to read from stdin.
        #[arg(short, long)]
        input: Option<PathBuf>,
    },
    /// Compare two data structures (JSON/struct text) node-by-node by vector
    /// similarity. Prints matched fields, what A is missing in B, and overall
    /// structural similarity. e.g. `physis-pro structsim a.json b.json`
    StructSim {
        /// First structure (JSON file, or indentation-structured text).
        a: PathBuf,
        /// Second structure to compare against.
        b: PathBuf,
        /// Minimum cosine for a field pair to count as a match (default 0.5).
        #[arg(long, default_value_t = 0.5)]
        threshold: f32,
    },
    /// Rank structures in a directory by similarity to a query structure.
    StructSearch {
        /// Directory of JSON / struct-text files to search.
        dir: PathBuf,
        /// Query structure to match against.
        query: PathBuf,
        /// Minimum cosine for a field pair to count as a match (default 0.5).
        #[arg(long, default_value_t = 0.5)]
        threshold: f32,
        /// Return at most this many ranked results (default 10).
        #[arg(long, default_value_t = 10)]
        max: usize,
    },
    /// Token-fixed RAG: retrieve the chunks of a directory whose packed token
    /// total stays within a fixed budget. Deterministic, overflow-proof context.
    Rag {
        /// Directory of documents to retrieve from (.txt/.md/.csv/.log/.json...).
        dir: PathBuf,
        /// The retrieval query.
        query: String,
        /// Hard token budget for the assembled context (default 1500).
        #[arg(long, default_value_t = 1500)]
        tokens: usize,
        /// Maximum number of chunks to select (default 8).
        #[arg(long, default_value_t = 8)]
        top_k: usize,
    },
    /// Record a study session: watch a directory and log every file you
    /// generate, edit or delete — with date, time, place, file type and order —
    /// into the running engine's Study Log (POST /api/v1/study/log). The
    /// learning/teaching app renders it on /study.
    Study {        /// Directory to watch (default: current directory)
        dir: PathBuf,
        /// Engine base URL
        #[arg(long, default_value = "http://127.0.0.1:19876")]
        url: String,
        /// Where you're studying (room, library, lab) — context stamped on every event
        #[arg(long, default_value = "studio")]
        place: String,
        /// X-Physis-User id (default "_default")
        #[arg(long, default_value = "_default")]
        user: String,
        /// Poll interval in seconds
        #[arg(long, default_value_t = 3)]
        interval: u64,
        /// Record the current snapshot as "opened" and exit (no loop)
        #[arg(long)]
        once: bool,
    },
}

/// Sources a `connect` run can poll.
#[derive(Subcommand)]
pub enum ConnectSource {
    /// Klipper printers via Moonraker's JSON-RPC API
    Moonraker {
        /// Moonraker base URL, e.g. http://printer.local:7125
        #[arg(long)]
        url: String,
        /// Machine id recorded on every event
        #[arg(long, default_value = "printer-01")]
        machine_id: String,
        /// Seconds between polls
        #[arg(long, default_value_t = 10)]
        interval: u64,
        /// Poll once and exit (cron / smoke test)
        #[arg(long)]
        once: bool,
        /// Also register events into this physis engine
        #[arg(long)]
        post_to: Option<String>,
        /// Tenant for those registrations
        #[arg(long, default_value = "_default")]
        user: String,
        /// Remember the last reported state here, so a restart does not
        /// re-report what was already reported
        #[arg(long)]
        state_file: Option<PathBuf>,
    },
}

/// Actions for the `spanner` command (Phase 7C durable-memory mirror).
#[cfg(feature = "spanner")]
#[derive(Subcommand)]
pub enum SpannerAction {
    /// Mirror the tenant's coherence graph up (upsert nodes + edges; with
    /// --mirror also hard-delete remote rows that vanished locally)
    Sync {
        /// Hard-delete remote rows absent locally (default true)
        #[arg(long, default_value_t = true)]
        mirror: bool,
    },
    /// Pull durable rows back and re-embed them under the live embedder
    Hydrate,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Licence gate before any work. Placed after `Cli::parse()` so `--help` and
    // `--version` still answer on an unlicensed host: an evaluator has to be
    // able to see what the tool is before we demand a key, and support needs
    // `--version` to triage an install that refuses to run.
    // `doctor` bypasses the gate so it can report licence issues.
    if !matches!(cli.command, Commands::Doctor) {
        let _license = physis_pro::license::enforce();
    }

    // The `machine` subcommand is pure: dispatch it before building the heavy
    // PhysisApp (ontology + embedder), so it runs with no disk state — ideal for
    // a faulty mount or a quick offline reduction.
    if let Commands::Machine { name, input } = &cli.command {
        let raw = match input.as_deref() {
            Some(p) if p != Path::new("-") => std::fs::read_to_string(p)?,
            _ => {
                use std::io::Read;
                let mut s = String::new();
                std::io::stdin().read_to_string(&mut s)?;
                s
            }
        };
        let value: serde_json::Value =
            serde_json::from_str(&raw).map_err(|e| anyhow::anyhow!("invalid JSON input: {e}"))?;
        let out = physis_pro::machines::run::run(name, &value)?;
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    // The `study` recorder only needs the filesystem + the running engine: run
    // it before building the heavy PhysisApp (ontology + embedder).
    if let Commands::Study { dir, url, place, user, interval, once } = &cli.command {
        return run_study_recorder(dir, url, place, user, *interval, *once);
    }

    // `pack` needs an embedder and nothing else. Dispatching it before
    // `PhysisApp::new` skips the startup ontology pass, which embeds ~650
    // entries and dominated the runtime: packing 35 chunks took 6m52s with the
    // pass and seconds without it. Any command that does not read the ontology
    // belongs here rather than below.
    if let Commands::Pack { dir, query, budget, top_k, diversity, show_context } = &cli.command {
        let config = PhysisProConfig::default();
        let root = physis_pro::pack::resolve_root(dir, None)?;
        let (embedder, kind) = physis_pro::embed::select_embedder(&config.physis);
        eprintln!("[physis] pack embedder: {kind}");

        // Cache persists across runs: the first pack of a tree pays for the
        // whole corpus, later packs pay only for what changed.
        let mut cache = physis_pro::pack::EmbedCache::load(&config.physis.data_dir);
        let result = physis_pro::pack::pack_cached(
            &root,
            query,
            *budget,
            *top_k,
            *diversity,
            embedder.as_ref(),
            kind,
            &mut cache,
        );
        cache.save(&config.physis.data_dir);
        let (hits, misses) = cache.stats();
        eprintln!("[physis] embed cache: {hits} hits, {misses} misses");
        print!("{}", physis_pro::pack::summarize(&result));
        if *show_context {
            println!("\n--- context ---\n{}", result.context);
        }
        return Ok(());
    }

    // `warm` populates the same on-disk cache the engine reads at startup, so
    // it must not pay the startup cost it exists to remove — it is dispatched
    // early for the same reason `pack` is.
    if let Commands::Warm { dir } = &cli.command {
        let config = PhysisProConfig::default();
        let (embedder, kind) = physis_pro::embed::select_embedder(&config.physis);
        eprintln!("[physis] warm embedder: {kind}");

        let mut cache = physis_pro::pack::EmbedCache::load(&config.physis.data_dir);
        let ontology = physis_pro::config::OntologyLoader::load_all(&config.physis);
        let started = std::time::Instant::now();
        let classifier = physis_pro::classify::CellClassifier::build_cached(
            &ontology,
            embedder.as_ref(),
            kind,
            &mut cache,
        );
        let (hits, misses) = cache.stats();
        println!(
            "ontology: {} cells, {hits} cached, {misses} computed in {:.1}s",
            classifier.cell_count(),
            started.elapsed().as_secs_f32()
        );

        if let Some(dir) = dir {
            let root = physis_pro::pack::resolve_root(dir, None)?;
            let started = std::time::Instant::now();
            // A pack over the tree is what populates the chunk embeddings; the
            // result is discarded, the cache it fills is the point.
            let result = physis_pro::pack::pack_cached(
                &root, "warm", 1, 1, 0.0, embedder.as_ref(), kind, &mut cache,
            );
            let (hits, misses) = cache.stats();
            println!(
                "{}: {} files, {} chunks, {hits} cached, {misses} computed in {:.1}s",
                root.display(),
                result.files_scanned,
                result.candidates,
                started.elapsed().as_secs_f32()
            );
        }

        cache.save(&config.physis.data_dir);
        println!("cache written to {}", config.physis.data_dir.display());
        return Ok(());
    }

    // Per-command entitlement. Kept as a single table rather than scattered
    // checks so that adding a paid capability means adding one line here, and
    // so the mapping between commands and SKUs stays auditable at a glance.
    let required_feature = match &cli.command {
        #[cfg(feature = "mcp")]
        Commands::Serve { .. } => Some("mcp"),
        Commands::Ingest => Some("ingest"),
        _ => None,
    };
    if let Some(feature) = required_feature {
        if !matches!(cli.command, Commands::Doctor) {
            let license = physis_pro::license::enforce();
            if let Err(e) = license.require(feature) {
                eprintln!("physis: {e}");
                std::process::exit(2);
            }
        }
    }

    // `serve` builds its own engine state inside the MCP server and never
    // touches `app`, so it is dispatched here — after the entitlement check,
    // before `PhysisApp::new`. Leaving it below cost ~68s of unused startup on
    // every launch, which is past the handshake timeout of every MCP client:
    // the server was being killed before it could answer `initialize`.
    #[cfg(feature = "mcp")]
    if let Commands::Serve { stdio } = cli.command {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let config = PhysisProConfig::default();
            if stdio {
                physis_pro::mcp::serve_stdio(config.physis).await
            } else {
                physis_pro::mcp::start_mcp_server(config.physis).await
            }
        })?;
        return Ok(());
    }

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
        #[cfg(feature = "mcp")]
        Commands::Serve { .. } => unreachable!("serve is dispatched early"),
        Commands::Watch { dirs, once } => {
            let output = app.run_watch(dirs, once)?;
            println!("{output}");
        }
        Commands::Stats => {
            let output = app.run_stats();
            println!("{output}");
        }
        Commands::Facet { kind, domain, mode, lifecycle, agency, scale, abstraction, sub_domain, sub_mode } => {
            let out = app.run_facet_query(
                kind.as_deref(), domain.as_deref(), mode.as_deref(),
                lifecycle.as_deref(), agency.as_deref(), scale.as_deref(),
                abstraction.as_deref(), sub_domain.as_deref(), sub_mode.as_deref(),
            );
            print!("{out}");
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
        Commands::Connect { source } => match source {
            ConnectSource::Moonraker { url, machine_id, interval, once, post_to, user, state_file } => {
                let opts = physis_pro::moonraker_connect::ConnectOptions {
                    url,
                    machine_id,
                    interval_secs: interval,
                    once,
                    post_to,
                    user,
                    token: std::env::var("PHYSIS_API_TOKEN").ok().filter(|t| !t.is_empty()),
                    state_file,
                };
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(physis_pro::moonraker_connect::run(opts))?;
            }
        },
        Commands::Timeline { input, json } => {
            let raw = std::fs::read_to_string(&input)?;
            let events: Vec<physis_pro::operational::OperationalEvent> =
                serde_json::from_str(&raw)?;
            let timeline = physis_pro::operational_report::OperationalTimeline::from_events(&events);
            if json {
                println!("{}", serde_json::to_string_pretty(&timeline)?);
            } else {
                println!("{}", timeline.render_markdown());
            }
        }
        // ── Semiotic node editing ("exchange parts") ──
        Commands::NodeEdit { id, label, domain, mode } => {
            let pin = match (domain, mode) {
                (Some(d), Some(m)) => Some((d, m)),
                (None, None) => None,
                _ => anyhow::bail!("--domain and --mode must be supplied together"),
            };
            println!("{}", app.run_node_edit(&id, &label, pin)?);
        }
        Commands::NodeDelete { id } => {
            println!("{}", app.run_node_delete(&id)?);
        }
        Commands::NodeSearch { query, budget, top } => {
            print!("{}", app.run_node_search(&query, budget, top)?);
        }
        Commands::Vault { dir, git } => {
            println!("{}", app.run_vault_import(&dir, git)?);
        }
        Commands::History { file } => {
            println!("{}", app.run_history_import(&file)?);
        }
        Commands::Praxis { file } => {
            println!("{}", app.run_praxis_backfill(&file)?);
        }
        #[cfg(feature = "spanner")]
        Commands::Spanner { action } => {
            let rt = tokio::runtime::Runtime::new()?;
            let (sync, mirror) = match action {
                SpannerAction::Sync { mirror } => (true, mirror),
                SpannerAction::Hydrate => (false, false),
            };
            let out = rt.block_on(app.run_spanner(sync, mirror))?;
            println!("{out}");
        }

        // ── Business commands ──
        Commands::Report { shift, html } => {
            let raw = std::fs::read_to_string(&shift)?;
            let input: physis_pro::report::ShiftReportInput = serde_json::from_str(&raw)?;
            if html {
                println!("{}", physis_pro::report::render_shift_report_html(&input));
            } else {
                println!("{}", physis_pro::report::render_shift_report_it(&input));
            }
        }
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
        Commands::Discover { dir, json_out, threshold, ai } => {
            if !ai {
                // Default: pure embedding-gap discovery — measured coverage
                // against the live grid, draft entries from the gaps.
                let output = app.run_discover(&dir, json_out.as_deref(), threshold)?;
                println!("{output}");
                return Ok(());
            }
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
        Commands::Studio { dir, html_out, demo } => {
            if demo {
                physis_pro::demo_corpus::seed_demo_corpus(&dir)?;
            }
            let out_path = html_out.unwrap_or_else(|| dir.join("studio.html"));
            let result = app.run_studio(&dir, &out_path)?;
            println!("{result}");
        }
        Commands::StructSim { a, b, threshold } => {
            let nodes_a = load_structure_nodes(&a)?;
            let nodes_b = load_structure_nodes(&b)?;
            let embedder: Box<dyn VectorEmbed> =
                Box::new(RandomProjectionEmbedder::new(config.physis.embed_dim));
            let g = physis_pro::structsim::compare_node_lists(&nodes_a, &nodes_b, embedder.as_ref(), threshold);
            println!("=== Structure Similarity: {} vs {} ===", a.display(), b.display());
            println!("  nodes A: {} · nodes B: {}", g.nodes_a.len(), g.nodes_b.len());
            println!("  overall similarity: {:.3}", g.similarity);
            println!("  matched fields (≥ {:.2}): {}", threshold, g.matches.len());
            for m in &g.matches {
                println!("    {}  ↔  {}   ({:.3})", m.a_path, m.b_path, m.score);
            }
            println!("  missing in B (fields A has, B lacks/renamed): {}", g.missing_in_b.len());
            for p in &g.missing_in_b {
                println!("    - {p}");
            }
            println!("  extra in B (fields B has, A lacks): {}", g.extra_in_b.len());
            for p in &g.extra_in_b {
                println!("    + {p}");
            }
        }
        Commands::StructSearch { dir, query, threshold, max } => {
            let q_nodes = load_structure_nodes(&query)?;
            let embedder: Box<dyn VectorEmbed> =
                Box::new(RandomProjectionEmbedder::new(config.physis.embed_dim));
            let mut ranked: Vec<(String, f32)> = Vec::new();
            for entry in WalkDir::new(&dir).into_iter().filter_map(|e| e.ok()) {
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.path();
                let nodes = match load_structure_nodes(path) {
                    Ok(n) if !n.is_empty() => n,
                    _ => continue,
                };
                let sim = physis_pro::structsim::compare_node_lists(
                    &q_nodes,
                    &nodes,
                    embedder.as_ref(),
                    threshold,
                )
                .similarity;
                ranked.push((path.display().to_string(), sim));
            }
            ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            println!("=== Structure Search: {} ranked by similarity ===", query.display());
            for (path, sim) in ranked.iter().take(max) {
                println!("  {sim:.3}  {path}");
            }
            if ranked.is_empty() {
                println!("  (no parseable structures found under {})", dir.display());
            }
        }
        Commands::Rag { dir, query, tokens, top_k } => {
            let docs = ingest::ingest_all(std::slice::from_ref(&dir));
            if docs.is_empty() {
                anyhow::bail!("no documents found under {}", dir.display());
            }
            let texts: Vec<String> = docs.iter().map(|d| d.body.clone()).collect();
            // Was hard-coded to RandomProjectionEmbedder, which made this
            // command — the retrieval demo — rank on random projections.
            let (embedder, _kind) = physis_pro::embed::select_embedder(&config.physis);
            let (result, context) = physis_pro::rag::retrieve_from_texts(
                &texts,
                &query,
                embedder.as_ref(),
                tokens,
                top_k,
            );
            println!("=== Token-Fixed RAG ===");
            println!("  query: {query}");
            println!("  corpus: {} docs · budget: {tokens} tokens · top_k: {top_k}", docs.len());
            println!("  selected: {} chunks · total {} tokens · truncated: {}",
                result.chunks.len(), result.total_tokens, result.truncated);
            for c in &result.chunks {
                println!("    [{:.3}] ({} tokens) doc#{}", c.score, c.tokens, c.id);
            }
            println!("\n--- assembled context (≤ {tokens} tokens) ---\n");
            println!("{context}");
        }
        Commands::Machine { .. } => unreachable!("machine subcommand handled before app init"),
        Commands::Study { .. } => unreachable!("study subcommand handled before app init"),
        Commands::Pack { .. } => unreachable!("pack is dispatched early"),
        Commands::Warm { .. } => unreachable!("warm is dispatched early"),
        Commands::Doctor => {
            run_doctor(&config)?;
        }
    }

    Ok(())
}

fn run_doctor(config: &PhysisProConfig) -> anyhow::Result<()> {
    println!("🔍 Physis Doctor\n");
    let mut issues = 0;

    // 1. Data directory
    let data_dir = &config.physis.data_dir;
    println!("📁 Data directory: {}", data_dir.display());
    if !data_dir.exists() {
        println!("  ❌ Does not exist (will be created on first use)");
        issues += 1;
    } else if let Err(e) = std::fs::read_dir(data_dir) {
        println!("  ❌ Not readable: {e}");
        issues += 1;
    } else {
        println!("  ✅ Exists and readable");
    }

    // 2. Scan root
    let scan_root = std::env::var("PHYSIS_SCAN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| data_dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))));
    println!("\n🔎 Scan root: {}", scan_root.display());
    if !scan_root.exists() {
        println!("  ❌ Does not exist");
        issues += 1;
    } else {
        println!("  ✅ Exists");
        if let Ok(canon) = scan_root.canonicalize() {
            if let Ok(data_canon) = data_dir.canonicalize() {
                if canon == data_canon {
                    println!("  ⚠️  Scan root equals data dir — set PHYSIS_SCAN_ROOT to parent for project scanning");
                    issues += 1;
                } else if canon.starts_with(&data_canon) {
                    println!("  ⚠️  Scan root is inside data dir — scanning will be limited");
                    issues += 1;
                } else {
                    println!("  ✅ Outside data dir");
                }
            }
        }
    }

    // 3. Config file
    let config_path = data_dir.join("config.json");
    println!("\n⚙️  Config: {}", config_path.display());
    if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(raw) => match serde_json::from_str::<serde_json::Value>(&raw) {
                Ok(_) => println!("  ✅ Valid JSON"),
                Err(e) => {
                    println!("  ❌ Invalid JSON: {e}");
                    issues += 1;
                }
            },
            Err(e) => {
                println!("  ❌ Not readable: {e}");
                issues += 1;
            }
        }
    } else {
        println!("  ℹ️  No config file (defaults will be used)");
    }

    // 4. Embedder
    println!("\n🧠 Embedder: {}", config.physis.embed_dim);
    let (embedder, kind) = physis_pro::embed::select_embedder(&config.physis);
    println!("  ✅ Loaded: {kind}");
    let _ = embedder.embed("doctor test");
    println!("  ✅ Embed test passed");

    // 5. Ontology
    println!("\n📚 Ontology:");
    let ont = OntologyLoader::load_all(&config.physis);
    let total = ont.human_domains.len() + ont.machine_domains.len() + ont.custom_domains.len();
    println!("  ✅ Loaded: {} domains ({} human + {} machine + {} custom)",
        total,
        ont.human_domains.len(),
        ont.machine_domains.len(),
        ont.custom_domains.len());

    // 6. License
    println!("\n🎫 License:");
    match physis_pro::license::load() {
        Ok(lic) => {
            println!("  ✅ Valid: {} → {} (seats: {})", lic.org, lic.exp, lic.seats);
        }
        Err(e) => {
            println!("  ⚠️  {e}");
        }
    }

    // Summary
    println!("\n{}", "─".repeat(40));
    if issues == 0 {
        println!("✅ All checks passed");
    } else {
        println!("⚠️  {issues} issue(s) found — see above");
    }

    Ok(())
}

/// Watch a directory and post study events to the running engine. `prev` holds
/// the last snapshot; every change diff maps to a generated / edited / deleted
/// event carrying date, time, place and path. With `--once`, records what is
/// already on disk as "opened" and exits.
fn run_study_recorder(
    dir: &Path,
    url: &str,
    place: &str,
    user: &str,
    interval: u64,
    once: bool,
) -> anyhow::Result<()> {
    use physis_pro::study::{diff_snapshots, snapshot_dir, StudyAction, StudyEvent};
    if !dir.is_dir() {
        anyhow::bail!("not a directory: {}", dir.display());
    }
    let api = format!("{}/api/v1/study/log", url.trim_end_matches('/'));
    let client = reqwest::blocking::Client::new();

    if once {
        let snap = snapshot_dir(dir)?;
        if snap.is_empty() {
            return Ok(());
        }
        let now = chrono::Utc::now();
        let events: Vec<StudyEvent> = snap
            .iter()
            .enumerate()
            .map(|(i, f)| {
                StudyEvent::new(
                    now - chrono::Duration::milliseconds(i as i64),
                    StudyAction::Opened,
                    f.path.clone(),
                    place.to_string(),
                )
            })
            .collect();
        post_study_events(&client, &api, user, &events)?;
        println!("recorded {} files as opened (place: {place})", events.len());
        return Ok(());
    }

    let mut prev = snapshot_dir(dir)?;
    println!("watching {} every {interval}s → {api} (place: {place})", dir.display());
    loop {
        std::thread::sleep(std::time::Duration::from_secs(interval));
        let cur = snapshot_dir(dir)?;
        let changes = diff_snapshots(&prev, &cur);
        prev = cur;
        if changes.is_empty() {
            continue;
        }
        let now = chrono::Utc::now();
        let events: Vec<StudyEvent> = changes
            .iter()
            .enumerate()
            .map(|(i, (p, a))| {
                StudyEvent::new(
                    now - chrono::Duration::milliseconds(i as i64),
                    *a,
                    p.clone(),
                    place.to_string(),
                )
            })
            .collect();
        post_study_events(&client, &api, user, &events)?;
    }
}

fn post_study_events(
    client: &reqwest::blocking::Client,
    api: &str,
    user: &str,
    events: &[physis_pro::study::StudyEvent],
) -> anyhow::Result<()> {
    let resp = client
        .post(api)
        .header("X-Physis-User", user)
        .json(&serde_json::json!({ "events": events }))
        .send()?;
    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("POST {api} → {status}");
    }
    println!(
        "→ {} events{}",
        events.len(),
        events
            .iter()
            .map(|e| format!(" [{}] {}", e.action.as_str(), e.path))
            .collect::<Vec<_>>()
            .join("")
    );
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

/// Load a structure file into flattened [`StructNode`]s. Accepts JSON (parsed
/// and walked recursively) or indentation-structured text (one `key: Type` per
/// line, 2-space indents = nesting). Returns an error if the file can't be read.
fn load_structure_nodes(path: &Path) -> anyhow::Result<Vec<physis_pro::structsim::StructNode>> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {}", path.display(), e))?;
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
        Ok(physis_pro::structsim::flatten_json(&value))
    } else {
        Ok(physis_pro::structsim::flatten_struct_text(&text))
    }
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

    // ── Semiotic node editing via the CLI ──────────────────────────────────

    /// Build a PhysisApp over an isolated HOME (so the shared sled store lives in
    /// a temp dir), forced to random-projection for speed.
    fn cli_app(dir: &std::path::Path) -> physis_pro::cli::PhysisApp {
        let mut cfg = PhysisProConfig::default();
        cfg.physis.data_dir = dir.join(".physis");
        cfg.physis.embedders.primary = Some(physis_pro::config::EmbedderConfig {
            kind: physis_pro::config::EmbedderKindConfig::RandomProjection,
            model_dir: String::new(),
            enabled: true,
        });
        physis_pro::cli::PhysisApp::new(cfg.physis.clone())
    }

    #[test]
    fn test_cli_node_edit_swaps_content_keeps_structure() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = cli_app(dir.path());

        // Register a node into the shared store (mirrors /scan + /coherence/register).
        let id = app.register_persisted_node("milling spindle bearing").unwrap();

        // Edit without a pin.
        let out = app.run_node_edit(&id, "welding robot joint recalibration", None).unwrap();
        assert!(out.contains(&id), "identity survives the swap: {out}");

        // Edit with a pin.
        let pinned = app.run_node_edit(
            &id,
            "welding robot joint recalibration (edited)",
            Some(("FABRICATE".into(), "MAINTAIN".into())),
        ).unwrap();
        assert!(pinned.contains("FABRICATE×MAINTAIN"), "pin echoed: {pinned}");

        // Unknown id errors.
        assert!(app.run_node_edit("missing", "x", None).is_err());

        // Delete, then the id is gone.
        app.run_node_delete(&id).unwrap();
        assert!(app.run_node_delete(&id).is_err(), "second delete is a no-op");
    }

    #[test]
    fn test_cli_node_search_bounds_the_response() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = cli_app(dir.path());

        for label in ["milling spindle bearing wear", "furnace temperature profile", "welding robot joint calibration"] {
            app.register_persisted_node(label).unwrap();
        }

        let out = app.run_node_search("spindle bearing", 20, 12).unwrap();
        assert!(out.contains("budget 20"), "budget echoed: {out}");
        // The packed result is bounded: total tokens can't exceed the budget.
        let header = out.lines().next().unwrap();
        let tokens: usize = header.split("tokens").next().unwrap()
            .split(',').nth(1).unwrap().trim().parse().unwrap();
        assert!(tokens <= 20, "total tokens {tokens} must respect budget 20");
        assert!(out.lines().nth(1).is_some(), "at least one result listed");

        // An empty store search still returns a bounded header.
        let dir2 = tempfile::tempdir().unwrap();
        let mut app2 = cli_app(dir2.path());
        let out2 = app2.run_node_search("anything", 100, 5).unwrap();
        assert!(out2.contains("0 result(s)"), "empty store: {out2}");
    }

    #[test]
    fn test_cli_watch_ingests_and_removes() {
        let dir = tempfile::tempdir().unwrap();
        let watched = dir.path().join("notes");
        std::fs::create_dir_all(&watched).unwrap();
        let file = watched.join("topology.md");
        std::fs::write(&file, "# Topology\n## Graphs\n").unwrap();
        let mut app = cli_app(dir.path());

        // First sweep registers the file as labeled core nodes.
        let out = app.run_watch(vec![watched.clone()], true).unwrap();
        assert!(out.contains("1 new"), "first sweep sees the new file: {out}");
        assert!(out.contains("registered"), "registers nodes: {out}");

        // Idle sweep: nothing to do.
        let out2 = app.run_watch(vec![watched.clone()], true).unwrap();
        assert!(out2.contains("0 new"), "second sweep sees nothing new: {out2}");

        // Content change → re-register; removal → nodes dropped.
        std::fs::write(&file, "# Topology edited\n").unwrap();
        let out3 = app.run_watch(vec![watched.clone()], true).unwrap();
        assert!(out3.contains("1 changed"), "content change detected: {out3}");

        std::fs::remove_file(&file).unwrap();
        let out4 = app.run_watch(vec![watched.clone()], true).unwrap();
        assert!(out4.contains("1 deleted"), "removal detected: {out4}");

        // Nothing left in the store for the removed file's prefix.
        let out5 = app.run_node_search("Topology", 100, 10).unwrap();
        assert!(out5.contains("0 result(s)"), "removed nodes are gone: {out5}");
    }

    #[test]
    fn test_cli_vault_import_registers_nodes() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        std::fs::create_dir_all(vault.join("notes")).unwrap();
        std::fs::write(vault.join("notes").join("alpha.md"), "# Alpha\n\n## Sub\n\ncontent here").unwrap();
        std::fs::write(vault.join("notes").join("beta.txt"), "Beta title\nplain body text").unwrap();
        let mut app = cli_app(dir.path());

        let out = app.run_vault_import(&vault, 0).unwrap();
        assert!(out.contains("2 docs"), "two notes imported: {out}");
        assert!(out.contains("registered"), "labels registered: {out}");

        // The notes are recallable by content via node-search.
        let search = app.run_node_search("alpha", 100, 10).unwrap();
        assert!(search.contains("Alpha"), "vault note recallable: {search}");

        // Re-importing is idempotent (dedupe by label).
        let out2 = app.run_vault_import(&vault, 0).unwrap();
        assert!(out2.contains("deduped"), "re-import dedupes: {out2}");

        // Missing dir errors.
        assert!(app.run_vault_import(&dir.path().join("nope"), 0).is_err());
    }

    #[test]
    fn test_cli_history_import_registers_nodes() {
        let dir = tempfile::tempdir().unwrap();
        let bookmarks = dir.path().join("bookmarks.html");
        std::fs::write(&bookmarks, r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p><DT><A HREF="https://shopfloor.example/">Shop floor</A></DL><p>"#).unwrap();
        let mut app = cli_app(dir.path());

        let out = app.run_history_import(&bookmarks).unwrap();
        assert!(out.contains("bookmarks.html"), "file named: {out}");
        assert!(out.contains("1 items"), "one bookmark: {out}");
        assert!(out.contains("registered"), "labels registered: {out}");

        // Recallable by content.
        let search = app.run_node_search("shop floor", 100, 10).unwrap();
        assert!(search.contains("Shop floor"), "bookmark recallable: {search}");

        // Re-import dedupes.
        let out2 = app.run_history_import(&bookmarks).unwrap();
        assert!(out2.contains("deduped"), "re-import dedupes: {out2}");

        // Unsupported extension errors.
        let csv = dir.path().join("data.csv");
        std::fs::write(&csv, "a,b\n").unwrap();
        assert!(app.run_history_import(&csv).is_err());
    }

    #[test]
    fn test_cli_praxis_backfill_asserts_verdicts() {
        let dir = tempfile::tempdir().unwrap();
        let export = dir.path().join("praxis.json");
        std::fs::write(&export, r#"{
            "notebook": [
              {"content": "shipped the release","mood": 9},
              {"content": "tired","mood": 2}
            ],
            "trackers": [{"name": "reading", "progress": 0.9}]
        }"#).unwrap();
        let mut app = cli_app(dir.path());

        let out = app.run_praxis_backfill(&export).unwrap();
        assert!(out.contains("3 items (2 success, 0 inert, 1 failure)"), "verdicts: {out}");
        assert!(out.contains("registered"), "nodes registered: {out}");

        // Re-backfill dedupes.
        let out2 = app.run_praxis_backfill(&export).unwrap();
        assert!(out2.contains("deduped"), "re-backfill dedupes: {out2}");

        // Missing file errors.
        assert!(app.run_praxis_backfill(&dir.path().join("nope.json")).is_err());
    }

    #[test]
    fn test_cli_facet_query_filters_ontology() {
        let dir = tempfile::tempdir().unwrap();
        let app = cli_app(dir.path());

        // Full listing returns rows with a table header.
        let all = app.run_facet_query(None, None, None, None, None, None, None, None, None);
        assert!(all.contains("entries match"), "listing header: {all}");
        assert!(all.contains("ENTRY"), "table header present");

        // Unknown facet value is tolerated (no filter) → same listing as no filter.
        let bogus = app.run_facet_query(None, None, None, Some("NOWHERE"), None, None, None, None, None);
        assert_eq!(bogus.trim_end(), all.trim_end(), "unknown facet value ignored");

        // A filter that matches nothing returns a zero-count message, not an error.
        let none = app.run_facet_query(None, None, None, Some("DESIGN"), None, None, None, Some("zzz-no-such-sub"), None);
        assert!(none.contains("0 entries match"), "empty result is a valid answer: {none}");

        // Case-insensitive facet matching works when entries carry the facet.
        let operate = app.run_facet_query(None, None, None, Some("operate"), None, None, None, None, None);
        let also = app.run_facet_query(None, None, None, Some("OPERATE"), None, None, None, None, None);
        assert_eq!(operate.trim_end(), also.trim_end(), "case-insensitive facet match");

        // Kind restriction narrows the search space.
        let office = app.run_facet_query(Some("office"), None, None, None, None, None, None, None, None);
        assert!(office.contains("entries match"), "office kind listed");
    }
}
