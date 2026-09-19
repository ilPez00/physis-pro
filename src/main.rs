use clap::{Parser, Subcommand};
use physis_pro::vfs::orchestrator::VfsOrchestrator;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[cfg(feature = "cli")]
use physis_pro::cli::PhysisApp;
use physis_pro::config::{OntologyLoader, PhysisProConfig};
use physis_pro::embed::{RandomProjectionEmbedder, VectorEmbed};
use physis_pro::estimate::estimate_project;
use physis_pro::graph::FileThemeGraph;
use physis_pro::industrial_benchmark::secom::{self, BenchmarkConfig};
use physis_pro::ingest;
use physis_pro::inventory::InventoryTracker;
use physis_pro::models::{cosine_sim, DomainDef, Score};
use physis_pro::ported::act::{bearing_on_with, run as act_run, Selection};
#[cfg(feature = "cli")]
use physis_pro::ported::system_cli::SystemArgs;
use physis_pro::quality::QualityTracker;
use physis_pro::wiki;

/// The object/action registry surface: what can be done, to what, and the
/// recorded outcome of doing it.
#[derive(Subcommand)]
pub enum ActionCommand {
    /// List loaded capabilities and registered objects
    List,
    /// Show one capability by name
    Show { capability: String },
    /// Call one capability: `action call <verb> <object> [key=value ...]`
    Call { line: Vec<String> },
}

/// The agent surface: who is in the roster, and which one answers a request.
#[derive(Subcommand)]
pub enum AgentCommand {
    /// List the roster with probed availability
    List,
    /// Show one agent's card
    Show { id: String },
    /// Invoke an agent: `agent call <id> <prompt...>`
    Call { id: String, prompt: Vec<String> },
    /// Ask JEV a question grounded in packed workspace context
    Ask {
        question: String,
        #[arg(long, default_value_t = 3000)]
        budget: usize,
        /// Cap the files packed as context. A cold embedding cache over a large
        /// tree can otherwise run for minutes; 0 means no bound.
        #[arg(long, default_value_t = 150)]
        max_files: usize,
        /// Ground on a saved pack (`pack --save <file>`) instead of packing now
        #[arg(long)]
        pack: Option<PathBuf>,
    },
    /// Route a request: `agent route <verb> <object-kind> [--agent ID] [--domain D] [--mode M]`
    Route {
        verb: String,
        object_kind: String,
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        mode: Option<String>,
    },
    /// Fold the observation log into the task graph
    Tasks {
        /// Observation log (JSONL). Defaults to $PHYSIS_OBSERVATIONS or
        /// .physis/observations.jsonl
        #[arg(long)]
        log: Option<PathBuf>,
    },
}

/// Editable intent: the operator's statement of what this workspace is for.
#[derive(Subcommand)]
pub enum IntentCommand {
    /// Show every intent with its recent edits
    Show,
    /// Set an intent: `intent set <id> <statement...>`
    Set {
        id: String,
        statement: Vec<String>,
        /// Who is making the edit (recorded in the provenance)
        #[arg(long, default_value = "cli")]
        by: String,
        /// Why the edit was made (recorded in the provenance)
        #[arg(long, default_value = "")]
        reason: String,
    },
    /// Show one intent's full edit history
    History { id: String },
}

/// Editable settings: command syntax and provider/model configuration.
#[derive(Subcommand)]
pub enum SettingsCommand {
    /// Show the effective settings (shipped defaults layered with yours)
    Show,
    /// Print the path of your settings file
    Path,
    /// Set a key, e.g. `settings set providers.jev.model oczen/mimo-v2.5-free`
    Set { key: String, value: String },
}

