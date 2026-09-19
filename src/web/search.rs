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

/// One recall candidate, scored inside the tenant lock: dot-product against
/// the query embedding (same as `concepts_search_handler`), asserted verdict
/// riding along. `kind` is static — only "node" and "goal" exist.
struct ScoredCandidate {
    kind: &'static str,
    id: String,
    label: String,
    embedding: Vec<f32>,
    score: f32,
    asserted: Option<f32>,
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

    // Score every candidate against the query *inside* the tenant lock and
    // keep only the score-ranked prefix that MMR + the token budget could
    // still reach (keep >= 4x top_k, floor 32). The rest stays uncloned:
    // this used to copy every node id, label and 384-dim embedding out of
    // the lock per request, twice (again into the RagCorpus below).
    // Scoring is the same dot-product `concepts_search_handler` uses — same
    // embedder-consistency contract. The asserted verdict rides along so
    // recall becomes outcome memory: a top hit can say "you already tried
    // this and it failed." Unlabeled goals (pure vectors) are skipped —
    // nothing browsable to return.
    let candidates: Vec<ScoredCandidate> = state.with_user(uid, |s| {
        let mut out: Vec<ScoredCandidate> = Vec::new();
        let score = |emb: &[f32]| q_emb.iter().zip(emb).map(|(a, b)| a * b).sum::<f32>();
        for n in s.core.nodes.values() {
            if let Some(label) = n.label.as_deref() {
                if !label.trim().is_empty() {
                    out.push(ScoredCandidate {
                        kind: "node",
                        id: n.id.clone(),
                        label: label.to_string(),
                        embedding: n.embedding.clone(),
                        score: score(&n.embedding),
                        asserted: n.asserted,
                    });
                }
            }
        }
        for g in &s.goals {
            let label = match &g.label {
                Some(l) if !l.trim().is_empty() => l.clone(),
                _ => continue,
            };
            out.push(ScoredCandidate {
                kind: "goal",
                id: g.id.clone(),
                label,
                embedding: g.embedding.clone(),
                score: score(&g.embedding),
                asserted: None,
            });
        }
        out.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let keep = top_k.saturating_mul(4).max(32);
        out.truncate(keep);
        out
    });

    // Reuse the token-fixed retriever over a corpus built from node labels
    // (embeddings already computed — no re-embed). Chunk id = candidate index.
    let corpus = rag::RagCorpus {
        chunks: candidates
            .iter()
            .enumerate()
            .map(|(i, c)| rag::RagChunk {
                id: i,
                text: c.label.clone(),
                tokens: rag::count_tokens(&c.label),
                embedding: c.embedding.clone(),
            })
            .collect(),
    };
    let retriever = rag::TokenFixedRetriever::new(token_budget, top_k);
    let result = retriever.retrieve(&q_emb, &corpus);

    let hits: Vec<SearchNodeHit> = result
        .chunks
        .iter()
        .filter_map(|c| {
            let cand = candidates.get(c.id)?;
            Some(SearchNodeHit {
                kind: cand.kind.to_string(),
                id: cand.id.clone(),
                label: cand.label.clone(),
                score: c.score,
                tokens: c.tokens,
                verdict: cand.asserted,
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

    (
        result.budget,
        result.total_tokens,
        result.truncated,
        hits,
        failures,
    )
}

/// GET /api/v1/concepts/search — PLAN 11C conceptual search.
///
/// Same retrieval corpus as `/search/nodes` (tenant coherence labels, no
/// re-embed) but a different contract: no token packing, a raw-cosine
/// `threshold` floor, quality re-rank via `cell_standing`, and grouping by
/// effective semiotic cell. Pure ranking lives in
/// `models::rank_concept_hits`; this handler only supplies embeddings,
/// cells, and standings.
pub async fn concepts_search_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<ConceptSearchQuery>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    let threshold = params.threshold.unwrap_or(0.0);
    let limit = params.limit.unwrap_or(50).clamp(1, 500);
    let query = params.query.clone();
    let q_emb = embed_text(state.shared.clone(), query.clone()).await;

    let scored: Vec<crate::models::ConceptRaw> = state.with_user(&uid, |s| {
        let mut out = Vec::new();
        for n in s.core.nodes.values() {
            let Some(label) = n.label.as_deref() else {
                continue;
            };
            if label.trim().is_empty() {
                continue;
            }
            let raw: f32 = q_emb.iter().zip(&n.embedding).map(|(a, b)| a * b).sum();
            if raw < threshold {
                continue;
            }
            // Effective cell: pin wins, else classifier top hit — the same
            // resolution `observable()` uses. Standings are snapshotted
            // below (outside the tenant lock) so this closure never takes
            // the quality lock while holding the user lock.
            let cell = n.cell_pin.clone().or_else(|| {
                state
                    .shared
                    .classifier
                    .classify(&n.embedding)
                    .first()
                    .map(|c| (c.domain.clone(), c.mode.clone()))
            });
            out.push((
                n.id.clone(),
                label.to_string(),
                raw,
                cell.map(|(d, m)| format!("{d}/{m}")),
                n.asserted,
            ));
        }
        out
    });

    // Quality standings, snapshotted outside the tenant lock (lock order:
    // quality after user state would invert the harness/coherence order).
    let standings: std::collections::HashMap<String, f32> = {
        let quality = state.quality.lock().unwrap_or_else(|e| e.into_inner());
        let mut m = std::collections::HashMap::new();
        for cell in scored.iter().filter_map(|(_, _, _, c, _)| c.as_deref()) {
            let key = cell.replace('/', "\x00");
            m.entry(cell.to_string())
                .or_insert_with(|| quality.cell_standing(&key));
        }
        m
    };
    let rescored: Vec<crate::models::ConceptCandidate> = scored
        .into_iter()
        .map(|(id, label, raw, cell, verdict)| {
            let standing = cell
                .as_deref()
                .and_then(|c| standings.get(c).copied())
                .unwrap_or(1.0);
            (id, label, raw, cell, verdict, raw * standing)
        })
        .collect();

    let mut hits = crate::models::rank_concept_hits(rescored);
    hits.truncate(limit);
    let groups = hits.iter().map(|h| h.group).max().unwrap_or(0);
    Json(serde_json::json!({
        "query": query,
        "threshold": threshold,
        "count": hits.len(),
        "groups": groups,
        "results": hits,
    }))
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

    let semantic = state.shared.embedder_health.semantic;
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
        p.push_str(&format!(
            "[{}] ({}) {}{}\n",
            i + 1,
            h.kind,
            h.label,
            verdict
        ));
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
        assert!(
            p.contains("[1] (node) switch to OOM-prone batch size [FAILED]"),
            "{p}"
        );
        assert!(
            p.contains("[2] (node) stream the corpus in chunks [WORKED]"),
            "{p}"
        );
    }

    #[test]
    fn empty_hits_still_yield_a_well_formed_prompt() {
        let p = ask_prompt("anything?", &[], false);
        assert!(p.contains("GRAPH ENTRIES:"));
        assert!(!p.contains("[1]"));
    }
}
