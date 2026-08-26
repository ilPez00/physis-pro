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
    let (budget, total_tokens, truncated, results) =
        retrieve_nodes(&state, &uid, req.query.clone(), req.token_budget, req.top_k).await;
    Json(SearchNodesResponse {
        query: req.query,
        budget,
        total_tokens,
        truncated,
        results,
    })
}

/// The retrieval half shared by [`search_nodes_handler`] and
/// [`ask_handler`]: embed with the same embedder that embedded every node
/// (embedder-consistency constraint), rank tenant coherence + goal labels by
/// cosine, pack to the token budget.
pub(crate) async fn retrieve_nodes(
    state: &SharedState,
    uid: &str,
    query: String,
    token_budget: usize,
    top_k: usize,
) -> (usize, usize, bool, Vec<SearchNodeHit>) {
    let shared = state.shared.clone();
    let q_emb = embed_text(shared, query.clone()).await;

    // Collect (kind, id, label, embedding) from the tenant's store. Unlabeled
    // goals (pure vectors) are skipped — nothing browsable to return.
    let candidates: Vec<(String, String, String, Vec<f32>)> = state.with_user(uid, |s| {
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
    let retriever = rag::TokenFixedRetriever::new(token_budget, top_k);
    let result = retriever.retrieve(&q_emb, &corpus);

    let hits: Vec<SearchNodeHit> = result
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

    (result.budget, result.total_tokens, result.truncated, hits)
}

/// POST /api/v1/ask — plain-English question over the graph (PLAN 3.2).
///
/// Retrieval first (always works, no provider needed), synthesis second: when
/// a semantic provider is configured the retrieved entries are handed to it as
/// grounded context and the model must cite them; without one the endpoint
/// degrades to the raw ranked matches instead of failing — an answer that
/// needs an API key to appear would mostly not appear.
pub async fn ask_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<AskRequest>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    let (budget, total_tokens, truncated, hits) = retrieve_nodes(
        &state,
        &uid,
        req.question.clone(),
        req.token_budget,
        req.top_k,
    )
    .await;

    let semantic = state.shared.semantic;
    let mut answer = None;
    let mut provider = None;
    let mut note = match () {
        _ if !semantic => Some("no semantic provider configured — showing raw matches".to_string()),
        _ if hits.is_empty() => Some("nothing in the graph matches yet".to_string()),
        _ => None,
    };

    if note.is_none() {
        let cascade = state.shared.cascade.clone();
        let messages = vec![
            crate::ai::provider::Message {
                role: "system".into(),
                content: crate::ai::provider::Content::Text(ask_prompt(&req.question, &hits)),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            crate::ai::provider::Message {
                role: "user".into(),
                content: crate::ai::provider::Content::Text(req.question.clone()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ];
        let hint = crate::ai::provider::RouteHint {
            task_type: "smart".into(),
            has_image: false,
        };
        match cascade
            .complete(&messages, None, &hint, Some(ASK_MAX_TOKENS))
            .await
        {
            Ok((msg, prov)) => {
                answer = Some(msg.content.unwrap_or_default());
                provider = Some(prov);
            }
            Err(e) => {
                log::warn!("[ask] synthesis failed, falling back to matches: {e}");
                note = Some("answer generation failed — showing raw matches".to_string());
            }
        }
    }

    Json(serde_json::json!({
        "question": req.question,
        "semantic": semantic,
        "answer": answer,
        "provider": provider,
        "note": note,
        "budget": budget,
        "total_tokens": total_tokens,
        "truncated": truncated,
        "results": hits,
    }))
}

/// Ceiling for synthesized answers — this is a grounded digest, not an essay;
/// the entries themselves stay in the panel below the answer.
const ASK_MAX_TOKENS: u32 = 300;

/// Build the grounding system prompt for [`ask_handler`]. Pure so the citation
/// contract is unit-testable without a live provider.
fn ask_prompt(question: &str, hits: &[SearchNodeHit]) -> String {
    let mut p = String::from(
        "Answer the question using ONLY the numbered graph entries below. \
         Cite entries as [n] where n is the entry number. If the entries do \
         not contain the answer, say exactly: \
         \"The graph does not cover that yet.\"\n\n",
    );
    p.push_str("QUESTION: ");
    p.push_str(question);
    p.push_str("\n\nGRAPH ENTRIES:\n");
    for (i, h) in hits.iter().enumerate() {
        p.push_str(&format!("[{}] ({}) {}\n", i + 1, h.kind, h.label));
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_numbers_entries_and_demands_citations() {
        let hits = vec![
            SearchNodeHit {
                kind: "node".into(),
                id: "n1".into(),
                label: "hydraulic press tonnage drift".into(),
                score: 0.9,
                tokens: 4,
            },
            SearchNodeHit {
                kind: "goal".into(),
                id: "g1".into(),
                label: "cut scrap rate".into(),
                score: 0.7,
                tokens: 3,
            },
        ];
        let p = ask_prompt("what do I have on presses?", &hits);
        assert!(
            p.contains("[1] (node) hydraulic press tonnage drift"),
            "{p}"
        );
        assert!(p.contains("[2] (goal) cut scrap rate"), "{p}");
        assert!(p.contains("Cite entries as [n]"));
        assert!(p.contains("what do I have on presses?"));
    }

    #[test]
    fn empty_hits_still_yield_a_well_formed_prompt() {
        let p = ask_prompt("anything?", &[]);
        assert!(p.contains("GRAPH ENTRIES:"));
        assert!(!p.contains("[1]"));
    }
}
