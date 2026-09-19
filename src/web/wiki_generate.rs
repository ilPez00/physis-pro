//! Phase 9.6 — `/api/v1/wiki/generate`: wiki-gen over the live tenant holarchy
//! (exposes `crate::wiki_gen` outline + prose + tour through the HTTP API).
//!
//! Pipeline per request: tenant's coherence nodes (labels) + resolved holarchy
//! edges → outline grouping → optional LLM naming via the shared provider
//! cascade → deterministic page prose → dependency-ordered tour. Coverage is
//! guaranteed by construction: the outline core never drops a node, and LLM
//! failure only degrades naming/summaries.

use super::*;
use crate::ai::provider::{Content, RouteHint};

/// POST /api/v1/wiki/generate request.
#[derive(Debug, Deserialize)]
pub struct WikiGenerateRequest {
    /// Max members per outline group before it splits.
    #[serde(default = "default_max_group")]
    pub max_group: usize,
    /// Max pages in the execution tour.
    #[serde(default = "default_tour_cap")]
    pub tour_cap: usize,
    /// Skip the LLM entirely — deterministic names + no summaries.
    #[serde(default)]
    pub deterministic: bool,
}

fn default_max_group() -> usize {
    8
}
fn default_tour_cap() -> usize {
    20
}

/// GET /api/v1/wiki/generate/tour — lightweight: just the ordered tour.
pub async fn wiki_tour_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    let (labels, edges) = state.with_user(&uid, |s| {
        let labels: Vec<crate::wiki_gen::NodeInput> = s
            .core
            .nodes
            .values()
            .filter_map(|n| {
                n.label
                    .as_ref()
                    .map(|l| crate::wiki_gen::NodeInput { label: l.clone() })
            })
            .collect();
        let edges: crate::communities::EdgeList = s
            .core
            .resolved_edges()
            .into_iter()
            .filter_map(|(a, b, _w)| match (a, b) {
                (Some(a), Some(b)) => Some((a, b)),
                _ => None,
            })
            .collect();
        (labels, edges)
    });
    let tour = crate::wiki_gen::execution_tour(&labels, &edges, 50);
    Json(serde_json::json!({ "count": tour.len(), "tour": tour }))
}

/// POST /api/v1/wiki/generate — full outline + pages (+ tour).
pub async fn wiki_generate_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<WikiGenerateRequest>,
) -> Json<serde_json::Value> {
    let uid = user_key(&headers);
    let semantic_ok = state.shared.embedder_health.semantic;
    let (nodes, raw_edges) = state.with_user(&uid, |s| {
        let nodes: Vec<crate::wiki_gen::NodeInput> = s
            .core
            .nodes
            .values()
            .filter_map(|n| {
                n.label
                    .as_ref()
                    .map(|l| crate::wiki_gen::NodeInput { label: l.clone() })
            })
            .collect();
        let edges: Vec<(Option<String>, Option<String>, f32)> = s.core.resolved_edges();
        (nodes, edges)
    });

    if nodes.is_empty() {
        return Json(serde_json::json!({
            "error": "no labeled coherence nodes for this tenant; ingest or scan first",
            "pages": [], "tour": []
        }));
    }

    // Convert to unweighted EdgeList between known labels.
    let label_set: std::collections::HashSet<String> =
        nodes.iter().map(|n| n.label.clone()).collect();
    let edges: crate::communities::EdgeList = raw_edges
        .into_iter()
        .filter_map(|(a, b, _w)| match (a, b) {
            (Some(a), Some(b)) if a != b => Some((a, b)),
            _ => None,
        })
        .filter(|(a, b)| label_set.contains(a) && label_set.contains(b))
        .collect();

    // Pass 1: coverage-guaranteed grouping (pure).
    let groups = crate::wiki_gen::group_nodes(&nodes, &edges, req.max_group);

    // Pass 2: LLM naming through the provider cascade unless disabled or the
    // embedder is degraded (RP noise would make group samples meaningless).
    let use_llm = !req.deterministic && semantic_ok;
    let mut pages = crate::wiki_gen::name_groups(&groups, &|_p: &str| anyhow::bail!("fallback"));

    async fn ask(
        cascade: &Arc<ProviderCascade>,
        prompt: String,
        hint: &str,
    ) -> anyhow::Result<String> {
        let messages = vec![Message {
            role: "user".into(),
            content: Content::Text(prompt),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }];
        let rh = RouteHint {
            task_type: hint.into(),
            has_image: false,
        };
        let (msg, _prov) = cascade
            .complete(&messages, None, &rh, Some(512))
            .await
            .map_err(|_| anyhow::anyhow!("model unavailable"))?;
        Ok(msg.content.unwrap_or_default())
    }

    // Build the naming payload from groups, call the model ONCE, then feed the
    // canned-style response through the pure name_groups (same code path tests use).
    if use_llm && !groups.is_empty() {
        let payload: Vec<serde_json::Value> = groups
            .iter()
            .map(|g| serde_json::json!({"id": g.id, "sample_labels": g.members.iter().take(5).collect::<Vec<_>>()}))
            .collect();
        let ask_prompt = format!(
            "You name clusters of knowledge-graph nodes. Return STRICT JSON:              {{\"pages\":[{{\"id\",\"title\",\"scope\"}}]}} covering ONLY these ids. Payload: {}",
            serde_json::to_string(&payload).unwrap()
        );
        if let Ok(resp) = ask(&state.shared.cascade, ask_prompt, "WIKI_NAMING").await {
            let resp_arc: Arc<String> = Arc::new(resp);
            let naming_fn = move |_req: &str| -> anyhow::Result<String> { Ok((*resp_arc).clone()) };
            pages = crate::wiki_gen::name_groups(&groups, &naming_fn);
        }
    }

    // Tour over the same graph.
    let tour_inputs: Vec<crate::wiki_gen::NodeInput> = nodes.clone();
    let tour = crate::wiki_gen::execution_tour(&tour_inputs, &edges, req.tour_cap);

    // Prose per page. One summary call per page (bounded by member sample);
    // failure leaves the deterministic skeleton intact.
    let mut rendered: Vec<serde_json::Value> = Vec::with_capacity(pages.len());
    for p in &pages {
        let md = if use_llm {
            let prompt = format!(
                "{title}: members {members}",
                title = p.title,
                members = p
                    .members
                    .iter()
                    .take(10)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            match ask(&state.shared.cascade,
                      format!("Write ONE concise paragraph introducing this wiki page for an                                industrial knowledge base. STRICT plain prose. Page data: {prompt}"),
                      "WIKI_SUMMARY").await
            {
                Ok(summary) => crate::wiki_gen::render_page(p, &|_q: &str| Ok(summary.clone())),
                Err(_) => crate::wiki_gen::render_page(p, &|_q: &str| anyhow::bail!("skip")),
            }
        } else {
            crate::wiki_gen::render_page(p, &|_q: &str| anyhow::bail!("skip"))
        };
        rendered.push(serde_json::json!({
            "title": p.title,
            "scope": p.scope,
            "members": p.members,
            "markdown": md,
        }));
    }

    Json(serde_json::json!({
        "count": rendered.len(),
        "pages": rendered,
        "tour": { "count": tour.len(), "order": tour },
        "degraded": !semantic_ok,
    }))
}
