//! MCP (Model Context Protocol) server — expose Physis as an
//! MCP service on `127.0.0.1:9876`.

#[cfg(feature = "mcp")]
mod mcp_impl {
    /// Cells reported by `physis_classify`, and exemplar entries per cell.
    /// Enough to recognise the classification; not the whole ontology.
    const CLASSIFY_TOP_CELLS: usize = 5;
    const CLASSIFY_EXAMPLES: usize = 5;

    /// Histogram rows and doc rows reported by `physis_workflow`.
    const WORKFLOW_HEAD: usize = 10;
    const WORKFLOW_DOCS: usize = 15;

    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    use rmcp::handler::server::ServerHandler;
    use rmcp::model::{
        CallToolRequestParam, CallToolResult, Content, ListToolsResult, PaginatedRequestParam,
        ServerCapabilities, ServerInfo, Tool,
    };
    use rmcp::service::{RequestContext, RoleServer, ServiceExt};
    use rmcp::Error as McpError;
    use serde_json::json;
    use serde_json::Value as JsonValue;

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use crate::actor::PDCActor;
    use crate::classify::CellClassifier;
    use crate::config::{OntologyLoader, PhysisConfig};
    use crate::dream::DreamEngine;
    use crate::embed::VectorEmbed;
    use crate::gantt::{AbstractDomain, GanttScheduler};
    use crate::mapper::OntologyMapper;
    use crate::models::{Experience, Goal};

    /// Shared state served by the MCP server.
    pub struct PhysisContext {
        pub config: PhysisConfig,
        pub mapper: OntologyMapper,
        pub actor: PDCActor,
        pub dreams: DreamEngine,
        pub goals: Vec<Goal>,
        pub embedder: Box<dyn VectorEmbed>,
        pub classifier: CellClassifier,
        /// Provenance label of the active embedder, used to namespace the
        /// embedding cache so vectors from two models never mix.
        pub embedder_kind: &'static str,
        /// Persistent chunk-embedding cache shared by every `physis_pack` call.
        pub pack_cache: crate::pack::EmbedCache,
        /// Last content index built by `physis_find`, with the root it covers.
        ///
        /// Rebuilding it per call re-ingests and re-tokenizes the whole tree —
        /// seconds of work to answer a question that takes milliseconds once the
        /// index exists, which is the difference between a tool an agent reaches
        /// for and one it avoids. Reused only when the root matches and the
        /// tree's signature is unchanged, so an edit is still picked up.
        pub find_index: Option<(std::path::PathBuf, crate::content::ContentIndex)>,
        pub scheduler: GanttScheduler,
        pub experiences: Vec<Experience>,
        /// Semiotic memory (Phase 10c A1): the same store the web server and
        /// CLI write — a mounted agent reads and writes the real graph, not a
        /// private sandbox.
        pub core: crate::core::PhysisCore,
        /// Why-made history over that memory.
        pub ledger: crate::ledger::CausalLedger,
        /// Shared sled store (`{data_dir}/state.sled`); `None` when the
        /// directory cannot be opened — tools then work in-memory only.
        pub db: Option<sled::Db>,
        /// Declarative agent route table (Phase 19 B1): semantic cells bound to
        /// harness actions, persisted to `{data_dir}/routes.json`.
        pub routes: crate::routes::RouteTable,
    }

    /// Thread-safe shared reference.
    pub type SharedContext = Arc<Mutex<PhysisContext>>;

    /// Handler implementing the MCP ServerHandler trait for Physis.
    #[derive(Clone)]
    pub struct PhysisMcpServer {
        ctx: SharedContext,
    }

    impl PhysisMcpServer {
        pub fn new(config: PhysisConfig) -> Self {
            // Startup has to fit inside an MCP client's handshake timeout, so
            // every phase is timed on stderr rather than guessed at.
            let t0 = std::time::Instant::now();
            let lap = |t: &mut std::time::Instant, what: &str| {
                eprintln!("physis: startup {what} {:.2}s", t.elapsed().as_secs_f32());
                *t = std::time::Instant::now();
            };
            let mut t = t0;

            let ontology = OntologyLoader::load_all(&config);
            lap(&mut t, "ontology-load");
            let mapper = OntologyMapper::new(ontology.clone(), config.embed_dim);
            let actor = PDCActor::new(config.pdca_stagnant_threshold, config.pdca_stagnant_window);
            let dreams = DreamEngine::new();
            lap(&mut t, "mapper+actor+dreams");
            // Same selection cascade the CLI and web server use. Hard-coding
            // RandomProjectionEmbedder here made every tool answer from random
            // projections instead of semantics — silently, since a random
            // projection returns plausible-looking scores for any query.
            let (embedder, embedder_kind) = crate::embed::select_embedder(&config);
            eprintln!(
                "physis: MCP embedder = {embedder_kind} (dim {})",
                embedder.dimension()
            );
            lap(&mut t, "select-embedder");
            // Load the cache first: the classifier is the expensive part of
            // startup, and it is exactly what the cache is for.
            let mut pack_cache = crate::pack::EmbedCache::load(&config.data_dir);
            let classifier = CellClassifier::build_cached(
                &ontology,
                embedder.as_ref(),
                embedder_kind,
                &mut pack_cache,
            );
            let (hits, misses) = pack_cache.stats();
            eprintln!("physis: ontology seeds embedded — {hits} cached, {misses} computed");
            pack_cache.save(&config.data_dir);
            lap(&mut t, "classifier");
            // Memory (Phase 10c A1): open the shared store and rehydrate the
            // default tenant, so a mounted agent reads/writes the same graph
            // the web server and CLI do. Failure degrades to session-only.
            let db = sled::open(config.data_dir.join("state.sled"))
                .map_err(|e| eprintln!("physis: sled unavailable ({e}); memory is session-only"))
                .ok();
            let mut core = crate::core::PhysisCore::new();
            core.set_embedder_id(embedder_kind);
            if let Some(db) = &db {
                let (loaded, skipped) = core.load_nodes(db, "_default");
                if loaded > 0 || skipped > 0 {
                    eprintln!("physis: memory restored {loaded} nodes ({skipped} skipped)");
                }
            }
            let ledger = db
                .as_ref()
                .and_then(
                    |db| match crate::ledger::CausalLedger::load(db, "_default") {
                        Ok(l) => Some(l),
                        Err(e) => {
                            eprintln!("physis: ledger load failed ({e}); starting empty");
                            None
                        }
                    },
                )
                .unwrap_or_default();
            lap(&mut t, "memory");
            eprintln!("physis: startup TOTAL {:.2}s", t0.elapsed().as_secs_f32());
            // Computed before `config` moves into the context below.
            let routes_path = config.data_dir.join("routes.json");
            let ctx = Arc::new(Mutex::new(PhysisContext {
                config,
                mapper,
                actor,
                dreams,
                goals: Vec::new(),
                embedder,
                classifier,
                embedder_kind,
                pack_cache,
                find_index: None,
                scheduler: GanttScheduler::new(Box::new(AbstractDomain)),
                experiences: Vec::new(),
                core,
                ledger,
                db,
                routes: crate::routes::RouteTable::load(&routes_path),
            }));
            PhysisMcpServer { ctx }
        }
    }

    fn tool(name: &'static str, description: &'static str, input_schema: JsonValue) -> Tool {
        Tool::new(
            name,
            description,
            input_schema.as_object().cloned().unwrap_or_default(),
        )
    }

    /// Tool results go to a language model, not a parser, so they are rendered
    /// as indented text: the model spends no tokens on braces and quotes, and
    /// long snippets stay readable instead of arriving as escaped one-liners.
    fn json_content(value: &JsonValue) -> Result<Content, McpError> {
        Ok(Content::text(super::render_value(value, 0)))
    }

