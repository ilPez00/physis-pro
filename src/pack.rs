//! File-aware token-fixed context packing.
//!
//! [`crate::rag`] packs a token budget from a list of strings. That is the right
//! core, but an agent needs provenance: it cannot act on "here is some text", it
//! needs "src/web.rs:2624-2701". This module walks a directory, chunks the files
//! it finds into line windows, keeps the path and line range beside each chunk,
//! and returns the packed result with that provenance attached.
//!
//! Nothing here calls a model API. Embedding is local, the token count is a
//! dependency-free heuristic, and selection is deterministic given the same
//! corpus — the same query returns the same context on every run, which is what
//! makes the output reviewable and the cost predictable.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::embed::VectorEmbed;
use crate::rag::{assemble_context, count_tokens, RagCorpus, TokenFixedRetriever};
use crate::scanner::is_text_file;

/// Lines per chunk. Small enough that a 4k budget holds several independent
/// regions, large enough that a function usually survives intact.
const CHUNK_LINES: usize = 40;

/// Files above this size are skipped: a multi-megabyte generated file will
/// dominate the corpus without carrying proportional meaning, and embedding it
/// costs more than it returns.
const MAX_FILE_BYTES: u64 = 512 * 1024;

/// One chunk of a file, with the provenance an agent needs to act on it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackedChunk {
    /// Path relative to the pack root, so output is stable across machines.
    pub path: String,
    /// 1-indexed inclusive first line.
    pub start_line: usize,
    /// 1-indexed inclusive last line.
    pub end_line: usize,
    /// Cosine similarity to the query.
    pub score: f32,
    /// Token cost of this chunk, by the same counter that enforced the budget.
    pub tokens: usize,
    pub text: String,
}

/// The result of a pack: what was selected, what it cost, what was left out.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackResult {
    /// Assembled context, ready to paste into a prompt.
    pub context: String,
    /// Total tokens of `context`. Guaranteed `<= budget`.
    pub tokens: usize,
    /// The budget this pack was constrained to.
    pub budget: usize,
    /// Selected chunks in score order, with provenance.
    pub chunks: Vec<PackedChunk>,
    /// Candidate chunks that did not fit. This number is the argument for the
    /// product: it is the context a naive retriever would have pulled in blind.
    pub dropped: usize,
    /// Chunks considered in total.
    pub candidates: usize,
    /// Files walked.
    pub files_scanned: usize,
}

/// A chunk before retrieval: text plus where it came from.
struct Candidate {
    path: String,
    start_line: usize,
    end_line: usize,
    text: String,
}

/// Split one file's contents into line-window candidates.
///
/// Chunks that are entirely whitespace are dropped: they embed to noise and
/// waste budget.
fn chunk_file(rel_path: &str, contents: &str) -> Vec<Candidate> {
    let lines: Vec<&str> = contents.lines().collect();
    let mut out = Vec::new();

    for (idx, window) in lines.chunks(CHUNK_LINES).enumerate() {
        let text = window.join("\n");
        if text.trim().is_empty() {
            continue;
        }
        let start_line = idx * CHUNK_LINES + 1;
        out.push(Candidate {
            path: rel_path.to_string(),
            start_line,
            end_line: start_line + window.len() - 1,
            text,
        });
    }
    out
}

/// Collect candidate chunks under `root`.
///
/// Skips the usual build and VCS noise. This is deliberately a denylist rather
/// than a language allowlist: a corpus of docs, configs and logs is as valid a
/// target as a code tree, and an allowlist would silently exclude it.
fn collect_candidates(root: &Path) -> (Vec<Candidate>, usize) {
    const SKIP_DIRS: [&str; 8] = [
        ".git",
        "target",
        "node_modules",
        "vendor",
        "dist",
        "build",
        ".venv",
        "__pycache__",
    ];

    let mut candidates = Vec::new();
    let mut files_scanned = 0usize;

    let walker = WalkDir::new(root).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        !(e.file_type().is_dir() && SKIP_DIRS.contains(&name.as_ref()))
    });

    for entry in walker.filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.metadata().map(|m| m.len() > MAX_FILE_BYTES).unwrap_or(true) {
            continue;
        }
        if !is_text_file(entry.path()) {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        files_scanned += 1;

        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .to_string();
        candidates.extend(chunk_file(&rel, &contents));
    }

    (candidates, files_scanned)
}

