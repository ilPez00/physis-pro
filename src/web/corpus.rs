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
use crate::corpus::{self, CommunitySummary, MemberFacts};
use crate::web::SharedState;
use axum::{
    extract::{Json, State},
    http::HeaderMap,
};

/// Ceiling for a synthesized global answer. Larger than `ask`'s: a global
/// answer names several themes, and each one costs a sentence.
const GLOBAL_MAX_TOKENS: u32 = 500;

/// Gather the tenant's labeled nodes, their edges in label space, and the
/// facets each label carries.
///
/// Facets come from wherever the graph actually records them: the semiotic
/// cell from the node's pin, the verdict from its asserted outcome, and the
/// mechanism from the causal ledger when an event names this label as its
/// object. Place and period live on `FileAddress`, which belongs to file nodes
/// rather than coherence nodes, so they stay `None` here — `MemberFacts` omits
/// absent facets from the digest rather than guessing them.
fn collect_facts(
    state: &SharedState,
    uid: &str,
) -> (Vec<MemberFacts>, crate::communities::EdgeList) {
    state.with_user(uid, |s| {
        // Latest mechanism per artifact. The ledger is append-only and ordered
        // by seq, so a later event simply overwrites an earlier one here.
        let mut mechanism_of: std::collections::HashMap<&str, &'static str> =
            std::collections::HashMap::new();
        for e in s.ledger.events() {
            mechanism_of.insert(e.object.as_str(), e.mechanism.as_str());
        }

        let names: std::collections::HashMap<String, String> = s
            .core
            .nodes
            .values()
            .filter_map(|n| n.label.as_ref().map(|l| (n.id.clone(), l.clone())))
            .collect();

        let facts: Vec<MemberFacts> = s
            .core
            .nodes
            .values()
            .filter_map(|n| {
                let label = n.label.as_deref()?;
                if label.trim().is_empty() {
                    return None;
                }
                Some(MemberFacts {
                    label: label.to_string(),
                    cell: n.cell_pin.clone(),
                    verdict: n.asserted,
                    place: None,
                    period: None,
                    mechanism: mechanism_of
                        .get(label)
                        .or_else(|| mechanism_of.get(n.id.as_str()))
                        .map(|m| m.to_string()),
                })
            })
            .collect();

        let edges: crate::communities::EdgeList = s
            .core
            .resolved_edges()
            .into_iter()
            .filter_map(|(a, b, _w)| match (a, b) {
                (Some(a), Some(b)) if a != b => match (names.get(&a), names.get(&b)) {
                    (Some(la), Some(lb)) if la != lb => Some((la.clone(), lb.clone())),
                    _ => None,
                },
                _ => None,
            })
            .collect();

        (facts, edges)
    })
}

/// Detect communities over `edges` and summarize them, carrying each member's
/// facets across.
///
/// Split out from the handler so the whole summary pipeline is unit-testable
/// without an HTTP stack. Labeled nodes carrying no edge are gathered into one
/// singleton community at full cohesion — the same convention the communities
/// rollup uses, so a node is never invisible to corpus-level questions just
/// because nothing links to it.
fn build_summaries(
    facts: &[MemberFacts],
    edges: &crate::communities::EdgeList,
) -> Vec<CommunitySummary> {
    let report = crate::communities::detect_communities(edges, 16);
    let by_label: std::collections::HashMap<&str, &MemberFacts> =
        facts.iter().map(|f| (f.label.as_str(), f)).collect();

    let mut input: Vec<(u32, f32, Vec<MemberFacts>)> = (0..report.n_communities())
        .map(|i| {
            let members: Vec<MemberFacts> = report
                .members(i as u32)
                .into_iter()
                .map(|label| {
                    by_label
                        .get(label)
                        .map(|f| (*f).clone())
                        // An edge endpoint with no node behind it still counts
                        // as a member: dropping it would break the coverage
                        // contract (`members` must list every member).
                        .unwrap_or_else(|| MemberFacts::new(label))
                })
                .collect();
            (i as u32, report.cohesion[i], members)
        })
        .collect();

    let connected: std::collections::HashSet<&str> =
        report.nodes.iter().map(|s| s.as_str()).collect();
    let singles: Vec<MemberFacts> = facts
        .iter()
        .filter(|f| !connected.contains(f.label.as_str()))
        .cloned()
        .collect();
    if !singles.is_empty() {
        input.push((input.len() as u32, 1.0, singles));
    }

    corpus::summarize(&input)
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

    let semantic = state.shared.semantic;
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

    fn facts(label: &str, cell: Option<(&str, &str)>, verdict: Option<f32>) -> MemberFacts {
        MemberFacts {
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
