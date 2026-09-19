//! Token-fixed RAG — retrieve chunks subject to a hard token budget.
//!
//! Unlike unbounded top-k retrieval, the context handed to the model is
//! *guaranteed* to fit a fixed token window: chunks are ranked by cosine to the
//! query, then greedily packed until the running token total would exceed
//! `token_budget` (or `top_k` is reached). This makes prompt assembly
//! deterministic and overflow-proof regardless of corpus size — the "fixed"
//! in token-fixed RAG.

use crate::embed::VectorEmbed;
use crate::models::cosine_sim;
use serde::{Deserialize, Serialize};

/// A token counter. The default heuristic needs no model files; a real BPE
/// tokenizer can be supplied by implementing this trait.
pub trait TokenCounter {
    fn count(&self, text: &str) -> usize;
}

/// Dependency-free subword-ish estimate: whitespace-split words, with long
/// words spilling into extra tokens, plus one token per punctuation/separator.
/// Mirrors how BPE over-tokenizes long words well enough for budgeting.
pub struct HeuristicTokenizer;

impl TokenCounter for HeuristicTokenizer {
    fn count(&self, text: &str) -> usize {
        count_tokens(text)
    }
}

/// Estimate token count of `text` with the heuristic counter.
pub fn count_tokens(text: &str) -> usize {
    let mut n = 0usize;
    let mut word = String::new();
    for c in text.chars() {
        if c.is_whitespace() {
            if !word.is_empty() {
                n += tokenize_word(&word);
                word.clear();
            }
        } else if c.is_alphanumeric() {
            word.push(c);
        } else {
            if !word.is_empty() {
                n += tokenize_word(&word);
                word.clear();
            }
            n += 1;
        }
    }
    if !word.is_empty() {
        n += tokenize_word(&word);
    }
    n.max(1)
}

fn tokenize_word(w: &str) -> usize {
    let len = w.chars().count();
    1 + (len.saturating_sub(6)) / 4
}

/// One retrievable chunk of text with its precomputed token count + embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagChunk {
    pub id: usize,
    pub text: String,
    pub tokens: usize,
    pub embedding: Vec<f32>,
}

/// A corpus of chunks, ready for retrieval.
#[derive(Debug, Clone, Default)]
pub struct RagCorpus {
    pub chunks: Vec<RagChunk>,
}

impl RagCorpus {
    /// Build a corpus from raw texts, embedding each and counting its tokens.
    ///
    /// Embedding is submitted in fixed-size batches rather than one call per
    /// corpus. A single ONNX call over a thousand 512-token sequences allocates
    /// activations for all of them at once and is killed by the OOM reaper on
    /// an ordinary machine; batching bounds peak memory independently of corpus
    /// size, at no cost in throughput.
    pub fn build(texts: &[String], embedder: &dyn VectorEmbed) -> Self {
        /// Sequences per embedder call. Large enough to keep the model busy,
        /// small enough that peak activation memory stays flat.
        const EMBED_BATCH: usize = 32;

        let tokens: Vec<usize> = texts.iter().map(|t| count_tokens(t)).collect();
        let mut embeddings: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
        for batch in texts.chunks(EMBED_BATCH) {
            let refs: Vec<&str> = batch.iter().map(|s| s.as_str()).collect();
            embeddings.extend(embedder.embed_batch(&refs));
        }
        let chunks = texts
            .iter()
            .enumerate()
            .map(|(i, t)| RagChunk {
                id: i,
                text: t.clone(),
                tokens: tokens[i],
                embedding: embeddings.get(i).cloned().unwrap_or_default(),
            })
            .collect();
        Self { chunks }
    }

    /// Total token footprint of the whole corpus.
    pub fn total_tokens(&self) -> usize {
        self.chunks.iter().map(|c| c.tokens).sum()
    }
}

/// One chunk selected for the final context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedChunk {
    pub id: usize,
    pub text: String,
    pub score: f32,
    pub tokens: usize,
}

/// The result of a token-fixed retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalResult {
    /// Chunks actually selected, in score-descending order.
    pub chunks: Vec<RetrievedChunk>,
    /// Sum of `tokens` over `chunks` — guaranteed <= `budget`.
    pub total_tokens: usize,
    /// The budget this retrieval was constrained to.
    pub budget: usize,
    /// True when not every candidate fit (context was truncated to fit budget).
    pub truncated: bool,
}

/// A token-fixed retriever: packs the highest-scoring chunks that fit.
pub struct TokenFixedRetriever {
    pub budget: usize,
    pub top_k: usize,
    /// MMR-like diversity (0 = pure score, 1 = max diversity). When > 0,
    /// each candidate's effective score is discounted by its cosine to already
    /// selected chunks, reducing redundancy in the packed context.
    pub diversity: f32,
}

