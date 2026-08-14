//! llmwiki_physis — library API for in-process wiki indexing + search.
//!
//! Lets host crates (notably `physis_pro`) call the wiki engine directly
//! instead of shelling out to the `llmwiki` binary. Phase 1 exposes the
//! Tantivy BM25 path, which builds with `default-features = false` (no
//! `fastembed`/`usearch`). The `semantic` feature adds the vector path.
//!
//! The binary (`src/main.rs`) keeps its own module tree and is unaffected.

pub mod indexer;
pub mod inverted;
pub mod parser;
pub mod search;
#[cfg(feature = "semantic")]
pub mod semantic;

pub use indexer::{ensure_index_dirs, index_root, load_meta, FileMeta, IndexStats};
pub use inverted::{get_match_locations, keyword_search};
pub use search::{format_results, results_to_json, rrf_merge, SearchResult};

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

/// Build/refresh the on-disk index at `wiki_root` (Tantivy BM25; plus the
/// vector index when the `semantic` feature is enabled).
pub fn index(wiki_root: &Path, incremental: bool) -> Result<IndexStats> {
    indexer::index_root(wiki_root, incremental)
}

/// BM25 keyword search → ranked results with page descriptions + match
/// locations. The in-process equivalent of `llmwiki search --keyword`.
pub fn search_bm25(wiki_root: &Path, query: &str, top_k: usize) -> Result<Vec<SearchResult>> {
    let meta_map: HashMap<String, FileMeta> = load_meta(wiki_root)
        .unwrap_or_default()
        .into_iter()
        .map(|m| (m.path.clone(), m))
        .collect();

    let raw = keyword_search(wiki_root, query, top_k)?;
    Ok(raw
        .into_iter()
        .enumerate()
        .map(|(i, (path, score))| {
            let description = meta_map
                .get(&path)
                .map(|m| m.description.clone())
                .unwrap_or_default();
            let match_locations = get_match_locations(wiki_root, &path, query);
            SearchResult {
                rank: i + 1,
                path,
                score,
                description,
                match_locations,
            }
        })
        .collect())
}
