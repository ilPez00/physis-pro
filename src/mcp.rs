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

    use crate::actor::PDCActor;
    use crate::classify::CellClassifier;
    use crate::config::{OntologyLoader, PhysisConfig};
    use crate::dream::DreamEngine;
    use crate::embed::{RandomProjectionEmbedder, VectorEmbed};
    use crate::mapper::OntologyMapper;
    use crate::models::Goal;

    /// Shared state served by the MCP server.
    pub struct PhysisContext {
        pub config: PhysisConfig,
        pub mapper: OntologyMapper,
        pub actor: PDCActor,
        pub dreams: DreamEngine,
        pub goals: Vec<Goal>,
        pub embedder: Box<dyn VectorEmbed>,
        pub classifier: CellClassifier,
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
            let ontology = OntologyLoader::load_all(&config);
            let mapper = OntologyMapper::new(ontology.clone(), config.embed_dim);
            let actor = PDCActor::new(config.pdca_stagnant_threshold, config.pdca_stagnant_window);
            let dreams = DreamEngine::new();
            let embedder: Box<dyn VectorEmbed> = Box::new(RandomProjectionEmbedder::new(config.embed_dim));
            let classifier = CellClassifier::build(&ontology, embedder.as_ref());
            let ctx = Arc::new(Mutex::new(PhysisContext {
                config,
                mapper,
                actor,
                dreams,
                goals: Vec::new(),
                embedder,
                classifier,
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
                    let planned = ctx.actor.plan(&ctx.goals);
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
                    ctx.actor.act(&exps, &mut goals);
                    ctx.goals = goals;
                    Ok(CallToolResult::success(vec![json_content(&json!({"experience_id": exp_id}))?]))
                }

                "physis_pdca_stats" => {
                    let ctx = self.ctx.lock().await;
                    let stats = ctx.actor.stats(&ctx.goals);
                    Ok(CallToolResult::success(vec![json_content(&json!(stats))?]))
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

                name => Ok(CallToolResult::error(vec![
                    json_content(&json!({"error": format!("unknown tool: {name}")}))?])),
            }
        }
    }

    /// Start the Physis MCP server on `127.0.0.1:9876`.
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
}

#[cfg(feature = "mcp")]
pub use mcp_impl::*;
