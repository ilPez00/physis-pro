//! HTTP surface for search-by-content.
//!
//! `/api/v1/search/nodes` and `/api/v1/ask` rank **labels** — the names the
//! graph already knows. This ranks the **text inside files**, at the section
//! the author wrote it in, so a query can find the paragraph that answers it
//! even though nobody ever labeled that paragraph.
//!
//! The index lives on [`AppCtx`] and is reused across requests. Rebuilding is
//! a full read + embed of the corpus; doing that per keystroke is the
//! difference between a search box and a progress bar.

use super::*;
use crate::content::{ContentIndex, SearchMode, SearchOptions};
use crate::web::{sandbox_existing, SharedState};
use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
};

#[derive(Debug, Deserialize)]
pub struct ContentSearchRequest {
    pub query: String,
    /// Directory to search. Defaults to the server's scan root, so the common
    /// case needs no argument and cannot escape the sandbox either way.
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    /// `hybrid` (default) · `semantic` · `lexical`.
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub place: Option<String>,
    #[serde(default)]
    pub period: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub path_contains: Option<String>,
    #[serde(default = "default_snippet")]
    pub snippet_chars: usize,
    /// Force a rebuild even when the filesystem signature is unchanged — the
    /// escape hatch for the one edit size+mtime cannot see.
    #[serde(default)]
    pub refresh: bool,
}

fn default_top_k() -> usize {
    20
}

fn default_snippet() -> usize {
    crate::content::DEFAULT_SNIPPET_CHARS
}

/// POST /api/v1/search/content
pub async fn content_search_handler(
    State(state): State<SharedState>,
    Json(req): Json<ContentSearchRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let root = resolve_root(&state, req.dir.as_deref())?;
    let query = req.query.trim().to_string();
    if query.is_empty() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "query must not be empty",
        ));
    }

    let started = std::time::Instant::now();
    let (index, rebuilt) = ensure_index(&state, &root, req.refresh).await?;
    let build_ms = started.elapsed().as_millis() as u64;

    // The query goes through the same embedder that embedded every section —
    // the embedder-consistency constraint. A different one would make cosine
    // meaningless while still returning a confident-looking ranking.
    let q_emb = if index.semantic {
        embed_text(state.shared.clone(), query.clone()).await
    } else {
        Vec::new()
    };

    let opts = SearchOptions {
        mode: req
            .mode
            .as_deref()
            .map(SearchMode::parse)
            .unwrap_or_default(),
        top_k: req.top_k.clamp(1, 200),
        snippet_chars: req.snippet_chars.clamp(80, 2000),
        place: req.place.filter(|s| !s.trim().is_empty()),
        period: req.period.filter(|s| !s.trim().is_empty()),
        kind: req.kind.filter(|s| !s.trim().is_empty()),
        path_contains: req.path_contains.filter(|s| !s.trim().is_empty()),
    };

    let search_started = std::time::Instant::now();
    let result = index.search(&query, &q_emb, &opts);
    let search_ms = search_started.elapsed().as_millis() as u64;

    let mut value = serde_json::to_value(&result).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(obj) = value.as_object_mut() {
        obj.insert("query".into(), serde_json::json!(query));
        obj.insert("root".into(), serde_json::json!(root.display().to_string()));
        obj.insert("rebuilt".into(), serde_json::json!(rebuilt));
        obj.insert("index_ms".into(), serde_json::json!(build_ms));
        obj.insert("search_ms".into(), serde_json::json!(search_ms));
        // Whether the semantic channel exists at all, and whether the vectors
        // behind it are real. A degraded (random-projection) embedder ranks
        // confidently and wrongly, so the surface says so rather than letting
        // the operator infer meaning from noise.
        obj.insert("semantic_index".into(), serde_json::json!(index.semantic));
        obj.insert("degraded".into(), serde_json::json!(state.shared.degraded));
        obj.insert(
            "requested_mode".into(),
            serde_json::json!(opts.mode.as_str()),
        );
    }
    Ok(Json(value))
}

/// GET /api/v1/search/content/section?path=…&section=…
///
/// The full text behind a hit, for the preview pane. Reading the file again
/// through the sandbox rather than caching bodies keeps the index small and
/// means the preview shows what is on disk *now*, not what was indexed.
#[derive(Debug, Deserialize)]
pub struct SectionParams {
    pub path: String,
    /// Breadcrumb as returned by a hit (`A › B`). Absent = whole document.
    #[serde(default)]
    pub section: Option<String>,
}

