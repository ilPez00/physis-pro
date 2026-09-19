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
use crate::rag::{
    bm25_terms, count_tokens, rank_hybrid, Bm25Index, RagCorpus, TokenFixedRetriever,
};
use crate::scanner::is_text_file;
use physis_core::worldstate::extract_code_identifiers;

/// Lines per chunk. Small enough that a 4k budget holds several independent
/// regions, large enough that a function usually survives intact.
const CHUNK_LINES: usize = 40;

/// Files above this size are skipped: a multi-megabyte generated file will
/// dominate the corpus without carrying proportional meaning, and embedding it
/// costs more than it returns.
const MAX_FILE_BYTES: u64 = 512 * 1024;

/// Which leg orders the candidates before the token budget is filled.
///
/// `Cosine` is the frozen baseline and the default: a change to a retrieval
/// default is a product decision, and it is made only when an equal-budget
/// measurement on a frozen holdout says so (`HybridVerdict::ships_fusion` is
/// the repository's own rule, and
/// `docs/plans/2026-09-16-day3-equal-budget-retrieval.md` is the measurement).
///
/// `Hybrid` fuses cosine with the Okapi BM25 leg by reciprocal rank fusion —
/// both legs and the fusion already live in `physis_core::rag`; this module
/// only chooses between them and then applies the same token budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PackMode {
    /// Rank by cosine to the query. The shipped default.
    #[default]
    Cosine,
    /// Rank by `rank_hybrid` (cosine + BM25, fused by RRF).
    Hybrid,
    /// Rank by the lexical leg alone, falling back to cosine when the query
    /// shares no term with the corpus.
    ///
    /// Three frozen holdouts measured a plain BM25 baseline ahead of the cosine
    /// pack on evidence spans at the same budget (5/9, 4/12, 7/12 against
    /// 3/9, 3/12, 4/12) and 1-2 s against 65-105 s
    /// (`docs/plans/2026-09-16-day3-retrieval-repair.md` §9).
    ///
    /// The fallback is not decoration: BM25 scores every document zero when the
    /// query shares no term, and `Bm25Index::rank` then returns insertion order,
    /// which would hand the caller an arbitrary prefix of the corpus. The
    /// fallback only fires on an empty result, so it cannot lower a score the
    /// pure lexical leg already achieves.
    Lexical,
    /// Choose between `Lexical` and `Cosine` from the query's own shape: the
    /// lexical leg when the query carries a code identifier, cosine otherwise.
    ///
    /// Four frozen holdouts measured the two legs trading places (3-1 to BM25,
    /// `docs/plans/2026-09-16-day3-retrieval-repair.md` §9). The mechanism is
    /// visible in the per-class breakdown: BM25 wins queries whose surface form
    /// carries the answer, cosine wins definition-seeking queries, because term
    /// frequency prefers windows that *mention* an identifier over the window
    /// that *defines* it. This variant acts on that difference instead of
    /// picking a side for every query.
    ///
    /// The detector is not new: `extract_code_identifiers` already exists in
    /// `physis-core/src/worldstate.rs`, added for E60 after the chunker scored
    /// precision 0.224 on this repository's own prose. This is a rule, not a
    /// tuned parameter — there is nothing to fit, so it can be specified before
    /// the holdout it is scored on.
    Auto,
}

impl std::str::FromStr for PackMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "cosine" => Ok(PackMode::Cosine),
            "hybrid" => Ok(PackMode::Hybrid),
            "lexical" => Ok(PackMode::Lexical),
            "auto" => Ok(PackMode::Auto),
            other => Err(format!(
                "unknown pack mode {other:?}; expected `cosine`, `hybrid`, `lexical` or `auto`"
            )),
        }
    }
}

