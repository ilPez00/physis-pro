use super::*;
use crate::web::{sandbox_existing, SharedState};
use axum::{
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
};

/// POST /api/v1/rag/retrieve — scan a sandboxed dir, embed each doc, and return
/// the chunks whose packed token total stays within `tokens`. Deterministic and
/// overflow-proof.
pub async fn rag_retrieve_handler(
    State(state): State<SharedState>,
    Json(req): Json<RagRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let canonical = sandbox_existing(&state, &req.dir)?;
    let shared = state.shared.clone();
    let query = req.query.clone();
    let budget = req.tokens;
    let top_k = req.top_k;
    let (result, context) =
        tokio::task::spawn_blocking(move || -> anyhow::Result<(rag::RetrievalResult, String)> {
            let docs = crate::ingest::ingest_all(&[canonical]);
            if docs.is_empty() {
                anyhow::bail!("no documents found under scan dir");
            }
            let texts: Vec<String> = docs.iter().map(|d| d.body.clone()).collect();
            let corpus = RagCorpus::build(&texts, shared.embedder.as_ref());
            let q_emb = shared.embedder.embed(&query);
            let retriever = rag::TokenFixedRetriever::new(budget, top_k);
            let result = retriever.retrieve(&q_emb, &corpus);
            let context = rag::assemble_context(&result, "\n\n---\n\n");
            Ok((result, context))
        })
        .await
        .map_err(|e| {
            err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("rag failed: {e}"),
            )
        })?
        .map_err(|e| {
            err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("rag failed: {e}"),
            )
        })?;

    Ok(Json(serde_json::json!({
        "total_tokens": result.total_tokens,
        "budget": result.budget,
        "truncated": result.truncated,
        "chunks": result.chunks,
        "context": context,
    })))
}

/// POST /api/v1/search/nodes — budget-bounded similarity search over coherence +
/// goal nodes. The query is embedded once; node/goal labels are ranked by cosine
/// to it, then greedily packed until the running token total would exceed the
/// budget (or `top_k` is reached). The response is therefore token-fixed.
pub async fn search_nodes_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<SearchNodesRequest>,
) -> Json<SearchNodesResponse> {
    let uid = user_key(&headers);
    let query = req.query.clone();
    let shared = state.shared.clone();
    let q_emb = embed_text(shared, query.clone()).await;

    // Collect (kind, id, label, embedding) from the tenant's store. Unlabeled
    // goals (pure vectors) are skipped — nothing browsable to return.
    let candidates: Vec<(String, String, String, Vec<f32>)> = state.with_user(&uid, |s| {
        let mut out: Vec<(String, String, String, Vec<f32>)> = Vec::new();
        for n in s.core.nodes.values() {
            if let Some(label) = n.label.as_deref() {
                if !label.trim().is_empty() {
                    out.push((
                        "node".to_string(),
                        n.id.clone(),
                        label.to_string(),
                        n.embedding.clone(),
                    ));
                }
            }
        }
        for g in &s.goals {
            let label = match &g.label {
                Some(l) if !l.trim().is_empty() => l.clone(),
                _ => continue,
            };
            out.push(("goal".to_string(), g.id.clone(), label, g.embedding.clone()));
        }
        out
    });

    // Reuse the token-fixed retriever over a corpus built from node labels
    // (embeddings already computed — no re-embed). Chunk id = candidate index.
    let corpus = rag::RagCorpus {
        chunks: candidates
            .iter()
            .enumerate()
            .map(|(i, (_, _, label, emb))| rag::RagChunk {
                id: i,
                text: label.clone(),
                tokens: rag::count_tokens(label),
                embedding: emb.clone(),
            })
            .collect(),
    };
    let retriever = rag::TokenFixedRetriever::new(req.token_budget, req.top_k);
    let result = retriever.retrieve(&q_emb, &corpus);

    let results: Vec<SearchNodeHit> = result
        .chunks
        .iter()
        .filter_map(|c| {
            let (kind, id, label, _) = candidates.get(c.id)?;
            Some(SearchNodeHit {
                kind: kind.clone(),
                id: id.clone(),
                label: label.clone(),
                score: c.score,
                tokens: c.tokens,
            })
        })
        .collect();

    Json(SearchNodesResponse {
        query,
        budget: result.budget,
        total_tokens: result.total_tokens,
        truncated: result.truncated,
        results,
    })
}