#[derive(Parser)]
#[command(
    name = "physis-pro",
    version,
    about = "Industrial backoffice intelligence engine"
)]
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
    DeepScan { dir: PathBuf },
    /// Query the ontology trie
    Query {
        query: String,
        #[arg(short, long, default_value = "10")]
        max_results: usize,
    },
    /// Object/action registry: list capabilities and objects, show one, call one
    Action {
        #[command(subcommand)]
        command: ActionCommand,
    },
    /// Agent roster, adapters and routing ladder
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    /// Editable settings: command syntax, providers, models and key env names
    Settings {
        #[command(subcommand)]
        command: SettingsCommand,
    },
    /// Editable intent: what this workspace is for, with provenance
    Intent {
        #[command(subcommand)]
        command: IntentCommand,
    },
    /// Show the seven attention terms and the weights the ranking applies
    Attention,
    /// Decision-native classification: ask a Jev-compatible endpoint for the
    /// cell and read the probabilities, instead of generating a label.
    Decide {
        /// The state to classify
        state: String,
        /// Classify into the semiotic grid (domain × mode)
        #[arg(long)]
        classify: bool,
        /// Accept the cell without review at or above this confidence
        #[arg(long, default_value_t = 0.5)]
        threshold: f64,
    },
    /// Draw everything as text: agents, actions, tasks, routes and settings
    Draw {
        /// Workspace root (default: current directory)
        root: Option<PathBuf>,
        /// Observation log for the task graph
        #[arg(long)]
        log: Option<PathBuf>,
        /// Line width
        #[arg(long, default_value_t = 100)]
        width: usize,
    },
    /// Generate stochastic dreams
    Dream {
        #[arg(short, long, default_value = "5")]
        count: usize,
    },
    /// Evaluate a dream
    Evaluate { id: String, grade: Score },
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
    /// Correct an artifact using the Physis corrective feedback loop.
    ///
    /// Generates code from an intent, observes it in the Python environment,
    /// and makes minimal structural mutations based on environmental feedback
    /// until the artifact coheres or max attempts is reached.
    Correct {
        /// Description of what the user wants
        intent: String,
        /// Maximum number of correction attempts
        #[arg(long, default_value_t = 10)]
        max_attempts: usize,
        /// Timeout for Python execution in seconds
        #[arg(long, default_value_t = 5.0)]
        timeout: f64,
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
        /// Write the pack to a JSON file for reuse: `agent ask --pack <file>`
        /// grounds on it without re-embedding the tree.
        #[arg(long)]
        save: Option<PathBuf>,
        /// Cap the files walked. A cold embedding cache over a large tree can
        /// run for minutes; 0 means no bound.
        #[arg(long, default_value_t = 0)]
        max_files: usize,
        /// Name the intent this pack is built under; recorded in the packet.
        #[arg(long)]
        intent: Option<String>,
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
    /// Estimate token usage and scan/index time for a directory.
    Estimate {
        /// Directory to estimate (default: current directory)
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
    /// Ask "where/when/why" about ingested files (Phase 10 datawork): facet
    /// query over the causal ledger — space, time, and cause at once.
    Where {
        /// Free-text filter over artifact paths
        query: Option<String>,
        /// Space facet: object paths starting with this root
        #[arg(long)]
        place: Option<String>,
        /// Cause facet: mechanism or actor (`generated`, `watch`, `vault-import`, …)
        #[arg(long)]
        cause: Option<String>,
        /// Time facet: only events within the last N days
        #[arg(long)]
        within: Option<i64>,
        /// Time facet: exact period bucket `YYYY-MM`
        #[arg(long)]
        during: Option<String>,
    },
    /// Search inside the corpus: heading-scoped sections ranked by keyword
    /// (BM25) and meaning (cosine) together.
    ///
    /// `query` recalls the labels the graph already knows; `find` recalls what
    /// the documents actually say — the paragraph nobody labeled. Quote a
    /// phrase to require it verbatim: physis-pro find '"no fault found"'
    Find {
        /// What to look for. Quoted spans must appear verbatim.
        query: String,
        /// Directory to search (default: the first configured watch dir)
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Ranking: hybrid (default) | semantic | lexical
        #[arg(long)]
        mode: Option<String>,
        /// Space facet: folder under the search root
        #[arg(long)]
        place: Option<String>,
        /// Time facet: period bucket `YYYY-MM`
        #[arg(long)]
        period: Option<String>,
        /// File type facet: extension without the dot (`md`, `txt`, `eml`)
        #[arg(long)]
        kind: Option<String>,
        /// Maximum sections to print
        #[arg(long, default_value = "10")]
        top_k: usize,
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
    Classify { file: PathBuf },
    /// Render a timeline/incident view from normalized operational events.
    ///
    /// Input: a JSON file holding an array of OperationalEvent records, e.g.
    /// [{"event_key":"k1",
    ///   "observed_at":"2026-08-24T08:00:00Z",
    ///   "kind":"machine",         <- machine|document|email|workflow|system|verdict
    ///   "status":"order",         <- order|incident|inert|info
    ///   "subject":"CNC-04",
    ///   "machine_id":"CNC-04",
    ///   "provenance":{"connector":"industrial","source":"machine"}}]
    ///
    /// Required per record: event_key, observed_at (RFC 3339), kind, status,
    /// subject, provenance{connector, source}; everything else is optional.
    #[command(verbatim_doc_comment)]
    Timeline {
        /// JSON file containing an array of OperationalEvent records
        input: PathBuf,
        /// Emit JSON instead of markdown
        #[arg(long)]
        json: bool,
    },
    /// Run diagnostics: config, data dir, scan root, embedder, license
    Doctor,
    /// Run the semantic embedder self-test over the cascade-selected embedder.
    ///
    /// PH-004: the packet gate runs this on CI. Exit 0 iff the selected
    /// embedder passes `embed::semantic_self_test`; exit 1 with a per-probe
    /// verdict otherwise. Under `PHYSIS_EMBEDDER=random-projection` the verdict
    /// is expected to FAIL (that is the instrument working, not a bug).
    Selftest,
    /// Print the licence on this host: SKU, org, seats, expiry, maintenance,
    /// feature list, revocation status. Bypasses the licence gate so an
    /// unlicensed host can still report its state. `--json` for scripts.
    License {
        /// Path to a licence file to inspect instead of the default location.
        #[arg(long)]
        path: Option<PathBuf>,
        /// Print the licence as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Build/refresh the fast-path disk catalog for a directory tree (PLAN
    /// 1.3). Metadata-only walk; re-hashes just stat-changed files; persists
    /// to sled so the next run starts warm. No embedder, no ontology.
    Index {
        /// Directory to index (default: current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
        /// Keep running, re-indexing every --interval seconds and printing deltas
        #[arg(long)]
        watch: bool,
        /// Seconds between passes in --watch mode
        #[arg(long, default_value_t = 10)]
        interval: u64,
    },
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
        /// Retract an existing pin: the classifier decides the cell again.
        /// Excludes --domain/--mode.
        #[arg(long)]
        clear_pin: bool,
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
    /// Remember one labeled memory with an asserted outcome verdict
    /// (−1 failed, 0 inert, +1 worked). CLI twin of the MCP
    /// `physis_remember` tool — the write half of the dev loop
    /// (`node-search` is the read half). Dedupes by label.
    Note {
        /// Text to remember; also becomes its searchable label
        text: String,
        /// Coherence verdict −1..1 (default: 0 inert)
        #[arg(long, default_value_t = 0.0)]
        verdict: f32,
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
    /// list, chat JSONL, shell history, Claude session JSONL) into the shared
    /// store as labeled core nodes
    History {
        /// Path to the export file (.html/.htm, .json, .opml/.xml, .jsonl/.txt,
        /// .zsh_history/.bash_history, Claude UUID.jsonl)
        file: PathBuf,
    },
    /// Prepopulate the store from this machine: shell histories, Claude agent
    /// sessions, and the Hermes memory file. Missing sources are skipped;
    /// re-runs dedupe by label
    Onboard {
        /// Newest N agent sessions to import (default 30)
        #[arg(long, default_value_t = 30)]
        max_sessions: usize,
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
    /// Run the live Ontology Studio web server (axum) for interactive browsing,
    /// editing, and feedback on the semiotic grid. This is the moved `physis-core
    /// studio` server, gated behind the `web` feature.
    #[cfg(feature = "web")]
    StudioServer {
        /// Port to listen on (default 3000)
        #[arg(long, default_value_t = 3000)]
        port: u16,
        /// ONNX model directory (model.onnx + tokenizer.json). Optional — falls
        /// back to deterministic random projection when absent or unloadable.
        #[arg(long)]
        model: Option<String>,
    },
    /// The 10-minute demo (Phase 17 U5): turn a prospect folder into a shift
    /// report + an ontology studio in one command. Reads every `*.csv` of shift
    /// records (or `--demo` seeds a sample), renders `report.html`/`report.md`
    /// and `studio.html` in the same folder, and prints what to show first.
    Demo {
        /// Folder with the prospect's shift CSVs and documents.
        dir: PathBuf,
        /// Seed a sample shift CSV + demo corpus first (idempotent — real data
        /// is never overwritten).
        #[arg(long)]
        demo: bool,
    },
    /// Search the wiki (llmwiki + file-theme graph)
    Wiki { query: String },
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
    /// Every reducer's request schema is discoverable:
    /// `physis-pro machine <name> --schema` prints a ready-to-edit example
    /// that pipes straight back in (QA 8.7).
    Machine {
        /// Machine name (chronos, topos, harmony, polis, soma, elenchus, lex,
        /// empedocles, euclid, techne, erosp, erosa, plutos, krisis, pathos,
        /// chroma, kinesis, apodeixis).
        name: String,
        /// JSON input file; omit or use '-' to read from stdin.
        #[arg(short, long)]
        input: Option<PathBuf>,
        /// Print a ready-to-edit example input JSON for <name> and exit.
        #[arg(long)]
        schema: bool,
        /// Run the machine's engine-side reduce instead of the stateless one:
        /// register what it perceives into the persisted graph and bridge it
        /// into the holarchy. Supported by chronos, euclid and empedocles.
        #[arg(long)]
        reduce: bool,
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
    Study {
        /// Directory to watch (default: current directory)
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
    /// Run the SECOM industrial benchmark: classify semiconductor manufacturing
    /// observations, apply quality feedback from ground truth, and measure
    /// before/after accuracy.
    Benchmark {
        #[command(subcommand)]
        dataset: BenchmarkDataset,
    },
    /// Phase 9.3 — mine decisions from agent session transcripts (JSONL):
    /// episodes → cue-based candidates → grounding gate → title + embedding
    /// dedup. Prints one grounded decision per line as JSON.
    DecisionsMine {
        /// Transcript file(s) to mine (JSONL). Use `-` for stdin.
        files: Vec<PathBuf>,
        /// Adapter to parse with (see `--list`)
        #[arg(long)]
        adapter: Option<String>,
        /// List registered transcript adapters and exit
        #[arg(long)]
        list: bool,
        /// Embedding-dedup cosine threshold (raise on semantic embedders)
        #[arg(long, default_value_t = 0.999)]
        threshold: f32,
        /// Also register each surviving decision as a labeled core node (`decision:` prefix)
        #[arg(long)]
        register: bool,
    },
    /// Show the recurring themes across the persisted graph: cluster the
    /// coherence nodes into communities and summarize each (Phase 11). Offline
    /// and deterministic — no provider needed; reads the "_default" tenant.
    Themes {
        /// Only show communities with at least this many members
        #[arg(long, default_value_t = 1)]
        min_size: usize,
        /// Emit JSON instead of markdown
        #[arg(long)]
        json: bool,
    },
    /// Workflow scanner — audit a directory's documentation: which docs exist
    /// for AI agents vs humans, when each was last touched, and (in a git repo)
    /// who touched it. Answers "who did what where" over the tree, walking the
    /// harness dirs (`.claude`, `.opencode`, …) the base scan skips.
    Workflow {
        /// Directory to audit (default: current directory)
        dir: PathBuf,
        /// Emit JSON instead of markdown
        #[arg(long)]
        json: bool,
        /// Record the audit to the causal ledger of the running engine too
        #[arg(long)]
        record: bool,
        /// Engine base URL for --record
        #[arg(long, default_value = "http://127.0.0.1:19876")]
        url: String,
        /// X-Physis-User id for --record (default "_default")
        #[arg(long, default_value = "_default")]
        user: String,
    },
    /// Open the unified dashboard (physis-pro-web on :19876)
    Dashboard {
        /// Web port (default 19876)
        #[arg(long, default_value_t = 19876)]
        port: u16,
        /// Print URL only, do not launch browser
        #[arg(long)]
        no_open: bool,
    },
    /// Chat via running physis-pro-web (/api/v1/chat)
    Chat {
        /// Prompt to send
        prompt: String,
    },
    /// Embed text via running physis-pro-web (/api/v1/embed)
    Embed {
        /// Text to embed
        text: String,
    },
    /// List MCP gateway tools + HTTP mapping; with name show mapping + live probe
    Mcp {
        /// Tool name (omit to list all 15)
        tool: Option<String>,
    },
    /// Shared workspace interface: inspect, list, find, pack, read, history,
    /// delegation, remember, run, export, and `serve` (MCP stdio). No licence
    /// or model is required; see `physis-pro system capabilities`.
    #[cfg(feature = "cli")]
    System(SystemArgs),
    /// Grounded question-answering over a document directory, with citations.
    /// Uses an oracle endpoint when configured (`PHYSIS_ORACLE_*`), falling
    /// back to an extractive tier otherwise; the answer always says which.
    Notebook {
        /// Directory of documents to answer from.
        dir: PathBuf,
        /// Question to answer from those documents.
        query: String,
        /// Hard token ceiling for the packed context.
        #[arg(long, default_value_t = 2000)]
        budget: usize,
        #[arg(long)]
        json: bool,
    },
    /// Run a command, record it, and say what is already believed about it —
    /// moved from physis-core's `act` (see `src/ported/act.rs`).
    ///
    /// Not a sandbox — `sh -c` already runs things. The contribution is that
    /// the intent is recorded BEFORE the command runs (so an action that
    /// hangs still leaves a record), the outcome is recorded with its real
    /// duration, and claims bearing on it are surfaced first — especially
    /// ones already Contradicted, which is the machine saying you
    /// established this does not work.
    #[command(name = "act")]
    Act {
        /// The command, as you would type it into a shell.
        command: String,
        /// Show what is already believed and STOP, without running anything.
        #[arg(long)]
        dry_run: bool,
        #[arg(long, default_value_t = 5)]
        top: usize,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum BenchmarkDataset {
    /// Run the SECOM semiconductor manufacturing benchmark
    Secom {
        /// Maximum number of observations to load (default: all)
        #[arg(long)]
        limit: Option<usize>,
        /// Random seed for train/test split (default: 42)
        #[arg(long, default_value_t = 42)]
        seed: u64,
        /// Fraction of data used for training (default: 0.5)
        #[arg(long, default_value_t = 0.5)]
        train_ratio: f32,
        /// Output directory for reports (default: benchmark-results/secom)
        #[arg(long)]
        output: Option<PathBuf>,
        /// Skip the quality feedback loop (baseline only)
        #[arg(long)]
        no_feedback: bool,
        /// Embedder kind: random (default) or onnx (requires --features embed-onnx)
        #[arg(long, default_value = "random")]
        embedder: String,
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
    /// Generic industrial adapter (MQTT or Modbus) — the Phase 1.8 real-time loop.
    /// Polls a machine for telemetry, detects anomalies, emits normalized
    /// OperationalEvents as JSON lines, and optionally registers them into a
    /// physis engine.
    Adapter {
        /// Protocol: mqtt | modbus
        #[arg(long, default_value = "mqtt")]
        protocol: String,
        /// Broker/host, or host:port (Modbus defaults to 502)
        #[arg(long)]
        endpoint: String,
        /// Machine id recorded on every event
        #[arg(long, default_value = "machine-01")]
        machine_id: String,
        /// MQTT telemetry topic (mqtt only; defaults to physis/<id>/reading)
        #[arg(long)]
        topic: Option<String>,
        /// Also register events into this physis engine (base URL)
        #[arg(long)]
        post_to: Option<String>,
        /// Tenant for those registrations
        #[arg(long, default_value = "_default")]
        user: String,
        /// Bearer token for the engine
        #[arg(long)]
        token: Option<String>,
        /// Seconds between polls
        #[arg(long, default_value_t = 10)]
        interval: u64,
        /// Poll once and exit (smoke test)
        #[arg(long)]
        once: bool,
        /// Temperature anomaly threshold (passed to detect_anomalies as temp_max)
        #[arg(long)]
        temp_max: Option<f32>,
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

/// Gateway tool -> (METHOD, HTTP path). Mirrors dashboard.html static list.
fn mcp_tool_mapping(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        "health" => ("GET", "/health"),
        "stats" => ("GET", "/api/v1/stats"),
        "query" => ("GET", "/api/v1/query"),
        "scan" => ("POST", "/api/v1/scan"),
        "ingest_prompt" => ("POST", "/api/v1/scan"),
        "dream_generate" => ("POST", "/api/v1/dream/generate"),
        "dream_evaluate" => ("POST", "/api/v1/dream/evaluate"),
        "coherence_snapshot" => ("GET", "/api/v1/coherence/snapshot"),
        "coherence_register" => ("POST", "/api/v1/coherence/register"),
        "context_filter" => ("POST", "/api/v1/context/filter"),
        "compress_logs" => ("POST", "/api/v1/compress/logs"),
        "pdca_plan" => ("POST", "/api/v1/pdca/plan"),
        "pdca_act" => ("POST", "/api/v1/pdca/act"),
        "pdca_stats" => ("GET", "/api/v1/pdca/stats"),
        "reconstruct" => ("POST", "/api/v1/reconstruct"),
        _ => return None,
    })
}

fn run_mcp_cli(tool: Option<&str>) -> anyhow::Result<()> {
    const TOOLS: [&str; 15] = [
        "health",
        "stats",
        "query",
        "scan",
        "ingest_prompt",
        "dream_generate",
        "dream_evaluate",
        "coherence_snapshot",
        "coherence_register",
        "context_filter",
        "compress_logs",
        "pdca_plan",
        "pdca_act",
        "pdca_stats",
        "reconstruct",
    ];
    let Some(name) = tool else {
        for t in TOOLS {
            let (m, p) = mcp_tool_mapping(t).unwrap();
            println!("{t:20} {m:4} {p}");
        }
        return Ok(());
    };
    let Some((method, path)) = mcp_tool_mapping(name) else {
        eprintln!("unknown tool: {name}");
        std::process::exit(2);
    };
    println!("{name}: {method} {path}");
    // Best-effort live probe if physis-pro-web is up.
    let client = reqwest::blocking::Client::new();
    let url = format!("http://127.0.0.1:19876{path}");
    let res = if method == "GET" {
        client.get(&url).send()
    } else {
        client.post(&url).json(&serde_json::json!({})).send()
    };
    match res {
        Ok(r) => {
            let body = r.text().unwrap_or_default();
            let cut = body.chars().take(300).collect::<String>();
            println!("{cut}");
        }
        Err(_) => println!("start physis-pro-web first"),
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    // Rust ignores SIGPIPE before main; without this, `physis-pro ... | head`
    // prints a "Broken pipe" panic after the output the user asked for, which
    // reads like a crash. Core's own main() and the `physis` front door both
    // do this; product commands (`system` included, since the 2026-09-14
    // split) need the same fix here rather than relying on a caller that no
    // longer exists.
    physis_core::unbreak_pipes();
    // MVP entry point (§1): `physis-pro <directory>` with no subcommand.
    //
    // Detected BEFORE clap, by asking clap itself which names are subcommands:
    // a bare existing directory that is not a subcommand opens the world loop;
    // everything else parses exactly as it always did. Doing it here rather than
    // as an extra positional keeps the existing CLI surface byte-for-byte
    // unchanged (an optional positional beside an optional subcommand made clap
    // demand a value for every `physis-pro <subcommand>` call).
    //
    // No licence gate on this path: the loop reuses only the licence-free
    // slices (`nav` index, `scanner`, `pack`, `act`, `physis_core::map`),
    // exactly like `physis-pro system` and `physis-pro act`.
    {
        use clap::CommandFactory;
        let argv: Vec<String> = std::env::args().skip(1).collect();
        let is_subcommand = |name: &str| {
            Cli::command()
                .get_subcommands()
                .any(|s| s.get_name() == name || s.get_all_aliases().any(|a| a == name))
        };
        if let Some(first) = argv.first() {
            if !first.starts_with('-') && !is_subcommand(first) {
                let dir = PathBuf::from(first);
                if dir.is_dir() {
                    let _license = physis_pro::license::load().ok();
                    return physis_pro::mvp::repl::run(&dir);
                }
            }
        }
    }
    let cli = Cli::parse();

    // QA 8.7: schema discovery is documentation, not operation — answer it
    // before the licence gate so an evaluator can learn the input formats.
    if let Commands::Machine {
        name, schema: true, ..
    } = &cli.command
    {
        match physis_pro::machines::run::schema(name) {
            Some(ex) => println!("{ex}"),
            None => {
                eprintln!("Unknown machine: {name}");
                eprintln!("Known: {}", physis_pro::machines::run::NAMES.join(", "));
                std::process::exit(2);
            }
        }
        return Ok(());
    }

    // Estimate token usage and scan/index time for a directory.
    if let Commands::Estimate { dir } = &cli.command {
        let dir = dir.clone().unwrap_or_else(|| PathBuf::from("."));
        match estimate_project(&dir) {
            Ok(est) => {
                println!("Estimate:");
                println!("  baseline tokens: {}", est.baseline_tokens);
                println!("  packed tokens: {}", est.packed_tokens);
                println!("  saved: {:.1}%", est.saved_percent);
                println!("  scan time: {:.2}s", est.scan_elapsed_secs);
                println!("  model time estimate: {:.2}s", est.model_time_est_secs);
            }
            Err(e) => {
                eprintln!("Error running estimate: {e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    // Licence gate before any work. Placed after `Cli::parse()` so `--help` and
    // `--version` still answer on an unlicensed host: an evaluator has to be able
    // to see what the tool is before we demand a key, and support needs
    // `--version` to triage an install that refuses to run.
    //
    // All commands except `doctor`, `benchmark`, `license` and `selftest`
    // require a valid licence. These bypass the gate so they can run without a
    // licence — useful for diagnostics, triage, and reporting the licence and
    // embedder state itself.
    if !matches!(
        &cli.command,
        Commands::Doctor
            | Commands::Benchmark { .. }
            | Commands::License { .. }
            | Commands::Selftest
            | Commands::Act { .. }
            | Commands::StudioServer { .. }
            | Commands::Index { .. }
            | Commands::System(..)
    ) {
        let _license = physis_pro::license::enforce();
    } else {
        // Even for these, verify a minimal licence exists so the
        // binary at least has a valid key embedded; we just don't enforce dates/
        // features for these help-of-last-resort commands.
        let _license = physis_pro::license::load().ok();
    }

    // The shared workspace interface (`system`) needs only the filesystem and
    // its own observation log: no licence, no ontology, no embedder. Dispatch
    // it before building the heavy PhysisApp, matching Core's former
    // `physis-core system` behaviour exactly (same Workspace, same contract).
    #[cfg(feature = "cli")]
    if let Commands::System(args) = &cli.command {
        return args.run();
    }

    // The `index` subcommand needs the filesystem and sled and nothing else:
    // dispatch it before building the heavy PhysisApp (ontology + embedder).
    if let Commands::Index {
        dir,
        watch,
        interval,
    } = &cli.command
    {
        return run_indexer(dir, *watch, *interval);
    }

    // The stateless `machine` subcommand is pure: dispatch it before building
    // the heavy PhysisApp (ontology + embedder), so it runs with no disk state
    // — ideal for a faulty mount or a quick offline reduction.
    //
    // `--reduce` is the other half (Phase 8): it writes into the persisted
    // graph, so it needs the embedder and the store and falls through to the
    // app path below. The payload is parsed once, here, because stdin can only
    // be read once — re-reading it after app init would find it empty.
    let mut machine_reduce: Option<serde_json::Value> = None;
    if let Commands::Machine {
        name,
        input,
        reduce,
        ..
    } = &cli.command
    {
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
        if *reduce {
            machine_reduce = Some(value);
        } else {
            let out = physis_pro::machines::run::run(name, &value)?;
            println!("{}", serde_json::to_string_pretty(&out)?);
            return Ok(());
        }
    }

    // The `study` recorder only needs the filesystem + the running engine: run
    // it before building the heavy PhysisApp (ontology + embedder).
    if let Commands::Study {
        dir,
        url,
        place,
        user,
        interval,
        once,
    } = &cli.command
    {
        return run_study_recorder(dir, url, place, user, *interval, *once);
    }

    // The `workflow` scanner only needs the filesystem (+ optional git):
    // dispatch before PhysisApp::new, which embeds ~650 ontology entries.
    if let Commands::Workflow {
        dir,
        json,
        record,
        url,
        user,
    } = &cli.command
    {
        let root = std::path::PathBuf::from(dir);
        let docs = physis_pro::workflow::scan_workflow(&root, None);
        let report = physis_pro::workflow::report(&root, docs);
        if *json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            print_workflow_report(&report);
        }
        if *record {
            // Push the audit's events into the running engine's single event
            // stream (POST /api/v1/events), so the timeline/ledger reflect it.
            let events = physis_pro::workflow::to_op_events(&report.docs);
            let client = reqwest::blocking::Client::new();
            let api = format!("{}/api/v1/events", url.trim_end_matches('/'));
            let _ = client
                .post(&api)
                .header("X-Physis-User", user)
                .json(&serde_json::json!({ "events": events }))
                .send();
        }
        return Ok(());
    }

    // `pack` needs an embedder and nothing else. Dispatching it before
    // `PhysisApp::new` skips the startup ontology pass, which embeds ~650
    // entries and dominated the runtime: packing 35 chunks took 6m52s with the
    // pass and seconds without it. Any command that does not read the ontology
    // belongs here rather than below.
    if let Commands::Pack {
        dir,
        query,
        budget,
        top_k,
        diversity,
        show_context,
        save,
        max_files,
        intent,
    } = &cli.command
    {
        let config = PhysisProConfig::default();
        let root = physis_pro::pack::resolve_root(dir, None)?;
        let (embedder, kind) = physis_pro::embed::select_embedder(&config.physis);
        eprintln!("[physis] pack embedder: {kind}");

        // Cache persists across runs: the first pack of a tree pays for the
        // whole corpus, later packs pay only for what changed.
        let mut cache = physis_pro::pack::EmbedCache::load(&config.physis.data_dir);
        let limit = if *max_files == 0 {
            None
        } else {
            Some(*max_files)
        };
        let result = physis_pro::pack::pack_cached_limited(
            &root,
            query,
            *budget,
            *top_k,
            *diversity,
            embedder.as_ref(),
            kind,
            &mut cache,
            limit,
        );
        let mut result = result;
        // A packet that does not say what it was for cannot be audited against
        // it, so the intent id travels with the pack.
        result.intent = intent.clone();
        cache.save(&config.physis.data_dir);
        let (hits, misses) = cache.stats();
        eprintln!("[physis] embed cache: {hits} hits, {misses} misses");
        print!("{}", physis_pro::pack::summarize(&result));
        if *show_context {
            println!("\n--- context ---\n{}", result.context);
        }
        // Pre-pack: persist the assembled context and its provenance so a
        // later `agent ask --pack <file>` is grounded without re-embedding.
        if let Some(path) = save {
            let text = serde_json::to_string_pretty(&result)?;
            std::fs::write(path, text)?;
            eprintln!(
                "[physis] pack saved to {} ({} tokens, {} chunks)",
                path.display(),
                result.tokens,
                result.chunks.len()
            );
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
                &root,
                "warm",
                1,
                1,
                0.0,
                embedder.as_ref(),
                kind,
                &mut cache,
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

    // Dashboard/Chat/Embed/Mcp are thin HTTP clients over physis-pro-web:
    // dispatch before PhysisApp::new (no embedder/ontology needed). Licence
    // gate above already ran — these REQUIRE a licence (not in bypass list).
    if let Commands::Dashboard { port, no_open } = &cli.command {
        let url = format!("http://127.0.0.1:{port}/dashboard");
        println!("{url}");
        if !no_open {
            let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
        }
        return Ok(());
    }
    if let Commands::Chat { prompt } = &cli.command {
        let client = reqwest::blocking::Client::new();
        let body = serde_json::json!({ "messages": [{ "role": "user", "content": prompt }] });
        match client
            .post("http://127.0.0.1:19876/api/v1/chat")
            .json(&body)
            .send()
        {
            Ok(resp) => {
                let v: serde_json::Value = resp.json().unwrap_or(serde_json::Value::Null);
                println!(
                    "{}",
                    v.get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("(empty)")
                );
            }
            Err(_) => println!("start physis-pro-web first"),
        }
        return Ok(());
    }
    if let Commands::Embed { text } = &cli.command {
        let client = reqwest::blocking::Client::new();
        match client
            .post("http://127.0.0.1:19876/api/v1/embed")
            .json(&serde_json::json!({ "text": text }))
            .send()
        {
            Ok(resp) => {
                let v: serde_json::Value = resp.json().unwrap_or(serde_json::Value::Null);
                let dim = v.get("dim").and_then(|d| d.as_u64()).unwrap_or(0);
                let vec: Vec<f32> = v
                    .get("embedding")
                    .and_then(|e| serde_json::from_value(e.clone()).ok())
                    .unwrap_or_default();
                let shown: Vec<String> = vec.iter().take(8).map(|x| format!("{x:.4}")).collect();
                println!("dim={dim} [{}…]", shown.join(", "));
            }
            Err(_) => println!("start physis-pro-web first"),
        }
        return Ok(());
    }
    if let Commands::Mcp { tool } = &cli.command {
        return run_mcp_cli(tool.as_deref());
    }

    // Per-command entitlement. Kept as a single table rather than scattered
    // checks so that adding a paid capability means adding one line here, and
    // so the mapping between commands and SKUs stays auditable at a glance.
    // `selftest` reports on the embedder and must run unlicensed (same family
    // as `doctor`); giving it an entitlement would make triage need a licence.
    let required_feature = match &cli.command {
        #[cfg(feature = "mcp")]
        Commands::Serve { .. } => Some("mcp"),
        Commands::Ingest => Some("ingest"),
        _ => None,
    };
    if let Some(feature) = required_feature {
        if !matches!(&cli.command, Commands::Doctor | Commands::Selftest) {
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

    // `license` is a pure read of the licence state — spinning up the embedder
    // and ontology first would make an air-gapped host wait minutes (or ~25 min
    // under ONNX) just to answer "what licence am I running?". Dispatched here,
    // before `PhysisApp::new`.
    if let Commands::License { path, json } = cli.command {
        let lic = match &path {
            Some(p) => {
                let blob = std::fs::read_to_string(p)
                    .map_err(|e| anyhow::anyhow!("cannot read licence {}: {e}", p.display()))?;
                physis_pro::license::verify_blob(&blob)
                    .map_err(|e| anyhow::anyhow!("licence is invalid: {e}"))?
            }
            None => {
                physis_pro::license::load().map_err(|e| anyhow::anyhow!("no valid licence: {e}"))?
            }
        };
        let revoked = physis_pro::license::check_revoked(&lic.license_id, None).is_err();
        if json {
            let v = serde_json::json!({
                "license_id": lic.license_id,
                "org": lic.org,
                "sku": lic.tier().sku(),
                "tier": lic.tier().name(),
                "seats": lic.seats,
                "features": lic.features,
                "issued": lic.iat,
                "expires": lic.exp,
                "maintenance_expires": lic.maint_exp,
                "days_remaining": lic.days_remaining(),
                "revoked": lic.revoked || revoked,
            });
            println!("{}", serde_json::to_string_pretty(&v)?);
        } else {
            println!("license_id      : {}", lic.license_id);
            println!("licensed to     : {}", lic.org);
            println!(
                "sku             : {} ({})",
                lic.tier().sku(),
                lic.tier().name()
            );
            println!("seats           : {}", lic.seats);
            println!("features        : {}", lic.features.join(", "));
            println!("issued          : {}", lic.iat);
            println!(
                "expires         : {} ({} days left)",
                lic.exp,
                lic.days_remaining()
            );
            println!("maintenance until: {}", lic.maint_exp);
            println!("revoked         : {}", lic.revoked || revoked);
        }
        return Ok(());
    }

    // `notebook` answers from its own corpus, embedder and citation index — it
    // never touches the ontology/classifier PhysisApp builds, so it is
    // dispatched here rather than paying that startup cost.
    if let Commands::Notebook {
        dir,
        query,
        budget,
        json,
    } = &cli.command
    {
        let docs = physis_pro::ingest::ingest_all(std::slice::from_ref(dir));
        if docs.is_empty() {
            eprintln!("no documents found under {}", dir.display());
            std::process::exit(1);
        }
        let pairs: Vec<(String, String)> = docs.into_iter().map(|d| (d.title, d.body)).collect();
        // Was hard-coded to RandomProjectionEmbedder (lexical hash, not
        // meaning) — the same defect already fixed for the `rag` command
        // below (see its own comment), and for the MCP server, via a
        // self-test-gated selector: a real semantic embedder when one is
        // configured and passes the probe, `"random-projection"` labelled as
        // such otherwise, never silently claimed. `physis_core::embed::select`
        // (not Pro's `select_embedder`, which returns a different, Pro-only
        // `VectorEmbed` trait object `notebook::answer` cannot take) matches
        // the `&dyn physis_core::embed::VectorEmbed` this module already
        // expects, and needs no config — just the dimension already used here.
        let (embedder, embedder_kind) = physis_core::embed::select(384);
        let answer = physis_pro::ported::notebook::answer(
            &pairs,
            query,
            embedder.as_ref(),
            *budget,
            embedder_kind,
        )?;
        if *json {
            println!("{}", serde_json::to_string_pretty(&answer)?);
        } else {
            println!("{}", answer.render());
        }
        return Ok(());
    }

    // `act` needs only the claims ledger and the observation log — like
    // `notebook`, dispatched here rather than paying `PhysisApp::new()`'s
    // ontology/classifier startup cost.
    if let Commands::Act {
        command,
        dry_run,
        top,
        json,
    } = &cli.command
    {
        return cmd_act(command, *dry_run, *top, *json);
    }

    // `action` needs only the registry and the observation log, not the
    // ontology/classifier startup cost of `PhysisApp::new()`.
    if let Commands::Action { command } = &cli.command {
        return cmd_action(command);
    }

    if let Commands::Agent { command } = &cli.command {
        return cmd_agent(command);
    }

    if let Commands::Settings { command } = &cli.command {
        return cmd_settings(command);
    }

    if let Commands::Draw { root, log, width } = &cli.command {
        return cmd_draw(root.as_deref(), log.as_deref(), *width);
    }

    if let Commands::Intent { command } = &cli.command {
        return cmd_intent(command);
    }

    if let Commands::Attention = &cli.command {
        let w = physis_pro::nav::NavWeights::default();
        println!("attention terms (the ranking applies these weights):");
        for t in physis_pro::agents::attention::ATTENTION_TERMS {
            match physis_pro::agents::attention::weight_of(&w, t) {
                Some(w) => println!("  {t:<9} weight {w:.2}"),
                None => println!("  {t:<9} (carried by the score when it applies)"),
            }
        }
        return Ok(());
    }

    if let Commands::Decide {
        state,
        classify,
        threshold,
    } = &cli.command
    {
        return cmd_decide(state, *classify, *threshold);
    }

    let config = PhysisProConfig::default();
    let mut app = PhysisApp::new(config.physis.clone());

    match cli.command {
        Commands::Index { .. } => unreachable!("dispatched before PhysisApp::new"),
        Commands::License { .. } => unreachable!("dispatched before PhysisApp::new"),
        Commands::Estimate { .. } => unreachable!("dispatched before PhysisApp::new"),
        #[cfg(feature = "cli")]
        Commands::System(..) => unreachable!("dispatched before PhysisApp::new"),
        Commands::Notebook { .. } => unreachable!("dispatched before PhysisApp::new"),
        Commands::Act { .. } => unreachable!("dispatched before PhysisApp::new"),
        Commands::Action { .. } => unreachable!("dispatched before PhysisApp::new"),
        Commands::Agent { .. } => unreachable!("dispatched before PhysisApp::new"),
        Commands::Settings { .. } => unreachable!("dispatched before PhysisApp::new"),
        Commands::Draw { .. } => unreachable!("dispatched before PhysisApp::new"),
        Commands::Intent { .. } => unreachable!("dispatched before PhysisApp::new"),
        Commands::Decide { .. } => unreachable!("dispatched before PhysisApp::new"),
        Commands::Attention => unreachable!("dispatched before PhysisApp::new"),
        Commands::Dashboard { .. }
        | Commands::Chat { .. }
        | Commands::Embed { .. }
        | Commands::Mcp { .. } => unreachable!("dispatched before PhysisApp::new"),
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
            for path in &results {
                println!("  {}", path.join(" -> "));
            }
            // Trie hits come from an in-process scan; a fresh process has an
            // empty trie, so also recall from the persisted node store.
            let nodes = app.run_query_nodes(&query, max_results);
            if !nodes.is_empty() {
                println!("Persisted nodes:");
                for (id, label, score) in &nodes {
                    println!("  [{score:.3}] {label} ({id})");
                }
            }
            if results.is_empty() && nodes.is_empty() {
                println!("No results found for: {query}");
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
        Commands::Studio {
            dir,
            html_out,
            demo,
        } => {
            let out = html_out.unwrap_or_else(|| dir.join("studio.html"));
            let report = app.run_studio(&dir, &out, demo)?;
            println!("{report}");
        }
        #[cfg(feature = "web")]
        Commands::StudioServer { port, model } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(physis_pro::ported::studio::run_with_model(port, model))?;
        }
        Commands::Demo { dir, demo } => {
            run_demo(&mut app, &dir, demo)?;
        }
        Commands::Stats => {
            let output = app.run_stats();
            println!("{output}");
        }
        Commands::Facet {
            kind,
            domain,
            mode,
            lifecycle,
            agency,
            scale,
            abstraction,
            sub_domain,
            sub_mode,
        } => {
            let out = app.run_facet_query(
                kind.as_deref(),
                domain.as_deref(),
                mode.as_deref(),
                lifecycle.as_deref(),
                agency.as_deref(),
                scale.as_deref(),
                abstraction.as_deref(),
                sub_domain.as_deref(),
                sub_mode.as_deref(),
            );
            print!("{out}");
        }
        Commands::Where {
            query,
            place,
            cause,
            within,
            during,
        } => {
            let out = app.run_where(
                query.as_deref(),
                place.as_deref(),
                cause.as_deref(),
                within,
                during.as_deref(),
            );
            print!("{out}");
        }
        Commands::Find {
            query,
            dir,
            mode,
            place,
            period,
            kind,
            top_k,
        } => {
            let out = app.run_find(
                &query,
                dir.as_deref(),
                mode.as_deref(),
                place.as_deref(),
                period.as_deref(),
                kind.as_deref(),
                top_k,
            );
            print!("{out}");
        }
        Commands::Config => {
            match physis_pro::config::PhysisConfig::load_effective() {
                Ok(eff) => {
                    // Redact the secret: report presence, never the value.
                    let mut cfg_v =
                        serde_json::to_value(&eff.config).unwrap_or(serde_json::Value::Null);
                    if let Some(agent) = cfg_v.get_mut("agent") {
                        if let Some(key) = agent.get_mut("jev_api_key") {
                            *key = serde_json::Value::String(
                                if key.as_str().is_some_and(|k| !k.trim().is_empty()) {
                                    "<set>".to_string()
                                } else {
                                    "<unset>".to_string()
                                },
                            );
                        }
                    }
                    let v = serde_json::json!({
                        "config": cfg_v,
                        "origins": eff.origins.iter().map(|(k, o)| (k, o.to_string())).collect::<std::collections::HashMap<_, _>>(),
                        "config_path": eff.config.data_dir.join("config.json").display().to_string(),
                    });
                    println!("{}", serde_json::to_string_pretty(&v)?);
                }
                Err(e) => {
                    eprintln!("config error: {e}");
                    std::process::exit(2);
                }
            }
        }
        Commands::Translate { text, lense } => {
            let output = app.run_translate(&text, lense.as_deref());
            println!("{output}");
        }
        Commands::Correct {
            intent,
            max_attempts,
            timeout,
        } => {
            let script_path = std::env::temp_dir().join("physis_correct.py");
            let script_content = format!(
                r#"import sys
sys.path.insert(0, "/home/gio/dev/physis-pro/physis_loop")
from physis_loop.artifact import Artifact, Intent
from physis_loop.observation import Observation, Constraints
from physis_loop.diagnosis import Diagnosis
from physis_loop.mutation import minimal_mutation, Mutation
from physis_loop.generator import PlaceholderGenerator
from physis_loop.python_environment import PythonEnvironment
from physis_loop.core import run_corrective_loop

gen = PlaceholderGenerator()
env = PythonEnvironment(timeout={timeout})

final_artifact, history = run_corrective_loop(
    "{intent}",
    gen,
    env,
    max_attempts={max_attempts},
)

import json
print(json.dumps({{
    "intent": "{intent}",
    "max_attempts": {max_attempts},
    "timeout": {timeout},
    "attempts": history.current_attempt(),
    "source": final_artifact.data.get("source_code", "") if final_artifact else None,
    "last_diagnosis": {{
        "violated_constraint": last_diag.violated_constraint if last_diag else None,
        "probable_cause": last_diag.probable_cause if last_diag else None,
    }} if last_diag else None,
    "history": history.current_attempt(),
}}))
"#
            );
            std::fs::write(&script_path, &script_content).expect("failed to write python script");
            let output = std::process::Command::new("python3")
                .arg(script_path.to_str().unwrap())
                .output()
                .expect("failed to run python3");
            print!("{}", String::from_utf8_lossy(&output.stdout));
            if !output.stderr.is_empty() {
                eprintln!("Python stderr: {}", String::from_utf8_lossy(&output.stderr));
            }
            let _ = std::fs::remove_file(&script_path);
        }
        Commands::Classify { file } => {
            let output = app.run_classify(&file)?;
            println!("{output}");
        }
        Commands::Connect { source } => match source {
            ConnectSource::Moonraker {
                url,
                machine_id,
                interval,
                once,
                post_to,
                user,
                state_file,
            } => {
                let opts = physis_pro::moonraker_connect::ConnectOptions {
                    url,
                    machine_id,
                    interval_secs: interval,
                    once,
                    post_to,
                    user,
                    token: std::env::var("PHYSIS_API_TOKEN")
                        .ok()
                        .filter(|t| !t.is_empty()),
                    state_file,
                };
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(physis_pro::moonraker_connect::run(opts))?;
            }
            ConnectSource::Adapter {
                protocol,
                endpoint,
                machine_id,
                topic,
                post_to,
                user,
                token,
                interval,
                once,
                temp_max,
            } => {
                let mut thresholds = std::collections::HashMap::new();
                if let Some(t) = temp_max {
                    thresholds.insert("temp_max".to_string(), t);
                }
                let opts = physis_pro::industrial::AdapterRunOptions {
                    protocol,
                    endpoint,
                    machine_id,
                    topic,
                    post_to,
                    user,
                    token: token.or_else(|| {
                        std::env::var("PHYSIS_API_TOKEN")
                            .ok()
                            .filter(|t| !t.is_empty())
                    }),
                    interval_secs: interval,
                    once,
                    thresholds,
                    modbus_map: None,
                };
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(physis_pro::industrial::run_adapter(opts))?;
            }
        },
        Commands::Timeline { input, json } => {
            let raw = std::fs::read_to_string(&input)?;
            let events: Vec<physis_pro::operational::OperationalEvent> =
                serde_json::from_str(&raw)?;
            let timeline =
                physis_pro::operational_report::OperationalTimeline::from_events(&events);
            if json {
                println!("{}", serde_json::to_string_pretty(&timeline)?);
            } else {
                println!("{}", timeline.render_markdown());
            }
        }
        // ── Semiotic node editing ("exchange parts") ──
        Commands::NodeEdit {
            id,
            label,
            domain,
            mode,
            clear_pin,
        } => {
            let pin = match (domain, mode, clear_pin) {
                (Some(d), Some(m), false) => physis_pro::core::PinEdit::Set(d, m),
                (None, None, false) => physis_pro::core::PinEdit::Keep,
                (None, None, true) => physis_pro::core::PinEdit::Clear,
                _ => anyhow::bail!(
                    "--clear-pin excludes --domain/--mode; supply one pin intent only"
                ),
            };
            println!("{}", app.run_node_edit(&id, &label, pin)?);
        }
        Commands::NodeDelete { id } => {
            println!("{}", app.run_node_delete(&id)?);
        }
        Commands::NodeSearch { query, budget, top } => {
            print!("{}", app.run_node_search(&query, budget, top)?);
        }
        Commands::Note { text, verdict } => {
            println!("{}", app.run_note(&text, verdict)?);
        }
        Commands::Vault { dir, git } => {
            println!("{}", app.run_vault_import(&dir, git)?);
        }
        Commands::History { file } => {
            println!("{}", app.run_history_import(&file)?);
        }
        Commands::Onboard { max_sessions } => {
            println!("{}", app.run_onboard(max_sessions)?);
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
            let _ = tracker.report_failure(&feedback, &app.cell_centroids, correct.as_deref());
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
            if config.industrial_endpoints.is_empty() {
                println!("Process monitoring: no industrial endpoints configured.");
                println!("  Add MQTT/Modbus endpoints to config.industrial_endpoints.");
            } else {
                println!(
                    "Process monitoring — checking {} endpoint(s):",
                    config.industrial_endpoints.len()
                );
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(async {
                    for ep in &config.industrial_endpoints {
                        let clean = ep
                            .trim_start_matches("mqtt://")
                            .trim_start_matches("tcp://")
                            .trim_start_matches("modbus://");
                        let host = clean.split(':').next().unwrap_or(clean);
                        let port: u16 = clean
                            .split(':')
                            .nth(1)
                            .and_then(|p| p.parse().ok())
                            .unwrap_or(1883);
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
        Commands::Inventory => match sled::open(&config.inventory_db) {
            Ok(db) => {
                let count = db.iter().count();
                println!("Inventory database: {count} item(s)");
                println!("  Path: {}", config.inventory_db.display());
                println!("  Order mode: {}", config.order_mode);
                println!("  Order cooldown: {} hours", config.order_cooldown_hours);
                if count > 0 {
                    println!("\n  Items (first 20):");
                    let internal = [
                        "inventory_items",
                        "consumption",
                        "last_order_at",
                        "order_log",
                    ];
                    for result in db.iter().take(20) {
                        match result {
                            Ok((key, value)) => {
                                let k = String::from_utf8_lossy(&key);
                                if internal.contains(&k.as_ref()) {
                                    continue; // bookkeeping, not stock
                                }
                                let v = String::from_utf8_lossy(&value);
                                println!("    {k}: {v}");
                            }
                            Err(e) => eprintln!("    Error reading item: {e}"),
                        }
                    }
                }
                println!("\n  Use 'physis-pro order <item> <qty>' to suggest a restock.");
                // QA 8.4: show the persisted order log, not just the item count.
                if let Ok(tracker) = InventoryTracker::load(&db, config.order_cooldown_hours) {
                    if !tracker.order_log.is_empty() {
                        println!("\n  Order log (last 10):");
                        for s in tracker.order_log.iter().rev().take(10) {
                            println!(
                                "    [{}] {} x {} — {}",
                                s.created_at.format("%Y-%m-%d %H:%M"),
                                s.sku,
                                s.suggested_qty,
                                s.reason
                            );
                        }
                    }
                }
            }
            Err(e) => {
                println!(
                    "Could not open inventory database at {}.",
                    config.inventory_db.display()
                );
                println!("  Error: {e}");
                println!("  Run 'physis-pro ingest' to initialize it.");
            }
        },
        Commands::Order { item, qty } => {
            // QA 8.4: this used to print and vanish. Now it goes through the
            // InventoryTracker (cooldown + order log) and lands in sled.
            match sled::open(&config.inventory_db) {
                Ok(db) => {
                    let mut tracker = InventoryTracker::load(&db, config.order_cooldown_hours)
                        .unwrap_or_else(|_| InventoryTracker::new(config.order_cooldown_hours));
                    match tracker.place_manual_order(&item, qty as i32) {
                        Ok(s) => {
                            tracker.save(&db)?;
                            println!(
                                "Order logged: {} x {} ({}) — requires manual approval (order_mode={})",
                                s.sku,
                                s.suggested_qty,
                                s.reason,
                                config.order_mode
                            );
                            if s.forecast_daily_usage > 0.0 {
                                println!("  30-day forecast: {:.1}/day", s.forecast_daily_usage);
                            }
                            println!("  See 'physis-pro inventory' for the order history.");
                        }
                        Err(remaining) => {
                            let hrs = remaining.num_hours();
                            let mins = remaining.num_minutes() % 60;
                            println!(
                                "Order suppressed: {item} was ordered {hrs}h{mins}m ago \
                                 (cooldown {}h). Nothing recorded.",
                                config.order_cooldown_hours
                            );
                        }
                    }
                }
                Err(e) => {
                    println!(
                        "Could not open inventory database at {}.",
                        config.inventory_db.display()
                    );
                    println!("  Error: {e}");
                }
            }
        }
        Commands::Wiki { query } => {
            let wiki_path = config
                .wiki_path
                .unwrap_or_else(|| config.physis.data_dir.join("wiki"));
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
                    eprintln!(
                        "No theme graph found at {}. Run 'physis-pro ingest' first.",
                        graph_path.display()
                    );
                    eprintln!("Error: {e}");
                    return Ok(());
                }
            };
            browse_graph_interactive(&graph, dir.as_deref())?;
        }
        Commands::RebuildGraph => {
            run_ingest_pipeline(&config, "Rebuilding graph from", false)?;
        }
        Commands::Discover {
            dir,
            json_out,
            threshold,
            ai,
        } => {
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
            let wiki_dir = config
                .wiki_path
                .unwrap_or_else(|| config.physis.data_dir.join("wiki"));
            let graph = match FileThemeGraph::load(&graph_path) {
                Ok(g) => g,
                Err(e) => {
                    eprintln!(
                        "No theme graph found at {}. Run 'physis-pro ingest' first.",
                        graph_path.display()
                    );
                    eprintln!("Error: {e}");
                    return Ok(());
                }
            };
            graph.export_to_wiki(&wiki_dir)?;
            println!("Wiki pages exported to {}", wiki_dir.display());
            println!(
                "  {} categories, {} themes, {} files",
                graph.list_categories().len(),
                graph.themes.len(),
                graph.files.len()
            );
            println!("  Index: {}/index.md", wiki_dir.display());

            // Also generate an uberwiki index file listing all themes and files
            let uber_path = config.physis.data_dir.join("uberwiki_index.md");
            let mut uber = String::new();
            uber.push_str("# Physis Uberwiki Index\n\n");
            uber.push_str("Generated from the File Theme Graph.\n\n");
            uber.push_str("## Quick Links\n\n");
            uber.push_str("- [[theme_graph_index|File Theme Graph]]\n");
            uber.push_str(&format!(
                "- [[wiki|LLM Wiki]] — {}/index.md\n\n",
                wiki_dir.display()
            ));
            uber.push_str("## By Theme\n\n");
            for theme in &graph.themes {
                let safe = theme.name.to_lowercase().replace(' ', "_");
                uber.push_str(&format!(
                    "- [[{}|{}]]  [{}×{}]\n",
                    safe, theme.name, theme.domain, theme.mode
                ));
            }
            uber.push_str("\n## By File\n\n");
            for file in &graph.files {
                let safe = file.title.to_lowercase().replace(' ', "_");
                uber.push_str(&format!(
                    "- [[{}|{}]] — {}\n",
                    safe,
                    file.title,
                    file.path.display()
                ));
            }
            std::fs::write(&uber_path, &uber)?;
            println!("  Uberwiki index: {}", uber_path.display());
        }
        Commands::StructSim { a, b, threshold } => {
            let nodes_a = load_structure_nodes(&a)?;
            let nodes_b = load_structure_nodes(&b)?;
            let embedder: Box<dyn VectorEmbed> =
                Box::new(RandomProjectionEmbedder::new(config.physis.embed_dim));
            let g = physis_pro::structsim::compare_node_lists(
                &nodes_a,
                &nodes_b,
                embedder.as_ref(),
                threshold,
            );
            println!(
                "=== Structure Similarity: {} vs {} ===",
                a.display(),
                b.display()
            );
            println!(
                "  nodes A: {} · nodes B: {}",
                g.nodes_a.len(),
                g.nodes_b.len()
            );
            println!("  overall similarity: {:.3}", g.similarity);
            println!("  matched fields (≥ {:.2}): {}", threshold, g.matches.len());
            for m in &g.matches {
                println!("    {}  ↔  {}   ({:.3})", m.a_path, m.b_path, m.score);
            }
            println!(
                "  missing in B (fields A has, B lacks/renamed): {}",
                g.missing_in_b.len()
            );
            for p in &g.missing_in_b {
                println!("    - {p}");
            }
            println!(
                "  extra in B (fields B has, A lacks): {}",
                g.extra_in_b.len()
            );
            for p in &g.extra_in_b {
                println!("    + {p}");
            }
        }
        Commands::StructSearch {
            dir,
            query,
            threshold,
            max,
        } => {
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
            println!(
                "=== Structure Search: {} ranked by similarity ===",
                query.display()
            );
            for (path, sim) in ranked.iter().take(max) {
                println!("  {sim:.3}  {path}");
            }
            if ranked.is_empty() {
                println!("  (no parseable structures found under {})", dir.display());
            }
        }
        Commands::Rag {
            dir,
            query,
            tokens,
            top_k,
        } => {
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
            println!(
                "  corpus: {} docs · budget: {tokens} tokens · top_k: {top_k}",
                docs.len()
            );
            println!(
                "  selected: {} chunks · total {} tokens · truncated: {}",
                result.chunks.len(),
                result.total_tokens,
                result.truncated
            );
            for c in &result.chunks {
                println!("    [{:.3}] ({} tokens) doc#{}", c.score, c.tokens, c.id);
            }
            println!("\n--- assembled context (≤ {tokens} tokens) ---\n");
            println!("{context}");
        }
        Commands::Machine { name, .. } => {
            // Only the `--reduce` path reaches here; the stateless dispatch
            // returned before the app was built.
            let input = machine_reduce
                .take()
                .expect("stateless machine dispatch returns before app init");
            let out = app.run_machine_reduce(&name, &input)?;
            print!("{out}");
        }
        Commands::Study { .. } => unreachable!("study subcommand handled before app init"),
        Commands::Workflow { .. } => unreachable!("workflow subcommand handled before app init"),
        Commands::DecisionsMine {
            files,
            adapter,
            list,
            threshold,
            register,
        } => {
            if list || files.is_empty() {
                println!("Registered transcript adapters:");
                for n in physis_pro::sessions::registered() {
                    println!("  {n}");
                }
                if list {
                    return Ok(());
                }
                println!("\nUsage: physis-pro decisions-mine <files...> [--adapter NAME]");
                return Ok(());
            }
            let adapter_name = adapter.clone().unwrap_or_else(|| {
                physis_pro::sessions::registered()
                    .first()
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            });
            let adap =
                physis_pro::sessions::get_adapter(&adapter_name).map_err(|e| anyhow::anyhow!(e))?;

            // Read every transcript line through the adapter → events.
            use std::io::BufRead;
            let mut events: Vec<physis_pro::sessions::Event> = Vec::new();
            for f in &files {
                let reader: Box<dyn std::io::BufRead> = if f.as_os_str() == "-" {
                    Box::new(std::io::BufReader::new(std::io::stdin()))
                } else {
                    match std::fs::File::open(f) {
                        Ok(file) => Box::new(std::io::BufReader::new(file)),
                        Err(e) => {
                            eprintln!("cannot open transcript {}: {e}", f.display());
                            continue;
                        }
                    }
                };
                for line in reader.lines() {
                    let line = line?;
                    // Substring prefilter before JSON parse (P2 pattern).
                    if !line.contains('"') && !line.contains("assistant") && !line.contains("tool")
                    {
                        continue;
                    }
                    if let Some(mut evs) = adap.normalize(&line) {
                        events.append(&mut evs);
                    }
                }
            }

            // Mine → gate → title dedup (pure), then embedding dedup.
            let mut decisions = physis_pro::decisions::extract(&events);
            let embedder: Box<dyn physis_pro::embed::VectorEmbed> = Box::new(
                physis_pro::embed::RandomProjectionEmbedder::new(config.physis.embed_dim),
            );
            decisions = physis_pro::decisions::dedup_by_embedding(
                &decisions,
                &[],
                embedder.as_ref(),
                threshold,
            );

            println!(
                "mined {} decision(s) from {} event(s) via {adapter_name}",
                decisions.len(),
                events.len()
            );
            for c in &decisions {
                println!(
                    "{}",
                    serde_json::json!({
                        "decision": c.decision,
                        "quote": c.source_quote,
                        "verification": c.verification.as_str(),
                    })
                );
            }

            // Optionally register as labeled core nodes under `decision:` prefix.
            if register && !decisions.is_empty() {
                for c in &decisions {
                    let label = format!("decision: {}", c.decision);
                    match app.register_persisted_node(&label) {
                        Ok(id) => println!("  registered {id} ← {label}"),
                        Err(e) => eprintln!("warning: registration failed: {e}"),
                    }
                }
                println!(
                    "registered {} node(s) with 'decision:' label prefix",
                    decisions.len()
                );
            }
            return Ok(());
        }
        Commands::Themes { min_size, json } => {
            print!("{}", app.run_themes(min_size, json));
        }
        Commands::Benchmark { dataset } => match dataset {
            BenchmarkDataset::Secom {
                limit,
                seed,
                train_ratio,
                output,
                no_feedback,
                embedder,
            } => {
                let cfg = BenchmarkConfig {
                    seed,
                    train_ratio,
                    max_observations: limit,
                    embed_dim: config.physis.embed_dim,
                    output_dir: output.unwrap_or_else(|| PathBuf::from("benchmark-results/secom")),
                    no_feedback,
                    embedder,
                };
                let run = secom::run_benchmark(cfg)?;
                println!("{}", serde_json::to_string_pretty(&run)?);
                if !no_feedback {
                    println!(
                        "\nReport written to {}/latest.md",
                        run.config.output_dir.display()
                    );
                }
            }
        },
        Commands::Pack { .. } => unreachable!("pack is dispatched early"),
        Commands::Warm { .. } => unreachable!("warm is dispatched early"),
        Commands::Doctor => {
            run_doctor(&config)?;
        }
        // PH-004: the packet gate runs this on CI. Dispatched early like
        // `pack`/`warm`: it needs the cascade-selected embedder, not the
        // ontology classifier, so it must not pay the ~650-entry startup pass.
        Commands::Selftest => {
            return run_selftest(&config);
        }
    }

    Ok(())
}

fn cmd_settings(command: &SettingsCommand) -> anyhow::Result<()> {
    use physis_pro::settings::Settings;
    match command {
        SettingsCommand::Show => print!("{}", Settings::load().render()),
        SettingsCommand::Path => println!("{}", Settings::user_path().display()),
        SettingsCommand::Set { key, value } => {
            let path = Settings::user_path();
            // Start from the user's own file so a `set` never drops the rest of
            // their overrides. The shipped defaults are layered on load, so the
            // user file holds only what was explicitly set.
            let mut s = match std::fs::read_to_string(&path) {
                Ok(text) => Settings::parse(&text).map_err(|e| anyhow::anyhow!(e))?,
                Err(_) => Settings::default(),
            };
            s.set(key, value).map_err(|e| anyhow::anyhow!(e))?;
            s.save(&path).map_err(|e| anyhow::anyhow!(e))?;
            println!("set {key} = {value}");
            println!("written to {}", path.display());
        }
    }
    Ok(())
}

/// Read a JSONL observation log. A malformed line is skipped and counted, never
/// fatal: the log is append-only, and a torn line is a lost record rather than a
/// lost log.
fn read_observations(
    path: &std::path::Path,
) -> anyhow::Result<Vec<physis_core::observe::Observation>> {
    let text =
        std::fs::read_to_string(path).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    let mut bad = 0usize;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<physis_core::observe::Observation>(line) {
            Ok(o) => out.push(o),
            Err(_) => bad += 1,
        }
    }
    if bad > 0 {
        eprintln!("[physis] skipped {bad} unparseable observation line(s)");
    }
    Ok(out)
}

/// `decide`: decision-native classification. The grid is a coordinate system,
/// so the cell is a decision with a fixed option set — read from the endpoint's
/// probabilities, never generated and parsed back.
fn cmd_decide(state: &str, classify: bool, threshold: f64) -> anyhow::Result<()> {
    use physis_pro::agents::decision::DecisionClient;
    let client = DecisionClient::from_settings().ok_or_else(|| {
        anyhow::anyhow!(
            "no decision endpoint configured. Point one at a Jev-compatible server:\n  \
             physis-pro settings set providers.jev.endpoint http://127.0.0.1:8080\n  \
             physis-pro settings set providers.jev.key_env MY_KEY   # if it needs auth\n\
             (OpenJev/SemIf speak POST /v1/systemone; nothing is called until an endpoint is set)"
        )
    })?;
    eprintln!(
        "[physis] decision endpoint {} model {}",
        client.endpoint(),
        client.model()
    );
    if !classify {
        anyhow::bail!("nothing asked: pass --classify");
    }
    let domains = physis_pro::semiotic_grid::domains();
    let modes: Vec<String> = physis_pro::semiotic_grid::MODES
        .iter()
        .map(|m| m.to_string())
        .collect();
    let cell = client
        .classify_cell(state, &domains, &modes)
        .map_err(|e| anyhow::anyhow!(e))?;
    println!("{}", cell.render());
    println!(
        "{} (threshold {threshold})",
        if cell.accepted(threshold) {
            "accepted"
        } else {
            "low confidence — route to review"
        }
    );
    Ok(())
}

/// `intent`: the operator's statement of what the workspace is for, with the
/// provenance of every edit, persisted so it survives a restart.
fn cmd_intent(command: &IntentCommand) -> anyhow::Result<()> {
    use physis_pro::intent::IntentStore;
    let root = std::env::current_dir()?;
    match command {
        IntentCommand::Show => print!("{}", IntentStore::load(&root).render()),
        IntentCommand::History { id } => {
            let store = IntentStore::load(&root);
            match store.get(id) {
                Some(i) => {
                    println!("{}\n  {}\n", i.id, i.statement);
                    for e in &i.history {
                        println!(
                            "  {} by {} — {}{}",
                            e.at.to_rfc3339(),
                            e.by,
                            e.to,
                            if e.reason.is_empty() {
                                String::new()
                            } else {
                                format!(" ({})", e.reason)
                            }
                        );
                    }
                }
                None => anyhow::bail!("no such intent: {id}"),
            }
        }
        IntentCommand::Set {
            id,
            statement,
            by,
            reason,
        } => {
            let text = statement.join(" ");
            if text.trim().is_empty() {
                anyhow::bail!("an intent statement must not be empty");
            }
            let mut store = IntentStore::load(&root);
            store.set(id, &text, by, reason);
            store.save(&root).map_err(|e| anyhow::anyhow!(e))?;
            println!("intent {id} = {text}");
            println!("written to {}", IntentStore::path(&root).display());
        }
    }
    Ok(())
}

/// `draw`: everything the HUD shows, as text. Every section is produced by the
/// same renderer the TUI calls, so the two frontends cannot drift.
fn cmd_draw(
    root: Option<&std::path::Path>,
    log: Option<&std::path::Path>,
    width: usize,
) -> anyhow::Result<()> {
    use physis_pro::agents::{self, tasks::TaskGraph, view};
    use physis_pro::draw::{render, Section};
    use physis_pro::settings::Settings;

    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or(std::env::current_dir()?);
    let roster = agents::default_roster();

    // AGENTS: the same strip the HUD draws, plus one line per agent.
    let mut agents_body = view::render_strip(&roster);
    agents_body.push('\n');
    for a in &roster.agents {
        let s = agents::status(a);
        let dot = if s.available { "●" } else { "○" };
        agents_body.push_str(&format!(
            "  {dot} {:<10} {:<8} {:<28} {}\n",
            a.id,
            a.state.label(),
            s.transport,
            s.reason.as_deref().unwrap_or("")
        ));
    }

    // ACTIONS: capabilities and registered objects from the nav index.
    let actions_body = match physis_pro::registry::Registry::from_workspace(&root) {
        Ok(reg) => physis_pro::action::render_list(&reg),
        Err(e) => format!("registry unavailable: {e}"),
    };

    // TASKS: the graph folded from the observation log.
    let log_path = log.map(|p| p.to_path_buf()).unwrap_or_else(|| {
        root.join(
            std::env::var("PHYSIS_OBSERVATIONS")
                .unwrap_or_else(|_| ".physis/observations.jsonl".into()),
        )
    });
    let tasks_body = match read_observations(&log_path) {
        Ok(obs) => {
            let g = TaskGraph::fold(&obs);
            format!(
                "{} observations → {} tasks\n{}",
                obs.len(),
                g.tasks.len(),
                g.render()
            )
        }
        Err(e) => format!("no task log at {}: {e}", log_path.display()),
    };

    // ROUTES: the persisted table, if any.
    let routes_path = root.join(".physis/routes.json");
    let routes = physis_pro::routes::RouteTable::load(&routes_path);
    let mut routes_body = String::new();
    for r in routes.all() {
        routes_body.push_str(&format!(
            "  {:<20} {:<18} -> {}\n",
            r.name, r.cell, r.action
        ));
    }

    let sections = vec![
        Section::new(
            "intent",
            physis_pro::intent::IntentStore::load(&root).render(),
        ),
        Section::new("agents", agents_body),
        Section::new("actions", actions_body),
        Section::new("tasks", tasks_body),
        Section::new("routes", routes_body),
        Section::new("settings", Settings::load().render()),
    ];
    let title = format!("physis — {}", root.display());
    print!("{}", render(&title, &sections, width));
    Ok(())
}

fn cmd_agent(command: &AgentCommand) -> anyhow::Result<()> {
    use physis_pro::agents::{self, adapter, routing};
    let roster = agents::default_roster();
    match command {
        AgentCommand::List => {
            for a in &roster.agents {
                let s = agents::status(a);
                let dot = if s.available { "●" } else { "○" };
                println!(
                    "{dot} {:<10} {:<8} {:<28} {}",
                    a.id,
                    a.state.label(),
                    s.transport,
                    s.reason.as_deref().unwrap_or("")
                );
            }
        }
        AgentCommand::Show { id } => match roster.get(id) {
            Some(a) => println!("{}", agents::view::render_agent_card(a, None)),
            None => anyhow::bail!("no such agent: @{id}"),
        },
        AgentCommand::Call { id, prompt } => {
            let agent = roster
                .get(id)
                .ok_or_else(|| anyhow::anyhow!("no such agent: @{id}"))?;
            let adapter = adapter::adapter_for(agent).map_err(|e| anyhow::anyhow!("{e}"))?;
            match adapter.invoke(&adapter::Invocation::new(prompt.join(" "))) {
                Ok(out) => {
                    println!("{}", out.text);
                    if let Some(s) = out.structured {
                        if !s.is_empty() {
                            println!("{}", serde_json::to_string_pretty(&s)?);
                        }
                    }
                }
                Err(adapter::AdapterError::Schema { detail, reply }) => {
                    // A schema miss is information: show the reply, then say why
                    // it did not parse. Discarding the text loses the answer.
                    println!("{reply}");
                    anyhow::bail!("schema-invalid reply: {detail}");
                }
                Err(e) => return Err(anyhow::anyhow!("{e}")),
            }
        }
        AgentCommand::Ask {
            question,
            budget,
            max_files,
            pack,
        } => {
            // Ground the question in the workspace: either a pack saved earlier
            // (fast — no embedding) or one assembled now. Packing the most
            // relevant source first lets the agent answer from evidence instead
            // of exploring the tree on its own (measured: a bare `opencode run`
            // turn is ~43 s; a repo-exploring question ran past three minutes).
            let context = if let Some(path) = pack {
                let text = std::fs::read_to_string(path)
                    .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
                let saved: physis_pro::pack::PackResult = serde_json::from_str(&text)
                    .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
                eprintln!(
                    "[physis] loaded saved pack {} ({} tokens, {} chunks)",
                    path.display(),
                    saved.tokens,
                    saved.chunks.len()
                );
                saved.context
            } else {
                // The file bound keeps a cold cache from running for minutes.
                let config = PhysisProConfig::default();
                let root = std::env::current_dir()?;
                let (embedder, kind) = physis_pro::embed::select_embedder(&config.physis);
                eprintln!("[physis] jev ask embedder: {kind}");
                let mut cache = physis_pro::pack::EmbedCache::load(&config.physis.data_dir);
                let limit = if *max_files == 0 {
                    None
                } else {
                    Some(*max_files)
                };
                let packed = physis_pro::pack::pack_cached_limited(
                    &root,
                    question,
                    *budget,
                    12,
                    0.3,
                    embedder.as_ref(),
                    kind,
                    &mut cache,
                    limit,
                );
                cache.save(&config.physis.data_dir);
                eprintln!(
                    "[physis] packed {} tokens from {} chunks ({} files scanned, limit {:?})",
                    packed.tokens,
                    packed.chunks.len(),
                    packed.files_scanned,
                    limit
                );
                packed.context
            };

            let prompt = format!(
                "Answer using only the workspace context below. Cite paths as path:line.\n\n\
                 --- context ---\n{context}\n--- end context ---\n\nQuestion: {question}"
            );
            let agent = roster
                .get("jev")
                .ok_or_else(|| anyhow::anyhow!("no `jev` in the roster"))?;
            let adapter = adapter::adapter_for(agent).map_err(|e| anyhow::anyhow!("{e}"))?;
            // Measured 2026-09-19: the free Zen tier has a ~43 s floor for a
            // bare turn and a repo-exploring question ran 3 m 10 s. Attaching to
            // a warm `opencode serve` was measured slower (56 s), so it is not
            // used. The deadline is generous because killing a useful answer is
            // worse than waiting.
            let out = adapter
                .invoke(&adapter::Invocation::new(prompt).with_timeout(600))
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{}", out.text);
            if let Some(s) = out.structured {
                if !s.is_empty() {
                    println!("{}", serde_json::to_string_pretty(&s)?);
                }
            }
        }
        AgentCommand::Tasks { log } => {
            let path = log
                .clone()
                .or_else(|| {
                    std::env::var("PHYSIS_OBSERVATIONS")
                        .ok()
                        .filter(|p| !p.is_empty())
                        .map(PathBuf::from)
                })
                .unwrap_or_else(|| PathBuf::from(".physis/observations.jsonl"));
            let observations = read_observations(&path)?;
            let graph = physis_pro::agents::tasks::TaskGraph::fold(&observations);
            eprintln!(
                "[physis] folded {} observations from {} into {} tasks",
                observations.len(),
                path.display(),
                graph.tasks.len()
            );
            print!("{}", graph.render());
        }
        AgentCommand::Route {
            verb,
            object_kind,
            agent,
            domain,
            mode,
        } => {
            let caps = physis_pro::capability::default_capabilities();
            let table = physis_pro::routes::RouteTable::new();
            let mut req = routing::RouteRequest::new(verb, object_kind);
            if let Some(a) = agent {
                req = req.with_agent(a);
            }
            if let Some(d) = domain {
                req = req.with_region(d, mode.as_deref());
            }
            println!("{}", routing::choose(&roster, &caps, &table, &req).render());
        }
    }
    Ok(())
}

fn cmd_action(command: &ActionCommand) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let mut reg = physis_pro::registry::Registry::from_workspace(&root)?;
    match command {
        ActionCommand::List => print!("{}", physis_pro::action::render_list(&reg)),
        ActionCommand::Show { capability } => {
            println!(
                "{}",
                physis_pro::action::render_capability(&reg, capability)
            );
        }
        ActionCommand::Call { line } => {
            let text = line.join(" ");
            let (_result, rendered) =
                physis_pro::action::dispatch(&mut reg, &text).map_err(|e| anyhow::anyhow!(e))?;
            println!("{rendered}");
        }
    }
    Ok(())
}

fn cmd_act(command: &str, dry_run: bool, top: usize, json: bool) -> anyhow::Result<()> {
    let (embedder, embedder_kind) = physis_core::embed::select(384);
    let core = load_core();

    const SEMANTIC_FLOOR: f32 = 0.66;
    let selection = if embedder_kind == "random-projection" {
        Selection::CascadeRerank { pool: top.max(5) }
    } else {
        Selection::CascadeFloor {
            pool: top.max(5),
            floor: SEMANTIC_FLOOR,
        }
    };
    let bearing = bearing_on_with(&core, command, embedder.as_ref(), top, selection);

    let floor = if embedder_kind == "random-projection" {
        "\n  (random-projection: a lexical hash, not meaning. `act-recall` measures\n   this leg at recall 0.000 for a paraphrased command. Set PHYSIS_MODEL_DIR.)"
    } else {
        ""
    };

    if dry_run {
        let warnings: Vec<_> = bearing.iter().filter(|b| b.is_warning()).collect();
        if warnings.is_empty() {
            println!(
                "nothing already established bears on this. {} claim(s) checked.{floor}",
                core.hypotheses.len()
            );
        } else {
            println!("── ALREADY ESTABLISHED ──");
            for b in warnings {
                println!(
                    "  [{}] {:<13} rel {:.3}\n      {}",
                    b.id, b.status, b.relevance, b.statement
                );
            }
        }
        println!("\n(not run — drop --dry-run to execute)");
        return Ok(());
    }

    physis_core::store::ensure_data_dir()?;
    let a = act_run(command, bearing, &physis_core::observe::log_path())?;
    if json {
        println!("{}", serde_json::to_string_pretty(&a)?);
    } else {
        print!("{}", a.render());
    }
    if a.exit_code != Some(0) {
        std::process::exit(a.exit_code.unwrap_or(1));
    }
    Ok(())
}

fn load_core() -> physis_core::core::PhysisCore {
    let json = physis_core::store::read_optional(&physis_core::store::nodes_path());
    if json.trim().is_empty() {
        physis_core::core::PhysisCore::new()
    } else {
        physis_core::core::PhysisCore::from_json(&json)
            .unwrap_or_else(|_| physis_core::core::PhysisCore::new())
    }
}

fn run_selftest(config: &PhysisProConfig) -> anyhow::Result<()> {
    // PH-004: the packet gate runs `physis-pro selftest` on CI. This reports on
    // the cascade-selected embedder — the SAME cascade the server and CLI use —
    // per the frozen probe contract in packets/reports/PH-004-design.md.
    use physis_pro::embed::VectorEmbed as _;
    let (embedder, kind) = physis_pro::embed::select_embedder(&config.physis);
    println!("embedder: {kind} (dim={})", embedder.dimension());
    let finite = |v: &[f32]| !v.is_empty() && v.iter().all(|x| x.is_finite());
    let mut all_pass = true;
    for (a_text, b_text, d_text) in physis_pro::embed::SEMANTIC_PROBES {
        let a = embedder.embed(a_text);
        let b = embedder.embed(b_text);
        let d = embedder.embed(d_text);
        let (related, unrelated, ok) = if finite(&a) && finite(&b) && finite(&d) {
            let r = physis_pro::models::cosine_sim(&a, &b);
            let u = physis_pro::models::cosine_sim(&a, &d);
            (r, u, r > u + physis_pro::embed::SEMANTIC_PROBE_MARGIN)
        } else {
            (f32::NAN, f32::NAN, false)
        };
        println!("  probe {a_text:?} ~ {b_text:?} vs {d_text:?}: related={related:.3} unrelated={unrelated:.3} {}", if ok { "PASS" } else { "FAIL" });
        all_pass &= ok;
    }
    if all_pass {
        println!("semantic self-test: PASS");
        Ok(())
    } else {
        eprintln!("semantic self-test: FAIL");
        std::process::exit(1);
    }
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
        .unwrap_or_else(|_| {
            data_dir
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        });
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

    // 3. Config file — effective settings with per-field provenance.
    let config_path = data_dir.join("config.json");
    println!("\n⚙️  Config: {}", config_path.display());
    match physis_pro::config::PhysisConfig::load_effective() {
        Ok(eff) => {
            println!("  ✅ Effective settings loaded");
            for (k, o) in &eff.origins {
                if *o != physis_pro::config::Origin::Builtin {
                    println!("    {k}: {o}");
                }
            }
            // JEV presence only — the key value never prints.
            let j = &eff.config.agent;
            if j.jev_enabled && j.jev_api_key.as_ref().is_some_and(|k| !k.trim().is_empty()) {
                println!("    jev: enabled · model {} · has-key", j.jev_model);
            } else if j.jev_enabled {
                println!("    jev: enabled but no key — cascade unchanged");
            }
        }
        Err(e) => {
            println!("  ❌ {e}");
            issues += 1;
        }
    }

    // 4. Embedder
    println!("\n🧠 Embedder: {}", config.physis.embed_dim);
    let (embedder, kind) = physis_pro::embed::select_embedder(&config.physis);
    println!("  ✅ Loaded: {kind}");
    let _ = embedder.embed("doctor test");
    println!("  ✅ Embed test passed");

    // 4b. ONNX models
    println!("\n🤖 ONNX Models:");
    let model_dir = PathBuf::from("./models");
    let mut model_ok = true;
    let mut model_warn = String::new();

    let check_model = |name: &str, path: PathBuf| -> bool {
        if path.exists() {
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            println!("  ✅ {} ({} bytes)", name, size);
            true
        } else {
            println!("  ❌ {} — missing", name);
            false
        }
    };

    let generic_model = check_model("model.onnx", model_dir.join("model.onnx"));
    let generic_tok = check_model("tokenizer.json", model_dir.join("tokenizer.json"));
    let bge_model = check_model(
        "bge-base/model.onnx",
        model_dir.join("bge-base-en-v1.5/onnx/model.onnx"),
    );
    let bge_tok = check_model(
        "bge-base/tokenizer.json",
        model_dir.join("bge-base-en-v1.5/tokenizer.json"),
    );

    if !generic_model && !bge_model {
        model_ok = false;
        model_warn.push_str("No ONNX embedding model found. Run: ./scripts/download_models.sh\n");
    }
    if !generic_tok && !bge_tok {
        model_ok = false;
        model_warn.push_str("No tokenizer found. Run: ./scripts/download_models.sh\n");
    }

    if !model_ok {
        println!("  ⚠️  {}", model_warn.trim());
        issues += 1;
    }

    // 5. Ontology
    println!("\n📚 Ontology:");
    let ont = OntologyLoader::load_all(&config.physis);
    let total = ont.human_domains.len() + ont.machine_domains.len() + ont.custom_domains.len();
    println!(
        "  ✅ Loaded: {} domains ({} human + {} machine + {} custom)",
        total,
        ont.human_domains.len(),
        ont.machine_domains.len(),
        ont.custom_domains.len()
    );

    // 6. License
    println!("\n🎫 License:");
    match physis_pro::license::load() {
        Ok(lic) => {
            println!(
                "  ✅ Valid: {} → {} (seats: {})",
                lic.org, lic.exp, lic.seats
            );
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

/// Phase 17 U5 — the 10-minute demo: shift report + ontology studio, one run.
/// Reads every `*.csv` of shift records in the folder, renders `report.html` +
/// `report.md`, then runs the studio (`studio.html`) over the same folder, and
/// prints what to show a prospect first.
const DEMO_SHIFTS_CSV: &str = "\
machine_id,shift_length_secs,ideal_cycle_time_secs,total_count,good_count,downtime_secs,stop_reason,recorded_at,shift_label
CNC-01,28800,60,400,390,1800,breakdown,2026-08-24T06:00:00Z,Turno 1
CNC-01,28800,60,380,370,900,setup,2026-08-25T06:00:00Z,Turno 2
PRESS-07,28800,45,500,480,2700,breakdown,2026-08-24T06:00:00Z,Turno 1
PRESS-07,28800,45,520,515,600,minor,2026-08-25T06:00:00Z,Turno 2
";

fn run_demo(app: &mut PhysisApp, dir: &std::path::Path, seed: bool) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir).ok();
    if seed {
        // Seed the studio corpus idempotently (never overwrites real docs) and
        // a sample shift CSV when the folder has none.
        if physis_pro::ingest::ingest_all(&[dir.to_path_buf()]).is_empty() {
            physis_pro::demo_corpus::seed_demo_corpus(dir)?;
        }
        let shifts = dir.join("shifts.csv");
        if !shifts.exists() {
            std::fs::write(&shifts, DEMO_SHIFTS_CSV)?;
        }
    }

    // 1. Shift report from every *.csv of shift records.
    let mut shifts_seen = 0usize;
    let mut rejected = 0usize;
    for entry in std::fs::read_dir(dir)? {
        let p = entry?.path();
        if p.extension().and_then(|e| e.to_str()) != Some("csv") {
            continue;
        }
        let text = std::fs::read_to_string(&p)?;
        let (records, errors) = physis_pro::shifts::parse_shift_csv(&text);
        rejected += errors.len();
        if records.is_empty() {
            continue;
        }
        shifts_seen += records.len();
        let merged = physis_pro::shifts::aggregate(&records);
        let reports: Vec<physis_pro::oee::OeeReport> = merged.iter().map(|s| s.compute()).collect();
        let plant = (!reports.is_empty()).then(|| physis_pro::oee::plant_oee(&reports));
        let input = physis_pro::report::ShiftReportInput {
            oee: reports,
            plant,
            shift_label: Some(format!(
                "Demo — {}",
                p.file_name().unwrap_or_default().to_string_lossy()
            )),
            metadata: Some(physis_pro::report::ReportMetadata::new("demo-csv")),
            ..Default::default()
        };
        std::fs::write(
            dir.join("report.html"),
            physis_pro::report::render_shift_report_html(&input),
        )?;
        std::fs::write(
            dir.join("report.md"),
            physis_pro::report::render_shift_report_it(&input),
        )?;
    }

    if shifts_seen > 0 {
        println!(
            "shift report : {} ({} shifts, {} rows rejected)",
            dir.join("report.html").display(),
            shifts_seen,
            rejected
        );
    } else {
        println!(
            "shift report : none — no shift CSV under {} (add one, or pass --demo)",
            dir.display()
        );
    }

    // 2. Ontology studio over the same folder.
    let studio_out = dir.join("studio.html");
    println!("{}", app.run_studio(dir, &studio_out, false)?);
    Ok(())
}

/// Fast-path disk indexer (PLAN 1.3). One incremental pass, or a poll loop
/// with `--watch` that prints only what moved. Uses its own sled file next to
/// the engine's so it never contends for the web server's lock.
fn run_indexer(dir: &Path, watch: bool, interval_secs: u64) -> anyhow::Result<()> {
    // Anchor the store to the engine data dir (PHYSIS_DATA_DIR aware), not the
    // indexed tree — otherwise `physis-pro index .` catalogs its own sled file.
    let data_dir = std::env::var("PHYSIS_DATA_DIR")
        .ok()
        .filter(|p| !p.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| Path::new(".physis").to_path_buf());
    let db = sled::Config::new()
        .path(data_dir.join("index.sled"))
        .open()
        .map_err(|e| {
            anyhow::anyhow!(
                "cannot open index store {}index.sled: {e}",
                data_dir.display()
            )
        })?;

    let mut idx = physis_pro::indexer::FileIndex::load(&db, dir)?;
    let mut pass = || -> anyhow::Result<physis_pro::indexer::IndexReport> {
        let report = idx.index_once()?;
        idx.save(&db)?;
        Ok(report)
    };

    let first = pass()?;
    println!(
        "[index] {}",
        dir.canonicalize()
            .unwrap_or_else(|_| dir.to_path_buf())
            .display()
    );
    println!("[index] {first}");

    if !watch {
        return Ok(());
    }

    // Watch: quiet when nothing moved; a line per pass that changed something.
    let interval = std::time::Duration::from_secs(interval_secs.max(1));
    eprintln!(
        "[index] watching every {}s — Ctrl-C to stop",
        interval.as_secs()
    );
    loop {
        std::thread::sleep(interval);
        let report = pass()?;
        if report.new + report.changed + report.deleted > 0 {
            println!(
                "[{}] [index] {report}",
                chrono::Local::now().format("%H:%M:%S")
            );
        }
    }
}

/// Print a workflow report as human-readable markdown.
fn print_workflow_report(report: &physis_pro::workflow::WorkflowReport) {
    println!("# Workflow audit — {}", report.root);
    println!();
    println!("Total docs: {}", report.total);
    println!();
    println!("## By audience");
    for t in &report.by_audience {
        println!("- {}: {}", t.key, t.count);
    }
    if !report.by_harness.is_empty() {
        println!();
        println!("## By harness");
        for t in &report.by_harness {
            println!("- {}: {}", t.key, t.count);
        }
    }
    if !report.by_author.is_empty() {
        println!();
        println!("## By author");
        for t in &report.by_author {
            println!("- {}: {}", t.key, t.count);
        }
    }
    println!();
    println!("## Docs (newest first)");
    for d in &report.docs {
        let who = d
            .author
            .clone()
            .or_else(|| d.harness.clone())
            .unwrap_or_else(|| "?".to_string());
        let when = d.mtime.format("%Y-%m-%d %H:%M").to_string();
        println!(
            "- {}  [{} · {} · {}]",
            d.path,
            d.audience.as_str(),
            who,
            when
        );
    }
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
    println!(
        "watching {} every {interval}s → {api} (place: {place})",
        dir.display()
    );
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

fn run_ingest_pipeline(
    config: &PhysisProConfig,
    label: &str,
    sync_vfs: bool,
) -> anyhow::Result<()> {
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

    let ingested_at = chrono::Utc::now();
    for doc in &docs {
        let file_embedding = embedder.embed(&doc.body);
        // Address at capture time (Phase 10).
        let address = physis_pro::address::FileAddress::derive(
            &doc.path,
            doc.path.parent().unwrap_or(std::path::Path::new("/")),
            physis_pro::address::Mechanism::Generated,
            "ingest",
            ingested_at,
        );
        let file_id = graph.add_file(
            doc.path.clone(),
            &doc.title,
            &format!("{:?}", doc.kind),
            file_embedding.clone(),
            Some(address),
        );

        let mut theme_scores: Vec<(usize, f32)> = graph
            .themes
            .iter()
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

        let top_themes: Vec<String> = theme_scores
            .iter()
            .take(3)
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
        println!(
            "Virtual File System synchronized in {}",
            config.physis.data_dir.join("virtual").display()
        );
    }

    graph.save(&graph_path)?;
    println!("\nTheme graph saved to {}", graph_path.display());
    println!(
        "  {} themes, {} files, {} edges",
        graph.themes.len(),
        graph.files.len(),
        graph.edges.len()
    );
    Ok(())
}

/// Interactive graph browser — navigate themes by category, see files per theme.
fn browse_graph_interactive(
    graph: &FileThemeGraph,
    _filter_dir: Option<&Path>,
) -> anyhow::Result<()> {
    use std::io::{self, BufRead, Write};

    let categories = graph.list_categories();

    println!("\n━━━ Physis File Theme Graph ━━━");
    println!(
        "{} themes, {} files, {} edges\n",
        graph.themes.len(),
        graph.files.len(),
        graph.edges.len()
    );

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
                    let themes: Vec<String> = graph
                        .edges
                        .iter()
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
                    let file_count = graph
                        .edges
                        .iter()
                        .filter(|e| e.theme_id == theme.id)
                        .count();
                    let domain_mode = format!("{}×{}", theme.domain, theme.mode);
                    println!(
                        "  {}. {}  [{}]  ({} files)",
                        i + 1,
                        theme.name,
                        domain_mode,
                        file_count
                    );
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
                        println!(
                            "  Axis: {} ({})",
                            theme.axis_kind,
                            theme.category.as_deref().unwrap_or("")
                        );

                        // Show files
                        let files = graph.files_for_theme(theme.id);
                        if files.is_empty() {
                            println!("  No files linked to this theme.");
                        } else {
                            println!("\n  Files:");
                            for (fi, file) in files.iter().enumerate() {
                                println!(
                                    "    {}. {}  ({})",
                                    fi + 1,
                                    file.title,
                                    file.path.display()
                                );
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
                                                println!(
                                                    "    {}. {}  ({})",
                                                    fi + 1,
                                                    rf.title,
                                                    rf.path.display()
                                                );
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
            let penalty = tracker
                .cell_penalties
                .entry(f.top_cell.clone())
                .or_insert(0.0);
            *penalty = (*penalty + f.severity * 0.2).min(1.0);
            if let Some(ref _correct) = f.correct_domain {
                let boost = tracker.cell_boosts.entry(f.top_cell.clone()).or_insert(0.0);
                *boost += 0.1;
                if *boost > 1.0 {
                    *boost = 1.0;
                }
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
        graph.add_theme(
            "Email",
            Some("Communication"),
            "BOND",
            "WORK",
            "office",
            vec![0.9, 0.1],
        );
        graph.add_theme(
            "Meeting",
            Some("Communication"),
            "BOND",
            "CREATE",
            "office",
            vec![0.8, 0.2],
        );
        graph.add_theme(
            "Budget",
            Some("Finance"),
            "CONSTRUCT",
            "CREATE",
            "office",
            vec![0.1, 0.9],
        );
        graph.add_file(
            Path::new("msg.eml").to_path_buf(),
            "Q4 report",
            "Email",
            vec![0.85, 0.15],
            None,
        );
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
        graph.add_file(
            Path::new("invoices/2024-01.pdf").to_path_buf(),
            "Invoice Jan",
            "Invoice",
            vec![0.5; 4],
            None,
        );
        graph.add_file(
            Path::new("reports/q4.md").to_path_buf(),
            "Q4 Report",
            "Report",
            vec![0.5; 4],
            None,
        );

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
        let id = app
            .register_persisted_node("milling spindle bearing")
            .unwrap();

        use physis_pro::core::PinEdit;

        // Edit without a pin: says nothing about the cell — a pin would survive.
        let out = app
            .run_node_edit(&id, "welding robot joint recalibration", PinEdit::Keep)
            .unwrap();
        assert!(out.contains(&id), "identity survives the swap: {out}");

        // Edit with a pin.
        let pinned = app
            .run_node_edit(
                &id,
                "welding robot joint recalibration (edited)",
                PinEdit::Set("FABRICATE".into(), "MAINTAIN".into()),
            )
            .unwrap();
        assert!(
            pinned.contains("FABRICATE×MAINTAIN"),
            "pin echoed: {pinned}"
        );

        // Retract the pin explicitly: the classifier decides again.
        let cleared = app
            .run_node_edit(&id, "welding robot joint recalibration", PinEdit::Clear)
            .unwrap();
        assert!(
            !cleared.contains("FABRICATE×MAINTAIN"),
            "pin must be gone after Clear: {cleared}"
        );

        // Unknown id errors.
        assert!(app.run_node_edit("missing", "x", PinEdit::Keep).is_err());

        // Delete, then the id is gone.
        app.run_node_delete(&id).unwrap();
        assert!(
            app.run_node_delete(&id).is_err(),
            "second delete is a no-op"
        );
    }

    #[test]
    fn test_cli_node_search_bounds_the_response() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = cli_app(dir.path());

        for label in [
            "milling spindle bearing wear",
            "furnace temperature profile",
            "welding robot joint calibration",
        ] {
            app.register_persisted_node(label).unwrap();
        }

        let out = app.run_node_search("spindle bearing", 20, 12).unwrap();
        assert!(out.contains("budget 20"), "budget echoed: {out}");
        // The packed result is bounded: total tokens can't exceed the budget.
        let header = out.lines().next().unwrap();
        let tokens: usize = header
            .split("tokens")
            .next()
            .unwrap()
            .split(',')
            .nth(1)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
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
        assert!(
            out.contains("1 new"),
            "first sweep sees the new file: {out}"
        );
        assert!(out.contains("registered"), "registers nodes: {out}");

        // Idle sweep: nothing to do.
        let out2 = app.run_watch(vec![watched.clone()], true).unwrap();
        assert!(
            out2.contains("0 new"),
            "second sweep sees nothing new: {out2}"
        );

        // Content change → re-register; removal → nodes dropped.
        std::fs::write(&file, "# Topology edited\n").unwrap();
        let out3 = app.run_watch(vec![watched.clone()], true).unwrap();
        assert!(
            out3.contains("1 changed"),
            "content change detected: {out3}"
        );

        std::fs::remove_file(&file).unwrap();
        let out4 = app.run_watch(vec![watched.clone()], true).unwrap();
        assert!(out4.contains("1 deleted"), "removal detected: {out4}");

        // Nothing left in the store for the removed file's prefix.
        let out5 = app.run_node_search("Topology", 100, 10).unwrap();
        assert!(
            out5.contains("0 result(s)"),
            "removed nodes are gone: {out5}"
        );
    }

    #[test]
    fn test_cli_vault_import_registers_nodes() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        std::fs::create_dir_all(vault.join("notes")).unwrap();
        std::fs::write(
            vault.join("notes").join("alpha.md"),
            "# Alpha\n\n## Sub\n\ncontent here",
        )
        .unwrap();
        std::fs::write(
            vault.join("notes").join("beta.txt"),
            "Beta title\nplain body text",
        )
        .unwrap();
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
        std::fs::write(
            &bookmarks,
            r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p><DT><A HREF="https://shopfloor.example/">Shop floor</A></DL><p>"#,
        )
        .unwrap();
        let mut app = cli_app(dir.path());

        let out = app.run_history_import(&bookmarks).unwrap();
        assert!(out.contains("bookmarks.html"), "file named: {out}");
        assert!(out.contains("1 items"), "one bookmark: {out}");
        assert!(out.contains("registered"), "labels registered: {out}");

        // Recallable by content.
        let search = app.run_node_search("shop floor", 100, 10).unwrap();
        assert!(
            search.contains("Shop floor"),
            "bookmark recallable: {search}"
        );

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
        std::fs::write(
            &export,
            r#"{
            "notebook": [
              {"content": "shipped the release","mood": 9},
              {"content": "tired","mood": 2}
            ],
            "trackers": [{"name": "reading", "progress": 0.9}]
        }"#,
        )
        .unwrap();
        let mut app = cli_app(dir.path());

        let out = app.run_praxis_backfill(&export).unwrap();
        assert!(
            out.contains("3 items (2 success, 0 inert, 1 failure)"),
            "verdicts: {out}"
        );
        assert!(out.contains("registered"), "nodes registered: {out}");

        // Re-backfill dedupes.
        let out2 = app.run_praxis_backfill(&export).unwrap();
        assert!(out2.contains("deduped"), "re-backfill dedupes: {out2}");

        // Missing file errors.
        assert!(app
            .run_praxis_backfill(&dir.path().join("nope.json"))
            .is_err());
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
        let bogus = app.run_facet_query(
            None,
            None,
            None,
            Some("NOWHERE"),
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(
            bogus.trim_end(),
            all.trim_end(),
            "unknown facet value ignored"
        );

        // A filter that matches nothing returns a zero-count message, not an error.
        let none = app.run_facet_query(
            None,
            None,
            None,
            Some("DESIGN"),
            None,
            None,
            None,
            Some("zzz-no-such-sub"),
            None,
        );
        assert!(
            none.contains("0 entries match"),
            "empty result is a valid answer: {none}"
        );

        // Case-insensitive facet matching works when entries carry the facet.
        let operate = app.run_facet_query(
            None,
            None,
            None,
            Some("operate"),
            None,
            None,
            None,
            None,
            None,
        );
        let also = app.run_facet_query(
            None,
            None,
            None,
            Some("OPERATE"),
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(
            operate.trim_end(),
            also.trim_end(),
            "case-insensitive facet match"
        );

        // Kind restriction narrows the search space.
        let office = app.run_facet_query(
            Some("office"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(office.contains("entries match"), "office kind listed");
    }
}
