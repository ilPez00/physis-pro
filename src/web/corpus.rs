//! Phase 11 surface — corpus-level comprehension over the tenant graph.
//!
//! `/api/v1/search/nodes` and `/api/v1/ask` are top-k surfaces: they answer
//! "which entries look like this question". A question about the corpus itself
//! ("what do we keep failing at", "what are the recurring themes") has no
//! single node holding the answer, so no similarity ranking can surface it.
//!
//! These handlers rank *community summaries* instead of nodes: detection runs
//! over the same tenant graph the communities rollup uses, each community is
//! summarized deterministically by [`crate::corpus`], and the summaries — not
//! the members — become the retrieval corpus. Cost is bounded by the number of
//! communities, not by graph size.
//!
//! Read-only, like the communities rollup it extends: these handlers read the
//! core and the ledger and write only their own JSON response.

use super::*;
use crate::corpus::{self, CommunitySummary};
use crate::web::SharedState;
use axum::{
    extract::{Json, State},
    http::HeaderMap,
};

/// Ceiling for a synthesized global answer. Larger than `ask`'s: a global
/// answer names several themes, and each one costs a sentence.
const GLOBAL_MAX_TOKENS: u32 = 500;

/// Gather the tenant's labeled nodes, their edges in label space, and the
/// facets each label carries — delegating to the shared
/// [`crate::corpus::gather_facts`] so the CLI and the web surface summarize the
/// same data under the same rules.
fn collect_facts(
    state: &SharedState,
    uid: &str,
) -> (Vec<corpus::MemberFacts>, crate::communities::EdgeList) {
    state.with_user(uid, |s| corpus::gather_facts(&s.core, &s.ledger))
}

/// Detect communities and summarize them, delegating to the shared
/// [`crate::corpus::build_summaries`].
fn build_summaries(
    facts: &[corpus::MemberFacts],
    edges: &crate::communities::EdgeList,
) -> Vec<CommunitySummary> {
    corpus::build_summaries(facts, edges)
}

/// GET /api/v1/corpus/summaries — every community in this tenant's graph,
/// summarized: title, distinctive keywords, dominant facets, outcome tally and
/// a deterministic digest. Largest community first.
///
/// Offline by construction — no provider is consulted, so this is the surface
/// an air-gapped install still gets.
pub async fn summaries_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    let (facts, edges) = collect_facts(&state, &uid);
    let summaries = build_summaries(&facts, &edges);
    Json(serde_json::json!({
        "communities": summaries.len(),
        "nodes": facts.len(),
        "summaries": summaries,
    }))
}