/// A content-addressed embedding cache.
///
/// Packing re-embeds the whole corpus on every call, which for a 1000-chunk
/// tree costs minutes — fine for a batch job, useless for an agent that packs
/// several times a session. Chunk text is stable between runs, so the embedding
/// is cacheable by content hash: an unchanged file costs a hash instead of a
/// forward pass, and only genuinely new or edited chunks reach the model.
///
/// The cache is keyed by `(embedder_kind, dim, sha256(text))`. Including the
/// embedder identity is what makes it safe: vectors from two different models
/// are not comparable, and a cache that mixed them would silently return
/// nonsense scores rather than fail.
#[derive(Default, Serialize, Deserialize)]
pub struct EmbedCache {
    /// Hex sha256 (namespaced by embedder) → embedding.
    entries: std::collections::HashMap<String, Vec<f32>>,
    #[serde(skip)]
    hits: usize,
    #[serde(skip)]
    misses: usize,
}

impl EmbedCache {
    /// Load the cache for `data_dir`, or start empty when absent or corrupt.
    /// A corrupt cache is a performance problem, never a correctness one, so it
    /// is discarded silently rather than failing the pack.
    pub fn load(data_dir: &Path) -> Self {
        let path = Self::path(data_dir);
        std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn path(data_dir: &Path) -> PathBuf {
        data_dir.join("pack_embed_cache.json")
    }

    pub fn save(&self, data_dir: &Path) {
        let _ = std::fs::create_dir_all(data_dir);
        if let Ok(bytes) = serde_json::to_vec(self) {
            let _ = std::fs::write(Self::path(data_dir), bytes);
        }
    }

    /// Cache hits and misses from the last [`Self::embed_texts`] call.
    pub fn stats(&self) -> (usize, usize) {
        (self.hits, self.misses)
    }

    fn key(embedder_kind: &str, dim: usize, text: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        format!("{embedder_kind}:{dim}:{:x}", hasher.finalize())
    }

    /// Embed `texts`, serving what it can from cache and batching the rest.
    ///
    /// Order is preserved: the caller indexes results positionally against the
    /// input, so a reordering here would silently misattribute every embedding.
    pub fn embed_texts(
        &mut self,
        texts: &[String],
        embedder: &dyn VectorEmbed,
        embedder_kind: &str,
    ) -> Vec<Vec<f32>> {
        // Kept small deliberately. ONNX allocates per-thread activation arenas
        // sized by batch x sequence length, and at 32 the process was OOM-killed
        // on a 14 GB machine embedding a ~1100-chunk tree. Throughput barely
        // moves between 8 and 32; peak memory moves a lot.
        const EMBED_BATCH: usize = 8;

        let dim = embedder.dimension();
        let keys: Vec<String> = texts
            .iter()
            .map(|t| Self::key(embedder_kind, dim, t))
            .collect();

        let mut out: Vec<Option<Vec<f32>>> = keys
            .iter()
            .map(|k| self.entries.get(k).cloned())
            .collect();

        let missing: Vec<usize> = out
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| slot.is_none().then_some(i))
            .collect();

        self.hits = texts.len() - missing.len();
        self.misses = missing.len();

        for batch in missing.chunks(EMBED_BATCH) {
            let refs: Vec<&str> = batch.iter().map(|&i| texts[i].as_str()).collect();
            for (&i, embedding) in batch.iter().zip(embedder.embed_batch(&refs)) {
                self.entries.insert(keys[i].clone(), embedding.clone());
                out[i] = Some(embedding);
            }
        }

        out.into_iter().map(|slot| slot.unwrap_or_default()).collect()
    }
}

/// Pack the highest-scoring context for `query` from the corpus under `root`,
/// never exceeding `budget` tokens.
///
/// `diversity` in `0.0..=1.0` trades relevance for spread: at 0 the pack is
/// pure top-score and can return five near-identical chunks; raising it
/// discounts candidates that resemble what is already selected.
pub fn pack(
    root: &Path,
    query: &str,
    budget: usize,
    top_k: usize,
    diversity: f32,
    embedder: &dyn VectorEmbed,
) -> PackResult {
    let (candidates, files_scanned) = collect_candidates(root);

    if candidates.is_empty() {
        return PackResult {
            context: String::new(),
            tokens: 0,
            budget,
            chunks: Vec::new(),
            dropped: 0,
            candidates: 0,
            files_scanned,
        };
    }

    let texts: Vec<String> = candidates.iter().map(|c| c.text.clone()).collect();
    let corpus = RagCorpus::build(&texts, embedder);
    let query_embedding = embedder.embed(query);

    finish_pack(candidates, files_scanned, corpus, &query_embedding, budget, top_k, diversity)
}

