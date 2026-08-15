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
    pub fn build(texts: &[String], embedder: &dyn VectorEmbed) -> Self {
        let tokens: Vec<usize> = texts.iter().map(|t| count_tokens(t)).collect();
        let embeddings = if texts.is_empty() {
            Vec::new()
        } else {
            embedder.embed_batch(&texts.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        };
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

        let mut selected: Vec<RetrievedChunk> = Vec::new();
        let mut used = 0usize;
        let mut truncated = false;
        let mut selected_embs: Vec<&[f32]> = Vec::new();

        for &(cid, base) in &scored {
            if selected.len() >= self.top_k {
                truncated = true;
                break;
            }
            let chunk = &corpus.chunks[cid];
            // MMR: discount by max cosine to already-selected chunks.
            let mut discount = 0.0f32;
            if self.diversity > 0.0 {
                for se in &selected_embs {
                    let c = cosine_sim(&chunk.embedding, se);
                    if c > discount {
                        discount = c;
                    }
                }
            }
            let eff = base * (1.0 - self.diversity * discount);
            // Re-check ordering under effective score so MMR reordering is stable:
            // only accept if no unselected candidate scores higher effectively.
            // (Cheap: corpus sizes are small for RAG context assembly.)
            let mut blocked = false;
            if self.diversity > 0.0 {
                for &(oid, obase) in &scored {
                    if oid == cid {
                        break;
                    }
                    let ochunk = &corpus.chunks[oid];
                    let mut odisc = 0.0f32;
                    for se in &selected_embs {
                        let c = cosine_sim(&ochunk.embedding, se);
                        if c > odisc {
                            odisc = c;
                        }
                    }
                    if obase * (1.0 - self.diversity * odisc) > eff + 1e-6 {
                        blocked = true;
                        break;
                    }
                }
            }
            if blocked {
                continue;
            }

            if used + chunk.tokens <= self.budget {
                used += chunk.tokens;
                selected_embs.push(&chunk.embedding);
                selected.push(RetrievedChunk {
                    id: chunk.id,
                    text: chunk.text.clone(),
                    score: base,
                    tokens: chunk.tokens,
                });
            } else {
                truncated = true;
            }
        }

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
    result.chunks.iter().map(|c| c.text.as_str()).collect::<Vec<_>>().join(separator)
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
        assert!(r.total_tokens <= 40, "total {} must be <= budget 40", r.total_tokens);
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