    #[inline]
    fn args_map(
        request: &CallToolRequestParam,
    ) -> Result<&serde_json::Map<String, JsonValue>, McpError> {
        request
            .arguments
            .as_ref()
            .ok_or_else(|| McpError::invalid_params("no arguments provided", None))
    }

    fn get_str(request: &CallToolRequestParam, name: &str) -> Result<String, McpError> {
        args_map(request)?
            .get(name)
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| McpError::invalid_params(format!("missing string arg: {name}"), None))
    }

    fn get_f64(request: &CallToolRequestParam, name: &str) -> Result<f64, McpError> {
        args_map(request)?
            .get(name)
            .and_then(|v| v.as_f64())
            .ok_or_else(|| McpError::invalid_params(format!("missing numeric arg: {name}"), None))
    }

    fn get_i64(request: &CallToolRequestParam, name: &str) -> Option<i64> {
        let args = request.arguments.as_ref()?;
        args.get(name).and_then(|v| v.as_i64())
    }

    fn get_vec_f64(request: &CallToolRequestParam, name: &str) -> Result<Vec<f32>, McpError> {
        args_map(request)?
            .get(name)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_f64())
                    .map(|f| f as f32)
                    .collect()
            })
            .ok_or_else(|| McpError::invalid_params(format!("missing array arg: {name}"), None))
    }

    impl ServerHandler for PhysisMcpServer {
        fn get_info(&self) -> ServerInfo {
            ServerInfo {
                protocol_version: Default::default(),
                capabilities: ServerCapabilities::builder().enable_tools().build(),
                server_info: rmcp::model::Implementation {
                    name: "physis-pro".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
                instructions: Some(
                    "Physis industrial backoffice intelligence — classify, embed, goals, PDCA, dreams, and query."
                        .to_string(),
                ),
            }
        }

        async fn list_tools(
            &self,
            _request: PaginatedRequestParam,
            _context: RequestContext<RoleServer>,
        ) -> Result<ListToolsResult, McpError> {
            Ok(ListToolsResult {
                next_cursor: None,
                tools: vec![
                    tool("physis_health", "Check server health and embedder status", json!({
                        "type": "object", "properties": {}, "required": []
                    })),
                    tool("physis_classify", "Classify text against the ontology grid", json!({
                        "type": "object", "properties": {
                            "text": {"type": "string", "description": "Text to classify"}
                        }, "required": ["text"]
                    })),
                    tool("physis_embed", "Embed text into a vector", json!({
                        "type": "object", "properties": {
                            "text": {"type": "string", "description": "Text to embed"}
                        }, "required": ["text"]
                    })),
                    tool("physis_route", "Route text to the action bound to its semantic cell", json!({
                        "type": "object", "properties": {
                            "text": {"type": "string", "description": "Text to route to a cell-bound action"}
                        }, "required": ["text"]
                    })),
                    tool("physis_goals_list", "List all current goals with progress", json!({
                        "type": "object", "properties": {}, "required": []
                    })),
                    tool("physis_goals_add", "Add a new goal from a prompt", json!({
                        "type": "object", "properties": {
                            "prompt": {"type": "string", "description": "Goal description"}
                        }, "required": ["prompt"]
                    })),
                    tool("physis_goals_remove", "Remove a goal by ID", json!({
                        "type": "object", "properties": {
                            "id": {"type": "string", "description": "Goal ID to remove"}
                        }, "required": ["id"]
                    })),
                    tool("physis_pdca_plan", "Get the PDCA plan — which goals need action", json!({
                        "type": "object", "properties": {}, "required": []
                    })),
                    tool("physis_pdca_act", "Execute a PDCA action on a goal", json!({
                        "type": "object", "properties": {
                            "goal_id": {"type": "string", "description": "Goal ID"},
                            "state_vector": {"type": "array", "items": {"type": "number"}, "description": "New state vector"}
                        }, "required": ["goal_id", "state_vector"]
                    })),
                    tool("physis_pdca_stats", "Get PDCA cycle statistics", json!({
                        "type": "object", "properties": {}, "required": []
                    })),
                    tool("physis_dream_generate", "Generate novel dream vectors from goals", json!({
                        "type": "object", "properties": {
                            "count": {"type": "integer", "description": "Number of dreams", "default": 5}
                        }, "required": []
                    })),
                    tool("physis_dream_evaluate", "Evaluate (grade) a dream by ID", json!({
                        "type": "object", "properties": {
                            "id": {"type": "string", "description": "Dream ID"},
                            "grade": {"type": "number", "description": "Grade 0.0–1.0"}
                        }, "required": ["id", "grade"]
                    })),
                    tool("physis_query", "Query the ontology mapper", json!({
                        "type": "object", "properties": {
                            "query": {"type": "string", "description": "Search query"},
                            "max": {"type": "integer", "description": "Max results", "default": 10}
                        }, "required": ["query"]
                    })),
                    tool("physis_stats", "Get engine statistics", json!({
                        "type": "object", "properties": {}, "required": []
                    })),
                    tool("physis_wiki_search", "Search the physis wiki documentation (Tantivy BM25)", json!({
                        "type": "object", "properties": {
                            "query": {"type": "string", "description": "Search query"}
                        }, "required": ["query"]
                    })),
                    tool("physis_scan", "Scan a directory and populate the ontology trie", json!({
                        "type": "object", "properties": {
                            "path": {"type": "string", "description": "Directory to scan (default: data dir)"}
                        }, "required": []
                    })),
                    tool("physis_workflow", "Workflow audit — check documentation coverage for AI agents and humans, timestamps/metadata, and who did what where across a directory (walks harness dirs like .claude, .opencode, .hermes the base scan skips; attributes git authors when available)", json!({
                        "type": "object", "properties": {
                            "path": {"type": "string", "description": "Directory to audit (default: data dir)"}
                        }, "required": []
                    })),
                    tool("physis_gantt_schedule", "Manage the Gantt schedule — add tasks, recompute CPM, get status", json!({
                        "type": "object", "properties": {
                            "action": {"type": "string", "enum": ["add_task", "recompute", "status"], "description": "Action to perform"},
                            "task_id": {"type": "string", "description": "Task ID (used with add_task)"},
                            "duration_secs": {"type": "number", "description": "Duration in seconds (used with add_task)"},
                            "dependencies": {"type": "array", "items": {"type": "string"}, "description": "Task dependency IDs (used with add_task)"}
                        }, "required": ["action"]
                    })),
                    tool("physis_gantt_cross_correlate", "Cross-correlate PDCA progress with schedule adherence", json!({
                        "type": "object", "properties": {}, "required": []
                    })),
                    tool("physis_dream_advanced", "Generate advanced dreams with schedule/countermeasure context", json!({
                        "type": "object", "properties": {
                            "count": {"type": "integer", "description": "Number of dreams", "default": 5},
                            "context": {"type": "string", "enum": ["schedule", "countermeasure", "interpolation", "mixed"], "description": "Dream context type", "default": "mixed"}
                        }, "required": []
                    })),
                    tool("physis_pack", "Pack the highest-value context for a query from a local directory into a hard token budget. Runs entirely locally — no model API is called. Returns the assembled context plus per-chunk provenance (path and line range) and the number of candidates that did not fit.", json!({
                        "type": "object", "properties": {
                            "query": {"type": "string", "description": "What the context is needed for"},
                            "root": {"type": "string", "description": "Directory to pack from", "default": "."},
                            "budget": {"type": "integer", "description": "Hard token ceiling — never exceeded", "default": 4000},
                            "top_k": {"type": "integer", "description": "Maximum chunks to consider", "default": 50},
                            "diversity": {"type": "number", "description": "0 = pure relevance, 1 = maximum spread", "default": 0.3}
                        }, "required": ["query"]
                    })),
                    tool("physis_find", "Search INSIDE documents: heading-scoped sections of every file under a directory, ranked by keyword (BM25) and meaning (cosine) together. Use this when the answer is written in a file rather than remembered as a label — physis_recall ranks labels, this ranks text. Quote a span to require it verbatim. Returns each hit's file, heading chain and a snippet.", json!({
                        "type": "object", "properties": {
                            "query": {"type": "string", "description": "What to look for; \"quoted spans\" must appear verbatim"},
                            "root": {"type": "string", "description": "Directory to search", "default": "."},
                            "mode": {"type": "string", "description": "hybrid (default) | semantic | lexical"},
                            "place": {"type": "string", "description": "Folder facet, relative to the search root"},
                            "period": {"type": "string", "description": "Period facet, YYYY-MM"},
                            "kind": {"type": "string", "description": "File extension without the dot"},
                            "top_k": {"type": "integer", "default": 8}
                        }, "required": ["query"]
                    })),
                    // ── memory + why-made (Phase 10c A1) ──
                    tool("physis_remember", "Register a labeled memory into the shared semiotic graph (deduped by label — re-posting the same text updates it). Optional verdict in −1..1 records whether it works (+1), is inert (0), or fails (−1).", json!({
                        "type": "object", "properties": {
                            "input": {"type": "string", "description": "The text to remember; also becomes its searchable label"},
                            "verdict": {"type": "number", "description": "Coherence verdict −1..1 (optional)"}
                        }, "required": ["input"]
                    })),
                    tool("physis_recall", "Semantic recall over everything remembered (nodes + goals): embeds the query and returns the top labels packed to a token budget. This reads the graph; grounded LLM synthesis over these matches is available via the HTTP /api/v1/ask endpoint.", json!({
                        "type": "object", "properties": {
                            "query": {"type": "string"},
                            "top_k": {"type": "integer", "default": 8},
                            "token_budget": {"type": "integer", "default": 600}
                        }, "required": ["query"]
                    })),
                    tool("physis_why", "Why-made chain for one artifact from the causal ledger: what produced it and from what (lineage), what it fed (descendants), which goals it serves (purposes).", json!({
                        "type": "object", "properties": {
                            "path": {"type": "string", "description": "Artifact path as recorded in the ledger"}
                        }, "required": ["path"]
                    })),
                    tool("physis_serve", "Record that an artifact serves a goal (final cause). The goal must exist — list ids with physis_goals_list.", json!({
                        "type": "object", "properties": {
                            "object": {"type": "string", "description": "Artifact path"},
                            "goal_id": {"type": "string"}
                        }, "required": ["object", "goal_id"]
                    })),
                    tool("physis_causes", "Browse recorded causes: filter the ledger by free text, mechanism (generated/received/derived/corrected/cited/serves) or actor, optionally within the last N days. Newest first.", json!({
                        "type": "object", "properties": {
                            "query": {"type": "string", "description": "Free text over object/input/actor"},
                            "mechanism": {"type": "string"},
                            "within_days": {"type": "integer"}
                        }
                    })),
                    tool("physis_coherence_query", "Typed facet query over the live coherence graph — the 'objects' API. Filter nodes by asserted outcome (verdict=success|inert|failure), by semiotic cell (cell=Domain|Mode, e.g. Finance|Procure), and/or by label substring (q). Returns the same node projection as the HTTP /api/v1/coherence/nodes/all, so an agent can ask 'every failed node in Finance|Procure' without pulling the whole store.", json!({
                        "type": "object", "properties": {
                            "verdict": {"type": "string", "enum": ["success", "inert", "failure"], "description": "Only nodes with this asserted outcome"},
                            "cell": {"type": "string", "description": "Only nodes in this semiotic cell, as 'Domain|Mode'"},
                            "q": {"type": "string", "description": "Case-insensitive substring match against the node label"},
                            "limit": {"type": "integer", "default": 500}
                        }
                    })),
                    tool("physis_discover", "Ontology-gap discovery over a corpus: given a list of `texts`, report how many map to a known cell, how many the current grid misses, and draft new ontology domains for the gaps (loadable via EXTRA_ONTOLOGIES). This is how an agent grows the grid from its own documents — offline, no model needed.", json!({
                        "type": "object", "properties": {
                            "texts": {"type": "array", "items": {"type": "string"}, "description": "Corpus lines/texts to check against the live ontology"},
                            "min_cluster": {"type": "integer", "description": "Minimum gap-cluster size to propose as a new domain (default 3)"}
                        },
                        "required": ["texts"]
                    })),
                    tool("physis_themes", "Recurring themes across the remembered graph: cluster the coherence nodes into communities and summarize each (Phase 11). Returns, per community, its title, distinctive keywords, dominant semiotic cell / place / period / mechanism, outcome tally (positive/negative), and a deterministic digest. Offline and model-free — the shape of the corpus rather than one document. Use it to answer 'what do we keep failing at' or 'what recurs'.", json!({
                        "type": "object", "properties": {
                            "min_size": {"type": "integer", "description": "Only return communities with at least this many members (default 1)"}
                        }
                    })),
                ],
            })
        }

        async fn call_tool(
            &self,
            request: CallToolRequestParam,
            _context: RequestContext<RoleServer>,
        ) -> Result<CallToolResult, McpError> {
            match request.name.as_ref() {
                "physis_health" => {
                    let ctx = self.ctx.lock().await;
                    let result = json!({
                        "status": "ok",
                        "goals": ctx.goals.len(),
                        "dreams": ctx.dreams.dreams.len(),
                        "classifier_cells": ctx.classifier.cell_count(),
                    });
                    Ok(CallToolResult::success(vec![json_content(&result)?]))
                }

                "physis_pack" => {
                    let query = get_str(&request, "query")?;
                    let root = get_str(&request, "root").unwrap_or_else(|_| ".".to_string());
                    let budget = get_i64(&request, "budget").unwrap_or(4000).max(1) as usize;
                    let top_k = get_i64(&request, "top_k").unwrap_or(50).max(1) as usize;
                    let diversity = get_f64(&request, "diversity").unwrap_or(0.3) as f32;

                    // The root arrives from the agent, and an agent acting on a
                    // poisoned instruction will ask for `/etc` or `../../..`.
                    // Confine to PHYSIS_SCAN_ROOT when the operator set one.
                    let confine = std::env::var_os("PHYSIS_SCAN_ROOT").map(PathBuf::from);
                    let root = match crate::pack::resolve_root(
                        std::path::Path::new(&root),
                        confine.as_deref(),
                    ) {
                        Ok(p) => p,
                        Err(e) => {
                            return Ok(CallToolResult::error(vec![json_content(
                                &json!({"error": e.to_string()}),
                            )?]))
                        }
                    };

                    let mut ctx = self.ctx.lock().await;
                    // Split the borrow: `pack_cached` needs `&mut` on the cache
                    // and `&` on the embedder, which the borrow checker will not
                    // grant through a single `ctx` reference.
                    let PhysisContext {
                        embedder,
                        embedder_kind,
                        pack_cache,
                        config,
                        ..
                    } = &mut *ctx;

                    let result = crate::pack::pack_cached(
                        &root,
                        &query,
                        budget,
                        top_k,
                        diversity,
                        embedder.as_ref(),
                        embedder_kind,
                        pack_cache,
                    );
                    // Persist so a later session starts warm. Cheap next to the
                    // embedding it avoids.
                    pack_cache.save(&config.data_dir);
                    Ok(CallToolResult::success(vec![json_content(&json!(result))?]))
                }

                "physis_find" => {
                    let query = get_str(&request, "query")?;
                    let root = get_str(&request, "root").unwrap_or_else(|_| ".".to_string());
                    let top_k = get_i64(&request, "top_k").unwrap_or(8).max(1) as usize;
                    let mode = get_str(&request, "mode").ok();
                    let place = get_str(&request, "place").ok();
                    let period = get_str(&request, "period").ok();
                    let kind = get_str(&request, "kind").ok();

                    // Same confinement as physis_pack: an agent acting on a
                    // poisoned instruction will ask for `/etc` or `../../..`.
                    let confine = std::env::var_os("PHYSIS_SCAN_ROOT").map(PathBuf::from);
                    let root = match crate::pack::resolve_root(
                        std::path::Path::new(&root),
                        confine.as_deref(),
                    ) {
                        Ok(p) => p,
                        Err(e) => {
                            return Ok(CallToolResult::error(vec![json_content(
                                &json!({"error": e.to_string()}),
                            )?]))
                        }
                    };

                    let mut ctx = self.ctx.lock().await;
                    let PhysisContext {
                        embedder,
                        embedder_kind,
                        pack_cache,
                        config,
                        find_index,
                        ..
                    } = &mut *ctx;

                    // A cached index for this root is reused only while the
                    // tree's signature holds; any edit invalidates it and the
                    // rebuild below runs, so a reused index is never a stale one.
                    let signature = crate::content::scan_signature(&root);
                    let cached = find_index
                        .as_ref()
                        .is_some_and(|(r, ix)| *r == root && ix.signature == signature);
                    if !cached {
                    let docs = crate::ingest::ingest_all(std::slice::from_ref(&root));
                    if docs.is_empty() {
                        return Ok(CallToolResult::success(vec![json_content(&json!({
                            "root": root.display().to_string(),
                            "matched": 0,
                            "hits": [],
                            "note": "no readable documents under that directory",
                        }))?]));
                    }
                    let now = chrono::Utc::now();
                    // Addresses from filesystem metadata, not ingest time — the
                    // latter would bucket the whole corpus into this month and
                    // make the period facet meaningless.
                    let addresses: Vec<Option<crate::address::FileAddress>> = docs
                        .iter()
                        .map(|d| {
                            Some(crate::address::FileAddress::derive(
                                &d.path,
                                &root,
                                crate::address::Mechanism::Cited,
                                "mcp-find",
                                now,
                            ))
                        })
                        .collect();

                    let texts =
                        crate::content::ContentIndex::retrieval_texts(&root, &docs, &addresses);
                    // Random-projection vectors rank confidently and wrongly, so
                    // the semantic channel is omitted rather than faked; the
                    // response reports which ranking actually ran.
                    let semantic = *embedder_kind != "random-projection";
                    let embeddings = if semantic {
                        let out = pack_cache.embed_texts(&texts, embedder.as_ref(), embedder_kind);
                        pack_cache.save(&config.data_dir);
                        out
                    } else {
                        Vec::new()
                    };
                    *find_index = Some((
                        root.clone(),
                        crate::content::ContentIndex::build(&root, &docs, &addresses, embeddings),
                    ));
                    }
                    let index = &find_index
                        .as_ref()
                        .expect("index built or reused just above")
                        .1;
                    let opts = crate::content::SearchOptions {
                        mode: mode
                            .as_deref()
                            .map(crate::content::SearchMode::parse)
                            .unwrap_or_default(),
                        top_k: top_k.clamp(1, 50),
                        place,
                        period,
                        kind,
                        ..Default::default()
                    };
                    let q_emb = if index.semantic {
                        embedder.embed(&query)
                    } else {
                        Vec::new()
                    };
                    let result = index.search(&query, &q_emb, &opts);

                    let hits: Vec<JsonValue> = result
                        .hits
                        .iter()
                        .map(|h| {
                            json!({
                                "score": h.score,
                                "path": h.path,
                                "section": h.citation,
                                "snippet": h.snippet,
                                "place": h.place,
                                "period": h.period,
                            })
                        })
                        .collect();
                    Ok(CallToolResult::success(vec![json_content(&json!({
                        "root": root.display().to_string(),
                        "matched": result.matched,
                        "indexed_sections": result.indexed_sections,
                        "indexed_files": result.indexed_files,
                        "mode": result.mode.as_str(),
                        "semantic_index": index.semantic,
                        "hits": hits,
                    }))?]))
                }

                "physis_classify" => {
                    let text = get_str(&request, "text")?;
                    let ctx = self.ctx.lock().await;
                    let embedding = ctx.embedder.embed(&text);
                    let results = ctx.classifier.classify(&embedding);
                    // `CellScore::entries` is the cell's whole membership — the
                    // same few hundred labels for every query that lands there,
                    // ~29 KB of them. The caller needs to recognise the cell, so
                    // a handful of exemplars per cell is the whole signal.
                    let trimmed: Vec<JsonValue> = results
                        .iter()
                        .take(CLASSIFY_TOP_CELLS)
                        .map(|c| {
                            json!({
                                "cell": format!("{}/{}", c.domain, c.mode),
                                "score": c.score,
                                "examples": c.entries.iter().take(CLASSIFY_EXAMPLES).collect::<Vec<_>>(),
                                "entries": c.entries.len(),
                            })
                        })
                        .collect();
                    let result = json!({"results": trimmed, "cells_matched": results.len()});
                    Ok(CallToolResult::success(vec![json_content(&result)?]))
                }

                "physis_embed" => {
                    let text = get_str(&request, "text")?;
                    let ctx = self.ctx.lock().await;
                    let embedding = ctx.embedder.embed(&text);
                    let result = json!({"embedding": embedding, "dim": embedding.len()});
                    Ok(CallToolResult::success(vec![json_content(&result)?]))
                }

                "physis_route" => {
                    let text = get_str(&request, "text")?;
                    let ctx = self.ctx.lock().await;
                    let embedding = ctx.embedder.embed(&text);
                    let results = ctx.classifier.classify(&embedding);
                    let (domain, mode) = match results.first() {
                        Some(c) => (c.domain.clone(), c.mode.clone()),
                        None => (String::new(), String::new()),
                    };
                    let routes: Vec<JsonValue> = ctx
                        .routes
                        .for_cell(&domain, &mode)
                        .iter()
                        .map(|r| {
                            json!({
                                "name": r.name,
                                "cell": r.cell,
                                "action": r.action,
                                "examples": r.examples,
                                "enabled": r.enabled,
                            })
                        })
                        .collect();
                    let result = json!({
                        "cell": format!("{domain}/{mode}"),
                        "routes": routes,
                        "count": routes.len(),
                    });
                    Ok(CallToolResult::success(vec![json_content(&result)?]))
                }

                "physis_goals_list" => {
                    let ctx = self.ctx.lock().await;
                    let goals: Vec<JsonValue> = ctx.goals.iter().map(|g| {
                        json!({"id": g.id, "progress": g.progress, "domain": g.domain_name})
                    }).collect();
                    let result = json!({"goals": goals, "count": goals.len()});
                    Ok(CallToolResult::success(vec![json_content(&result)?]))
                }

                "physis_goals_add" => {
                    let prompt = get_str(&request, "prompt")?;
                    let mut ctx = self.ctx.lock().await;
                    let embedding = ctx.embedder.embed(&prompt);
                    let mut goal = Goal::new_vec(embedding);
                    goal.progress = 0.0;
                    let id = goal.id.clone();
                    ctx.goals.push(goal);
                    let result = json!({"id": id, "message": format!("goal created ({} chars)", prompt.len())});
                    Ok(CallToolResult::success(vec![json_content(&result)?]))
                }

                "physis_goals_remove" => {
                    let id = get_str(&request, "id")?;
                    let mut ctx = self.ctx.lock().await;
                    let before = ctx.goals.len();
                    ctx.goals.retain(|g| g.id != id);
                    let removed = before - ctx.goals.len();
                    Ok(CallToolResult::success(vec![json_content(
                        &json!({"removed": removed > 0}),
                    )?]))
                }

                "physis_pdca_plan" => {
                    let ctx = self.ctx.lock().await;
                    let priority_map: Option<std::collections::HashMap<String, f32>> = {
                        let cp: Vec<&str> = ctx
                            .scheduler
                            .tasks
                            .iter()
                            .filter(|t| t.on_critical_path)
                            .map(|t| t.id.as_str())
                            .collect();
                        if cp.is_empty() {
                            None
                        } else {
                            let mut m = std::collections::HashMap::new();
                            for t in cp {
                                m.insert(t.to_string(), 1.0);
                            }
                            Some(m)
                        }
                    };
                    let planned = ctx
                        .actor
                        .plan_with_schedule(&ctx.goals, priority_map.as_ref());
                    let goals: Vec<JsonValue> = planned
                        .iter()
                        .map(|g| json!({"id": g.id, "progress": g.progress}))
                        .collect();
                    let result = json!({"planned": goals, "count": goals.len()});
                    Ok(CallToolResult::success(vec![json_content(&result)?]))
                }

                "physis_pdca_act" => {
                    let goal_id = get_str(&request, "goal_id")?;
                    let state_vector = get_vec_f64(&request, "state_vector")?;
                    let mut ctx = self.ctx.lock().await;
                    let before = match ctx.goals.iter().find(|g| g.id == goal_id) {
                        Some(g) => g.embedding.clone(),
                        None => {
                            return Ok(CallToolResult::error(vec![json_content(
                                &json!({"error": "goal not found"}),
                            )?]));
                        }
                    };
                    let exp_id = ctx.actor.do_action(&goal_id, before, state_vector).id;
                    let exps = ctx.actor.experiences.clone();
                    let mut goals = std::mem::take(&mut ctx.goals);
                    // Split borrows to avoid conflict between fields
                    let actor: *mut PDCActor = &mut ctx.actor;
                    let scheduler: *mut crate::gantt::GanttScheduler = &mut ctx.scheduler;
                    unsafe {
                        (*actor).act_with_schedule(&exps, &mut goals, Some(&mut *scheduler));
                    }
                    ctx.experiences = exps;
                    ctx.goals = goals;
                    Ok(CallToolResult::success(vec![json_content(
                        &json!({"experience_id": exp_id}),
                    )?]))
                }

                "physis_pdca_stats" => {
                    let ctx = self.ctx.lock().await;
                    let stats_json = json!(ctx
                        .actor
                        .stats_with_schedule(&ctx.goals, Some(&ctx.scheduler)));
                    Ok(CallToolResult::success(vec![json_content(&stats_json)?]))
                }

                "physis_dream_generate" => {
                    let count = get_i64(&request, "count").unwrap_or(5) as usize;
                    let mut ctx = self.ctx.lock().await;
                    if ctx.goals.is_empty() {
                        return Ok(CallToolResult::success(vec![json_content(
                            &json!({"dreams": [], "count": 0, "reason": "no goals"}),
                        )?]));
                    }
                    if ctx.actor.is_working(&ctx.goals) {
                        return Ok(CallToolResult::success(vec![json_content(
                            &json!({"dreams": [], "count": 0, "reason": "PDCA active"}),
                        )?]));
                    }
                    let goals = ctx.goals.clone();
                    let dreams = ctx.dreams.generate_dreams(&goals, count);
                    let result = json!({
                        "dreams": dreams.iter().map(|d| json!({"id": d.id, "source": d.source, "grade": d.grade})).collect::<Vec<_>>(),
                        "count": dreams.len()
                    });
                    Ok(CallToolResult::success(vec![json_content(&result)?]))
                }

                "physis_dream_evaluate" => {
                    let id = get_str(&request, "id")?;
                    let grade = get_f64(&request, "grade")? as f32;
                    let mut ctx = self.ctx.lock().await;
                    let accepted = ctx.dreams.evaluate_dream(&id, grade);
                    Ok(CallToolResult::success(vec![json_content(
                        &json!({"accepted": accepted, "dream_id": id, "grade": grade}),
                    )?]))
                }

                "physis_query" => {
                    let query = get_str(&request, "query")?;
                    let max = get_i64(&request, "max").unwrap_or(10) as usize;
                    let ctx = self.ctx.lock().await;
                    let results = ctx.mapper.query(&query);
                    let limited: Vec<Vec<String>> = results.into_iter().take(max).collect();
                    Ok(CallToolResult::success(vec![json_content(
                        &json!({"count": limited.len(), "results": limited}),
                    )?]))
                }

                "physis_stats" => {
                    let ctx = self.ctx.lock().await;
                    let result = json!({
                        "mapper": ctx.mapper.stats(),
                        "goals": ctx.goals.len(),
                        "dreams": ctx.dreams.dreams.len(),
                    });
                    Ok(CallToolResult::success(vec![json_content(&result)?]))
                }

                "physis_wiki_search" => {
                    let query = get_str(&request, "query")?;
                    let ctx = self.ctx.lock().await;
                    let wiki_path = ctx.config.data_dir.join("wiki");
                    let graph_path = ctx.config.data_dir.join("theme_graph.json");
                    let results = crate::wiki::search_all(&query, &wiki_path, &graph_path);
                    Ok(CallToolResult::success(vec![json_content(
                        &json!({"count": results.len(), "results": results}),
                    )?]))
                }

                "physis_scan" => {
                    let mut ctx = self.ctx.lock().await;
                    let scan_dir = match get_str(&request, "path") {
                        Ok(p) => PathBuf::from(p),
                        Err(_) => ctx.config.data_dir.clone(),
                    };
                    let inserted = ctx.mapper.map_filesystem(&scan_dir, None);
                    Ok(CallToolResult::success(vec![json_content(
                        &json!({"inserted": inserted, "path": scan_dir.to_string_lossy().to_string()}),
                    )?]))
                }

                "physis_workflow" => {
                    let scan_dir = match get_str(&request, "path") {
                        Ok(p) => PathBuf::from(p),
                        Err(_) => self.ctx.lock().await.config.data_dir.clone(),
                    };
                    let docs = crate::workflow::scan_workflow(&scan_dir, None);
                    let report = crate::workflow::report(&scan_dir, docs);
                    // Histograms are long tails: the head carries the shape of
                    // the tree, the tail is one file per row. `total` still
                    // reports the full count, so nothing is silently lost.
                    let head = |v: &[crate::study::Tally]| {
                        v.iter().take(WORKFLOW_HEAD).cloned().collect::<Vec<_>>()
                    };
                    let summary = json!({
                        "path": report.root,
                        "total": report.total,
                        "by_audience": head(&report.by_audience),
                        "by_harness": head(&report.by_harness),
                        "by_author": head(&report.by_author),
                        "by_dir": head(&report.by_dir),
                        "dirs_total": report.by_dir.len(),
                        "docs": report.docs.iter().take(WORKFLOW_DOCS).collect::<Vec<_>>(),
                        "docs_total": report.docs.len(),
                    });
                    Ok(CallToolResult::success(vec![json_content(&summary)?]))
                }

                "physis_gantt_schedule" => {
                    let action = get_str(&request, "action")?;
                    let mut ctx = self.ctx.lock().await;
                    let result = match action.as_str() {
                        "add_task" => {
                            let task_id = get_str(&request, "task_id")?;
                            let duration = get_f64(&request, "duration_secs")?.max(0.0) as u64;
                            let dep_list: Vec<String> = args_map(&request)?
                                .get("dependencies")
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default();
                            let mut hasher = DefaultHasher::new();
                            task_id.hash(&mut hasher);
                            let node_data = hasher.finish();
                            let node_key = crate::graph::RawNodeKey { data: node_data };
                            let deps: Vec<crate::graph::RawNodeKey> = dep_list
                                .iter()
                                .map(|d| {
                                    let mut h = DefaultHasher::new();
                                    d.hash(&mut h);
                                    crate::graph::RawNodeKey { data: h.finish() }
                                })
                                .collect();
                            ctx.scheduler.add_task_with_duration(
                                node_key,
                                task_id.clone(),
                                duration,
                                0,
                                deps,
                            );
                            json!({"status": "ok", "task_id": task_id, "duration_secs": duration})
                        }
                        "recompute" => {
                            ctx.scheduler.recompute();
                            let health = ctx.scheduler.schedule_health();
                            json!({"status": "ok", "schedule_health": health})
                        }
                        "status" => {
                            let health = ctx.scheduler.schedule_health();
                            let tasks: Vec<JsonValue> = ctx
                                .scheduler
                                .tasks
                                .iter()
                                .map(|t| {
                                    json!({
                                        "id": t.id,
                                        "name": t.name,
                                        "duration": t.planned_duration_secs,
                                        "early_start": t.early_start,
                                        "early_finish": t.early_finish,
                                        "late_start": t.late_start,
                                        "late_finish": t.late_finish,
                                        "float": t.float,
                                        "critical": t.on_critical_path,
                                        "progress": t.progress,
                                    })
                                })
                                .collect();
                            let critical_path_ids: Vec<&str> = ctx
                                .scheduler
                                .tasks
                                .iter()
                                .filter(|t| t.on_critical_path)
                                .map(|t| t.id.as_str())
                                .collect();
                            let at_risk: Vec<&str> = ctx
                                .scheduler
                                .tasks_approaching_late_finish(3600)
                                .iter()
                                .map(|t| t.id.as_str())
                                .collect();
                            json!({
                                "schedule_health": health,
                                "task_count": tasks.len(),
                                "tasks": tasks,
                                "critical_path": critical_path_ids,
                                "at_risk": at_risk,
                            })
                        }
                        _ => json!({"error": format!("unknown action: {action}")}),
                    };
                    Ok(CallToolResult::success(vec![json_content(&result)?]))
                }

                "physis_gantt_cross_correlate" => {
                    let ctx = self.ctx.lock().await;
                    let progress_pairs: Vec<(String, f32)> = ctx
                        .goals
                        .iter()
                        .map(|g| (g.id.clone(), g.progress))
                        .collect();
                    let r = ctx.scheduler.cross_correlate(&progress_pairs);
                    let result = json!({
                        "pearson_r": r,
                        "interpretation": if r.abs() > 0.7 {
                            "strong correlation"
                        } else if r.abs() > 0.4 {
                            "moderate correlation"
                        } else {
                            "weak or no correlation"
                        }
                    });
                    Ok(CallToolResult::success(vec![json_content(&result)?]))
                }

                "physis_dream_advanced" => {
                    let count = get_i64(&request, "count").unwrap_or(5) as usize;
                    let context =
                        get_str(&request, "context").unwrap_or_else(|_| "mixed".to_string());
                    let mut ctx = self.ctx.lock().await;
                    if ctx.goals.is_empty() {
                        return Ok(CallToolResult::success(vec![json_content(
                            &json!({"dreams": [], "count": 0, "reason": "no goals"}),
                        )?]));
                    }
                    if ctx.actor.is_working(&ctx.goals) {
                        return Ok(CallToolResult::success(vec![json_content(
                            &json!({"dreams": [], "count": 0, "reason": "PDCA active"}),
                        )?]));
                    }
                    let use_schedule = context == "schedule" || context == "mixed";
                    let use_cm = context == "countermeasure" || context == "mixed";
                    let goals = ctx.goals.clone();
                    let exps = ctx.experiences.clone();
                    // Extract scheduler and dreams from ctx to avoid borrow conflicts
                    let scheduler: *const crate::gantt::GanttScheduler = &ctx.scheduler;
                    let dreams = &mut ctx.dreams;
                    let sched_ref = if use_schedule {
                        Some(unsafe { &*scheduler })
                    } else {
                        None
                    };
                    let exps_ref = if use_cm { Some(&exps[..]) } else { None };
                    let generated =
                        dreams.generate_dreams_with_context(&goals, count, sched_ref, exps_ref);
                    let result = json!({
                        "dreams": generated.iter().map(|d| json!({
                            "id": d.id,
                            "source": d.source,
                            "grade": d.grade,
                            "dream_type": d.dream_type,
                        })).collect::<Vec<_>>(),
                        "count": generated.len(),
                        "context_type": context,
                    });
                    Ok(CallToolResult::success(vec![json_content(&result)?]))
                }

                // ── memory + why-made (Phase 10c A1) ──
                "physis_remember" => {
                    let input = get_str(&request, "input")?;
                    let verdict = get_f64(&request, "verdict").unwrap_or(0.0).clamp(-1.0, 1.0);
                    let mut ctx = self.ctx.lock().await;
                    if input.trim().is_empty() {
                        return Ok(CallToolResult::error(vec![json_content(
                            &json!({"error": "empty input"}),
                        )?]));
                    }
                    let embedding = ctx.embedder.embed(&input);
                    let id = ctx
                        .core
                        .register_node_vec_labeled(embedding, Some(input.clone()));
                    ctx.core.assert_coherence(&id, verdict as f32);
                    if let Some(db) = &ctx.db {
                        if let Err(e) = ctx.core.persist_node(db, "_default", &id) {
                            eprintln!("physis: persist failed: {e}");
                        }
                    }
                    Ok(CallToolResult::success(vec![json_content(&json!({
                        "id": id,
                        "label": input,
                        "verdict": verdict,
                        "nodes": ctx.core.nodes.len(),
                    }))?]))
                }

                "physis_recall" => {
                    let query = get_str(&request, "query")?;
                    let top_k = get_i64(&request, "top_k").unwrap_or(8).max(1) as usize;
                    let budget = get_i64(&request, "token_budget").unwrap_or(600).max(1) as usize;
                    let ctx = self.ctx.lock().await;
                    let q_emb = ctx.embedder.embed(&query);
                    // Same corpus contract as the web /api/v1/search/nodes:
                    // labeled coherence nodes + goals, ranked cosine, packed.
                    // Verdict (−1/0/+1 asserted outcome) rides along so recall
                    // is outcome memory: a top hit can say "you already tried
                    // this and it failed."
                    let candidates: Vec<(String, String, Vec<f32>, Option<f32>)> = ctx
                        .core
                        .nodes
                        .values()
                        .filter_map(|n| {
                            let label = n.label.as_deref()?;
                            (!label.trim().is_empty()).then(|| {
                                (label.to_string(), n.id.clone(), n.embedding.clone(), n.asserted)
                            })
                        })
                        .chain(ctx.goals.iter().filter_map(|g| {
                            g.label
                                .as_ref()
                                .map(|l| (l.clone(), g.id.clone(), g.embedding.clone(), None))
                        }))
                        .collect();
                    let corpus = crate::rag::RagCorpus {
                        chunks: candidates
                            .iter()
                            .enumerate()
                            .map(|(i, (text, _, emb, _))| crate::rag::RagChunk {
                                id: i,
                                text: text.clone(),
                                tokens: crate::rag::count_tokens(text),
                                embedding: emb.clone(),
                            })
                            .collect(),
                    };
                    let result = crate::rag::TokenFixedRetriever::new(budget, top_k)
                        .retrieve(&q_emb, &corpus);
                    let hits: Vec<JsonValue> = result
                        .chunks
                        .iter()
                        .filter_map(|c| {
                            let (label, id, _, verdict) = candidates.get(c.id)?;
                            Some(json!({
                                "id": id, "label": label,
                                "score": c.score, "tokens": c.tokens,
                                "verdict": verdict,
                            }))
                        })
                        .collect();
                    // Recorded failures among the recalled set — the "already
                    // failed here" flag, surfaced up front for the agent.
                    let failures: Vec<JsonValue> = hits
                        .iter()
                        .filter(|h| h["verdict"] == json!(-1.0))
                        .cloned()
                        .collect();
                    Ok(CallToolResult::success(vec![json_content(&json!({
                        "query": query,
                        "results": hits,
                        "failures": failures,
                        "total_tokens": result.total_tokens,
                        "truncated": result.truncated,
                        "note": "verdict: -1 failed, 0 inert, +1 worked, null unjudged. grounded synthesis over these matches: HTTP /api/v1/ask",
                    }))?]))
                }

                "physis_why" => {
                    let path = get_str(&request, "path")?;
                    let ctx = self.ctx.lock().await;
                    let view = |ev: &crate::ledger::CauseEvent| {
                        json!({
                            "seq": ev.seq, "at": ev.at.to_rfc3339(),
                            "mechanism": ev.mechanism.as_str(), "actor": ev.actor,
                            "object": ev.object, "input": ev.input,
                        })
                    };
                    Ok(CallToolResult::success(vec![json_content(&json!({
                        "path": path,
                        "lineage": ctx.ledger.lineage(&path).iter().map(|e| view(e)).collect::<Vec<_>>(),
                        "descendants": ctx.ledger.descendants(&path).iter().map(|e| view(e)).collect::<Vec<_>>(),
                        "purposes": ctx.ledger.purposes(&path).iter().map(|e| view(e)).collect::<Vec<_>>(),
                    }))?]))
                }

                "physis_serve" => {
                    let object = get_str(&request, "object")?;
                    let goal_id = get_str(&request, "goal_id")?;
                    let mut ctx = self.ctx.lock().await;
                    if !ctx.goals.iter().any(|g| g.id == goal_id) {
                        return Ok(CallToolResult::error(vec![json_content(
                            &json!({"error": format!("goal {goal_id} not found — list ids with physis_goals_list")}),
                        )?]));
                    }
                    let ev = ctx.ledger.record(
                        crate::address::Mechanism::Serves,
                        "agent",
                        object.clone(),
                        Some(goal_id.clone()),
                    );
                    let ev_view = json!({
                        "seq": ev.seq, "at": ev.at.to_rfc3339(),
                        "mechanism": ev.mechanism.as_str(),
                    });
                    if let Some(db) = &ctx.db {
                        if let Err(e) = ctx.ledger.save(db, "_default") {
                            eprintln!("physis: ledger persist failed: {e}");
                        }
                    }
                    Ok(CallToolResult::success(vec![json_content(&json!({
                        "recorded": ev_view,
                        "object": object, "goal_id": goal_id,
                    }))?]))
                }

                "physis_causes" => {
                    let query = request
                        .arguments
                        .as_ref()
                        .and_then(|a| a.get("query"))
                        .and_then(|v| v.as_str())
                        .map(String::from)
                        .unwrap_or_default();
                    let mechanism = request
                        .arguments
                        .as_ref()
                        .and_then(|a| a.get("mechanism"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let within_days = get_i64(&request, "within_days");
                    let ctx = self.ctx.lock().await;
                    let now = chrono::Utc::now();
                    let mut matches: Vec<JsonValue> = ctx
                        .ledger
                        .events()
                        .iter()
                        .filter(|ev| {
                            if !query.is_empty() {
                                let hay = format!(
                                    "{} {} {}",
                                    ev.object,
                                    ev.input.as_deref().unwrap_or(""),
                                    ev.actor
                                );
                                if !hay.to_lowercase().contains(&query.to_lowercase()) {
                                    return false;
                                }
                            }
                            if let Some(m) = &mechanism {
                                if ev.mechanism.as_str() != m.as_str()
                                    && ev.actor.to_lowercase() != m.to_lowercase()
                                {
                                    return false;
                                }
                            }
                            if let Some(d) = within_days {
                                if (now - ev.at).num_days() > d {
                                    return false;
                                }
                            }
                            true
                        })
                        .map(|ev| {
                            json!({
                                "at": ev.at.to_rfc3339(),
                                "mechanism": ev.mechanism.as_str(), "actor": ev.actor,
                                "object": ev.object, "input": ev.input,
                            })
                        })
                        .collect();
                    matches.reverse(); // newest first
                    Ok(CallToolResult::success(vec![json_content(
                        &json!({"total": matches.len(), "matches": matches}),
                    )?]))
                }

                "physis_themes" => {
                    let min_size = get_i64(&request, "min_size").unwrap_or(1).max(0) as usize;
                    let ctx = self.ctx.lock().await;
                    if ctx.core.nodes.is_empty() {
                        return Ok(CallToolResult::success(vec![json_content(&json!({
                            "communities": 0,
                            "nodes": 0,
                            "note": "the coherence graph is empty — remember something first",
                            "summaries": [],
                        }))?]));
                    }
                    let (facts, edges) =
                        crate::corpus::gather_facts(&ctx.core, &ctx.ledger);
                    let summaries = crate::corpus::build_summaries(&facts, &edges);
                    let visible: Vec<&crate::corpus::CommunitySummary> = summaries
                        .iter()
                        .filter(|s| s.size >= min_size)
                        .collect();
                    Ok(CallToolResult::success(vec![json_content(&json!({
                        "communities": summaries.len(),
                        "nodes": facts.len(),
                        "summaries": visible,
                    }))?]))
                }

                "physis_coherence_query" => {
                    let verdict = request
                        .arguments
                        .as_ref()
                        .and_then(|a| a.get("verdict"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let cell = request
                        .arguments
                        .as_ref()
                        .and_then(|a| a.get("cell"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let q = request
                        .arguments
                        .as_ref()
                        .and_then(|a| a.get("q"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let limit = get_i64(&request, "limit").unwrap_or(500).max(1) as usize;
                    let ctx = self.ctx.lock().await;
                    let verdict_rating = match verdict.as_deref() {
                        Some("success") => Some(crate::models::CoherenceRating::Success),
                        Some("inert") => Some(crate::models::CoherenceRating::Inert),
                        Some("failure") => Some(crate::models::CoherenceRating::Failure),
                        _ => None,
                    };
                    let cell_tuple = cell.as_ref().and_then(|c| {
                        let mut it = c.splitn(2, '|');
                        match (it.next(), it.next()) {
                            (Some(d), Some(m)) if !d.is_empty() && !m.is_empty() => {
                                Some((d.to_string(), m.to_string()))
                            }
                            _ => None,
                        }
                    });
                    let nodes: Vec<JsonValue> = crate::web::coherence::query_coherence_nodes(
                        &ctx.core.nodes,
                        &ctx.classifier,
                        verdict_rating,
                        cell_tuple,
                        q.as_deref(),
                        limit,
                    )
                    .into_iter()
                    .map(|nv| {
                        json!({
                            "id": nv.id,
                            "label": nv.label,
                            "score": nv.score,
                            // NodeView carries the raw asserted outcome plus the
                            // derived rating; `verdict` was never a field on it.
                            "verdict": nv.rating,
                            "asserted": nv.asserted,
                            "cell": nv.cell,
                            "pinned": nv.pinned,
                            "created_at": nv.created_at.to_rfc3339(),
                        })
                    })
                    .collect();
                    Ok(CallToolResult::success(vec![json_content(
                        &json!({"total": nodes.len(), "results": nodes}),
                    )?]))
                }

                "physis_discover" => {
                    // Corpus texts → coverage + draft ontology for the gaps.
                    let texts: Vec<String> = request
                        .arguments
                        .as_ref()
                        .and_then(|a| a.get("texts"))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .filter(|s| s.trim().len() > 10)
                                .collect()
                        })
                        .unwrap_or_default();
                    if texts.is_empty() {
                        return Ok(CallToolResult::error(vec![json_content(
                            &json!({"error": "texts must be a non-empty array of strings"}),
                        )?]));
                    }
                    let min_cluster = get_i64(&request, "min_cluster").unwrap_or(3).max(1) as usize;
                    let cfg = crate::discovery::DiscoveryConfig {
                        min_cluster,
                        auto_retune: true,
                        ..Default::default()
                    };
                    let ctx = self.ctx.lock().await;
                    let report = crate::discovery::discover(
                        &texts,
                        &ctx.classifier,
                        ctx.embedder.as_ref(),
                        &cfg,
                    );
                    let onto = crate::discovery::to_ontology_json(&report, "discovered");
                    drop(ctx);
                    Ok(CallToolResult::success(vec![json_content(&json!({
                        "covered": report.covered,
                        "unmapped": report.unmapped,
                        "total": report.total,
                        "proposals": report.proposals.len(),
                        "draft_ontology": onto,
                    }))?]))
                }

                name => Ok(CallToolResult::error(vec![json_content(
                    &json!({"error": format!("unknown tool: {name}")}),
                )?])),
            }
        }
    }

    /// Serve MCP over stdin/stdout — the transport every mainstream client uses.
    ///
    /// The client spawns this process and frames JSON-RPC over the pipe, so
    /// **stdout belongs to the protocol**. A single stray `println!` anywhere on
    /// this path corrupts the stream and the client reports the server as
    /// broken with no useful diagnostic. Every message here goes to stderr,
    /// which clients surface as server logs.
    pub async fn serve_stdio(config: PhysisConfig) -> anyhow::Result<()> {
        use rmcp::transport::stdio;

        // Built before the transport is attached: constructing the context loads
        // the ontology and embedder, and any diagnostics that emits are safer
        // before the pipe is live.
        let server = PhysisMcpServer::new(config);
        eprintln!("physis: MCP server ready on stdio");

        let service = server.serve(stdio()).await?;
        service.waiting().await?;
        Ok(())
    }

    /// Start the Physis MCP server on `127.0.0.1:9876`.
    ///
    /// Retained for the hosted deployment. No mainstream MCP client can attach
    /// to a bare TCP socket — prefer [`serve_stdio`] for local integrations.
    pub async fn start_mcp_server(config: PhysisConfig) -> anyhow::Result<()> {
        use tokio::net::TcpListener;

        let server = PhysisMcpServer::new(config);

        let listener = TcpListener::bind("127.0.0.1:9876").await?;
        println!("Physis MCP server listening on 127.0.0.1:9876");

        loop {
            let (stream, addr) = listener.accept().await?;
            log::info!("MCP client connected: {addr}");
            let server = server.clone();
            tokio::spawn(async move {
                if let Err(e) = server.serve(stream).await {
                    log::warn!("MCP connection {addr} error: {e}");
                }
            });
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn request(args: JsonValue) -> CallToolRequestParam {
            CallToolRequestParam {
                name: "test_tool".into(),
                arguments: args.as_object().cloned(),
            }
        }

        #[test]
        fn get_str_extracts_and_rejects() {
            let req = request(json!({"text": "hello", "count": 3}));
            assert_eq!(get_str(&req, "text").unwrap(), "hello");
            assert!(get_str(&req, "missing").is_err());
            assert!(
                get_str(&req, "count").is_err(),
                "non-string must not coerce"
            );
        }

        #[test]
        fn get_f64_accepts_ints_and_floats() {
            let req = request(json!({"weight": 0.5, "count": 3, "text": "x"}));
            assert_eq!(get_f64(&req, "weight").unwrap(), 0.5);
            assert_eq!(get_f64(&req, "count").unwrap(), 3.0);
            assert!(get_f64(&req, "text").is_err());
            assert!(get_f64(&req, "missing").is_err());
        }

        #[test]
        fn get_i64_is_optional() {
            let req = request(json!({"limit": 10, "weight": 0.5}));
            assert_eq!(get_i64(&req, "limit"), Some(10));
            assert_eq!(get_i64(&req, "missing"), None);
            assert_eq!(
                get_i64(&req, "weight"),
                None,
                "float must not coerce to int"
            );
        }

        #[test]
        fn get_vec_f64_downcasts_and_skips_non_numeric() {
            let req = request(json!({"vec": [1.0, 2, "skip", 3.5], "text": "x"}));
            assert_eq!(get_vec_f64(&req, "vec").unwrap(), vec![1.0f32, 2.0, 3.5]);
            assert!(get_vec_f64(&req, "text").is_err());
        }

        #[test]
        fn missing_arguments_map_errors() {
            let req = CallToolRequestParam {
                name: "test_tool".into(),
                arguments: None,
            };
            assert!(args_map(&req).is_err());
            assert!(get_str(&req, "any").is_err());
            assert_eq!(get_i64(&req, "any"), None);
        }
    }
}

#[cfg(feature = "mcp")]
pub use mcp_impl::*;

/// JSON -> indented plain text. Nulls and empty collections are dropped: they
/// carry no information for the reader and every omitted line is context back.
fn render_value(value: &serde_json::Value, depth: usize) -> String {
    let pad = "  ".repeat(depth);
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            if s.contains('\n') {
                let inner = "  ".repeat(depth + 1);
                format!(
                    "\n{inner}{}",
                    s.trim_end().replace('\n', &format!("\n{inner}"))
                )
            } else {
                s.clone()
            }
        }
        serde_json::Value::Array(items) => {
            let mut out = String::new();
            for (i, item) in items.iter().enumerate() {
                match item {
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                        out.push_str(&format!("\n{pad}[{}]\n", i + 1));
                        out.push_str(&render_value(item, depth + 1));
                    }
                    _ => out.push_str(&format!("\n{pad}- {}", render_value(item, depth))),
                }
            }
            out
        }
        serde_json::Value::Object(map) => {
            let mut out = String::new();
            for (k, v) in map {
                if v.is_null()
                    || v.as_str().is_some_and(|t| t.trim().is_empty())
                    || v.as_array().is_some_and(|a| a.is_empty())
                    || v.as_object().is_some_and(|o| o.is_empty())
                {
                    continue;
                }
                match v {
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                        out.push_str(&format!("{pad}{k}:\n{}\n", render_value(v, depth + 1)));
                    }
                    _ => out.push_str(&format!("{pad}{k}: {}\n", render_value(v, depth))),
                }
            }
            out
        }
    }
}

#[cfg(test)]
mod render_tests {
    use super::render_value;
    use serde_json::json;

    #[test]
    fn renders_hits_without_json_punctuation() {
        let out = render_value(
            &json!({"matched": 2, "empty": [], "gone": null,
                    "hits": [{"path": "src/core.rs", "snippet": "line one\nline two"}]}),
            0,
        );
        assert!(out.contains("matched: 2"), "{out}");
        assert!(!out.contains("empty"), "{out}");
        assert!(!out.contains("gone"), "{out}");
        assert!(!out.contains('{'), "{out}");
        assert!(out.contains("path: src/core.rs"), "{out}");
        assert!(out.contains("line two"), "{out}");
    }
}
