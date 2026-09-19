//! Token-fixed RAG — a re-export of [`physis_core::rag`].
//!
//! This module used to be a 411-line fork of the Core module. It drifted: Core
//! gained the G5 hybrid work (`Bm25Index`, `rank_by_cosine`, `fuse_rrf`,
//! `rank_hybrid`, `HybridVerdict`, `measure_hybrid_vs_cosine`, and the
//! `bm25` field on `RagCorpus`) and the fork did not. `src/pack.rs` imports
//! this module, so the packer was the one retrieval path built on a copy that
//! could not see the fusion the rest of the codebase already used — measured
//! on 2026-09-16 as trailing a plain BM25 baseline at equal token budget
//! (`docs/plans/2026-09-16-day3-equal-budget-retrieval.md`).
//!
//! The fork is gone. There is one token-fixed retriever in the workspace, it
//! lives in Core, and this path re-exports it so every existing
//! `crate::rag::…` / `physis_pro::rag::…` caller keeps working unchanged.
//! `src/ported/studio.rs` already imported `physis_core::rag` directly and was
//! the correct pattern all along.

pub use physis_core::rag::*;

/// Build a token-fixed corpus from texts with a **product-side** embedder.
///
/// The two `VectorEmbed` traits are not interchangeable: Core's is the minimal
/// interface (`embed` / `embed_batch` / `dimension`) and the product's adds
/// `embed_image`. `RagCorpus::build` takes Core's, so a product embedder cannot
/// be handed to it. This builds the identical corpus — same chunk ids, same
/// token counter, same BM25 leg — from the product trait, so product callers
/// do not have to spell the struct out (and cannot forget the lexical leg).
pub fn corpus_from_texts(texts: &[String], embedder: &dyn crate::embed::VectorEmbed) -> RagCorpus {
    RagCorpus {
        chunks: texts
            .iter()
            .enumerate()
            .map(|(i, t)| RagChunk {
                id: i,
                text: t.clone(),
                tokens: count_tokens(t),
                embedding: embedder.embed(t),
            })
            .collect(),
        bm25: Bm25Index::build(texts),
    }
}
