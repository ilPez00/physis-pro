//! MCP (Model Context Protocol) server — expose Physis as an
//! MCP service on `127.0.0.1:9876`.

#[cfg(feature = "mcp")]
mod mcp_impl {
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    use rmcp::handler::server::ServerHandler;
    use rmcp::model::{
        CallToolRequestParam, CallToolResult, Content, ListToolsResult,
        PaginatedRequestParam, ServerCapabilities, ServerInfo, Tool,
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
    use crate::gantt::{GanttScheduler, AbstractDomain};
    use crate::mapper::OntologyMapper;
    use crate::models::{Goal, Experience};

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
        pub scheduler: GanttScheduler,
        pub experiences: Vec<Experience>,
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
            let classifier =
                CellClassifier::build_cached(&ontology, embedder.as_ref(), embedder_kind, &mut pack_cache);
            let (hits, misses) = pack_cache.stats();
            eprintln!("physis: ontology seeds embedded — {hits} cached, {misses} computed");
            pack_cache.save(&config.data_dir);
            lap(&mut t, "classifier");
            eprintln!("physis: startup TOTAL {:.2}s", t0.elapsed().as_secs_f32());
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
                scheduler: GanttScheduler::new(Box::new(AbstractDomain)),
                experiences: Vec::new(),
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

    fn json_content(value: &JsonValue) -> Result<Content, McpError> {
        Content::json(value).map_err(|e| {
            McpError::internal_error("json serialization failed", Some(json!({"reason": e.to_string()})))
        })
    }

    #[inline]
    fn args_map(request: &CallToolRequestParam) -> Result<&serde_json::Map<String, JsonValue>, McpError> {
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
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).map(|f| f as f32).collect())
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

                "physis_classify" => {
                    let text = get_str(&request, "text")?;
                    let ctx = self.ctx.lock().await;
                    let embedding = ctx.embedder.embed(&text);
                    let results = ctx.classifier.classify(&embedding);
                    let result = json!({"results": results});
                    Ok(CallToolResult::success(vec![json_content(&result)?]))
                }

                "physis_embed" => {
                    let text = get_str(&request, "text")?;
                    let ctx = self.ctx.lock().await;
                    let embedding = ctx.embedder.embed(&text);
                    let result = json!({"embedding": embedding, "dim": embedding.len()});
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
                    Ok(CallToolResult::success(vec![json_content(&json!({"removed": removed > 0}))?]))
                }

                "physis_pdca_plan" => {
                    let ctx = self.ctx.lock().await;
                    let priority_map: Option<std::collections::HashMap<String, f32>> = {
                        let cp: Vec<&str> = ctx.scheduler.tasks.iter()
                            .filter(|t| t.on_critical_path)
                            .map(|t| t.id.as_str())
                            .collect();
                        if cp.is_empty() { None } else {
                            let mut m = std::collections::HashMap::new();
                            for t in cp { m.insert(t.to_string(), 1.0); }
                            Some(m)
                        }
                    };
                    let planned = ctx.actor.plan_with_schedule(&ctx.goals, priority_map.as_ref());
                    let goals: Vec<JsonValue> = planned.iter().map(|g| {
                        json!({"id": g.id, "progress": g.progress})
                    }).collect();
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
                            return Ok(CallToolResult::error(vec![json_content(&json!({"error": "goal not found"}))?]));
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
                    Ok(CallToolResult::success(vec![json_content(&json!({"experience_id": exp_id}))?]))
                }

                "physis_pdca_stats" => {
                    let ctx = self.ctx.lock().await;
                    let stats_json = json!(ctx.actor.stats_with_schedule(&ctx.goals, Some(&ctx.scheduler)));
                    Ok(CallToolResult::success(vec![json_content(&stats_json)?]))
                }