/// One chunk of a file, with the provenance an agent needs to act on it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackedChunk {
    /// Path relative to the pack root, so output is stable across machines.
    pub path: String,
    /// 1-indexed inclusive first line.
    pub start_line: usize,
    /// 1-indexed inclusive last line.
    pub end_line: usize,
    /// The ranking leg's score for this chunk: cosine to the query under
    /// `PackMode::Cosine`, a reciprocal-rank-fusion score under `Hybrid`, an
    /// Okapi BM25 score under `Lexical`. Comparable within a pack, not across
    /// modes — and an adjacency-expanded chunk carries its parent's score.
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
        if entry
            .metadata()
            .map(|m| m.len() > MAX_FILE_BYTES)
            .unwrap_or(true)
        {
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

    /// Write the cache atomically: temp file in the same directory, then
    /// rename over the target.
    ///
    /// A plain `fs::write` truncates before writing, so an interrupted or
    /// failing write leaves an empty file and destroys the cache. That is not
    /// hypothetical: a full disk truncated a warmed cache to zero bytes and,
    /// because the error was discarded, the only symptom was the next run
    /// silently re-embedding everything — 80 s of work, reported as nothing.
    /// `rename` within one filesystem is atomic, so a reader sees either the
    /// previous cache or the new one, never a torn one.
    ///
    /// Failures are reported on stderr rather than returned: a cache that
    /// cannot be written is a performance problem, not a reason to fail the
    /// pack the caller actually asked for. But it is never silent.
    pub fn save(&self, data_dir: &Path) {
        if let Err(e) = self.try_save(data_dir) {
            eprintln!(
                "physis: could not write embedding cache to {} — {e}. \
                 The next run will recompute embeddings.",
                Self::path(data_dir).display()
            );
        }
    }

    fn try_save(&self, data_dir: &Path) -> std::io::Result<()> {
        let bytes = serde_json::to_vec(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        // A lost rename only costs a rebuild here, so the cheaper variant is
        // the right trade for a cache.
        crate::atomic::write_atomic(&Self::path(data_dir), &bytes)
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

        let mut out: Vec<Option<Vec<f32>>> =
            keys.iter().map(|k| self.entries.get(k).cloned()).collect();

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

        out.into_iter()
            .map(|slot| slot.unwrap_or_default())
            .collect()
    }
}

/// Pack the highest-scoring context for `query` from the corpus under `root`,
/// never exceeding `budget` tokens.
///
/// `diversity` in `0.0..=1.0` trades relevance for spread: at 0 the pack is
/// pure top-score and can return five near-identical chunks; raising it
/// discounts candidates that resemble what is already selected.
#[allow(clippy::too_many_arguments)]
pub fn pack(
    root: &Path,
    query: &str,
    budget: usize,
    top_k: usize,
    diversity: f32,
    mode: PackMode,
    expand: bool,
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
    let corpus = crate::rag::corpus_from_texts(&texts, embedder);
    let query_embedding = embedder.embed(query);

    finish_pack(
        candidates,
        files_scanned,
        corpus,
        &query_embedding,
        query,
        budget,
        top_k,
        diversity,
        mode,
        expand,
    )
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
    mode: PackMode,
    expand: bool,
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
        // The lexical leg costs one pass over texts already in memory. It is
        // built unconditionally so `PackMode::Hybrid` is a selection change,
        // not a re-embed — the corpus is identical either way.
        bm25: Bm25Index::build(&texts),
    };

    // The query is deliberately not cached: it is one forward pass, it is
    // usually novel, and caching it would grow the file without bound.
    let query_embedding = embedder.embed(query);

    finish_pack(
        candidates,
        files_scanned,
        corpus,
        &query_embedding,
        query,
        budget,
        top_k,
        diversity,
        mode,
        expand,
    )
}

/// Separator between chunks in the assembled context.
const CONTEXT_SEPARATOR: &str = "\n\n---\n\n";
/// Does the query carry a token shaped like a code identifier?
///
/// Delegates to the detector E60 already uses rather than inventing a second
/// one: `extract_code_identifiers` qualifies a token containing `::`, a path
/// with a dot, a `.rs` ending, an underscore, or an ALL-CAPS word of length 4+.
///
/// Note what that does **not** catch: PascalCase type names. `Bm25Index` and
/// `CellClassifier` read as prose to this detector, so `Auto` sends them to the
/// cosine leg — which is what the holdouts say those queries want, since a bare
/// type name is a definition-seeking query and cosine is the leg that finds
/// definitions. `choose_worker` and `count_tokens` are underscore-shaped, so
/// they go lexical, which is also where they were won. The routing and the
/// measured behaviour agree; neither was fitted to the other.
fn query_carries_a_code_identifier(query: &str) -> bool {
    extract_code_identifiers(&[query])
        .first()
        .is_some_and(|keys| !keys.is_empty())
}