impl TokenFixedRetriever {
    pub fn new(budget: usize, top_k: usize) -> Self {
        Self {
            budget,
            top_k,
            diversity: 0.0,
        }
    }

    pub fn with_diversity(mut self, diversity: f32) -> Self {
        self.diversity = diversity.clamp(0.0, 1.0);
        self
    }

    /// Retrieve the chunks whose packed token total stays within `budget`.
    ///
    /// Ordering: candidates are scored by cosine to `query_emb` (with optional
    /// MMR discount). We then greedily take chunks in that order, skipping any
    /// whose `tokens` would push the running total over `budget`. Because we
    /// skip-and-continue, a slightly lower-scoring smaller chunk can still fit
    /// after a larger one is rejected — maximizing what fits within the budget.
    pub fn retrieve(&self, query_emb: &[f32], corpus: &RagCorpus) -> RetrievalResult {
        if corpus.chunks.is_empty() || self.budget == 0 {
            return RetrievalResult {
                chunks: Vec::new(),
                total_tokens: 0,
                budget: self.budget,
                truncated: false,
            };
        }

        // Base scores, then MMR discount if diversity requested.
        let mut scored: Vec<(usize, f32)> = corpus
            .chunks
            .iter()
            .map(|c| (c.id, cosine_sim(query_emb, &c.embedding)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Greedy MMR: each round takes the best remaining chunk by effective
        // score, then discounts what is left against what was just taken.
        //
        // The first version scanned, for each candidate, every higher-scoring
        // chunk and skipped the candidate if any of them beat it once
        // discounted — but it never excluded the chunks it had already
        // selected. A selected chunk's self-similarity is 1.0, so it kept its
        // full `base * (1 - diversity)` and permanently out-scored nearly
        // everything below it: with the CLI default `--diversity 0.3`, a pack
        // of physis-core/src returned 2 chunks and 4 of 3000 budgeted tokens,
        // silently handing an agent an empty context instead of a packed one.
        // Removing a chunk from `remaining` when it is selected is what makes
        // the comparison mean "better than what is still on the table".
        //
        // `remaining` carries (chunk id, base score, max cosine to anything
        // already selected). The discount is updated incrementally against the
        // one new selection per round, so this is O(top_k * n) cosines rather
        // than the O(n^2) per candidate the first version paid.
        let mut remaining: Vec<(usize, f32, f32)> = scored
            .into_iter()
            .map(|(id, base)| (id, base, 0.0))
            .collect();
        let mut selected: Vec<RetrievedChunk> = Vec::new();
        let mut used = 0usize;

        while selected.len() < self.top_k {
            let mut best: Option<usize> = None;
            let mut best_eff = f32::NEG_INFINITY;
            for (pos, &(cid, base, discount)) in remaining.iter().enumerate() {
                // A chunk that cannot fit the remaining budget is not a
                // candidate this round; it stays in `remaining`, so it still
                // counts as truncation below.
                if used + corpus.chunks[cid].tokens > self.budget {
                    continue;
                }
                let eff = base * (1.0 - self.diversity * discount);
                if eff > best_eff {
                    best_eff = eff;
                    best = Some(pos);
                }
            }
            let Some(pos) = best else { break };

            let (cid, base, _) = remaining.remove(pos);
            let chunk = &corpus.chunks[cid];
            used += chunk.tokens;
            selected.push(RetrievedChunk {
                id: chunk.id,
                text: chunk.text.clone(),
                score: base,
                tokens: chunk.tokens,
            });
            if self.diversity > 0.0 {
                for entry in remaining.iter_mut() {
                    let c = cosine_sim(&corpus.chunks[entry.0].embedding, &chunk.embedding);
                    if c > entry.2 {
                        entry.2 = c;
                    }
                }
            }
        }

        // Anything still on the table did not make it — because top_k was
        // reached, or because nothing left fits the budget.
        let truncated = !remaining.is_empty();

        RetrievalResult {
            chunks: selected,
            total_tokens: used,
            budget: self.budget,
            truncated,
        }
    }
}

/// Assemble the retrieved chunks into a single context string.
/// The returned text's token estimate is <= `result.total_tokens`.
pub fn assemble_context(result: &RetrievalResult, separator: &str) -> String {
    result
        .chunks
        .iter()
        .map(|c| c.text.as_str())
        .collect::<Vec<_>>()
        .join(separator)
}

/// Convenience: build a corpus from texts, embed the query, retrieve
/// token-fixed, and return both the result and the assembled context string.
pub fn retrieve_from_texts(
    texts: &[String],
    query: &str,
    embedder: &dyn VectorEmbed,
    budget: usize,
    top_k: usize,
) -> (RetrievalResult, String) {
    let corpus = RagCorpus::build(texts, embedder);
    let q_emb = embedder.embed(query);
    let retriever = TokenFixedRetriever::new(budget, top_k);
    let result = retriever.retrieve(&q_emb, &corpus);
    let ctx = assemble_context(&result, "\n\n---\n\n");
    (result, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::RandomProjectionEmbedder;

    fn emb() -> RandomProjectionEmbedder {
        RandomProjectionEmbedder::new(64)
    }

    /// Regression: with `diversity > 0` the retriever selected two chunks and
    /// stopped, whatever the budget. The old MMR filter compared each candidate
    /// against the chunks it had already selected, whose self-similarity is 1.0,
    /// so the first pick out-scored everything below it forever. The CLI default
    /// is `--diversity 0.3`, so every packed context shipped that way: 4 of 3000
    /// budgeted tokens on physis-core/src.
    #[test]
    fn diversity_does_not_starve_the_budget() {
        // Long enough, and enough of them, that the budget is what binds —
        // not top_k, which would hide a starved pack behind its own cap.
        let texts: Vec<String> = (0..200)
            .map(|i| {
                format!(
                    "chunk {i} about turbine rotor bearing vibration, maintenance \
                     scheduling, lubrication intervals and the shift report that \
                     records them for line {i}"
                )
            })
            .collect();
        let e = emb();
        let corpus = RagCorpus::build(&texts, &e);
        let q = e.embed("turbine bearing vibration");

        let plain = TokenFixedRetriever::new(3000, 500).retrieve(&q, &corpus);
        let diverse = TokenFixedRetriever::new(3000, 500)
            .with_diversity(0.3)
            .retrieve(&q, &corpus);

        // Diversity re-orders what is picked; it must not decide how much is.
        assert_eq!(
            diverse.chunks.len(),
            plain.chunks.len(),
            "diversity 0.3 packed {} chunks where plain relevance packed {}",
            diverse.chunks.len(),
            plain.chunks.len()
        );
        assert!(
            diverse.total_tokens > 3000 / 2,
            "packed only {} of a 3000-token budget",
            diverse.total_tokens
        );
        assert!(diverse.total_tokens <= 3000);
    }

    #[test]
    fn token_count_is_monotonic_and_nonzero() {
        assert_eq!(count_tokens(""), 1);
        assert!(count_tokens("hello world") >= count_tokens("hello"));
        // Long word should cost more than a short word.
        assert!(count_tokens("antidisestablishmentarianism") > count_tokens("cat"));
    }

    #[test]
    fn budget_is_respected() {
        let texts: Vec<String> = (0..20)
            .map(|i| format!("chunk number {} with some filler text to have length", i))
            .collect();
        let e = emb();
        let corpus = RagCorpus::build(&texts, &e);
        let q = e.embed("query about chunk number five");
        let r = TokenFixedRetriever::new(40, 100).retrieve(&q, &corpus);
        assert!(
            r.total_tokens <= 40,
            "total {} must be <= budget 40",
            r.total_tokens
        );
    }

    #[test]
    fn top_k_caps_selection() {
        let texts: Vec<String> = (0..20).map(|i| format!("text {i}")).collect();
        let e = emb();
        let corpus = RagCorpus::build(&texts, &e);
        let q = e.embed("text");
        let r = TokenFixedRetriever::new(10_000, 3).retrieve(&q, &corpus);
        assert!(r.chunks.len() <= 3);
    }

    #[test]
    fn selection_sorted_by_score() {
        let texts = vec![
            "apple fruit red".to_string(),
            "banana fruit yellow".to_string(),
            "turbine rotor speed".to_string(),
        ];
        let e = emb();
        let corpus = RagCorpus::build(&texts, &e);
        let q = e.embed("fruit banana");
        let r = TokenFixedRetriever::new(10_000, 10).retrieve(&q, &corpus);
        // First selected chunk should score >= the rest.
        for w in &r.chunks[1..] {
            assert!(r.chunks[0].score >= w.score);
        }
    }

    #[test]
    fn deterministic_across_runs() {
        let texts: Vec<String> = (0..15).map(|i| format!("doc {} content here", i)).collect();
        let e = emb();
        let corpus = RagCorpus::build(&texts, &e);
        let q = e.embed("doc");
        let r1 = TokenFixedRetriever::new(80, 50).retrieve(&q, &corpus);
        let r2 = TokenFixedRetriever::new(80, 50).retrieve(&q, &corpus);
        assert_eq!(r1.chunks.len(), r2.chunks.len());
        assert_eq!(r1.total_tokens, r2.total_tokens);
    }

    #[test]
    fn assemble_never_exceeds_token_estimate() {
        let texts = vec!["alpha beta gamma".to_string(), "delta epsilon".to_string()];
        let e = emb();
        let (r, ctx) = retrieve_from_texts(&texts, "alpha", &e, 30, 10);
        assert!(r.total_tokens <= 30);
        assert!(ctx.contains("alpha"));
    }
}