/// Cached counterpart to [`pack`]: identical output, but chunk embeddings are
/// served from `cache` where possible. The caller owns the cache so it can be
/// persisted across runs — that persistence is the entire point.
#[allow(clippy::too_many_arguments)]
pub fn pack_cached(
    root: &Path,
    query: &str,
    budget: usize,
    top_k: usize,
    diversity: f32,
    embedder: &dyn VectorEmbed,
    embedder_kind: &str,
    cache: &mut EmbedCache,
) -> PackResult {
    let (candidates, files_scanned) = collect_candidates(root);

    if candidates.is_empty() {
        return PackResult {
            context: String::new(),
            tokens: 0,
            budget,
            chunks: Vec::new(),
            dropped: 0,
            candidates: 0,
            files_scanned,
        };
    }

    let texts: Vec<String> = candidates.iter().map(|c| c.text.clone()).collect();
    let embeddings = cache.embed_texts(&texts, embedder, embedder_kind);

    // Assembled directly rather than via `RagCorpus::build`, which would embed
    // again and discard everything the cache just saved.
    let corpus = RagCorpus {
        chunks: texts
            .iter()
            .enumerate()
            .map(|(i, t)| crate::rag::RagChunk {
                id: i,
                text: t.clone(),
                tokens: crate::rag::count_tokens(t),
                embedding: embeddings[i].clone(),
            })
            .collect(),
    };

    // The query is deliberately not cached: it is one forward pass, it is
    // usually novel, and caching it would grow the file without bound.
    let query_embedding = embedder.embed(query);

    finish_pack(candidates, files_scanned, corpus, &query_embedding, budget, top_k, diversity)
}

/// Shared tail of [`pack`] and [`pack_cached`]: retrieve, then reattach the
/// provenance that the retriever does not carry.
#[allow(clippy::too_many_arguments)]
fn finish_pack(
    candidates: Vec<Candidate>,
    files_scanned: usize,
    corpus: RagCorpus,
    query_embedding: &[f32],
    budget: usize,
    top_k: usize,
    diversity: f32,
) -> PackResult {

    let retriever = TokenFixedRetriever::new(budget, top_k).with_diversity(diversity);
    let result = retriever.retrieve(query_embedding, &corpus);

    // `RetrievedChunk::id` indexes back into the corpus, which was built in
    // candidate order — so the id recovers the provenance we set aside.
    let chunks: Vec<PackedChunk> = result
        .chunks
        .iter()
        .filter_map(|rc| {
            candidates.get(rc.id).map(|c| PackedChunk {
                path: c.path.clone(),
                start_line: c.start_line,
                end_line: c.end_line,
                score: rc.score,
                tokens: rc.tokens,
                text: rc.text.clone(),
            })
        })
        .collect();

    let context = assemble_context(&result, "\n\n---\n\n");

    PackResult {
        // Report the packed total, not a recount of the joined string: the
        // separators are not part of what the budget bought, and recounting
        // would make the number drift above `budget` for large packs.
        tokens: result.total_tokens,
        context,
        budget,
        dropped: candidates.len().saturating_sub(chunks.len()),
        candidates: candidates.len(),
        chunks,
        files_scanned,
    }
}

/// Render a pack as the compact summary a human reads in a terminal.
pub fn summarize(result: &PackResult) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "packed {}/{} tokens from {} chunks ({} candidates over {} files, {} dropped)\n",
        result.tokens,
        result.budget,
        result.chunks.len(),
        result.candidates,
        result.files_scanned,
        result.dropped
    ));
    for c in &result.chunks {
        out.push_str(&format!(
            "  {:>6.3}  {}:{}-{}  ({} tokens)\n",
            c.score, c.path, c.start_line, c.end_line, c.tokens
        ));
    }
    out
}

/// Token count of an arbitrary string, using the same counter the packer uses
/// to enforce the budget. Exposed so callers can measure a baseline against it.
pub fn tokens_of(text: &str) -> usize {
    count_tokens(text)
}