/// POST /api/v1/corpus/ask — a question about the corpus as a whole.
///
/// Same two-stage contract as `/api/v1/ask`: retrieval always works (the
/// ranked summaries come back regardless), synthesis is added when a semantic
/// provider is configured. The difference is the unit — communities, not
/// nodes — and the prompt says so, so the model answers about distribution and
/// recurrence instead of quoting one document.
pub async fn global_ask_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<GlobalAskRequest>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    let (facts, edges) = collect_facts(&state, &uid);
    let summaries = build_summaries(&facts, &edges);

    // Rank summaries by cosine of the question to each digest. Digests are
    // embedded through the content-addressed cache: they only change when
    // membership or facets change, so a repeated global question re-embeds
    // nothing.
    let shared = state.shared.clone();
    let q_emb = embed_text(shared.clone(), req.question.clone()).await;
    let texts: Vec<String> = summaries.iter().map(|s| s.retrieval_text()).collect();
    let (embeddings, _hits, _misses) = embed_texts_cached(shared, texts).await;

    let mut ranked: Vec<(usize, f32)> = embeddings
        .iter()
        .enumerate()
        .map(|(i, e)| (i, crate::models::cosine_sim(&q_emb, e)))
        .collect();
    ranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    let context = corpus::pack_global(&summaries, &ranked, req.token_budget, req.top_k);

    let semantic = state.shared.embedder_health.semantic;
    let mut answer = None;
    let mut provider = None;
    let mut note = match () {
        _ if !semantic => {
            Some("no semantic provider configured — showing ranked communities".to_string())
        }
        _ if context.hits.is_empty() => Some("no communities in the graph yet".to_string()),
        _ => None,
    };

    if note.is_none() {
        let cascade = state.shared.cascade.clone();
        let messages = vec![
            crate::ai::provider::Message {
                role: "system".into(),
                content: crate::ai::provider::Content::Text(corpus::global_prompt(
                    &req.question,
                    &summaries,
                    &context,
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
            .complete(&messages, None, &hint, Some(GLOBAL_MAX_TOKENS))
            .await
        {
            Ok((msg, prov)) => {
                answer = Some(msg.content.unwrap_or_default());
                provider = Some(prov);
            }
            Err(e) => {
                log::warn!("[corpus/ask] synthesis failed, falling back to summaries: {e}");
                note = Some("answer generation failed — showing ranked communities".to_string());
            }
        }
    }

    // Echo the cited summaries in full so a client can render the evidence
    // behind every `[n]` without a second round trip.
    let cited: Vec<&CommunitySummary> = context
        .hits
        .iter()
        .filter_map(|h| summaries.iter().find(|s| s.id == h.id))
        .collect();

    Json(serde_json::json!({
        "question": req.question,
        "semantic": semantic,
        "answer": answer,
        "provider": provider,
        "note": note,
        "budget": context.budget,
        "total_tokens": context.total_tokens,
        "truncated": context.truncated,
        "communities": summaries.len(),
        "hits": context.hits,
        "cited": cited,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(label: &str, cell: Option<(&str, &str)>, verdict: Option<f32>) -> corpus::MemberFacts {
        corpus::MemberFacts {
            label: label.to_string(),
            cell: cell.map(|(d, m)| (d.to_string(), m.to_string())),
            verdict,
            place: None,
            period: None,
            mechanism: None,
        }
    }

    #[test]
    fn connected_nodes_group_isolated_nodes_become_one_singleton_community() {
        let members = vec![
            facts("spindle bearing preload", None, None),
            facts("spindle bearing replacement", None, None),
            facts("invoice reconciliation run", None, None),
        ];
        let edges: crate::communities::EdgeList = vec![(
            "spindle bearing preload".to_string(),
            "spindle bearing replacement".to_string(),
        )];

        let summaries = build_summaries(&members, &edges);
        let total: usize = summaries.iter().map(|s| s.size).sum();
        assert_eq!(total, 3, "every labeled node must land in some community");
        assert!(summaries.iter().any(|s| s
            .members
            .contains(&"invoice reconciliation run".to_string())));
    }

    #[test]
    fn summaries_carry_facets_and_outcomes_through_detection() {
        let members = vec![
            facts(
                "agent loop retry storm",
                Some(("MACHINE", "PROCESS")),
                Some(-1.0),
            ),
            facts(
                "agent loop budget guard",
                Some(("MACHINE", "PROCESS")),
                Some(1.0),
            ),
        ];
        let edges: crate::communities::EdgeList = vec![(
            "agent loop retry storm".to_string(),
            "agent loop budget guard".to_string(),
        )];

        let summaries = build_summaries(&members, &edges);
        assert_eq!(summaries.len(), 1);
        let s = &summaries[0];
        assert_eq!(s.dominant_cell.as_deref(), Some("MACHINE/PROCESS"));
        assert_eq!(s.failures, 1);
        assert_eq!(s.wins, 1);
        assert!(s.keywords.contains(&"agent".to_string()));
        assert!(s.digest.contains("1 positive, 1 negative"));
    }

    #[test]
    fn empty_graph_yields_no_summaries_rather_than_an_error() {
        let summaries = build_summaries(&[], &Vec::new());
        assert!(summaries.is_empty());
    }
}