                "physis_dream_generate" => {
                    let count = get_i64(&request, "count").unwrap_or(5) as usize;
                    let mut ctx = self.ctx.lock().await;
                    if ctx.goals.is_empty() {
                        return Ok(CallToolResult::success(vec![
                            json_content(&json!({"dreams": [], "count": 0, "reason": "no goals"}))?]));
                    }
                    if ctx.actor.is_working(&ctx.goals) {
                        return Ok(CallToolResult::success(vec![
                            json_content(&json!({"dreams": [], "count": 0, "reason": "PDCA active"}))?]));
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
                    Ok(CallToolResult::success(vec![
                        json_content(&json!({"accepted": accepted, "dream_id": id, "grade": grade}))?]))
                }

                "physis_query" => {
                    let query = get_str(&request, "query")?;
                    let max = get_i64(&request, "max").unwrap_or(10) as usize;
                    let ctx = self.ctx.lock().await;
                    let results = ctx.mapper.query(&query);
                    let limited: Vec<Vec<String>> = results.into_iter().take(max).collect();
                    Ok(CallToolResult::success(vec![
                        json_content(&json!({"count": limited.len(), "results": limited}))?]))
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
                    Ok(CallToolResult::success(vec![
                        json_content(&json!({"count": results.len(), "results": results}))?]))
                }

                "physis_scan" => {
                    let mut ctx = self.ctx.lock().await;
                    let scan_dir = match get_str(&request, "path") {
                        Ok(p) => PathBuf::from(p),
                        Err(_) => ctx.config.data_dir.clone(),
                    };
                    let inserted = ctx.mapper.map_filesystem(&scan_dir, None);
                    Ok(CallToolResult::success(vec![
                        json_content(&json!({"inserted": inserted, "path": scan_dir.to_string_lossy().to_string()}))?]))
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
                                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                .unwrap_or_default();
                            let mut hasher = DefaultHasher::new();
                            task_id.hash(&mut hasher);
                            let node_data = hasher.finish();
                            let node_key = crate::graph::RawNodeKey { data: node_data };
                            let deps: Vec<crate::graph::RawNodeKey> = dep_list.iter().map(|d| {
                                let mut h = DefaultHasher::new();
                                d.hash(&mut h);
                                crate::graph::RawNodeKey { data: h.finish() }
                            }).collect();
                            ctx.scheduler.add_task_with_duration(node_key, task_id.clone(), duration, 0, deps);
                            json!({"status": "ok", "task_id": task_id, "duration_secs": duration})
                        }
                        "recompute" => {
                            ctx.scheduler.recompute();
                            let health = ctx.scheduler.schedule_health();
                            json!({"status": "ok", "schedule_health": health})
                        }
                        "status" => {
                            let health = ctx.scheduler.schedule_health();
                            let tasks: Vec<JsonValue> = ctx.scheduler.tasks.iter().map(|t| {
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
                            }).collect();
                            let critical_path_ids: Vec<&str> = ctx.scheduler.tasks.iter()
                                .filter(|t| t.on_critical_path)
                                .map(|t| t.id.as_str())
                                .collect();
                            let at_risk: Vec<&str> = ctx.scheduler.tasks_approaching_late_finish(3600)
                                .iter().map(|t| t.id.as_str()).collect();
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
                    let progress_pairs: Vec<(String, f32)> = ctx.goals.iter()
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
                    let context = get_str(&request, "context").unwrap_or_else(|_| "mixed".to_string());
                    let mut ctx = self.ctx.lock().await;
                    if ctx.goals.is_empty() {
                        return Ok(CallToolResult::success(vec![
                            json_content(&json!({"dreams": [], "count": 0, "reason": "no goals"}))?]));
                    }
                    if ctx.actor.is_working(&ctx.goals) {
                        return Ok(CallToolResult::success(vec![
                            json_content(&json!({"dreams": [], "count": 0, "reason": "PDCA active"}))?]));
                    }
                    let use_schedule = context == "schedule" || context == "mixed";
                    let use_cm = context == "countermeasure" || context == "mixed";
                    let goals = ctx.goals.clone();
                    let exps = ctx.experiences.clone();
                    // Extract scheduler and dreams from ctx to avoid borrow conflicts
                    let scheduler: *const crate::gantt::GanttScheduler = &ctx.scheduler;
                    let dreams = &mut ctx.dreams;
                    let sched_ref = if use_schedule { Some(unsafe { &*scheduler }) } else { None };
                    let exps_ref = if use_cm { Some(&exps[..]) } else { None };
                    let generated = dreams.generate_dreams_with_context(
                        &goals, count, sched_ref, exps_ref,
                    );
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

                name => Ok(CallToolResult::error(vec![
                    json_content(&json!({"error": format!("unknown tool: {name}")}))?])),
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
            assert!(get_str(&req, "count").is_err(), "non-string must not coerce");
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
            assert_eq!(get_i64(&req, "weight"), None, "float must not coerce to int");
        }

        #[test]
        fn get_vec_f64_downcasts_and_skips_non_numeric() {
            let req = request(json!({"vec": [1.0, 2, "skip", 3.5], "text": "x"}));
            assert_eq!(get_vec_f64(&req, "vec").unwrap(), vec![1.0f32, 2.0, 3.5]);
            assert!(get_vec_f64(&req, "text").is_err());
        }

        #[test]
        fn missing_arguments_map_errors() {
            let req = CallToolRequestParam { name: "test_tool".into(), arguments: None };
            assert!(args_map(&req).is_err());
            assert!(get_str(&req, "any").is_err());
            assert_eq!(get_i64(&req, "any"), None);
        }
    }
}

#[cfg(feature = "mcp")]
pub use mcp_impl::*;
