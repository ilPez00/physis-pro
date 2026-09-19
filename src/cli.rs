//! CLI application orchestrator — parses subcommands via clap and dispatches
//! to PhysisApp methods for scanning, querying, dreaming, and evaluating.

use std::collections::HashMap;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::actor::PDCActor;
use crate::config::{OntologyLoader, PhysisConfig};
use crate::dream::DreamEngine;
use crate::embed::VectorEmbed;
use crate::graph::FileThemeGraph;
use crate::ingest;
use crate::linguistic::{LinguisticLense, LinguisticRouter};
use crate::mapper::OntologyMapper;
use crate::models::{cosine_sim, DomainDef, Goal, Score};
use crate::network::NetworkScanner;
use crate::output;

#[derive(Parser)]
#[command(
    name = "physis",
    version,
    about = "Ontological mapper and PDCA dream engine"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Scan a directory and build ontological map
    Scan {
        /// Directory to scan (default: current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
        /// Output format (wiki, json, mermaid)
        #[arg(short, long, default_value = "wiki")]
        format: String,
    },
    /// Deep AI-powered scan of files and images
    DeepScan {
        /// Directory to scan (default: current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
    /// Query the ontology trie
    Query {
        /// Search query
        query: String,
        #[arg(short, long, default_value = "10")]
        max_results: usize,
    },
    /// Generate stochastic dreams
    Dream {
        /// Number of dreams to generate
        #[arg(short, long, default_value = "5")]
        count: usize,
    },
    /// Evaluate a dream
    Evaluate {
        /// Dream ID
        id: String,
        /// Grade (0.0 - 1.0)
        grade: Score,
    },
    /// Start MCP server
    #[cfg(feature = "mcp")]
    Serve,
    /// Watch directories for changes
    Watch {
        /// Directories to watch
        dirs: Vec<PathBuf>,
    },
    /// Show stats
    Stats,
    /// Print config
    Config,
    /// Translate text through a linguistic lense
    Translate {
        /// Text to translate
        text: String,
        /// Lense to apply (wenyan, piraha, sanskrit). If omitted, applies all enabled.
        #[arg(short, long)]
        lense: Option<String>,
    },
    /// Classify a text file against the semiotic grid and output a map
    Classify {
        /// Path to .txt file
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
    /// Ontology Studio — the sellable demo wedge. Drop a folder of docs in,
    /// get a self-contained HTML report + a loadable discovered_ontology.json
    /// that grows the grid itself.
    Studio {
        /// Documents directory to ingest (or seed with samples when --demo).
        dir: PathBuf,
        /// Where to write the HTML report (default: <dir>/studio.html).
        #[arg(long)]
        html_out: Option<PathBuf>,
        /// Seed a sample corpus if the dir is empty, then run (idempotent).
        #[arg(long)]
        demo: bool,
    },
    /// Print the licence on this host: SKU, org, seats, expiry, maintenance,
    /// feature list, revocation status. Serverless — reads the configured
    /// default location (PHYSIS_LICENSE env, PHYSIS_LICENSE_FILE, or
    /// .physis/license.key). `--json` for scripts and the SDK.
    License {
        /// Path to a licence file to inspect instead of the default location.
        #[arg(long)]
        path: Option<PathBuf>,
        /// Print the licence as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Run diagnostics: config, data dir, scan root, embedder, license
    Doctor,
}

/// Top-level application state: config, ontology, mapper, PDCA actor, dream engine, goals.
pub struct PhysisApp {
    /// Engine configuration.
    pub config: PhysisConfig,
    /// Loaded ontology domains.
    pub ontology: OntologyLoader,
    /// Ontological mapper with trie and vector search.
    pub mapper: OntologyMapper,
    /// PDCA cycle orchestrator.
    pub actor: PDCActor,
    /// Dream generation engine.
    pub dreams: DreamEngine,
    /// Current vector-space goals.
    pub goals: Vec<Goal>,
    /// Embedder for classify.
    embedder: Box<dyn VectorEmbed>,
    /// Embedder provenance kind (mirrors the web server's stamp so CLI edits
    /// don't get skipped by `load_nodes`'s embedder-mismatch check).
    pub embedder_kind: &'static str,
    /// Pre-computed centroids per DOMAIN\x00MODE cell.
    pub cell_centroids: HashMap<String, Vec<f32>>,
    /// Nearest-entry classifier, built once (one full-ontology embedding pass).
    /// Arc-shared process-wide (PH-009) — see `CellClassifier::build_shared`.
    pub classifier: std::sync::Arc<crate::classify::CellClassifier>,
    /// The sled handle, opened at most once per app.
    ///
    /// sled takes an exclusive file lock and releases it asynchronously on
    /// drop, so re-opening the same path once per command raced the previous
    /// handle's teardown and intermittently failed. `Db` is an `Arc` inside,
    /// so caching one and cloning it is a refcount bump, not a second open.
    db: std::sync::OnceLock<Option<sled::Db>>,
}

impl PhysisApp {
    /// Build a new PhysisApp from config — loads ontology, creates mapper, actor, dream engine.
    pub fn new(config: PhysisConfig) -> Self {
        let ontology = OntologyLoader::load_all(&config);
        let mapper = OntologyMapper::new(ontology.clone(), config.embed_dim);
        let actor = PDCActor::new(config.pdca_stagnant_threshold, config.pdca_stagnant_window);
        let dreams = DreamEngine::new();

        let (embedder, embedder_kind): (Box<dyn VectorEmbed>, &'static str) =
            crate::embed::select_embedder(&config);
        // Embedder provenance on stderr — the CLI equivalent of the web banner.
        // A silent fall-through to random projection makes every verdict noise
        // (PREMORTEM P0); at least say which embedder is answering.
        eprintln!("[physis] embedder: dim={}", embedder.dimension());
        if embedder_kind == "random-projection" {
            // QA 8.3: RP is deterministic but NON-semantic — measured behavior
            // is compressed scores (~0.88–1.00 for everything) and top cells
            // that are often wrong. Say so where it matters.
            eprintln!(
                "[physis] WARNING: random-projection embedder is non-semantic — \
                 classify scores are compressed and top cells unreliable. \
                 Install real weights: scripts/download_models.sh"
            );
        }

        // ONE full-ontology embedding pass: build the nearest-entry classifier,
        // then derive the legacy mean centroids from its entry embeddings.
        // (Previously this constructor embedded all ~650 entries for centroids
        // AND each command rebuilt the classifier — a second identical pass.)
        // Served from the shared on-disk cache — see `physis-pro warm`. This
        // constructor is what made every subcommand pay ~78s of startup.
        let mut seed_cache = crate::pack::EmbedCache::load(&config.data_dir);
        let classifier = crate::classify::CellClassifier::build_shared(
            &ontology,
            embedder.as_ref(),
            embedder_kind,
            config.embed_dim,
            &mut seed_cache,
        );
        seed_cache.save(&config.data_dir);
        let cell_centroids = classifier.cell_centroids();

        Self {
            config,
            ontology,
            mapper,
            actor,
            dreams,
            goals: Vec::new(),
            embedder,
            embedder_kind,
            cell_centroids,
            classifier,
            db: std::sync::OnceLock::new(),
        }
    }

    /// Scan a directory and return ontology output in the requested format (wiki/json/mermaid).
    pub fn run_scan(&mut self, dir: &std::path::Path, format: &str) -> String {
        let goals = self.mapper.map_filesystem(dir, None);
        self.goals = goals;
        // trie lives in mapper; dreams uses vector operations directly

        // Causal record (Phase 10 D4), the same one `POST /api/v1/scan` writes.
        // Without it a server-less install could never populate the ledger:
        // `physis-pro where` answered "the causal ledger is empty — run a scan"
        // to a user who had just run one, because only the HTTP path recorded.
        if let Some(db) = self.db() {
            record_scan_cause(&db, dir);
        }

        match format {
            "json" => output::format_json_graph(&self.mapper.trie),
            "mermaid" => output::format_mermaid_mindmap(&self.mapper.trie, "Physis Scan"),
            _ => output::format_wiki(&self.mapper.trie, &self.goals, "Physis Scan"),
        }
    }

    /// Search the ontology trie for paths matching the query tokens.
    pub fn run_query(&self, query: &str, max_results: usize) -> Vec<Vec<String>> {
        let words: Vec<&str> = query.split_whitespace().collect();
        let tids: Vec<u32> = words
            .iter()
            .filter_map(|w| self.mapper.trie.token_id(w))
            .collect();
        if tids.is_empty() {
            return vec![];
        }
        self.mapper.trie.prefix_search(&tids, 2, max_results)
    }

    /// Semantic fallback for `query`: search the persisted coherence nodes of
    /// the "_default" tenant — the same store /scan, watch mode, vault and
    /// history imports register into. The trie only holds what an in-process
    /// scan inserted, so a fresh `physis-pro query` used to answer "No results"
    /// even with a full graph on disk (QA 8.2). Empty when persistence is off.
    pub fn run_query_nodes(&self, query: &str, max_results: usize) -> Vec<(String, String, Score)> {
        let Some((_, core)) = self.open_persisted_core() else {
            return vec![];
        };
        if core.nodes.is_empty() {
            return vec![];
        }
        core.search_nodes(&self.embedder.embed(query), max_results)
    }

    /// Open the shared sled store (same file the web server uses), at most once
    /// per app. Later calls clone the cached handle instead of re-locking the
    /// file. `None` means the store is unavailable; the reason is printed once.
    fn db(&self) -> Option<sled::Db> {
        self.db
            .get_or_init(|| {
                let dir = &self.config.data_dir;
                if let Err(e) = std::fs::create_dir_all(dir) {
                    eprintln!("[physis] cannot create data dir {} ({e})", dir.display());
                    return None;
                }
                let path = dir.join("state.sled");
                match sled::open(&path) {
                    Ok(db) => Some(db),
                    Err(e) => {
                        eprintln!("[physis] cannot open {} ({e})", path.display());
                        None
                    }
                }
            })
            .clone()
    }

    /// Open the shared sled store (same file the web server uses) and rehydrate
    /// the "_default" tenant's coherence nodes, or `None` when persistence is off.
    fn open_persisted_core(&self) -> Option<(sled::Db, crate::core::PhysisCore)> {
        let db = self.db()?;
        let mut core = crate::core::PhysisCore::new();
        core.set_embedder_id(self.embedder_kind);
        let (loaded, skipped) = core.load_nodes(&db, "_default");
        if loaded > 0 || skipped > 0 {
            eprintln!("[physis] restored {loaded} nodes ({skipped} skipped: embedder mismatch)");
        }
        Some((db, core))
    }

    /// Register a labeled node into the shared store (what /scan and
    /// /coherence/register do on the web side) and return its id.
    pub fn register_persisted_node(&mut self, label: &str) -> anyhow::Result<String> {
        let Some((db, mut core)) = self.open_persisted_core() else {
            anyhow::bail!("node store unavailable (persistence off)");
        };
        let embedding = self.embedder.embed(label);
        let id = core.register_node_vec_labeled(embedding, Some(label.to_string()));
        core.persist_node(&db, "_default", &id)?;
        Ok(id)
    }

    /// CLI twin of the MCP `physis_remember` tool: remember one labeled
    /// memory with an asserted outcome verdict (−1 failed, 0 inert, +1
    /// worked) so scripts and harnesses can write outcome memory without a
    /// running server. Dedupes by label like every other registration path.
    pub fn run_note(&mut self, label: &str, verdict: f32) -> anyhow::Result<String> {
        if label.trim().is_empty() {
            anyhow::bail!("note text must not be empty");
        }
        let verdict = verdict.clamp(-1.0, 1.0);
        let Some((db, mut core)) = self.open_persisted_core() else {
            anyhow::bail!("node store unavailable (persistence off)");
        };
        let embedding = self.embedder.embed(label);
        let id = core.register_node_vec_labeled(embedding, Some(label.to_string()));
        core.assert_coherence(&id, verdict);
        core.persist_node(&db, "_default", &id)?;
        Ok(format!(
            "note {id} remembered (verdict {verdict:+.1}, {} node(s))",
            core.nodes.len(),
        ))
    }

    /// "Exchange parts" edit from the CLI: swap a node's content, preserving its
    /// structure. Pin a cell with `--domain`/`--mode` to override auto-derivation.
    pub fn run_node_edit(
        &mut self,
        id: &str,
        label: &str,
        pin: crate::core::PinEdit,
    ) -> anyhow::Result<String> {
        let Some((db, mut core)) = self.open_persisted_core() else {
            anyhow::bail!("node store unavailable (persistence off)");
        };
        if !core.nodes.contains_key(id) {
            anyhow::bail!("node {id} not found");
        }
        let new_embedding = self.embedder.embed(label);
        let outcome = core.edit_node(id, label, new_embedding, pin, Some(self.embedder_kind))?;
        core.persist_node(&db, "_default", &outcome.node_id)?;
        let mut out = format!(
            "node {} → '{}' (asserted {:?}, coherence {:.3})",
            outcome.node_id, outcome.label, outcome.asserted, outcome.coherence_score,
        );
        if let Some((domain, mode)) = &outcome.pinned_cell {
            out.push_str(&format!(", pinned {}×{}", domain, mode));
        }
        Ok(out)
    }

    /// Run a machine's **engine-side** reduce against the persisted `_default`
    /// graph (Phase 8 read-back), serverless.
    ///
    /// `physis-pro machine <name>` reduces a payload to JSON and forgets it.
    /// This registers what the machine perceived, bridges it into the holarchy
    /// and writes it through, so the CLI and the HTTP surface close the same
    /// loop — both go through `machines::run::reduce`, so a machine cannot mean
    /// one thing over HTTP and another on the command line.
    pub fn run_machine_reduce(
        &mut self,
        name: &str,
        input: &serde_json::Value,
    ) -> anyhow::Result<String> {
        let Some((db, mut core)) = self.open_persisted_core() else {
            anyhow::bail!(
                "node store unavailable (persistence off) — reduce writes into the \
                 graph, so there is nothing to write to"
            );
        };
        let outcome = crate::machines::run::reduce(name, input, &mut core, self.embedder.as_ref())?;
        let mut persisted = 0usize;
        for id in &outcome.registered {
            core.persist_node(&db, "_default", id)?;
            persisted += 1;
        }
        let mut out = format!(
            "{} reduced: score {:+.3}, {} node(s) registered ({} → {}), {persisted} persisted\n",
            outcome.machine,
            outcome.score,
            outcome.registered.len(),
            outcome.nodes_before,
            outcome.nodes_after,
        );
        if outcome.registered.is_empty() {
            // Registration dedupes by label, so this is the normal second run,
            // not a failure — say so rather than leaving a bare zero.
            out.push_str(
                "[physis] nothing new: every primitive was already in the graph \
                 (registration dedupes by label)\n",
            );
        }
        for id in outcome.registered.iter().take(20) {
            if let Some(node) = core.nodes.get(id) {
                out.push_str(&format!(
                    "  {id}  {}\n",
                    node.label.as_deref().unwrap_or("(unlabeled)")
                ));
            }
        }
        Ok(out)
    }

    /// Delete a node and every reference to it from the CLI.
    pub fn run_node_delete(&mut self, id: &str) -> anyhow::Result<String> {
        let Some((db, mut core)) = self.open_persisted_core() else {
            anyhow::bail!("node store unavailable (persistence off)");
        };
        if !core.delete_node(id) {
            anyhow::bail!("node {id} not found");
        }
        db.remove(crate::core::PhysisCore::node_key("_default", id))?;
        Ok(format!("node {id} deleted"))
    }

    /// Fixed-token search over the persisted coherence + goal nodes.
    pub fn run_node_search(
        &mut self,
        query: &str,
        budget: usize,
        top: usize,
    ) -> anyhow::Result<String> {
        let Some((_db, core)) = self.open_persisted_core() else {
            anyhow::bail!("node store unavailable (persistence off)");
        };
        let q_emb = self.embedder.embed(query);

        // Coherence nodes (labeled) — goals aren't persisted via this store in
        // the CLI path, so node search covers the shared coherence register.
        let candidates: Vec<(String, String, Vec<f32>)> = core
            .nodes
            .values()
            .filter_map(|n| {
                let label = n.label.as_deref()?;
                if label.trim().is_empty() {
                    return None;
                }
                Some((n.id.clone(), label.to_string(), n.embedding.clone()))
            })
            .collect();

        let corpus = crate::rag::RagCorpus {
            chunks: candidates
                .iter()
                .enumerate()
                .map(|(i, (_, label, emb))| crate::rag::RagChunk {
                    id: i,
                    text: label.clone(),
                    tokens: crate::rag::count_tokens(label),
                    embedding: emb.clone(),
                })
                .collect(),
        };
        let result = crate::rag::TokenFixedRetriever::new(budget, top).retrieve(&q_emb, &corpus);

        let mut out = format!(
            "{} result(s), {} tokens (budget {})\n",
            result.chunks.len(),
            result.total_tokens,
            result.budget
        );
        for c in &result.chunks {
            if let Some((id, label, _)) = candidates.get(c.id) {
                out.push_str(&format!(
                    "  {:.3}  [{}] {} ({} tokens)\n",
                    c.score, id, label, c.tokens
                ));
            }
        }
        Ok(out)
    }

    /// Generate `count` dreams from the current goal set.
    pub fn run_dream(&mut self, count: usize) -> Vec<crate::models::Dream> {
        self.dreams.generate_dreams(&self.goals, count)
    }

    /// Load the persisted quality tracker (penalties/boosts survive restarts
    /// via `quality_failures.json`); empty when nothing was ever reported.
    fn load_quality(&self) -> crate::quality::QualityTracker {
        let path = self.config.data_dir.join("quality_failures.json");
        crate::quality::load_or_new(&path)
    }

    /// Effective cell of a persisted node in `DOMAIN/MODE` display form: pin
    /// wins, else the classifier's top hit — the same resolution
    /// `observable()` uses on the web side.
    fn effective_cell(&self, node: &crate::models::CoherenceNode) -> Option<String> {
        node.cell_pin
            .clone()
            .map(|(d, m)| format!("{d}/{m}"))
            .or_else(|| {
                self.classifier
                    .classify(&node.embedding)
                    .first()
                    .map(|c| format!("{}/{}", c.domain, c.mode))
            })
    }

    /// PLAN 11C conceptual search over the persisted `_default` store: cosine
    /// against the shared embedder, quality re-rank via `cell_standing`,
    /// grouping by effective cell. Same corpus as `run_node_search`, no token
    /// packing — `threshold` is the raw-cosine floor, `limit` caps the rows.
    pub fn run_concepts(
        &mut self,
        query: &str,
        threshold: f32,
        limit: usize,
    ) -> anyhow::Result<String> {
        let Some((_db, core)) = self.open_persisted_core() else {
            anyhow::bail!("node store unavailable (persistence off)");
        };
        let q_emb = self.embedder.embed(query);
        let quality = self.load_quality();
        let mut scored = Vec::new();
        for n in core.nodes.values() {
            let Some(label) = n.label.as_deref() else {
                continue;
            };
            if label.trim().is_empty() {
                continue;
            }
            let raw: f32 = q_emb.iter().zip(&n.embedding).map(|(a, b)| a * b).sum();
            if raw < threshold {
                continue;
            }
            let cell = self.effective_cell(n);
            let standing = cell
                .as_deref()
                .map(|c| quality.cell_standing(&c.replace('/', "\x00")))
                .unwrap_or(1.0);
            scored.push((
                n.id.clone(),
                label.to_string(),
                raw,
                cell,
                n.asserted,
                raw * standing,
            ));
        }
        let mut hits = crate::models::rank_concept_hits(scored);
        hits.truncate(limit.clamp(1, 500));
        let groups = hits.iter().map(|h| h.group).max().unwrap_or(0);
        let mut out = format!(
            "{} concept(s) in {} group(s) for '{query}' (threshold {threshold:.2})\n",
            hits.len(),
            groups,
        );
        for h in &hits {
            let verdict = match h.verdict {
                Some(v) if v <= -0.5 => "failed",
                Some(v) if v >= 0.5 => "worked",
                Some(_) => "inert",
                None => "unjudged",
            };
            let cell = h.cell.as_deref().unwrap_or("—");
            out.push_str(&format!(
                "  {:.3}->{:.3}  [g{} {cell}] [{}] {} (verdict {verdict})\n",
                h.score, h.adjusted, h.group, h.id, h.label,
            ));
        }
        Ok(out)
    }

    /// PLAN 11B "see all instances" over the CLI: every node sharing
    /// conceptual similarity with `id`, via the same
    /// `find_conceptual_instances` the web endpoint serves.
    pub fn run_node_instances(
        &mut self,
        id: &str,
        threshold: f32,
        limit: usize,
    ) -> anyhow::Result<String> {
        let Some((_db, core)) = self.open_persisted_core() else {
            anyhow::bail!("node store unavailable (persistence off)");
        };
        if !core.nodes.contains_key(id) {
            anyhow::bail!("node {id} not found");
        }
        let instances =
            core.find_conceptual_instances(id, &self.classifier, threshold, limit.clamp(1, 500));
        let mut out = format!(
            "{} instance(s) of {id} (threshold {threshold:.2})\n",
            instances.len(),
        );
        for i in &instances {
            let cell = i
                .cell
                .as_ref()
                .map(|(d, m)| format!("({d}/{m})"))
                .unwrap_or_default();
            out.push_str(&format!(
                "  {:.3}  [{}] {} {cell}\n",
                i.score.unwrap_or(0.0),
                i.id,
                i.label,
            ));
        }
        Ok(out)
    }

    /// Jump to a node: fragment, verdict, cell, plus its top instances.
    pub fn run_jump(&mut self, id: &str) -> anyhow::Result<String> {
        let Some((_db, core)) = self.open_persisted_core() else {
            anyhow::bail!("node store unavailable (persistence off)");
        };
        let Some(node) = core.nodes.get(id) else {
            anyhow::bail!("node {id} not found");
        };
        let instances = core.find_conceptual_instances(id, &self.classifier, 0.3, 10);
        let verdict = match node.asserted {
            Some(v) if v <= -0.5 => "failed",
            Some(v) if v >= 0.5 => "worked",
            Some(_) => "inert",
            None => "unjudged",
        };
        let cell = self.effective_cell(node).unwrap_or_else(|| "—".into());
        let fragment = node.extract_fragment().unwrap_or_default();
        let label = node.label.as_deref().unwrap_or("(unlabeled)");
        let mut out = format!(
            "{}  {label}\n  fragment: {fragment}\n  verdict: {verdict}\n  cell: {cell}\n  instances: {}\n",
            node.id,
            instances.len(),
        );
        for i in &instances {
            out.push_str(&format!(
                "  {:.3}  [{}] {}\n",
                i.score.unwrap_or(0.0),
                i.id,
                i.label,
            ));
        }
        Ok(out)
    }

    /// Token-budgeted context around a node: the node itself plus its top
    /// instances, packed greedily to `budget` (same contract as node-search,
    /// applied to one neighbourhood instead of the whole store).
    pub fn run_node_context(&mut self, id: &str, budget: usize) -> anyhow::Result<String> {
        let Some((_db, core)) = self.open_persisted_core() else {
            anyhow::bail!("node store unavailable (persistence off)");
        };
        let Some(node) = core.nodes.get(id) else {
            anyhow::bail!("node {id} not found");
        };
        let instances = core.find_conceptual_instances(id, &self.classifier, 0.0, 100);
        let mut members: Vec<(String, String, Vec<f32>)> = vec![(
            node.id.clone(),
            node.label.clone().unwrap_or_default(),
            node.embedding.clone(),
        )];
        for i in &instances {
            if let Some(n) = core.nodes.get(&i.id) {
                members.push((
                    n.id.clone(),
                    n.label.clone().unwrap_or_default(),
                    n.embedding.clone(),
                ));
            }
        }
        let corpus = crate::rag::RagCorpus {
            chunks: members
                .iter()
                .enumerate()
                .map(|(ix, (_, label, emb))| crate::rag::RagChunk {
                    id: ix,
                    text: label.clone(),
                    tokens: crate::rag::count_tokens(label),
                    embedding: emb.clone(),
                })
                .collect(),
        };
        // Rank neighbourhood members by similarity to the node itself (query
        // vector = the node's embedding, not a text query).
        let result = crate::rag::TokenFixedRetriever::new(budget, members.len())
            .retrieve(&node.embedding, &corpus);
        let mut out = format!(
            "context of {id}: {} chunk(s), {} tokens (budget {})\n",
            result.chunks.len(),
            result.total_tokens,
            result.budget,
        );
        for c in &result.chunks {
            if let Some((nid, label, _)) = members.get(c.id) {
                out.push_str(&format!("  [{nid}] {label} ({} tokens)\n", c.tokens));
            }
        }
        Ok(out)
    }

    /// Import a knowledge vault into the shared store as labeled core nodes.
    /// Reads markdown/plain-text notes (title + headings → labels) and — when
    /// `git_max > 0` — the recent commit history, embedding each label with the
    /// shared embedder so the vault is recallable by content in the wiki.
    pub fn run_vault_import(
        &mut self,
        dir: &std::path::Path,
        git_max: usize,
    ) -> anyhow::Result<String> {
        if !dir.is_dir() {
            anyhow::bail!("vault directory not found: {}", dir.display());
        }
        let docs = crate::vault::scan_vault(dir);
        let mut all: Vec<crate::vault::VaultDoc> = docs;
        if git_max > 0 {
            let commits = crate::vault::scan_git_log(dir, git_max);
            if !commits.is_empty() {
                all.extend(commits);
            }
        }
        if all.is_empty() {
            return Ok(format!("vault[{}]: no notes found", dir.display()));
        }

        // Label + embeddable text per doc; embed with the shared embedder.
        let pairs = crate::vault::collect_labels(&all);
        let labels_total = pairs.len();
        let texts: Vec<&str> = pairs.iter().map(|(_, body)| body.as_str()).collect();
        let embeddings = self.embedder.embed_batch(&texts);

        let Some((db, mut core)) = self.open_persisted_core() else {
            anyhow::bail!("node store unavailable (persistence off)");
        };
        // Dedupe is by label: count nodes before/after to know how many were
        // actually new (the label index collapses repeats to the existing id).
        let before = core.nodes.len();
        for ((label, _), emb) in pairs.into_iter().zip(embeddings) {
            let id = core.register_node_vec_labeled(emb, Some(label));
            if let Err(e) = core.persist_node(&db, "_default", &id) {
                eprintln!("[physis] vault: node {id} not persisted: {e}");
            }
        }
        let registered = core.nodes.len().saturating_sub(before);
        let deduped = labels_total.saturating_sub(registered);

        let commit_count = all.iter().filter(|d| d.source.starts_with("git ")).count();

        Ok(format!(
            "vault[{}]: {} docs ({} notes + {} commits) → {} labels registered ({} deduped)",
            dir.display(),
            all.len(),
            all.len() - commit_count,
            commit_count,
            registered,
            deduped,
        ))
    }

    /// Import a personal-history export (browser bookmarks/history, OPML feed
    /// list, chat JSONL) into the shared store as labeled core nodes. Each item
    /// is embedded with the shared embedder so the life-log is recallable by
    /// content in the wiki; re-imports dedupe by label.
    pub fn run_history_import(&mut self, file: &std::path::Path) -> anyhow::Result<String> {
        let (docs, name) = crate::history::import_file(file)?;
        Ok(self.register_docs(&docs, &format!("history[{name}]")))
    }

    /// Shared sink for parsed docs (history/vault/onboard): embed the label
    /// bodies with the shared embedder and register them as labeled core
    /// nodes. Label dedupe happens in the core; the summary reports it.
    fn register_docs(&mut self, docs: &[crate::vault::VaultDoc], name: &str) -> String {
        if docs.is_empty() {
            return format!("{name}: no items found");
        }
        let pairs = crate::vault::collect_labels(docs);
        let labels_total = pairs.len();
        let texts: Vec<&str> = pairs.iter().map(|(_, body)| body.as_str()).collect();
        let embeddings = self.embedder.embed_batch(&texts);

        let Some((db, mut core)) = self.open_persisted_core() else {
            return format!("{name}: node store unavailable (persistence off)");
        };
        let before = core.nodes.len();
        for ((label, _), emb) in pairs.into_iter().zip(embeddings) {
            let id = core.register_node_vec_labeled(emb, Some(label));
            if let Err(e) = core.persist_node(&db, "_default", &id) {
                eprintln!("[physis] {name}: node {id} not persisted: {e}");
            }
        }
        let registered = core.nodes.len().saturating_sub(before);
        let deduped = labels_total.saturating_sub(registered);
        format!(
            "{name}: {} items → {} labels registered ({} deduped)",
            docs.len(),
            registered,
            deduped,
        )
    }

    /// Prepopulate the store from the operator's own machine: shell histories,
    /// Claude agent sessions, and the Hermes long-term memory file. Missing
    /// sources are skipped (reported, not an error) so a fresh machine yields
    /// a partial onboard instead of a failure. Re-runs dedupe by label.
    pub fn run_onboard(&mut self, max_sessions: usize) -> anyhow::Result<String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let home = std::path::PathBuf::from(home);
        let mut lines = Vec::new();

        for hist in [".zsh_history", ".bash_history"] {
            let p = home.join(hist);
            if !p.is_file() {
                lines.push(format!("shell[{}]: not found, skipped", p.display()));
                continue;
            }
            match crate::history::import_file(&p) {
                Ok((docs, _)) => lines.push(self.register_docs(&docs, &format!("shell[{}]", p.display()))),
                Err(e) => lines.push(format!("shell[{}]: {e}", p.display())),
            }
        }

        // Claude sessions: newest first, bounded (each session embeds).
        let mut sessions: Vec<(std::time::SystemTime, std::path::PathBuf)> = Vec::new();
        let projects = home.join(".claude/projects");
        if projects.is_dir() {
            let mut stack = vec![projects];
            while let Some(dir) = stack.pop() {
                let Ok(rd) = std::fs::read_dir(&dir) else { continue };
                for entry in rd.filter_map(|e| e.ok()) {
                    let p = entry.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p.extension().and_then(|e| e.to_str()) == Some("jsonl")
                        && p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(crate::history::looks_like_claude_session_hint)
                    {
                        // Only session-shaped stems; chat logs go via `history`.
                        let mtime = entry.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
                        sessions.push((mtime, p));
                    }
                }
            }
        }
        sessions.sort_by_key(|(mtime, _)| std::cmp::Reverse(*mtime));
        sessions.truncate(max_sessions.max(1));
        if sessions.is_empty() {
            lines.push("sessions: none found, skipped".to_string());
        }
        for (_, p) in &sessions {
            match crate::history::import_file(p) {
                Ok((docs, _)) => lines.push(self.register_docs(
                    &docs,
                    &format!("session[{}]", p.file_stem().and_then(|s| s.to_str()).unwrap_or("?")),
                )),
                Err(e) => lines.push(format!("session[{}]: {e}", p.display())),
            }
        }

        // Hermes long-term memory: single markdown doc.
        let mem = home.join(".hermes/memories/MEMORY.md");
        if mem.is_file() {
            match std::fs::read_to_string(&mem) {
                Ok(body) if body.trim().len() >= 8 => {
                    let doc = crate::vault::VaultDoc {
                        source: mem.display().to_string(),
                        title: "agent memory".to_string(),
                        body,
                        headings: vec![],
                    };
                    lines.push(self.register_docs(&[doc], "memory[hermes]"));
                }
                _ => lines.push("memory[hermes]: empty, skipped".to_string()),
            }
        } else {
            lines.push("memory[hermes]: not found, skipped".to_string());
        }

        // Hermes plans: markdown project plans, vault-style (one doc per file).
        let plans = home.join(".hermes/plans");
        if plans.is_dir() {
            let docs = crate::vault::scan_vault(&plans);
            lines.push(self.register_docs(&docs, "plans[hermes]"));
        } else {
            lines.push("plans[hermes]: not found, skipped".to_string());
        }

        #[cfg(feature = "sql")]
        {
            // Opencode sessions: newest first, bounded (32k messages total —
            // cap sessions and let the turn cap bound the rest).
            let ocdb = home.join(".local/share/opencode/opencode.db");
            if ocdb.is_file() {
                let mut n_docs = 0;
                let mut n_sess = 0;
                for (id, turns) in crate::history::read_opencode_turns(&ocdb, max_sessions) {
                    let docs = crate::history::agent_turn_docs("opencode", &id, &turns);
                    n_docs += docs.len();
                    n_sess += 1;
                    lines.push(self.register_docs(&docs, &format!("opencode[{id}]")));
                }
                if n_sess == 0 {
                    lines.push("opencode: store present, no turns found".to_string());
                } else {
                    lines.push(format!("opencode: {n_sess} sessions → {n_docs} turn docs (see lines above)"));
                }
            } else {
                lines.push("opencode: no store, skipped".to_string());
            }

            // Codex threads: newest first, bounded.
            let cxdb = home.join(".codex/thread_history_1.sqlite");
            if cxdb.is_file() {
                let mut n_docs = 0;
                let mut n_threads = 0;
                for (id, turns) in crate::history::read_codex_turns(&cxdb, max_sessions) {
                    let docs = crate::history::agent_turn_docs("codex", &id, &turns);
                    n_docs += docs.len();
                    n_threads += 1;
                    let short = id.chars().take(8).collect::<String>();
                    lines.push(self.register_docs(&docs, &format!("codex[{short}]")));
                }
                if n_threads == 0 {
                    lines.push("codex: store present, no turns found".to_string());
                } else {
                    lines.push(format!("codex: {n_threads} threads → {n_docs} turn docs (see lines above)"));
                }
            } else {
                lines.push("codex: no store, skipped".to_string());
            }

            // Mesh memory: the gossiped fleet store (traps/decisions/runbooks/
            // facts from every machine). This is the fleet-view pull.
            let memdb = home.join(".local/state/randomesh/memory/mem.db");
            if memdb.is_file() {
                let docs = crate::history::read_memories(&memdb);
                lines.push(self.register_docs(&docs, "mesh[memory]"));
            } else {
                lines.push("mesh[memory]: no store, skipped".to_string());
            }
        }
        #[cfg(not(feature = "sql"))]
        {
            lines.push("opencode/codex/mesh: skipped (sql feature off)".to_string());
        }

        Ok(lines.join("\n"))
    }

    /// Backfill coherence from a Praxis life-log export: read notebook entries +
    /// trackers, register each as a labeled behavioral node, and assert the
    /// derived verdict (success +1 / inert 0 / failure −1).
    pub fn run_praxis_backfill(&mut self, file: &std::path::Path) -> anyhow::Result<String> {
        if !file.is_file() {
            anyhow::bail!("praxis export not found: {}", file.display());
        }
        let text = std::fs::read_to_string(file)
            .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", file.display()))?;
        let records = crate::praxis::parse_export(&text);
        if records.is_empty() {
            return Ok(format!("praxis[{}]: no items found", file.display()));
        }

        let texts: Vec<&str> = records.iter().map(|r| r.body.as_str()).collect();
        let embeddings = self.embedder.embed_batch(&texts);

        let Some((db, mut core)) = self.open_persisted_core() else {
            anyhow::bail!("node store unavailable (persistence off)");
        };
        let before = core.nodes.len();
        let mut successes = 0usize;
        let mut inert = 0usize;
        let mut failures = 0usize;
        for (record, emb) in records.iter().zip(embeddings) {
            let label = format!("praxis:{}:{}", record.kind, record.title);
            let id = core.register_node_vec_labeled(emb, Some(label));
            core.assert_coherence(&id, record.status.as_score());
            if let Err(e) = core.persist_node(&db, "_default", &id) {
                eprintln!("[physis] praxis: node {id} not persisted: {e}");
            }
            match record.status {
                crate::praxis::BehaviourStatus::Success => successes += 1,
                crate::praxis::BehaviourStatus::Inert => inert += 1,
                crate::praxis::BehaviourStatus::Failure => failures += 1,
            }
        }
        let registered = core.nodes.len().saturating_sub(before);
        let deduped = records.len().saturating_sub(registered);
        Ok(format!(
            "praxis[{}]: {} items ({} success, {} inert, {} failure) → {} registered ({} deduped)",
            file.display(),
            records.len(),
            successes,
            inert,
            failures,
            registered,
            deduped,
        ))
    }

    /// Spanner Graph sink (Phase 7C): mirror the persisted coherence graph up to
    /// Spanner (durable memory), or pull durable rows back and re-embed them
    /// under the live embedder (new device / model swap). Off the hot path.
    /// `sync`: true = sync up (with `mirror` hard-deleting remote orphans);
    /// false = hydrate (pull + re-embed).
    #[cfg(feature = "spanner")]
    pub async fn run_spanner(&mut self, sync: bool, mirror: bool) -> anyhow::Result<String> {
        let sink = crate::spanner::SpannerSink::from_env()?;
        // Rehydrate the persisted "_default" tenant so we sync real data.
        let Some((_db, core)) = self.open_persisted_core() else {
            anyhow::bail!("node store unavailable (persistence off)");
        };
        if sync {
            // The default tenant's causal ledger rides along (Phase 10b):
            // why-made chains join the durable wiki under the cause: namespace.
            let ledger = self.db().and_then(|db| {
                crate::ledger::CausalLedger::load(&db, "_default")
                    .map_err(|e| eprintln!("[physis] ledger load failed: {e}"))
                    .ok()
            });
            if mirror {
                let stats = crate::spanner::sync_full(&core, &sink).await?;
                let (cn, ce) = match &ledger {
                    Some(l) => crate::spanner::sync_ledger(&sink, l).await?,
                    None => (0, 0),
                };
                Ok(format!(
                    "spanner sync: {} nodes + {} edges upserted, {} nodes + {} edges deleted · ledger: {cn} artifact nodes + {ce} cause edges",
                    stats.nodes_upserted,
                    stats.edges_upserted,
                    stats.nodes_deleted,
                    stats.edges_deleted,
                ))
            } else {
                let (n, e) = crate::spanner::sync(&core, &sink).await?;
                let (cn, ce) = match &ledger {
                    Some(l) => crate::spanner::sync_ledger(&sink, l).await?,
                    None => (0, 0),
                };
                Ok(format!("spanner sync: {n} nodes + {e} edges upserted · ledger: {cn} nodes + {ce} edges"))
            }
        } else {
            let Some((db, mut core)) = self.open_persisted_core() else {
                anyhow::bail!("node store unavailable (persistence off)");
            };
            let rows = sink.fetch_nodes().await?;
            let pulled = rows.len();
            let re_embedded =
                crate::spanner::hydrate_into(&mut core, self.embedder.as_ref(), &rows);
            for id in core.nodes.keys().cloned().collect::<Vec<_>>() {
                let _ = core.persist_node(&db, "_default", &id);
            }
            Ok(format!(
                "spanner hydrate: pulled {pulled} durable rows, re-embedded {re_embedded}"
            ))
        }
    }

    /// Grade a dream by ID. Returns false if no dream with that ID exists.
    pub fn run_evaluate(&mut self, id: &str, grade: Score) -> bool {
        self.dreams.evaluate_dream(id, grade)
    }

    /// Deep AI-powered scan — sends file content to LLM for ontological map extraction.
    pub async fn run_deep_scan(&mut self, dir: &std::path::Path) -> anyhow::Result<String> {
        use crate::ai::agent::{run_agent, AgentConfig};
        use crate::ai::provider::ProviderCascade;
        use crate::ai::tools::ToolRegistry;
        use crate::scanner::scan_project;
        use base64::prelude::*;

        println!("Starting Deep Scan on {}...", dir.display());
        let cascade = ProviderCascade::from_env();
        let tools = ToolRegistry::new();
        let mut global_map = crate::models::OntologicalMap::new();

        let files = scan_project(dir, None);
        println!("Found {} candidate files.", files.len());

        for file in files {
            println!("Processing {}...", file.path);
            let ext = file.ext.to_lowercase();

            let result = if [".png", ".jpg", ".jpeg"].contains(&ext.as_str()) {
                // Image Deep Scan
                let bytes = std::fs::read(&file.abs_path)?;
                let b64 = BASE64_STANDARD.encode(bytes);
                let config = AgentConfig {
                    system_prompt: r#"You are Physis Vision. Extract an ontological map (JSON).
Output ONLY a JSON object with this EXACT structure:
{
  "entities": {
    "entity_id": {"id":"entity_id", "name":"Human Name", "kind":"kind", "description":"...", "attributes":{}}
  },
  "relationships": [
    {"source":"entity_id1", "target":"entity_id2", "predicate":"verb", "weight":1.0}
  ]
}
No other text."#.into(),
                    ..Default::default()
                };
                run_agent(
                    &cascade,
                    &tools,
                    &config,
                    &[],
                    &format!("Analyze image: {}", file.path),
                    Some(&b64),
                    "DATA",
                    None,
                )
                .await
            } else {
                // Text Deep Scan
                let text = std::fs::read_to_string(&file.abs_path)?;
                let config = AgentConfig {
                    system_prompt: r#"You are Physis. Extract an ontological map (JSON).
Output ONLY a JSON object with this EXACT structure:
{
  "entities": {
    "entity_id": {"id":"entity_id", "name":"Human Name", "kind":"kind", "description":"...", "attributes":{}}
  },
  "relationships": [
    {"source":"entity_id1", "target":"entity_id2", "predicate":"verb", "weight":1.0}
  ]
}
No other text."#.into(),
                    ..Default::default()
                };
                run_agent(
                    &cascade,
                    &tools,
                    &config,
                    &[],
                    &format!("Analyze text ({}): \n{}", file.path, text),
                    None,
                    "DATA",
                    None,
                )
                .await
            };

            if let Ok(output) = result {
                if let (Some(s), Some(e)) = (output.text.find('{'), output.text.rfind('}')) {
                    if let Ok(new_map) =
                        serde_json::from_str::<crate::models::OntologicalMap>(&output.text[s..=e])
                    {
                        global_map.merge(new_map);
                        println!("Merged ontology for {}.", file.path);
                    }
                }
            }
        }

        Ok(serde_json::to_string_pretty(&global_map)?)
    }

    /// Watch directories for file changes using hash-cache diffing. With
    /// `once`, perform a single ingest sweep and return (cron/smoke-test
    /// friendly); otherwise loop forever, re-scanning on `scan_interval_secs`.
    ///
    /// Each sweep registers new/changed files as labeled core nodes with the
    /// shared embedder (so the wiki stays fed without a manual scan), drops
    /// nodes for removed files, and advances the hash cache. Returns a summary
    /// of the sweep.
    pub fn run_watch(&mut self, dirs: Vec<PathBuf>, once: bool) -> anyhow::Result<String> {
        loop {
            let sweep = self.watch_sweep(dirs.clone())?;
            if once {
                return Ok(sweep);
            }
            println!("{sweep}");
            std::thread::sleep(std::time::Duration::from_secs(
                self.config.network_scan_interval_secs,
            ));
        }
    }

    /// One watch sweep: diff `dirs` against the hash cache, ingest new/changed
    /// files as labeled core nodes (shared embedder, persisted), delete nodes
    /// for removed files, advance the cache. Returns a summary string.
    fn watch_sweep(&mut self, dirs: Vec<PathBuf>) -> anyhow::Result<String> {
        let cache_path = self.config.data_dir.join("hash_cache.json");
        let scanner = NetworkScanner::new(
            dirs.clone(),
            cache_path,
            self.config.network_scan_interval_secs,
        );
        let diffs = scanner.scan_all();

        let mut new = 0usize;
        let mut changed = 0usize;
        let mut deleted = 0usize;
        let mut ingested = 0usize;
        let mut removed = 0usize;

        // Only touch the store when there is something to do.
        let any_change = diffs
            .iter()
            .any(|d| d.summary.new + d.summary.changed + d.summary.deleted > 0);
        if any_change {
            let Some((db, mut core)) = self.open_persisted_core() else {
                anyhow::bail!("node store unavailable (persistence off)");
            };

            // Register new/changed content as labeled core nodes. The app
            // embedder carries the same provenance the web server stamps, so
            // these nodes survive a web reload (embedder-mismatch skip).
            let mut labels: Vec<String> = Vec::new();
            for diff in &diffs {
                new += diff.summary.new;
                changed += diff.summary.changed;
                deleted += diff.summary.deleted;
                for f in diff.new.iter().chain(diff.changed.iter()) {
                    labels.extend(f.labels());
                }
            }
            if !labels.is_empty() {
                let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
                let embeddings = self.embedder.embed_batch(&refs);
                for (emb, label) in embeddings.into_iter().zip(labels) {
                    let id = core.register_node_vec_labeled(emb, Some(label));
                    if let Err(e) = core.persist_node(&db, "_default", &id) {
                        eprintln!("[physis] watch: node {id} not persisted: {e}");
                    }
                    ingested += 1;
                }
            }

            // Removed files: their nodes live under the structural path
            // (`dir → file` plus per-line labels). Delete them and their rows.
            for diff in &diffs {
                for p in &diff.deleted {
                    let prefix = p.replace('/', " → ");
                    for id in core.delete_nodes_by_label_prefix(&prefix) {
                        let _ = db.remove(crate::core::PhysisCore::node_key("_default", &id));
                        removed += 1;
                    }
                }
            }
        }

        // Advance the hash cache so the next sweep only sees real changes.
        scanner.update_cache(&diffs);

        let watched: Vec<String> = dirs.iter().map(|d| d.display().to_string()).collect();
        Ok(format!(
            "watch[{}]: {} new, {} changed, {} deleted — registered {} labeled nodes, removed {}",
            watched.join(", "),
            new,
            changed,
            deleted,
            ingested,
            removed,
        ))
    }

    /// Print trie statistics and PDCA metrics.
    pub fn run_stats(&self) -> String {
        let stats = self.mapper.stats();
        let mut out = String::new();
        out.push_str("=== Physis Stats ===\n");
        let mut pairs: Vec<_> = stats.into_iter().collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        for (k, v) in pairs {
            out.push_str(&format!("  {k}: {v}\n"));
        }
        let actor_stats = self.actor.stats(&self.goals);
        out.push_str(&format!("  total_actions: {}\n", actor_stats.total_actions));
        out.push_str(&format!("  avg_grade: {:.3}\n", actor_stats.avg_grade));
        if actor_stats.stagnant_count > 0 {
            out.push_str(&format!(
                "  stagnant_goals: {}\n",
                actor_stats.stagnant_count
            ));
        }
        out
    }

    /// Translate `text` through the specified lense (or all enabled lenses).
    pub fn run_translate(&self, text: &str, lense: Option<&str>) -> String {
        let router = LinguisticRouter::with_config(&self.config.linguistic);
        match lense {
            Some(l) => {
                let lense = match l.to_lowercase().as_str() {
                    "wenyan" => LinguisticLense::Wenyan,
                    "piraha" => LinguisticLense::Piraha,
                    "sanskrit" => LinguisticLense::Sanskrit,
                    _ => return format!("Unknown lense: {l}. Use: wenyan, piraha, sanskrit"),
                };
                router.route(text, lense)
            }
            None => {
                let results = router.route_all(text);
                if results.is_empty() {
                    return "No lenses are enabled.".to_string();
                }
                results
                    .iter()
                    .map(|(k, v)| format!("[{}] {}", k.as_str(), v))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
    }

    /// Classify a text file against the semiotic grid.
    /// Returns a formatted map: ranked domain×mode cells with scores + contributing entries.
    /// Ontology discovery over a directory: every non-empty line of every
    /// .txt/.md/.log/.csv file is one corpus text (notes and logs are
    /// line-oriented). Returns the human report; when `json_out` is set, the
    /// ready-to-edit ontology draft is written there.
    pub fn run_discover(
        &self,
        dir: &std::path::Path,
        json_out: Option<&std::path::Path>,
        threshold: Option<f32>,
    ) -> anyhow::Result<String> {
        const MAX_TEXTS: usize = 5000;
        let mut texts: Vec<String> = Vec::new();
        for entry in walkdir::WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let ok_ext = matches!(
                entry.path().extension().and_then(|e| e.to_str()),
                Some("txt" | "md" | "log" | "csv")
            );
            if !entry.file_type().is_file() || !ok_ext {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.len() > 10 {
                        texts.push(line.to_string());
                        if texts.len() >= MAX_TEXTS {
                            break;
                        }
                    }
                }
            }
            if texts.len() >= MAX_TEXTS {
                break;
            }
        }
        if texts.is_empty() {
            anyhow::bail!(
                "no corpus lines found under {} (.txt/.md/.log/.csv)",
                dir.display()
            );
        }

        let mut cfg = crate::discovery::DiscoveryConfig::default();
        if let Some(t) = threshold {
            cfg.coverage_threshold = t;
        }
        let report =
            crate::discovery::discover(&texts, &self.classifier, self.embedder.as_ref(), &cfg);

        let mut out = String::new();
        out.push_str("━━━ Physis Ontology Discovery ━━━\n");
        out.push_str(&format!(
            "Corpus: {} texts · covered {} · unmapped {} (threshold {:.2})\n",
            report.total, report.covered, report.unmapped, cfg.coverage_threshold
        ));
        out.push_str(&format!(
            "Best-entry cosine: min {:.2} · mean {:.2} · max {:.2}  \
             (tune --threshold between mean and max to surface gaps)\n",
            report.sim_min, report.sim_mean, report.sim_max
        ));
        if report.auto_retuned {
            out.push_str(&format!(
                "Auto-retuned: the requested threshold covered everything, so it was \
                 raised to {:.2} to surface the least-covered texts.\n",
                report.threshold_used
            ));
        }
        out.push('\n');
        for p in &report.proposals {
            out.push_str(&format!(
                "▶ {} — {} texts, coverage {:.2}, nearest cell {}×{}\n",
                p.name, p.count, p.coverage, p.domain, p.mode
            ));
            out.push_str(&format!("   hints: {}\n", p.hints.join(", ")));
            for s in &p.samples {
                let s = if s.len() > 90 { &s[..90] } else { s };
                out.push_str(&format!("   · {s}\n"));
            }
        }
        if report.proposals.is_empty() {
            out.push_str("No proposals — the current ontology covers this corpus.\n");
        }
        if let Some(path) = json_out {
            std::fs::write(
                path,
                crate::discovery::to_ontology_json(&report, "discovered"),
            )?;
            out.push_str(&format!("\nDraft ontology written to {}\n", path.display()));
        }
        Ok(out)
    }

    pub fn run_classify(&self, file: &std::path::Path) -> anyhow::Result<String> {
        // Determine if it's an image file
        let is_image = matches!(
            file.extension().and_then(|e| e.to_str()),
            Some("jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp")
        );

        let embedding = if is_image {
            let bytes = std::fs::read(file)
                .map_err(|e| anyhow::anyhow!("Cannot read {}: {}", file.display(), e))?;
            self.embedder
                .embed_image(&bytes)
                .ok_or_else(|| anyhow::anyhow!("Image embedding not available for this embedder"))
                .map_err(|e| {
                    anyhow::anyhow!("Failed to classify image {}: {}", file.display(), e)
                })?
        } else {
            let text = std::fs::read_to_string(file)
                .map_err(|e| anyhow::anyhow!("Cannot read {}: {}", file.display(), e))?;
            self.embedder.embed(&text)
        };

        // Nearest-entry scoring via the shared classifier — the same path the
        // web server and the gated accuracy test use. The previous mean-centroid
        // scoring predated the P3 fix and compressed every score toward zero.
        let results = self.classifier.classify(&embedding);

        let mut out = String::new();
        out.push_str("━━━ Physis Classification Map ━━━\n");
        out.push_str(&format!("File: {}\n\n", file.display()));
        for (i, r) in results.iter().enumerate() {
            if i == 0 {
                out.push_str(&format!(
                    "★ TOP: {}×{} — {:.3}\n",
                    r.domain, r.mode, r.score
                ));
                out.push_str(&format!(
                    "   Reasoning: nearest entries: {}\n",
                    r.entries
                        .iter()
                        .take(5)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                out.push('\n');
            } else if i < 5 {
                out.push_str(&format!("  {}×{} — {:.3}\n", r.domain, r.mode, r.score));
            }
        }
        out.push_str(&format!("\n{} cells scored (top 5 shown)\n", results.len()));
        Ok(out)
    }

    /// Ontology Studio — the sellable demo wedge.
    ///
    /// Ingest `dir`, build the file-theme graph (existing ontology coverage of
    /// the prospect's docs), run discovery to surface the gaps the *current*
    /// ontology can't name, and render one self-contained HTML file a
    /// non-technical buyer can open. Reuses the same embedder/classifier the
    /// rest of the engine uses, so it's deterministic and offline.
    ///
    /// This is what makes the "your ontology grows itself from your documents"
    /// pitch visible in one sitting: the left half is their data against the
    /// shipped 370-cell grid; the right half is what the grid *misses*.
    pub fn run_studio(
        &self,
        dir: &std::path::Path,
        html_out: &std::path::Path,
        demo: bool,
    ) -> anyhow::Result<String> {
        // --demo: seed a sample corpus idempotently, so a prospect can see the
        // whole wedge with zero setup. Only writes the samples when the dir has
        // no ingestable docs yet, so a real prospect's data is never clobbered
        // on re-run (matches the documented contract).
        if demo {
            std::fs::create_dir_all(dir).ok();
            if ingest::ingest_all(&[dir.to_path_buf()]).is_empty() {
                crate::demo_corpus::seed_demo_corpus(dir)?;
            }
        }

        let docs = ingest::ingest_all(&[dir.to_path_buf()]);
        if docs.is_empty() {
            anyhow::bail!(
                "no documents found under {} (.txt/.md/.csv/.log/.xlsx/.eml); pass --demo to seed a sample",
                dir.display()
            );
        }

        // Build the file-theme graph from the live ontology + the prospect's docs.
        // Mirror run_ingest_pipeline: themes = ontology domains, files = docs,
        // edges = top-5 similarity > 0.25.
        let mut graph = FileThemeGraph::new();
        let all_domains: Vec<(&str, &HashMap<String, DomainDef>)> = vec![
            ("office", &self.ontology.office_domains),
            ("human", &self.ontology.human_domains),
            ("machine", &self.ontology.machine_domains),
            ("agent", &self.ontology.agent_domains),
            ("engineering", &self.ontology.engineering_domains),
        ];
        for (_kind, domains) in &all_domains {
            for def in domains.values() {
                let mut text = def.name.clone();
                for hint in &def.hints {
                    text.push(' ');
                    text.push_str(hint);
                }
                let embedding = self.embedder.embed(&text);
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
        let mut file_texts: Vec<String> = Vec::with_capacity(docs.len());
        let ingested_at = chrono::Utc::now();
        for doc in &docs {
            let file_embedding = self.embedder.embed(&doc.body);
            // Address at capture time (Phase 10): one scan root per call.
            let address = crate::address::FileAddress::derive(
                &doc.path,
                doc.path.parent().unwrap_or(std::path::Path::new("/")),
                crate::address::Mechanism::Generated,
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
                .map(|t| (t.id, cosine_sim(&file_embedding, &t.embedding)))
                .filter(|(_, sim)| *sim > 0.25)
                .collect();
            theme_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            for (theme_id, score) in theme_scores.iter().take(5) {
                graph.add_edge(*theme_id, file_id, *score);
            }
            let body = doc.body.trim();
            if !body.is_empty() {
                for line in body.lines() {
                    let line = line.trim();
                    if line.len() > 10 {
                        file_texts.push(line.to_string());
                    }
                }
            }
        }

        // Discovery: what does the current grid fail to cover?
        let cfg = crate::discovery::DiscoveryConfig {
            auto_retune: true,
            ..Default::default()
        };
        let report =
            crate::discovery::discover(&file_texts, &self.classifier, self.embedder.as_ref(), &cfg);

        let html = render_studio_html(&graph, &report, dir, &docs);
        std::fs::write(html_out, &html)?;

        // Close the loop: the discovered gaps become a loadable ontology file
        // the engine can pick up via EXTRA_ONTOLOGIES — so "your ontology grows
        // itself from your documents" is demonstrable, not just shown in HTML.
        let onto_json = crate::discovery::to_ontology_json(&report, "discovered");
        let onto_path = dir.join("discovered_ontology.json");
        std::fs::write(&onto_path, &onto_json)?;

        // Typed ontology graph (the Palantir-like layer): themes/files become
        // typed nodes joined by named link types; discovered gaps become Concept
        // nodes on the same graph. One inspectable artifact carries the whole
        // picture.
        let mut typed = crate::ontology_graph::OntologyGraph::from_file_theme_graph(&graph);
        typed.merge_discovery(&report);
        let typed_path = dir.join("ontology_graph.json");
        typed.save(&typed_path)?;
        let events_path = dir.join("ontology_events.json");
        typed.save_events(&events_path)?;

        let mut out = String::new();
        out.push_str(&format!(
            "Ontology Studio → {}  ({} docs, {} themes, {} files, {} edges)\n",
            html_out.display(),
            docs.len(),
            graph.themes.len(),
            graph.files.len(),
            graph.edges.len(),
        ));
        out.push_str(&format!(
            "Coverage: {} of {} corpus lines map to a known cell · {} unmapped\n",
            report.covered, report.total, report.unmapped,
        ));
        out.push_str(&format!(
            "Discovered gaps: {} proposed domain(s) the current ontology can't name\n",
            report.proposals.len(),
        ));
        out.push_str(&format!(
            "Typed graph: {} nodes / {} links (themes, documents, discovered concepts)\n",
            typed.nodes.len(),
            typed.edges.len(),
        ));
        out.push_str(&format!(
            "Provenance: {} event(s) recorded (what the ontology learned, and from what)\n",
            typed.events.len(),
        ));
        out.push_str(&format!(
            "Generated ontology: {}  (add to EXTRA_ONTOLOGIES to grow the grid)\n",
            onto_path.display(),
        ));
        out.push_str(&format!("Open: file://{}\n", html_out.display()));
        Ok(out)
    }

    /// Query the ontology by orthogonal facets (beyond DOMAIN×MODE): filter the
    /// loaded domains by lifecycle / agency / scale / abstraction / sub-domain /
    /// sub-mode, optionally narrowed to one kind and/or domain. Prints a
    /// table of matching entries. Empty result → a message, not an error (an
    /// empty facet set is a legitimate answer, e.g. "no OPERATE+SELL entries").
    #[allow(clippy::too_many_arguments)] // 8 optional filters + kind; a builder type would be overkill for a CLI query
    /// Ask where/when/why about ingested artifacts (Phase 10 D5): facet query
    /// over the persisted "_default" causal ledger. Reads the same sled store
    /// the web server writes; no server needed.
    pub fn run_where(
        &self,
        query: Option<&str>,
        place: Option<&str>,
        cause: Option<&str>,
        within_days: Option<i64>,
        during: Option<&str>,
    ) -> String {
        let Some(db) = self.db() else {
            return "[physis] store unavailable (persistence off)\n".to_string();
        };
        let ledger = match crate::ledger::CausalLedger::load(&db, "_default") {
            Ok(l) => l,
            Err(e) => return format!("[physis] cannot load ledger: {e}\n"),
        };
        if ledger.is_empty() {
            return "[physis] the causal ledger is empty — run a scan, watch a \
                    directory, or import a vault first.\n"
                .to_string();
        }

        let now = chrono::Utc::now();
        let kept: Vec<&crate::ledger::CauseEvent> = ledger
            .events()
            .iter()
            .filter(|ev| {
                if let Some(q) = query {
                    let hay = format!(
                        "{} {} {}",
                        ev.object,
                        ev.input.as_deref().unwrap_or(""),
                        ev.actor
                    );
                    if !hay.to_lowercase().contains(&q.to_lowercase()) {
                        return false;
                    }
                }
                if let Some(p) = place {
                    if !ev.object.starts_with(p) {
                        return false;
                    }
                }
                if let Some(c) = cause {
                    let c = c.to_lowercase();
                    if ev.mechanism.as_str() != c && ev.actor.to_lowercase() != c {
                        return false;
                    }
                }
                if let Some(d) = within_days {
                    if (now - ev.at).num_days() > d {
                        return false;
                    }
                }
                if let Some(period) = during {
                    if crate::address::period_of(ev.at) != period {
                        return false;
                    }
                }
                true
            })
            .collect();

        let mut out = String::new();
        out.push_str(&format!(
            "{:>4}  {:<19}  {:<10}  {:<14}  {}\n",
            "seq", "when (UTC)", "mechanism", "actor", "object"
        ));
        for ev in kept.iter().rev().take(100) {
            out.push_str(&format!(
                "{:>4}  {:<19}  {:<10}  {:<14}  {}{}\n",
                ev.seq,
                ev.at.format("%Y-%m-%d %H:%M:%S"),
                ev.mechanism.as_str(),
                ev.actor,
                ev.object,
                ev.input
                    .as_ref()
                    .map(|i| format!("  ← {i}"))
                    .unwrap_or_default(),
            ));
        }
        // Facet rollups over everything that matched (not just the printed tail).
        let mut by_period: HashMap<String, usize> = HashMap::new();
        let mut by_mechanism: HashMap<String, usize> = HashMap::new();
        for ev in &kept {
            *by_period
                .entry(crate::address::period_of(ev.at))
                .or_default() += 1;
            *by_mechanism
                .entry(ev.mechanism.as_str().to_string())
                .or_default() += 1;
        }
        out.push_str(&format!("\n{} matching events", kept.len()));
        if kept.len() > 100 {
            out.push_str(" (showing newest 100)");
        }
        out.push_str("\n\nby period:\n");
        for (p, n) in sorted_counts(&by_period) {
            out.push_str(&format!("  {p:<12} {}\n", bar(n, kept.len())));
        }
        out.push_str("by mechanism:\n");
        for (m, n) in sorted_counts(&by_mechanism) {
            out.push_str(&format!("  {m:<12} {}\n", bar(n, kept.len())));
        }
        out
    }

    /// Phase 11 C5 — cluster the persisted coherence graph into communities and
    /// summarize each one: title, distinctive keywords, dominant facets, outcome
    /// tally, and a deterministic digest. Offline and deterministic, so it runs
    /// without a provider. Reads the "_default" tenant's store + ledger.
    pub fn run_themes(&self, min_size: usize, json: bool) -> String {
        let Some((_, core)) = self.open_persisted_core() else {
            return "[physis] store unavailable (persistence off)\n".to_string();
        };
        let Some(db) = self.db() else {
            return "[physis] store unavailable (persistence off)\n".to_string();
        };
        let ledger = match crate::ledger::CausalLedger::load(&db, "_default") {
            Ok(l) => l,
            Err(e) => return format!("[physis] cannot load ledger: {e}\n"),
        };
        if core.nodes.is_empty() {
            return "[physis] the coherence graph is empty — run a scan, watch a \
                    directory, or import a vault first.\n"
                .to_string();
        }

        let (facts, edges) = crate::corpus::gather_facts(&core, &ledger);
        let summaries = crate::corpus::build_summaries(&facts, &edges);
        let visible: Vec<_> = summaries.iter().filter(|s| s.size >= min_size).collect();

        if json {
            return serde_json::to_string_pretty(&serde_json::json!({
                "communities": summaries.len(),
                "nodes": facts.len(),
                "summaries": summaries,
            }))
            .unwrap_or_default();
        }

        let mut out = String::new();
        out.push_str(&format!(
            "Corpus themes — {} communit{}, {} nodes analysed\n",
            summaries.len(),
            if summaries.len() == 1 { "y" } else { "ies" },
            facts.len(),
        ));
        if visible.is_empty() {
            out.push_str("\nNo community meets --min-size. Lower it to see smaller clusters.\n");
            return out;
        }
        for s in &visible {
            out.push_str(&format!(
                "\n## {}  ·  {} nodes  ·  cohesion {:.2}\n",
                s.title, s.size, s.cohesion
            ));
            if !s.keywords.is_empty() {
                out.push_str(&format!("  keywords: {}\n", s.keywords.join(", ")));
            }
            let mut facets: Vec<String> = Vec::new();
            if let Some(c) = &s.dominant_cell {
                facets.push(format!("cell {c}"));
            }
            if let Some(p) = &s.dominant_place {
                facets.push(format!("at {p}"));
            }
            if let Some(p) = &s.dominant_period {
                facets.push(format!("during {p}"));
            }
            if let Some(m) = &s.dominant_mechanism {
                facets.push(format!("via {m}"));
            }
            if !facets.is_empty() {
                out.push_str(&format!("  {}\n", facets.join(" · ")));
            }
            if s.wins > 0 || s.failures > 0 {
                out.push_str(&format!(
                    "  outcomes: {} positive, {} negative\n",
                    s.wins, s.failures
                ));
            }
            out.push_str(&format!("  {}\n", s.digest));
            if !s.representatives.is_empty() {
                out.push_str(&format!("  e.g. {}\n", s.representatives.join("; ")));
            }
        }
        out
    }

    /// Query the ontology by orthogonal facets (beyond DOMAIN×MODE): filter the
    /// whole loaded ontology by lifecycle/agency/scale/abstraction/sub-domain/
    /// sub-mode. Tolerant by design — an unknown facet value is skipped, never
    /// a hard error (an empty result set is a legitimate answer, e.g. "no
    /// OPERATE+SELL entries").
    #[allow(clippy::too_many_arguments)] // mirrors the facet CLI's flat flags
    pub fn run_facet_query(
        &self,
        kind: Option<&str>,
        domain_filter: Option<&str>,
        mode_filter: Option<&str>,
        lifecycle: Option<&str>,
        agency: Option<&str>,
        scale: Option<&str>,
        abstraction: Option<&str>,
        sub_domain: Option<&str>,
        sub_mode: Option<&str>,
    ) -> String {
        let all_domains: Vec<(&str, &HashMap<String, DomainDef>)> = vec![
            ("office", &self.ontology.office_domains),
            ("human", &self.ontology.human_domains),
            ("machine", &self.ontology.machine_domains),
            ("agent", &self.ontology.agent_domains),
            ("engineering", &self.ontology.engineering_domains),
            ("semiotic", &self.ontology.semiotic_domains),
            ("category", &self.ontology.category_domains),
            ("natural", &self.ontology.natural_domains),
            ("social", &self.ontology.social_domains),
            ("abstract", &self.ontology.abstract_domains),
            // Phase 10 datawork axes: `facet --kind place|period`.
            ("place", &self.ontology.space_domains),
            ("period", &self.ontology.time_domains),
            ("custom", &self.ontology.custom_domains),
        ];

        // Parse facet filters once, tolerantly: an unknown value is ignored
        // (never a hard error) — the user asked "what OPERATE entries exist?",
        // not to prove the spellings.
        let want_lifecycle = facet_parse(lifecycle, &["DESIGN", "BUILD", "OPERATE", "RETIRE"]);
        let want_agency = facet_parse(agency, &["SELF", "OTHER", "AUTOMATED", "COLLECTIVE"]);
        let want_scale = facet_parse(
            scale,
            &["PERSONAL", "INTERPERSONAL", "ORGANIZATIONAL", "CIVIL"],
        );
        let want_abstraction = facet_parse(abstraction, &["CONCRETE", "ABSTRACT"]);
        let want_sub = sub_domain.map(|s| s.to_lowercase());
        let want_sub_mode = sub_mode.map(|s| s.to_lowercase());

        let mut rows: Vec<(String, String, String, String, String, String)> = Vec::new();
        for (kind_name, domains) in &all_domains {
            if let Some(k) = kind {
                if !k.eq_ignore_ascii_case(kind_name) {
                    continue;
                }
            }
            for def in domains.values() {
                let name = def.name.clone();
                let dom = def.domain.clone().unwrap_or_default();
                let mode = def.mode.clone().unwrap_or_default();
                if let Some(d) = domain_filter {
                    if !d.eq_ignore_ascii_case(&dom) {
                        continue;
                    }
                }
                if let Some(m) = mode_filter {
                    if !m.eq_ignore_ascii_case(&mode) {
                        continue;
                    }
                }

                let f = &def.facets;
                if let Some(l) = want_lifecycle {
                    if f.lifecycle.map(|x| x.as_str()) != Some(l) {
                        continue;
                    }
                }
                if let Some(a) = want_agency {
                    if f.agency.map(|x| x.as_str()) != Some(a) {
                        continue;
                    }
                }
                if let Some(s) = want_scale {
                    if f.scale.map(|x| x.as_str()) != Some(s) {
                        continue;
                    }
                }
                if let Some(ab) = want_abstraction {
                    if f.abstraction.map(|x| x.as_str()) != Some(ab) {
                        continue;
                    }
                }
                if let Some(sd) = &want_sub {
                    if f.sub_domain.as_deref().map(|s| s.to_lowercase()).as_ref() != Some(sd) {
                        continue;
                    }
                }
                if let Some(sm) = &want_sub_mode {
                    if f.sub_mode.as_deref().map(|s| s.to_lowercase()).as_ref() != Some(sm) {
                        continue;
                    }
                }

                let facet_str = f.simple();
                rows.push((
                    kind_name.to_string(),
                    dom,
                    mode,
                    name,
                    facet_str,
                    def.axis_kind.clone().unwrap_or_default(),
                ));
            }
        }

        rows.sort_by(|a, b| (&a.0, &a.1, &a.3).cmp(&(&b.0, &b.1, &b.3)));
        if rows.is_empty() {
            return "0 entries match these facets.\n".to_string();
        }
        let mut out = format!("{} entries match\n", rows.len());
        out.push_str(&format!(
            "{:<9} {:<12} {:<10} {:<38} {}\n",
            "KIND", "DOMAIN", "MODE", "ENTRY", "FACETS"
        ));
        for (kind, dom, mode, name, facets, axis) in rows {
            let display = if name.len() > 38 { &name[..35] } else { &name };
            let axis_suffix = if axis.is_empty() {
                String::new()
            } else {
                format!(" ({axis})")
            };
            out.push_str(&format!(
                "{:<9} {:<12} {:<10} {:<38} {}{}\n",
                kind, dom, mode, display, facets, axis_suffix
            ));
        }
        out
    }

    /// Search *inside* the corpus: heading-scoped sections ranked by keyword
    /// and meaning together (PLAN 12.1), with no server running.
    ///
    /// `query` recalls persisted node *labels*; this recalls the text those
    /// labels never captured, which is where the answer usually is. The index
    /// is built per invocation because a CLI process has nowhere to keep one —
    /// the embedding cache on the data dir is what makes the second run cheap.
    #[allow(clippy::too_many_arguments)] // one optional filter per facet; a CLI query is not a builder
    pub fn run_find(
        &self,
        query: &str,
        dir: Option<&std::path::Path>,
        mode: Option<&str>,
        place: Option<&str>,
        period: Option<&str>,
        kind: Option<&str>,
        top_k: usize,
    ) -> String {
        let root = match dir {
            Some(d) => d.to_path_buf(),
            // Default to the configured watch dirs' first entry, then the data
            // dir. Guessing the CWD would silently index whatever directory the
            // operator happened to be standing in.
            None => match self.config.watch_dirs.first() {
                Some(d) => d.clone(),
                None => self.config.data_dir.clone(),
            },
        };
        if !root.exists() {
            return format!(
                "[physis] nothing to search: {} does not exist. Pass --dir, or set \
                 watch_dirs in config.\n",
                root.display()
            );
        }

        let docs = ingest::ingest_all(std::slice::from_ref(&root));
        if docs.is_empty() {
            return format!("[physis] no readable documents under {}\n", root.display());
        }
        let now = chrono::Utc::now();
        // Filesystem metadata, not ingest time: the ingest timestamp is when we
        // read the file, so using it would bucket the whole corpus into the
        // current month and make --period useless.
        let addresses: Vec<Option<crate::address::FileAddress>> = docs
            .iter()
            .map(|d| {
                Some(crate::address::FileAddress::derive(
                    &d.path,
                    &root,
                    crate::address::Mechanism::Cited,
                    "cli-find",
                    now,
                ))
            })
            .collect();

        let texts = crate::content::ContentIndex::retrieval_texts(&root, &docs, &addresses);
        // Without a real semantic model the vectors are a random projection:
        // BM25 over real terms beats cosine over noise, so the semantic channel
        // is left out entirely rather than ranked on confidently.
        let semantic = self.embedder_kind != "random-projection";
        let embeddings = if semantic {
            let mut cache = crate::pack::EmbedCache::load(&self.config.data_dir);
            let out = cache.embed_texts(&texts, self.embedder.as_ref(), self.embedder_kind);
            cache.save(&self.config.data_dir);
            out
        } else {
            Vec::new()
        };
        let index = crate::content::ContentIndex::build(&root, &docs, &addresses, embeddings);

        let opts = crate::content::SearchOptions {
            mode: mode
                .map(crate::content::SearchMode::parse)
                .unwrap_or_default(),
            top_k: top_k.clamp(1, 200),
            place: place.map(str::to_string),
            period: period.map(str::to_string),
            kind: kind.map(str::to_string),
            ..Default::default()
        };
        let q_emb = if index.semantic {
            self.embedder.embed(query)
        } else {
            Vec::new()
        };
        let result = index.search(query, &q_emb, &opts);

        let mut out = String::new();
        out.push_str(&format!(
            "[physis] {} section(s) matched in {} — {} indexed from {} file(s), ranked by {}\n",
            result.matched,
            root.display(),
            result.indexed_sections,
            result.indexed_files,
            result.mode.as_str()
        ));
        if !index.semantic {
            out.push_str(
                "[physis] no semantic model loaded — keyword ranking only \
                 (paraphrases will not match)\n",
            );
        }
        if result.truncated_index {
            out.push_str("[physis] corpus larger than the index cap; showing what was indexed\n");
        }
        if result.hits.is_empty() {
            out.push_str("\nNothing in the corpus says that.\n");
            return out;
        }
        out.push('\n');
        for hit in &result.hits {
            out.push_str(&format!("{:.3}  {}\n", hit.score, hit.citation));
            let facets = [
                hit.place.clone(),
                hit.period.clone(),
                Some(hit.kind.clone()).filter(|k| !k.is_empty()),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
            if !facets.is_empty() {
                out.push_str(&format!("       {facets}\n"));
            }
            // Snippets carry newlines from the source; collapse them so one hit
            // stays one visual block in a terminal.
            let snippet = hit.snippet.split_whitespace().collect::<Vec<_>>().join(" ");
            out.push_str(&format!("       {snippet}\n\n"));
        }
        out
    }
}

/// Parse an optional facet filter value against the known spellings, matching
/// case-insensitively. Unknown/absent → `None` (no filter). Known → the
/// canonical spelling, so "operate" and "OPERATE" both filter correctly.
/// Append a `generated(scan)` event for `dir` to the persisted `_default`
/// ledger, so the CLI and the web server write the same history.
///
/// A free function rather than a method so it is testable without building a
/// whole `PhysisApp` (ontology + embedder + env).
///
/// Failures are reported on stderr and never abort the scan: a scan that
/// worked and failed to note *why* is still a scan, but it must not be silent
/// — an empty `where` with no explanation is exactly the symptom that hid the
/// missing record in the first place.
pub(crate) fn record_scan_cause(db: &sled::Db, dir: &std::path::Path) {
    let mut ledger = match crate::ledger::CausalLedger::load(db, "_default") {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[physis] scan not recorded in the causal ledger: {e}");
            return;
        }
    };
    // Canonical, like the HTTP path: the ledger keys artifacts by the resolved
    // path, so a relative `physis-pro scan .` and an absolute one must record
    // the same object rather than two histories for one directory.
    let object = dir
        .canonicalize()
        .unwrap_or_else(|_| dir.to_path_buf())
        .display()
        .to_string();
    ledger.record(crate::address::Mechanism::Generated, "scan", object, None);
    if let Err(e) = ledger.save(db, "_default") {
        eprintln!("[physis] scan not recorded in the causal ledger: {e}");
    }
}

/// Sort `(label, count)` pairs by count desc, then label — the rollup order
/// for `where` output.
fn sorted_counts(map: &HashMap<String, usize>) -> Vec<(String, usize)> {
    let mut v: Vec<(String, usize)> = map.clone().into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

/// Tiny ASCII bar proportional to `n` out of `total` (max 20 chars).
fn bar(n: usize, total: usize) -> String {
    if total == 0 {
        return String::new();
    }
    let width = (n * 20 / total).clamp(1, 20);
    format!("{:<6} {}", n, "█".repeat(width))
}

fn facet_parse(value: Option<&str>, allowed: &[&'static str]) -> Option<&'static str> {
    let v = value?;
    allowed.iter().find(|a| a.eq_ignore_ascii_case(v)).copied()
}

/// Escape a string for safe inclusion in HTML text/attribute context.
fn html_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

/// Render the studio as a single self-contained HTML document (inline CSS, no JS
/// deps) so a non-technical prospect can open it directly in a browser.
fn render_studio_html(
    graph: &FileThemeGraph,
    report: &crate::discovery::DiscoveryReport,
    dir: &std::path::Path,
    docs: &[ingest::IngestedDoc],
) -> String {
    let mut h =
        String::from("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    h.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    h.push_str("<title>Physis Ontology Studio</title>\n<style>\n");
    h.push_str(
        "body{font-family:-apple-system,Segoe UI,Roboto,Helvetica,Arial,sans-serif;\
        margin:0;color:#1a1f2b;background:#f4f6fa}\n",
    );
    h.push_str(
        ".wrap{max-width:960px;margin:0 auto;padding:32px 20px}\n\
        header{background:linear-gradient(120deg,#1f3a5f,#2a6f97);color:#fff;\
        border-radius:12px;padding:28px 32px;margin-bottom:24px}\n",
    );
    h.push_str("header h1{margin:0 0 6px;font-size:24px}\nheader p{margin:0;opacity:.9}\n");
    h.push_str(
        ".cards{display:flex;gap:12px;flex-wrap:wrap;margin:18px 0}\n\
        .card{flex:1;min-width:150px;background:#fff;border:1px solid #e2e8f0;\
        border-radius:10px;padding:14px 16px}\n",
    );
    h.push_str(".card .n{font-size:26px;font-weight:700;color:#1f3a5f}\n.card .l{font-size:12px;color:#64748b}\n");
    h.push_str("section{background:#fff;border:1px solid #e2e8f0;border-radius:10px;padding:20px 22px;margin-bottom:20px}\n");
    h.push_str("section h2{margin:0 0 12px;font-size:18px;color:#1f3a5f}\n");
    h.push_str(".cat{font-weight:600;color:#334155;margin:14px 0 6px}\n");
    h.push_str(".theme{padding:8px 10px;border-left:3px solid #2a6f97;background:#f8fafc;margin:4px 0;border-radius:0 6px 6px 0}\n");
    h.push_str(".theme .meta{font-size:12px;color:#64748b}\n");
    h.push_str(".gap{border-left:3px solid #e07a3f;background:#fdf6f0;padding:10px 12px;margin:8px 0;border-radius:0 6px 6px 0}\n");
    h.push_str(".gap .name{font-weight:600;color:#b45309}\n.gap .samp{font-size:12px;color:#64748b;margin-top:4px}\n");
    h.push_str(".muted{color:#94a3b8;font-size:13px}\n.footer{color:#94a3b8;font-size:12px;text-align:center;margin-top:8px}\n");
    h.push_str("table.docs{width:100%;border-collapse:collapse;font-size:13px}\n");
    h.push_str("table.docs th,table.docs td{text-align:left;padding:7px 9px;border-bottom:1px solid #eef2f7}\n");
    h.push_str("table.docs th{color:#64748b;font-weight:600;background:#f8fafc}\n");
    h.push_str("</style>\n</head>\n<body>\n<div class=\"wrap\">\n");
    h.push_str(&format!(
        "<header><h1>Physis Ontology Studio</h1><p>Adaptive industrial intelligence — \
        your documents, mapped onto a live, self-growing ontology.</p>\
        <p class=\"muted\">Source: {}</p></header>\n",
        html_escape(&dir.display().to_string()),
    ));

    h.push_str("<div class=\"cards\">\n");
    h.push_str(&format!(
        "<div class=\"card\"><div class=\"n\">{}</div><div class=\"l\">Documents ingested</div></div>\n",
        docs.len(),
    ));
    h.push_str(&format!(
        "<div class=\"card\"><div class=\"n\">{}</div><div class=\"l\">Ontology cells</div></div>\n",
        graph.themes.len(),
    ));
    h.push_str(&format!(
        "<div class=\"card\"><div class=\"n\">{}</div><div class=\"l\">Lines covered</div></div>\n",
        report.covered,
    ));
    h.push_str(&format!(
        "<div class=\"card\"><div class=\"n\">{}</div><div class=\"l\">Lines the grid misses</div></div>\n",
        report.unmapped,
    ));
    h.push_str("</div>\n");

    // Documents table — proves the ingestion line works: real bodies + meta.
    h.push_str("<section><h2>Documents ingested</h2>\n");
    h.push_str("<p class=\"muted\">Real text extraction (PDF/email) + structured fields when the pdf/email features are on.</p>\n");
    h.push_str("<table class=\"docs\"><tr><th>File</th><th>Type</th><th>Extracted</th></tr>\n");
    for d in docs {
        let meta_bits: Vec<String> = vec![
            d.meta.vendor.clone().map(|v| format!("Vendor: {v}")),
            d.meta.date.clone().map(|v| format!("Date: {v}")),
            d.meta.doc_number.clone().map(|v| format!("No: {v}")),
            d.meta.from.clone().map(|v| format!("From: {v}")),
            d.meta.subject.clone().map(|v| format!("Subject: {v}")),
        ]
        .into_iter()
        .flatten()
        .collect();
        let meta_str = if meta_bits.is_empty() {
            if d.body.trim().is_empty() {
                "no text extracted".to_string()
            } else {
                format!("{} chars", d.body.trim().len())
            }
        } else {
            meta_bits.join(" · ")
        };
        h.push_str(&format!(
            "<tr><td>{}</td><td>{:?}</td><td>{}</td></tr>\n",
            html_escape(&d.title),
            d.kind,
            html_escape(&meta_str),
        ));
    }
    h.push_str("</table>\n</section>\n");

    h.push_str("<section><h2>Your ontology coverage</h2>\n");
    h.push_str("<p class=\"muted\">Where your documents land in the shipped 370-cell grid.</p>\n");
    for cat in graph.list_categories() {
        h.push_str(&format!("<div class=\"cat\">{}</div>\n", html_escape(&cat)));
        for t in graph.themes_by_category(&cat) {
            let fc = graph.files_for_theme(t.id).len();
            h.push_str(&format!(
                "<div class=\"theme\"><div>{}</div>\
                <div class=\"meta\">{}×{} · {} file(s) linked</div></div>\n",
                html_escape(&t.name),
                html_escape(&t.domain),
                html_escape(&t.mode),
                fc,
            ));
        }
    }
    h.push_str("</section>\n");

    h.push_str("<section><h2>Where your ontology is thin</h2>\n");
    h.push_str(
        "<p class=\"muted\">Clusters of your text the current grid can't name — \
    these are the domains Physis would propose to add, grown from your own data.</p>\n",
    );
    if report.proposals.is_empty() {
        h.push_str(
            "<p>The current ontology already covers this corpus well — no gaps found.</p>\n",
        );
    } else {
        for p in &report.proposals {
            h.push_str(&format!(
                "<div class=\"gap\"><div class=\"name\">{}</div>\n",
                html_escape(&p.name)
            ));
            h.push_str(&format!(
                "<div class=\"meta\">{} texts · nearest cell {}×{} · coverage {:.2}</div>\n",
                p.count,
                html_escape(&p.domain),
                html_escape(&p.mode),
                p.coverage,
            ));
            h.push_str(&format!(
                "<div class=\"meta\">hints: {}</div>\n",
                html_escape(&p.hints.join(", "))
            ));
            for s in &p.samples {
                h.push_str(&format!("<div class=\"samp\">· {}</div>\n", html_escape(s)));
            }
            h.push_str("</div>\n");
        }
    }
    h.push_str("</section>\n");

    h.push_str(&format!(
        "<div class=\"footer\">Generated by physis-pro studio · best-entry cosine \
        min {:.2} / mean {:.2} / max {:.2} · threshold used {:.2}{}</div>\n",
        report.sim_min,
        report.sim_mean,
        report.sim_max,
        report.threshold_used,
        if report.auto_retuned {
            " (auto-raised)"
        } else {
            ""
        },
    ));
    h.push_str("</div>\n</body>\n</html>\n");
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// Test config with an explicit random-projection primary (PH-009): these
    /// tests exercise query/store logic, not embedding semantics, so they run
    /// offline without model probing (no ollama timeouts, no ONNX loads) and
    /// without racing on the process-wide PHYSIS_EMBEDDER env var.
    fn rp_config(data_dir: Option<std::path::PathBuf>) -> PhysisConfig {
        PhysisConfig {
            data_dir: data_dir.unwrap_or_else(|| PhysisConfig::default().data_dir),
            embedders: crate::config::EmbeddersConfig {
                primary: Some(crate::config::EmbedderConfig {
                    kind: crate::config::EmbedderKindConfig::RandomProjection,
                    model_dir: String::new(),
                    enabled: true,
                }),
                image: None,
            },
            ..PhysisConfig::default()
        }
    }

    #[test]
    fn test_cli_parser_scan() {
        let cli = Cli::try_parse_from(["physis", "scan", "/tmp"]);
        assert!(cli.is_ok());
        match cli.unwrap().command {
            Commands::Scan { dir, format } => {
                assert_eq!(dir, std::path::PathBuf::from("/tmp"));
                assert_eq!(format, "wiki");
            }
            _ => panic!("expected Scan command"),
        }
    }

    #[test]
    fn a_cli_scan_records_why_the_files_exist() {
        // The web `/scan` recorded `generated(scan)`; the CLI recorded nothing,
        // so `physis-pro where` on a server-less install answered "the causal
        // ledger is empty — run a scan" to a user who had just run one.
        let dir = tempfile::tempdir().expect("temp dir");
        let db = sled::open(dir.path().join("state.sled")).expect("sled");
        let scanned = dir.path().join("corpus");
        std::fs::create_dir_all(&scanned).expect("mkdir");

        record_scan_cause(&db, &scanned);

        let ledger = crate::ledger::CausalLedger::load(&db, "_default").expect("reload");
        let events = ledger.events();
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0].mechanism, crate::address::Mechanism::Generated);
        assert_eq!(events[0].actor, "scan");
        assert_eq!(
            events[0].object,
            scanned.canonicalize().unwrap().display().to_string(),
            "the object is the canonical path, so one directory is one history"
        );

        // Appending is idempotent in shape, not in count: a second scan is a
        // second producing act and the history says so.
        record_scan_cause(&db, &scanned);
        let ledger = crate::ledger::CausalLedger::load(&db, "_default").expect("reload");
        assert_eq!(ledger.events().len(), 2);
    }

    #[test]
    fn test_cli_parser_scan_with_format() {
        let cli = Cli::try_parse_from(["physis", "scan", "/tmp", "--format", "json"]);
        assert!(cli.is_ok());
        match cli.unwrap().command {
            Commands::Scan { format, .. } => assert_eq!(format, "json"),
            _ => panic!("expected Scan command"),
        }
    }

    #[test]
    fn test_cli_parser_query() {
        let cli = Cli::try_parse_from(["physis", "query", "hello world"]);
        assert!(cli.is_ok());
        match cli.unwrap().command {
            Commands::Query { query, max_results } => {
                assert_eq!(query, "hello world");
                assert_eq!(max_results, 10);
            }
            _ => panic!("expected Query command"),
        }
    }

    #[test]
    fn test_cli_parser_dream() {
        let cli = Cli::try_parse_from(["physis", "dream"]);
        assert!(cli.is_ok());
        match cli.unwrap().command {
            Commands::Dream { count } => assert_eq!(count, 5),
            _ => panic!("expected Dream command"),
        }
    }

    #[test]
    fn test_cli_parser_evaluate() {
        let cli = Cli::try_parse_from(["physis", "evaluate", "dream-1", "0.85"]);
        assert!(cli.is_ok());
        match cli.unwrap().command {
            Commands::Evaluate { id, grade } => {
                assert_eq!(id, "dream-1");
                assert!((grade - 0.85).abs() < 1e-6);
            }
            _ => panic!("expected Evaluate command"),
        }
    }

    #[test]
    fn test_cli_parser_stats() {
        let cli = Cli::try_parse_from(["physis", "stats"]);
        assert!(cli.is_ok());
        assert!(matches!(cli.unwrap().command, Commands::Stats));
    }

    #[test]
    fn test_cli_parser_config() {
        let cli = Cli::try_parse_from(["physis", "config"]);
        assert!(cli.is_ok());
        assert!(matches!(cli.unwrap().command, Commands::Config));
    }

    #[test]
    fn test_cli_parser_watch() {
        let cli = Cli::try_parse_from(["physis", "watch", "/dir1", "/dir2"]);
        assert!(cli.is_ok());
        match cli.unwrap().command {
            Commands::Watch { dirs } => assert_eq!(dirs.len(), 2),
            _ => panic!("expected Watch command"),
        }
    }

    #[test]
    fn test_cli_parser_unknown_command_fails() {
        let cli = Cli::try_parse_from(["physis", "bogus"]);
        assert!(cli.is_err());
    }

    #[test]
    fn test_cli_parser_missing_arg_fails() {
        let cli = Cli::try_parse_from(["physis", "query"]);
        assert!(cli.is_err());
    }

    #[test]
    fn test_cli_command_factory_usage() {
        let cli = Cli::command();
        let name = cli.get_name();
        assert_eq!(name, "physis");
    }

    #[test]
    fn test_app_new_with_default_config() {
        let config = rp_config(None);
        let app = PhysisApp::new(config);
        assert!(app.goals.is_empty());
        assert!(app.actor.stats(&[]).total_actions == 0);
    }

    #[test]
    fn test_run_stats_empty() {
        let config = rp_config(None);
        let app = PhysisApp::new(config);
        let stats = app.run_stats();
        assert!(stats.contains("Physis Stats"));
        assert!(stats.contains("total_actions: 0"));
    }

    #[test]
    fn test_run_query_empty() {
        let config = rp_config(None);
        let app = PhysisApp::new(config);
        let results = app.run_query("nonexistent", 5);
        assert!(results.is_empty());
    }

    /// Regression (QA 8.2): the trie is built by an in-process scan, so a
    /// fresh `query` process found nothing even with a full node store on
    /// disk. `run_query_nodes` must recall labels from sled instead.
    #[test]
    fn test_run_query_nodes_recalls_persisted_labels() {
        let dir = tempfile::tempdir().unwrap();
        let config = rp_config(Some(dir.path().to_path_buf()));

        // "First process": register labeled nodes (what /scan and watch do).
        {
            let mut app = PhysisApp::new(config.clone());
            app.register_persisted_node("quarterly budget planning")
                .unwrap();
            app.register_persisted_node("espresso machine descaling")
                .unwrap();
        }

        // "Second process": empty trie, yet the store answers.
        let app = PhysisApp::new(config);
        assert!(
            app.run_query("budget", 5).is_empty(),
            "trie is process-local"
        );
        let hits = app.run_query_nodes("budget", 5);
        assert!(
            !hits.is_empty(),
            "persisted nodes must answer after restart"
        );
        assert!(
            hits.iter()
                .any(|(_, label, _)| label == "quarterly budget planning"),
            "the registered label should come back, got {hits:?}"
        );
        let unrelated = app.run_query_nodes("descaling", 5);
        assert!(unrelated
            .iter()
            .any(|(_, l, _)| l == "espresso machine descaling"));
    }

    /// With no store on disk the fallback degrades to empty, not an error.
    #[test]
    fn test_run_query_nodes_without_store_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let config = rp_config(Some(dir.path().to_path_buf()));
        let app = PhysisApp::new(config);
        assert!(app.run_query_nodes("anything", 5).is_empty());
    }

    /// `run_note` is the CLI twin of MCP `physis_remember`: the label lands
    /// in the shared store with its verdict, recallable after "restart", and
    /// empty text is rejected rather than persisted as an unlabeled node.
    #[test]
    fn test_run_note_remembers_with_verdict() {
        let dir = tempfile::tempdir().unwrap();
        let config = rp_config(Some(dir.path().to_path_buf()));
        let mut app = PhysisApp::new(config.clone());
        let out = app
            .run_note("clippy lint E0599 on coherence edit", -1.0)
            .unwrap();
        assert!(out.contains("verdict -1.0"), "verdict echoed, got {out}");
        assert!(app.run_note("   ", 0.0).is_err(), "empty note rejected");
        // Drop before the "second process": the live sled handle holds an
        // exclusive lock, so a second opener on the same dir would fail.
        drop(app);

        // Second process: the verdict survived the round-trip to sled.
        let app = PhysisApp::new(config);
        let hits = app.run_query_nodes("coherence edit", 5);
        assert!(
            hits.iter()
                .any(|(_, l, _)| l == "clippy lint E0599 on coherence edit"),
            "note recallable, got {hits:?}"
        );
    }

    #[test]
    fn test_run_dream_empty_goals() {
        let config = rp_config(None);
        let mut app = PhysisApp::new(config);
        let dreams = app.run_dream(3);
        assert_eq!(dreams.len(), 0, "no goals means no dreams");
    }

    #[test]
    fn test_run_evaluate_nonexistent() {
        let config = rp_config(None);
        let mut app = PhysisApp::new(config);
        let result = app.run_evaluate("bogus", 0.5);
        assert!(!result);
    }
}