/// The immediate window neighbours of each ranked chunk, carrying the parent's
/// score.
///
/// The 40-line grid cuts a region in half whenever the evidence straddles a
/// boundary, and the second half routinely loses the greedy fill to an
/// unrelated chunk from another file — measured as 0 of 7 boundary-straddling
/// cases covered by any ranking leg
/// (`docs/plans/2026-09-16-day3-retrieval-repair.md`).
///
/// A neighbour **inherits its parent's score**: what the retrieval selected is
/// the region the two windows together span, and the evidence for that region
/// is the parent's score. It is not a retrieval hit of its own, so no similarity
/// the embedder never computed is invented for it.
///
/// The next window is offered before the previous one — a region is more often
/// cut after the evidence than before it — and a window that does not exist
/// (all-whitespace, so `chunk_file` dropped it) is simply absent from the map.
fn neighbour_candidates(ranked: &[(usize, f32)], candidates: &[Candidate]) -> Vec<(usize, f32)> {
    let mut by_start: std::collections::HashMap<(&str, usize), usize> =
        std::collections::HashMap::new();
    for (i, c) in candidates.iter().enumerate() {
        by_start.insert((c.path.as_str(), c.start_line), i);
    }

    let mut taken: std::collections::HashSet<usize> = ranked.iter().map(|(id, _)| *id).collect();
    let mut out: Vec<(usize, f32)> = Vec::new();
    for (id, score) in ranked {
        let (path, start) = (candidates[*id].path.as_str(), candidates[*id].start_line);
        for neighbour in [start + CHUNK_LINES, start.saturating_sub(CHUNK_LINES)] {
            if neighbour == start || neighbour == 0 {
                continue;
            }
            let Some(&ni) = by_start.get(&(path, neighbour)) else {
                continue;
            };
            if !taken.insert(ni) {
                continue;
            }
            out.push((ni, *score));
        }
    }
    out
}
/// Shared tail of [`pack`] and [`pack_cached`]: retrieve, then reattach the
/// provenance that the retriever does not carry.
#[allow(clippy::too_many_arguments)]
fn finish_pack(
    candidates: Vec<Candidate>,
    files_scanned: usize,
    corpus: RagCorpus,
    query_embedding: &[f32],
    query_text: &str,
    budget: usize,
    top_k: usize,
    diversity: f32,
    mode: PackMode,
    expand: bool,
) -> PackResult {
    let retriever = TokenFixedRetriever::new(budget, top_k).with_diversity(diversity);
    // `Auto` resolves to one of the two legs before any work happens, so the
    // arms below stay a plain match and the choice is inspectable.
    let mode = match mode {
        PackMode::Auto => {
            if query_carries_a_code_identifier(query_text) {
                PackMode::Lexical
            } else {
                PackMode::Cosine
            }
        }
        other => other,
    };
    // Same budget, same top_k, same MMR discount — only the ranking that feeds
    // them differs. That is what makes the two modes comparable.
    let result = match mode {
        PackMode::Cosine => retriever.retrieve(query_embedding, &corpus),
        PackMode::Hybrid => retriever
            .retrieve_with_ranking(&rank_hybrid(&corpus, query_embedding, query_text), &corpus),
        // `Auto` was resolved above; this arm is unreachable but keeps the
        // match exhaustive without a wildcard.
        PackMode::Auto | PackMode::Lexical => {
            let lexical: Vec<(usize, f32)> = corpus
                .bm25
                .rank(&bm25_terms(query_text))
                .into_iter()
                .filter(|(_, score)| *score > 0.0)
                .collect();
            if lexical.is_empty() {
                // No term in common: an empty pack would be the honest answer
                // and a useless one, so fall back to the semantic leg.
                retriever.retrieve(query_embedding, &corpus)
            } else {
                retriever.retrieve_with_ranking(&lexical, &corpus)
            }
        }
    };

    // (candidate id, score, tokens) in selection order. `RetrievedChunk::id`
    // indexes back into the corpus, which was built in candidate order — so the
    // id recovers the provenance we set aside.
    let mut selected: Vec<(usize, f32, usize)> = result
        .chunks
        .iter()
        .map(|rc| (rc.id, rc.score, rc.tokens))
        .collect();
    let mut used = result.total_tokens;

    // Adjacency expansion (see `neighbour_candidates`). The pool is enlarged,
    // then the *same* budget fill runs over it, so a neighbour must win its
    // place on the parent's score rather than find slack. Slack never exists:
    // the greedy fill saturates the budget by construction (2999/3000 measured
    // on every case), which is why the first version of this — "take the
    // neighbour if it fits" — added zero chunks in 12 of 12 cases and was a
    // no-op. Competing is the policy; fitting is not.
    if expand {
        let mut pool: Vec<(usize, f32)> = selected.iter().map(|(id, s, _)| (*id, *s)).collect();
        pool.extend(neighbour_candidates(&pool, &candidates));
        let refilled = retriever.retrieve_with_ranking(&pool, &corpus);
        selected = refilled
            .chunks
            .iter()
            .map(|rc| (rc.id, rc.score, rc.tokens))
            .collect();
        used = refilled.total_tokens;
    }

    let chunks: Vec<PackedChunk> = selected
        .iter()
        .filter_map(|(id, score, tokens)| {
            candidates.get(*id).map(|c| PackedChunk {
                path: c.path.clone(),
                start_line: c.start_line,
                end_line: c.end_line,
                score: *score,
                tokens: *tokens,
                text: c.text.clone(),
            })
        })
        .collect();

    let context = chunks
        .iter()
        .map(|c| c.text.as_str())
        .collect::<Vec<_>>()
        .join(CONTEXT_SEPARATOR);

    PackResult {
        // Report the packed total, not a recount of the joined string: the
        // separators are not part of what the budget bought, and recounting
        // would make the number drift above `budget` for large packs.
        tokens: used,
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

    /// A failed save must leave the previous cache intact.
    ///
    /// This exercises the failure that is reachable from a test — the temp
    /// file cannot be created — not the ENOSPC one that motivated the change
    /// (a full disk truncated a warmed cache to zero bytes). Both are handled
    /// by the same structure: nothing touches the target path until a complete
    /// temp file has been fsynced, so any earlier failure is a no-op. Short
    /// writes mid-`write_all` take the same path and are covered by
    /// construction rather than by this test.
    #[test]
    fn a_failed_save_leaves_the_previous_cache_intact() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = EmbedCache::default();
        cache
            .entries
            .insert("kind:2:abc".to_string(), vec![1.0, 2.0]);
        cache.save(dir.path());

        let before = std::fs::read(EmbedCache::path(dir.path())).unwrap();
        assert!(
            !before.is_empty(),
            "baseline cache should have been written"
        );

        // Make the directory unwritable so the temp file cannot be created.
        // The rename never happens, so the existing cache must survive.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let ro = std::fs::Permissions::from_mode(0o500);
            std::fs::set_permissions(dir.path(), ro).unwrap();

            let mut bigger = EmbedCache::default();
            bigger
                .entries
                .insert("kind:2:abc".to_string(), vec![1.0, 2.0]);
            bigger
                .entries
                .insert("kind:2:def".to_string(), vec![3.0, 4.0]);
            bigger.save(dir.path()); // reports on stderr, must not panic

            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        }

        let after = std::fs::read(EmbedCache::path(dir.path())).unwrap();
        assert_eq!(before, after, "a failed save must not damage the cache");

        let reloaded = EmbedCache::load(dir.path());
        assert_eq!(reloaded.entries.len(), 1);
    }

    #[test]
    fn save_then_load_round_trips_entries() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = EmbedCache::default();
        cache
            .entries
            .insert("kind:3:xyz".to_string(), vec![0.5, 0.25, 0.125]);
        cache.save(dir.path());

        let reloaded = EmbedCache::load(dir.path());
        assert_eq!(
            reloaded.entries.get("kind:3:xyz").unwrap(),
            &vec![0.5, 0.25, 0.125]
        );

        // No temp files left behind after a successful save.
        let strays: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains("tmp"))
            .collect();
        assert!(strays.is_empty(), "temp files left behind: {strays:?}");
    }

    #[test]
    fn neighbours_carry_the_parent_score_and_are_not_duplicated() {
        let body: String = (1..=80)
            .map(|i| format!("line {i} of the region\n"))
            .collect();
        let cands = chunk_file("answer.txt", &body);
        assert_eq!(cands.len(), 2, "80 lines must chunk into two windows");

        // One selected window: its sibling is offered at the parent's score.
        let ranked = vec![(0usize, 0.9f32)];
        assert_eq!(neighbour_candidates(&ranked, &cands), vec![(1, 0.9)]);

        // Both halves selected: nothing left to offer.
        let both = vec![(0usize, 0.9f32), (1usize, 0.8f32)];
        assert!(neighbour_candidates(&both, &cands).is_empty());

        // A window that chunk_file dropped (all whitespace) has no neighbour.
        let mut gapped: String = (1..=40).map(|i| format!("head {i}\n")).collect();
        gapped.push_str(&"\n".repeat(40));
        gapped.push_str(
            &(81..=120)
                .map(|i| format!("tail {i}\n"))
                .collect::<String>(),
        );
        let gapped_cands = chunk_file("gap.txt", &gapped);
        assert_eq!(gapped_cands.len(), 2, "the blank middle window is dropped");
        assert_eq!(gapped_cands[1].start_line, 81);
        assert!(
            neighbour_candidates(&[(0, 0.5)], &gapped_cands).is_empty(),
            "start+40 is the dropped window, not a candidate"
        );
    }

    /// Expansion must *compete* for the budget, not look for slack: the greedy
    /// fill saturates it, so a neighbour that only "fits" is never taken.
    #[test]
    fn expansion_displaces_a_lower_ranked_chunk_for_the_neighbour() {
        let dir = tempfile::tempdir().unwrap();
        // The first window matches the query; the second does not, so on its
        // own cosine it would never be selected.
        let answer: String = (1..=80)
            .map(|i| {
                if i <= 40 {
                    format!("boundary straddle marker line {i}\n")
                } else {
                    format!("continuation of that same region line {i}\n")
                }
            })
            .collect();
        write(dir.path(), "answer.txt", &answer);
        // Filler that *does* share the query's words, so it outranks the
        // continuation on its own score.
        for f in 0..8 {
            let body: String = (1..=120)
                .map(|i| format!("boundary straddle marker filler {f} line {i}\n"))
                .collect();
            write(dir.path(), &format!("noise{f}.txt"), &body);
        }

        let e = embedder();
        let pack_with = |expand: bool| {
            pack(
                dir.path(),
                "boundary straddle marker",
                600,
                50,
                0.0,
                PackMode::Cosine,
                expand,
                &e,
            )
        };
        let plain = pack_with(false);
        let expanded = pack_with(true);

        assert!(plain.tokens <= 600 && expanded.tokens <= 600);
        let half = |r: &PackResult, start: usize| {
            r.chunks
                .iter()
                .any(|c| c.path == "answer.txt" && c.start_line == start)
        };
        assert!(
            half(&plain, 1),
            "the matching window is selected either way"
        );
        assert!(
            !half(&plain, 41),
            "without expansion the continuation loses to filler: {:?}",
            plain
                .chunks
                .iter()
                .map(|c| (&c.path, c.start_line))
                .collect::<Vec<_>>()
        );
        assert!(
            half(&expanded, 41),
            "with expansion the continuation displaces filler: {:?}",
            expanded
                .chunks
                .iter()
                .map(|c| (&c.path, c.start_line))
                .collect::<Vec<_>>()
        );
    }

    /// The lexical leg must select on the query's own words, and must not hand
    /// back an arbitrary corpus prefix when the query shares no term.
    #[test]
    fn lexical_mode_prefers_the_matching_window_and_falls_back_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        let needle: String = (1..=40)
            .map(|i| format!("zirconium impeller calibration step {i}\n"))
            .collect();
        write(dir.path(), "needle.txt", &needle);
        for f in 0..4 {
            let body: String = (1..=120)
                .map(|i| format!("unrelated prose {f} line {i}\n"))
                .collect();
            write(dir.path(), &format!("other{f}.txt"), &body);
        }

        let e = embedder();
        let hit = pack(
            dir.path(),
            "zirconium impeller",
            3000,
            50,
            0.0,
            PackMode::Lexical,
            false,
            &e,
        );
        assert!(
            hit.chunks.iter().any(|c| c.path == "needle.txt"),
            "the window containing the query's words must be selected: {:?}",
            hit.chunks.iter().map(|c| &c.path).collect::<Vec<_>>()
        );
        assert!(hit.tokens <= 3000);

        // No shared term anywhere: the lexical leg yields nothing, and the
        // fallback must still produce a bounded, non-empty pack rather than an
        // id-ordered prefix of the corpus.
        let miss = pack(
            dir.path(),
            "qqqq zzzz",
            3000,
            50,
            0.0,
            PackMode::Lexical,
            false,
            &e,
        );
        assert!(miss.tokens <= 3000);
        assert!(!miss.chunks.is_empty(), "the fallback must produce context");
    }

    /// `Auto` must route on the query's shape, not on a coin toss.
    #[test]
    fn auto_routes_identifier_queries_to_the_lexical_leg() {
        for identifier in [
            "choose_worker",
            "count_tokens",
            "src/pack.rs",
            "MAX_FILE_BYTES",
            "physis_core::rag",
        ] {
            assert!(
                query_carries_a_code_identifier(identifier),
                "`{identifier}` should read as a code identifier"
            );
        }
        for prose in [
            "how does the engine decide that a change has gone stale",
            "which machine turns waypoints into places",
            "what must the engine never do to conflicting information",
            // PascalCase is prose to this detector — see the doc comment.
            "Bm25Index",
            "CellClassifier",
        ] {
            assert!(
                !query_carries_a_code_identifier(prose),
                "`{prose}` should read as prose"
            );
        }
    }

    /// End to end: an identifier query in `Auto` behaves like `Lexical`, and a
    /// prose query behaves like `Cosine`.
    #[test]
    fn auto_matches_the_leg_it_resolves_to() {
        let dir = tempfile::tempdir().unwrap();
        let needle: String = (1..=40)
            .map(|i| format!("zirconium impeller calibration step {i}\n"))
            .collect();
        write(dir.path(), "needle.txt", &needle);
        for f in 0..4 {
            let body: String = (1..=120)
                .map(|i| format!("unrelated prose {f} line {i}\n"))
                .collect();
            write(dir.path(), &format!("other{f}.txt"), &body);
        }

        let e = embedder();
        let run =
            |mode: PackMode, query: &str| pack(dir.path(), query, 600, 50, 0.0, mode, false, &e);
        let same_paths = |a: &PackResult, b: &PackResult| {
            let key = |r: &PackResult| {
                r.chunks
                    .iter()
                    .map(|c| (c.path.clone(), c.start_line))
                    .collect::<Vec<_>>()
            };
            key(a) == key(b)
        };

        let identifier = "zirconium_impeller";
        assert!(
            same_paths(
                &run(PackMode::Auto, identifier),
                &run(PackMode::Lexical, identifier)
            ),
            "an identifier query must resolve to the lexical leg"
        );

        let prose = "how does the calibration procedure work";
        assert!(
            same_paths(&run(PackMode::Auto, prose), &run(PackMode::Cosine, prose)),
            "a prose query must resolve to the cosine leg"
        );
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
            let result = pack(
                dir.path(),
                "content",
                budget,
                50,
                0.0,
                PackMode::Cosine,
                false,
                &embedder(),
            );
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

        let result = pack(
            dir.path(),
            "alpha",
            2000,
            10,
            0.0,
            PackMode::Cosine,
            false,
            &embedder(),
        );
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
            write(
                dir.path(),
                &format!("f{i}.txt"),
                &format!("subject {i}\n").repeat(50),
            );
        }

        // A budget far below the corpus must drop most candidates.
        let result = pack(
            dir.path(),
            "subject",
            40,
            50,
            0.0,
            PackMode::Cosine,
            false,
            &embedder(),
        );
        assert!(result.candidates > result.chunks.len());
        assert_eq!(result.dropped, result.candidates - result.chunks.len());
    }

    #[test]
    fn empty_corpus_returns_an_empty_pack_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = pack(
            dir.path(),
            "anything",
            1000,
            10,
            0.0,
            PackMode::Cosine,
            false,
            &embedder(),
        );
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

        let result = pack(
            dir.path(),
            "junk",
            4000,
            50,
            0.0,
            PackMode::Cosine,
            false,
            &embedder(),
        );
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