/// Resolve a caller-supplied root, rejecting anything outside `confine_to`.
///
/// The MCP server accepts a path from the agent, and an agent following a
/// malicious instruction will happily ask for `/etc` or `../../..`. Callers that
/// expose packing over a protocol must route through here.
pub fn resolve_root(requested: &Path, confine_to: Option<&Path>) -> anyhow::Result<PathBuf> {
    let canonical = requested
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("cannot resolve {}: {e}", requested.display()))?;

    if let Some(confine) = confine_to {
        let base = confine
            .canonicalize()
            .map_err(|e| anyhow::anyhow!("cannot resolve scan root {}: {e}", confine.display()))?;
        if !canonical.starts_with(&base) {
            anyhow::bail!(
                "{} is outside the permitted scan root {}",
                canonical.display(),
                base.display()
            );
        }
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::RandomProjectionEmbedder;

    fn embedder() -> RandomProjectionEmbedder {
        RandomProjectionEmbedder::new(64)
    }

    fn write(dir: &Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).unwrap();
    }

    #[test]
    fn chunking_records_one_indexed_inclusive_line_ranges() {
        let body: String = (1..=90).map(|i| format!("line {i}\n")).collect();
        let chunks = chunk_file("a.txt", &body);

        assert_eq!(chunks.len(), 3); // 40 + 40 + 10
        assert_eq!((chunks[0].start_line, chunks[0].end_line), (1, 40));
        assert_eq!((chunks[1].start_line, chunks[1].end_line), (41, 80));
        assert_eq!((chunks[2].start_line, chunks[2].end_line), (81, 90));
    }

    #[test]
    fn whitespace_only_chunks_are_dropped() {
        let body = "\n".repeat(120);
        assert!(chunk_file("blank.txt", &body).is_empty());
    }

    #[test]
    fn pack_never_exceeds_budget() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..12 {
            let body: String = (0..60).map(|l| format!("content {i} line {l}\n")).collect();
            write(dir.path(), &format!("f{i}.txt"), &body);
        }

        for budget in [50usize, 200, 800] {
            let result = pack(dir.path(), "content", budget, 50, 0.0, &embedder());
            assert!(
                result.tokens <= budget,
                "packed {} tokens over budget {budget}",
                result.tokens
            );
        }
    }

    #[test]
    fn pack_reports_provenance_for_every_chunk() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "alpha.txt", &"alpha content\n".repeat(50));

        let result = pack(dir.path(), "alpha", 2000, 10, 0.0, &embedder());
        assert!(!result.chunks.is_empty());
        for c in &result.chunks {
            assert_eq!(c.path, "alpha.txt");
            assert!(c.start_line >= 1, "line numbers are 1-indexed");
            assert!(c.end_line >= c.start_line);
        }
    }

    #[test]
    fn dropped_counts_what_did_not_fit() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..10 {
            write(dir.path(), &format!("f{i}.txt"), &format!("subject {i}\n").repeat(50));
        }

        // A budget far below the corpus must drop most candidates.
        let result = pack(dir.path(), "subject", 40, 50, 0.0, &embedder());
        assert!(result.candidates > result.chunks.len());
        assert_eq!(result.dropped, result.candidates - result.chunks.len());
    }

    #[test]
    fn empty_corpus_returns_an_empty_pack_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = pack(dir.path(), "anything", 1000, 10, 0.0, &embedder());
        assert_eq!(result.tokens, 0);
        assert!(result.chunks.is_empty());
        assert_eq!(result.dropped, 0);
    }

    #[test]
    fn skipped_directories_are_not_walked() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("target")).unwrap();
        write(&dir.path().join("target"), "junk.txt", &"junk\n".repeat(50));
        write(dir.path(), "real.txt", &"real\n".repeat(50));

        let result = pack(dir.path(), "junk", 4000, 50, 0.0, &embedder());
        assert!(
            result.chunks.iter().all(|c| !c.path.contains("target")),
            "target/ must not be packed"
        );
    }

    #[test]
    fn resolve_root_rejects_traversal_outside_the_confine() {
        let dir = tempfile::tempdir().unwrap();
        let inside = dir.path().join("inside");
        std::fs::create_dir(&inside).unwrap();

        assert!(resolve_root(&inside, Some(dir.path())).is_ok());

        // The classic agent-driven escape: a relative path climbing out.
        let escape = inside.join("..").join("..");
        assert!(
            resolve_root(&escape, Some(dir.path())).is_err(),
            "traversal above the confine must be rejected"
        );
    }

    #[test]
    fn resolve_root_without_a_confine_allows_any_existing_path() {
        let dir = tempfile::tempdir().unwrap();
        assert!(resolve_root(dir.path(), None).is_ok());
    }
}
