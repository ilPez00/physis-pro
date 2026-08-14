use std::collections::HashMap;
use std::path::PathBuf;

use rmcp::{
    ServerHandler,
    model::{ServerCapabilities, ServerInfo},
    schemars, tool,
};
use serde::Deserialize;

use crate::{indexer, inverted, parser, search, writer};

#[derive(Debug, Clone)]
pub struct WikiServer {
    wiki_root: PathBuf,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchParams {
    /// The search query string
    pub query: String,
    /// Search mode: "keyword", "semantic", or "hybrid" (default)
    pub mode: Option<String>,
    /// Maximum number of results to return (default: 5)
    pub top_k: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReadParams {
    /// Relative path to the wiki page (e.g. "notes/topic.md")
    pub path: String,
    /// Extract only the named section by heading text
    pub section: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WriteParams {
    /// Relative path for the wiki page (must end with .md)
    pub path: String,
    /// Full markdown content including YAML front matter
    pub content: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReindexParams {
    /// Only re-index files whose content has changed (default: true)
    pub incremental: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListParams {
    /// Filter results to pages whose path starts with this prefix
    pub path: Option<String>,
    /// Filter results to pages that include this tag
    pub tag: Option<String>,
}

#[tool(tool_box)]
impl WikiServer {
    pub fn new(wiki_root: PathBuf) -> Self {
        Self { wiki_root }
    }

    #[tool(description = "Search the wiki. mode: 'keyword' | 'semantic' | 'hybrid' (default). Returns JSON array of {rank, path, score, description} objects.")]
    fn search(&self, #[tool(aggr)] params: SearchParams) -> String {
        let top_k = params.top_k.unwrap_or(5);
        let mode = params.mode.as_deref().unwrap_or("hybrid");

        let results: Vec<search::SearchResult> = match mode {
            "semantic" => {
                #[cfg(feature = "semantic")]
                {
                    match crate::semantic::semantic_search(&self.wiki_root, &params.query, top_k) {
                        Ok(raw) => {
                            let all_meta =
                                indexer::load_meta(&self.wiki_root).unwrap_or_default();
                            let meta_map: HashMap<String, indexer::FileMeta> =
                                all_meta.into_iter().map(|m| (m.path.clone(), m)).collect();
                            raw.into_iter()
                                .enumerate()
                                .map(|(i, (path, score))| {
                                    let description = meta_map
                                        .get(&path)
                                        .map(|m| m.description.clone())
                                        .unwrap_or_default();
                                    search::SearchResult {
                                        rank: i + 1,
                                        path,
                                        score,
                                        description,
                                        match_locations: vec![],
                                    }
                                })
                                .collect()
                        }
                        Err(e) => {
                            return serde_json::json!({"error": e.to_string()}).to_string()
                        }
                    }
                }
                #[cfg(not(feature = "semantic"))]
                return serde_json::json!({"error": "semantic search not enabled"}).to_string();
            }
            "keyword" => {
                match inverted::keyword_search(&self.wiki_root, &params.query, top_k) {
                    Ok(raw) => {
                        let all_meta = indexer::load_meta(&self.wiki_root).unwrap_or_default();
                        let meta_map: HashMap<String, indexer::FileMeta> =
                            all_meta.into_iter().map(|m| (m.path.clone(), m)).collect();
                        raw.into_iter()
                            .enumerate()
                            .map(|(i, (path, score))| {
                                let description = meta_map
                                    .get(&path)
                                    .map(|m| m.description.clone())
                                    .unwrap_or_default();
                                let match_locations = inverted::get_match_locations(
                                    &self.wiki_root,
                                    &path,
                                    &params.query,
                                );
                                search::SearchResult {
                                    rank: i + 1,
                                    path,
                                    score,
                                    description,
                                    match_locations,
                                }
                            })
                            .collect()
                    }
                    Err(e) => return serde_json::json!({"error": e.to_string()}).to_string(),
                }
            }
            _ => {
                // hybrid (default): RRF fusion of keyword + semantic
                let kw_raw =
                    inverted::keyword_search(&self.wiki_root, &params.query, top_k)
                        .unwrap_or_default();
                #[cfg(feature = "semantic")]
                let sem_raw =
                    crate::semantic::semantic_search(&self.wiki_root, &params.query, top_k)
                        .unwrap_or_default();
                #[cfg(not(feature = "semantic"))]
                let sem_raw: Vec<(String, f32)> = vec![];
                let merged = search::rrf_merge(&kw_raw, &sem_raw, 60.0);

                let all_meta = indexer::load_meta(&self.wiki_root).unwrap_or_default();
                let meta_map: HashMap<String, indexer::FileMeta> =
                    all_meta.into_iter().map(|m| (m.path.clone(), m)).collect();
                merged
                    .into_iter()
                    .enumerate()
                    .map(|(i, (path, score))| {
                        let description = meta_map
                            .get(&path)
                            .map(|m| m.description.clone())
                            .unwrap_or_default();
                        let match_locations = inverted::get_match_locations(
                            &self.wiki_root,
                            &path,
                            &params.query,
                        );
                        search::SearchResult {
                            rank: i + 1,
                            path,
                            score,
                            description,
                            match_locations,
                        }
                    })
                    .collect()
            }
        };

        search::results_to_json(&results).to_string()
    }

    #[tool(description = "Read a wiki page. Returns full markdown content, or only the named section if 'section' is provided. Returns an error message if the file is not found.")]
    fn read(&self, #[tool(aggr)] params: ReadParams) -> String {
        let abs_path = self.wiki_root.join(&params.path);

        if !abs_path.exists() {
            return format!("Error: file not found: {}", params.path);
        }

        if abs_path.extension().and_then(|e| e.to_str()) != Some("md") {
            return format!("Error: not a markdown file: {}", params.path);
        }

        match std::fs::read_to_string(&abs_path) {
            Ok(content) => {
                if let Some(heading) = params.section {
                    match parser::extract_section(&content, &heading) {
                        Some(text) => text,
                        None => format!("Section not found: \"{}\"", heading),
                    }
                } else {
                    content
                }
            }
            Err(e) => format!("Error reading file: {}", e),
        }
    }

    #[tool(description = "Write or update a wiki page. The content must include valid YAML front matter with at least 'title' and 'description'. Returns JSON {created, reindexed} on success, error string on validation failure.")]
    fn write(&self, #[tool(aggr)] params: WriteParams) -> String {
        match writer::write_page(&self.wiki_root, &params.path, &params.content) {
            Ok(result) => serde_json::json!({
                "created": result.created,
                "reindexed": result.reindexed,
            })
            .to_string(),
            Err(e) => format!("Error: {:#}", e),
        }
    }

    #[tool(description = "Reindex the wiki, scanning all markdown files and rebuilding search indexes. Returns JSON {added, updated, removed, skipped}.")]
    fn reindex(&self, #[tool(aggr)] params: ReindexParams) -> String {
        let incremental = params.incremental.unwrap_or(true);
        match indexer::index_root(&self.wiki_root, incremental) {
            Ok(stats) => serde_json::json!({
                "added": stats.added,
                "updated": stats.updated,
                "removed": stats.removed,
                "skipped": stats.skipped,
            })
            .to_string(),
            Err(e) => format!("Error: {:#}", e),
        }
    }

    #[tool(description = "List indexed wiki pages, optionally filtered by path prefix or tag. Returns JSON array of {path, title, description, tags} objects.")]
    fn list(&self, #[tool(aggr)] params: ListParams) -> String {
        let metas = match indexer::load_meta(&self.wiki_root) {
            Ok(m) => m,
            Err(e) => return format!("Error: {:#}", e),
        };

        let filtered: Vec<_> = metas
            .iter()
            .filter(|m| {
                if let Some(ref p) = params.path {
                    if !m.path.starts_with(p.as_str()) {
                        return false;
                    }
                }
                if let Some(ref t) = params.tag {
                    if !m.tags.iter().any(|tag| tag == t) {
                        return false;
                    }
                }
                true
            })
            .map(|m| {
                serde_json::json!({
                    "path": m.path,
                    "title": m.title,
                    "description": m.description,
                    "tags": m.tags,
                })
            })
            .collect();

        serde_json::to_string(&filtered).unwrap_or_else(|_| "[]".to_string())
    }
}

#[tool(tool_box)]
impl ServerHandler for WikiServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "llmwiki MCP server. Use search/read/write/reindex/list tools to interact with your local wiki knowledge base.".into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

pub async fn run_mcp_server(wiki_root: PathBuf) -> anyhow::Result<()> {
    let server = WikiServer::new(wiki_root);
    let service = rmcp::serve_server(server, rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
