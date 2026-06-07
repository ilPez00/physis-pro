use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use usearch::{Index, IndexOptions, MetricKind};

use crate::indexer::FileMeta;

pub fn model_dir() -> PathBuf {
    let base = std::env::var("HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("USERPROFILE").map(PathBuf::from))
        .unwrap_or_else(|_| PathBuf::from("."));
    let dir = base.join(".llmwiki").join("models");
    std::fs::create_dir_all(&dir).ok();
    dir
}

pub fn load_embedding_model() -> Result<TextEmbedding> {
    TextEmbedding::try_new(
        InitOptions::new(EmbeddingModel::AllMiniLML6V2)
            .with_cache_dir(model_dir())
            .with_show_download_progress(false),
    )
    .context("Failed to load embedding model")
}

pub fn embed_texts(model: &mut TextEmbedding, texts: &[String]) -> Result<Vec<Vec<f32>>> {
    model
        .embed(texts, None)
        .context("Failed to embed texts")
}

#[cfg_attr(test, allow(dead_code))]
pub fn build_vector_index(wiki_root: &Path, meta: &[FileMeta]) -> Result<()> {
    if meta.is_empty() {
        return Ok(());
    }

    let mut model = load_embedding_model()?;

    let texts: Vec<String> = meta
        .iter()
        .map(|m| format!("{} {}", m.title, m.description))
        .collect();
    let embeddings = embed_texts(&mut model, &texts)?;
    let dimensions = embeddings[0].len();

    let options = IndexOptions {
        dimensions,
        metric: MetricKind::Cos,
        ..Default::default()
    };
    let index = Index::new(&options).map_err(|e| anyhow::anyhow!("{}", e))?;
    index
        .reserve(meta.len())
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    for (fm, embedding) in meta.iter().zip(embeddings.iter()) {
        index
            .add(fm.id as u64, embedding)
            .map_err(|e| anyhow::anyhow!("{}", e))
            .with_context(|| format!("Failed to add embedding for {}", fm.path))?;
    }

    let index_path = wiki_root.join(".index").join("vectors.usearch");
    index
        .save(index_path.to_str().unwrap())
        .map_err(|e| anyhow::anyhow!("{}", e))
        .context("Failed to save vector index")?;

    Ok(())
}

pub fn semantic_search(
    wiki_root: &Path,
    query: &str,
    top_k: usize,
) -> Result<Vec<(String, f32)>> {
    let mut model = load_embedding_model()?;
    let query_embeddings = embed_texts(&mut model, &[query.to_string()])?;
    let query_vec = query_embeddings
        .into_iter()
        .next()
        .context("Failed to embed query")?;

    let dimensions = query_vec.len();
    let index_path = wiki_root.join(".index").join("vectors.usearch");

    let options = IndexOptions {
        dimensions,
        metric: MetricKind::Cos,
        ..Default::default()
    };
    let index = Index::new(&options).map_err(|e| anyhow::anyhow!("{}", e))?;
    index
        .load(index_path.to_str().unwrap())
        .map_err(|e| anyhow::anyhow!("{}", e))
        .context("Vector index not found — run `llmwiki index` first")?;

    let results = index
        .search(&query_vec, top_k)
        .map_err(|e| anyhow::anyhow!("{}", e))
        .context("Vector search failed")?;

    let files_json_path = wiki_root.join(".index").join("files.json");
    let files_data = std::fs::read_to_string(&files_json_path)
        .context("files.json not found — run `llmwiki index` first")?;
    let path_to_id: HashMap<String, u32> = serde_json::from_str(&files_data)?;
    let id_to_path: HashMap<u32, String> =
        path_to_id.into_iter().map(|(path, id)| (id, path)).collect();

    let mut scored: Vec<(String, f32)> = results
        .keys
        .iter()
        .zip(results.distances.iter())
        .filter_map(|(&key, &dist)| {
            id_to_path.get(&(key as u32)).map(|path| {
                let score = 1.0 - dist;
                (path.clone(), score)
            })
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    Ok(scored)
}