pub async fn content_section_handler(
    State(state): State<SharedState>,
    Query(params): Query<SectionParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let path = sandbox_existing(&state, &params.path)?;
    let text = tokio::task::spawn_blocking(move || std::fs::read_to_string(&path))
        .await
        .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("read: {e}")))?
        .map_err(|e| err_response(StatusCode::NOT_FOUND, &format!("read: {e}")))?;

    let title = std::path::Path::new(&params.path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let sections = crate::doctree::parse(&title, &text);
    let outline: Vec<serde_json::Value> = sections
        .iter()
        .map(|s| serde_json::json!({ "path": s.path(), "level": s.level, "heading": s.heading }))
        .collect();

    let wanted = params.section.as_deref().map(normalize_breadcrumb);
    let body = match &wanted {
        Some(w) => sections
            .iter()
            .find(|s| normalize_breadcrumb(&s.path()) == *w)
            .map(|s| s.body.clone()),
        None => None,
    };

    Ok(Json(serde_json::json!({
        "path": params.path,
        "title": title,
        "section": params.section,
        // Falls back to the whole document when the requested section is gone:
        // a file edited since indexing should still preview, with the outline
        // showing what it became.
        "body": body.unwrap_or(text),
        "outline": outline,
        "matched_section": wanted.is_some() && body_matched(&sections, wanted.as_deref()),
    })))
}

fn body_matched(sections: &[crate::doctree::Section], wanted: Option<&str>) -> bool {
    match wanted {
        Some(w) => sections.iter().any(|s| normalize_breadcrumb(&s.path()) == w),
        None => false,
    }
}

/// Hits cite with `›`, doctree joins with `>`; compare on neither.
fn normalize_breadcrumb(s: &str) -> String {
    s.replace('›', ">")
        .split('>')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(">")
        .to_lowercase()
}

fn resolve_root(
    state: &SharedState,
    dir: Option<&str>,
) -> Result<PathBuf, (StatusCode, Json<ErrorResponse>)> {
    match dir {
        Some(d) if !d.trim().is_empty() => sandbox_existing(state, d),
        _ => Ok(state.scan_root()),
    }
}

/// Return the live index for `root`, building it if missing, stale or forced.
///
/// Returns `(index, rebuilt)` so the caller can report the cost honestly: a
/// first search that took four seconds is fine, the same wait on every search
/// is a bug, and the only way to tell them apart from outside is to say which
/// one happened.
async fn ensure_index(
    state: &SharedState,
    root: &std::path::Path,
    force: bool,
) -> Result<(Arc<ContentIndex>, bool), (StatusCode, Json<ErrorResponse>)> {
    let root_buf = root.to_path_buf();

    if !force {
        let cached = {
            let guard = state
                .content_index
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            guard.clone()
        };
        if let Some(idx) = cached {
            if idx.root == root_buf {
                // Metadata-only walk: cheap enough to run per request, which is
                // what makes "the index is current" a fact rather than a hope.
                let sig = {
                    let r = root_buf.clone();
                    tokio::task::spawn_blocking(move || crate::content::scan_signature(&r))
                        .await
                        .unwrap_or_default()
                };
                if sig == idx.signature {
                    return Ok((idx, false));
                }
            }
        }
    }

    let shared = state.shared.clone();
    let build_root = root_buf.clone();
    let index = tokio::task::spawn_blocking(move || -> ContentIndex {
        let docs = crate::ingest::ingest_all(std::slice::from_ref(&build_root));
        let now = chrono::Utc::now();
        // Addresses come from filesystem metadata (two stats per file), not
        // from ingest time: `IngestedDoc.timestamp` is when we read the file,
        // so using it as the time facet would bucket the entire corpus into
        // the current month and make the period filter useless.
        let addresses: Vec<Option<crate::address::FileAddress>> = docs
            .iter()
            .map(|d| {
                Some(crate::address::FileAddress::derive(
                    &d.path,
                    &build_root,
                    crate::address::Mechanism::Cited,
                    "content-index",
                    now,
                ))
            })
            .collect();

        let texts = ContentIndex::retrieval_texts(&build_root, &docs, &addresses);
        // Without a real semantic model the vectors are a random projection:
        // ranking by them is worse than not ranking, because it is confident.
        // Build lexical-only instead — BM25 over real terms beats cosine over
        // noise — and the response reports `semantic_index: false` so nobody
        // reads paraphrase matching into a keyword result.
        let embeddings = if !shared.semantic || texts.is_empty() {
            Vec::new()
        } else {
            let mut cache = crate::pack::EmbedCache::load(&shared.config.data_dir);
            let out = cache.embed_texts(&texts, shared.embedder.as_ref(), shared.embedder_kind);
            cache.save(&shared.config.data_dir);
            out
        };
        ContentIndex::build(&build_root, &docs, &addresses, embeddings)
    })
    .await
    .map_err(|e| {
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("content index failed: {e}"),
        )
    })?;

    let index = Arc::new(index);
    {
        let mut guard = state
            .content_index
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *guard = Some(index.clone());
    }
    Ok((index, true))
}
