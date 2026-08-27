use super::*;
use crate::web::{sandbox_existing, SharedState};
use axum::{
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
};

/// POST /api/v1/rag/retrieve — scan a sandboxed dir, embed each doc, and return
/// the chunks whose packed token total stays within `tokens`. Deterministic and
/// overflow-proof.
///
/// `unit: "section"` (Phase 11 C4) swaps the retrieval unit from whole document
/// to heading-scoped section: each document is parsed into its own heading tree
/// and every section is embedded as `breadcrumb + body`, so the vector carries
/// the context the author implied but never repeated, and each hit reports the
/// file and section path it came from. The default flat path is unchanged —
/// same corpus, same ids, same vectors — so existing callers keep their
/// behaviour.
pub async fn rag_retrieve_handler(
    State(state): State<SharedState>,
    Json(req): Json<RagRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let canonical = sandbox_existing(&state, &req.dir)?;
    let shared = state.shared.clone();
    let query = req.query.clone();
    let budget = req.tokens;
    let top_k = req.top_k;
    let by_section = req.unit == "section";
    let (result, context, sources) = tokio::task::spawn_blocking(
        move || -> anyhow::Result<(rag::RetrievalResult, String, Vec<serde_json::Value>)> {
            let docs = crate::ingest::ingest_all(&[canonical]);
            if docs.is_empty() {
                anyhow::bail!("no documents found under scan dir");
            }

            // Provenance runs parallel to `texts`: chunk id is the index into
            // both, so a hit can name its file (and section) without a second
            // pass over the corpus.
            let (texts, sources): (Vec<String>, Vec<serde_json::Value>) = if by_section {
                let mut texts = Vec::new();
                let mut sources = Vec::new();
                for d in &docs {
                    for sec in crate::doctree::parse(&d.title, &d.body) {
                        texts.push(sec.retrieval_text());
                        sources.push(serde_json::json!({
                            "path": d.path.display().to_string(),
                            "section": sec.path(),
                            "level": sec.level,
                        }));
                    }
                }
                (texts, sources)
            } else {
                (
                    docs.iter().map(|d| d.body.clone()).collect(),
                    docs.iter()
                        .map(|d| serde_json::json!({ "path": d.path.display().to_string() }))
                        .collect(),
                )
            };

            let corpus = RagCorpus::build(&texts, shared.embedder.as_ref());
            let q_emb = shared.embedder.embed(&query);
            let retriever = rag::TokenFixedRetriever::new(budget, top_k);
            let result = retriever.retrieve(&q_emb, &corpus);
            let context = rag::assemble_context(&result, "\n\n---\n\n");

            // Only the selected chunks' provenance is returned, in hit order.
            let selected: Vec<serde_json::Value> = result
                .chunks
                .iter()
                .filter_map(|c| sources.get(c.id).cloned())
                .collect();
            Ok((result, context, selected))
        },
    )
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
        "unit": if by_section { "section" } else { "chunk" },
        "total_tokens": result.total_tokens,
        "budget": result.budget,
        "truncated": result.truncated,
        "chunks": result.chunks,
        "sources": sources,
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
    let (budget, total_tokens, truncated, results, failures) =
        retrieve_nodes(&state, &uid, req.query.clone(), req.token_budget, req.top_k).await;
    Json(SearchNodesResponse {
        query: req.query,
        budget,
        total_tokens,
        truncated,
        results,
        failures,
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
) -> (usize, usize, bool, Vec<SearchNodeHit>, Vec<RecalledFailure>) {
    let shared = state.shared.clone();
    let q_emb = embed_text(shared, query.clone()).await;

    // Collect (kind, id, label, embedding, verdict) from the tenant's store.
    // The asserted verdict rides along so recall becomes outcome memory: a top
    // hit can say "you already tried this and it failed." Unlabeled goals
    // (pure vectors) are skipped — nothing browsable to return.
    let candidates: Vec<(String, String, String, Vec<f32>, Option<f32>)> = state.with_user(uid, |s| {
        let mut out: Vec<(String, String, String, Vec<f32>, Option<f32>)> = Vec::new();
        for n in s.core.nodes.values() {
            if let Some(label) = n.label.as_deref() {
                if !label.trim().is_empty() {
                    out.push((
                        "node".to_string(),
                        n.id.clone(),
                        label.to_string(),
                        n.embedding.clone(),
                        n.asserted,
                    ));
                }
            }
        }
        for g in &s.goals {
            let label = match &g.label {
                Some(l) if !l.trim().is_empty() => l.clone(),
                _ => continue,
            };
            out.push((
                "goal".to_string(),
                g.id.clone(),
                label,
                g.embedding.clone(),
                None,
            ));
        }
        out
    });

    // Reuse the token-fixed retriever over a corpus built from node labels
    // (embeddings already computed — no re-embed). Chunk id = candidate index.
    let corpus = rag::RagCorpus {
        chunks: candidates
            .iter()
            .enumerate()
            .map(|(i, (_, _, label, emb, _))| rag::RagChunk {
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
            let (kind, id, label, _, verdict) = candidates.get(c.id)?;
            Some(SearchNodeHit {
                kind: kind.clone(),
                id: id.clone(),
                label: label.clone(),
                score: c.score,
                tokens: c.tokens,
                verdict: *verdict,
            })
        })
        .collect();

    // Recorded failures among the recalled set — the "already failed here" flag.
    let failures: Vec<RecalledFailure> = hits
        .iter()
        .filter(|h| h.verdict == Some(-1.0))
        .map(|h| RecalledFailure {
            id: h.id.clone(),
            label: h.label.clone(),
            score: h.score,
        })
        .collect();

    (result.budget, result.total_tokens, result.truncated, hits, failures)
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
    let (budget, total_tokens, truncated, hits, failures) = retrieve_nodes(
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
        // Surface any recorded failures to the synthesizer so it can warn the
        // user away from an approach the graph already marks as failed.
        let warn_failures = !failures.is_empty();
        let messages = vec![
            crate::ai::provider::Message {
                role: "system".into(),
                content: crate::ai::provider::Content::Text(ask_prompt(
                    &req.question,
                    &hits,
                    warn_failures,
                )),
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
        "failures": failures,
    }))
}

/// Ceiling for synthesized answers — this is a grounded digest, not an essay;
/// the entries themselves stay in the panel below the answer.
const ASK_MAX_TOKENS: u32 = 300;

/// Build the grounding system prompt for [`ask_handler`]. Pure so the citation
/// contract is unit-testable without a live provider. When `warn_failures` is
/// set, the prompt instructs the synthesizer to flag any entry the graph marks
/// as a recorded failure (−1.0) so the agent doesn't recommend a known-bad path.
fn ask_prompt(question: &str, hits: &[SearchNodeHit], warn_failures: bool) -> String {
    let mut p = String::from(
        "Answer the question using ONLY the numbered graph entries below. \
         Cite entries as [n] where n is the entry number. If the entries do \
         not contain the answer, say exactly: \
         \"The graph does not cover that yet.\"\n\n",
    );
    if warn_failures {
        p.push_str(
            "WARNING: some entries below are flagged as RECORDED FAILURES \
             (the graph already shows they did not work). Do not recommend a \
             failed entry as a solution unless you explicitly warn that it has \
             failed before. Mark failed entries as [n]* in your answer.\n\n",
        );
    }
    p.push_str("QUESTION: ");
    p.push_str(question);
    p.push_str("\n\nGRAPH ENTRIES:\n");
    for (i, h) in hits.iter().enumerate() {
        let verdict = match h.verdict {
            Some(v) if v <= -0.5 => " [FAILED]",
            Some(v) if v >= 0.5 => " [WORKED]",
            Some(_) => " [INERT]",
            None => "",
        };
        p.push_str(&format!("[{}] ({}) {}{}\n", i + 1, h.kind, h.label, verdict));
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
                verdict: None,
            },
            SearchNodeHit {
                kind: "goal".into(),
                id: "g1".into(),
                label: "cut scrap rate".into(),
                score: 0.7,
                tokens: 3,
                verdict: None,
            },
        ];
        // warn_failures = false keeps the legacy (no-verdict) prompt shape.
        let p = ask_prompt("what do I have on presses?", &hits, false);
        assert!(
            p.contains("[1] (node) hydraulic press tonnage drift"),
            "{p}"
        );
        assert!(p.contains("[2] (goal) cut scrap rate"), "{p}");
        assert!(p.contains("Cite entries as [n]"));
        assert!(p.contains("what do I have on presses?"));
    }

    #[test]
    fn prompt_flags_recorded_failures_when_warned() {
        let hits = vec![
            SearchNodeHit {
                kind: "node".into(),
                id: "n1".into(),
                label: "switch to OOM-prone batch size".into(),
                score: 0.95,
                tokens: 5,
                verdict: Some(-1.0),
            },
            SearchNodeHit {
                kind: "node".into(),
                id: "n2".into(),
                label: "stream the corpus in chunks".into(),
                score: 0.6,
                tokens: 4,
                verdict: Some(1.0),
            },
        ];
        let p = ask_prompt("how should I embed the corpus?", &hits, true);
        // Failure warning emitted, and the failed entry is tagged [FAILED].
        assert!(p.contains("RECORDED FAILURES"), "{p}");
        assert!(p.contains("[1] (node) switch to OOM-prone batch size [FAILED]"), "{p}");
        assert!(p.contains("[2] (node) stream the corpus in chunks [WORKED]"), "{p}");
    }

    #[test]
    fn empty_hits_still_yield_a_well_formed_prompt() {
        let p = ask_prompt("anything?", &[], false);
        assert!(p.contains("GRAPH ENTRIES:"));
        assert!(!p.contains("[1]"));
    }
}
