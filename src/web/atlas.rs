//! Atlas — one call that returns a drawable graph of the tenant's knowledge.
//!
//! The existing surfaces each hold a piece: `/coherence/nodes/all` has nodes
//! with no edges, `/communities` has clusters in label space, `/files/lineage`
//! has causal chains per file. A visualization that stitched those together in
//! the browser would have to re-derive the joins on every render and would
//! disagree with the server the moment either side changed.
//!
//! So the join happens here, once, and the page draws what it is given.
//!
//! **Edges.** Holarchy edges are the recorded structure, but a graph that only
//! draws them is empty for any tenant whose nodes arrived through
//! `/coherence/register` — which is most of them. So the payload also carries
//! *similarity* edges: each node's nearest neighbours above a floor. They are
//! tagged with their kind and never merged, because "we recorded this link" and
//! "these two look alike" are different claims and a picture that blurs them
//! invents structure.

use super::*;
use crate::web::SharedState;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
};

#[derive(Debug, Deserialize)]
pub struct AtlasParams {
    /// Nodes to draw, highest coherence first. The similarity pass is O(n²·d),
    /// so this is also the cost knob.
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Nearest neighbours per node in the similarity layer.
    #[serde(default = "default_k")]
    pub k: usize,
    /// Cosine floor a similarity edge must clear.
    #[serde(default = "default_min_sim")]
    pub min_sim: f32,
}

fn default_limit() -> usize {
    280
}
fn default_k() -> usize {
    3
}
fn default_min_sim() -> f32 {
    0.55
}

/// One node as the atlas draws it.
#[derive(Debug, Clone, Serialize)]
pub struct AtlasNode {
    pub id: String,
    pub label: String,
    pub score: f32,
    pub asserted: Option<f32>,
    pub domain: Option<String>,
    pub mode: Option<String>,
    pub pinned: bool,
    pub created_at: String,
    pub community: usize,
    pub degree: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AtlasEdge {
    pub source: String,
    pub target: String,
    pub weight: f32,
    /// `holarchy` (recorded) or `similarity` (derived). Never merged.
    pub kind: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct AtlasCommunity {
    pub id: usize,
    pub size: usize,
    pub cohesion: f32,
    /// The highest-coherence member — what to call the cluster on screen.
    pub exemplar: String,
    /// Most common cell among members, when they agree on one.
    pub domain: Option<String>,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AtlasGraph {
    pub nodes: Vec<AtlasNode>,
    pub edges: Vec<AtlasEdge>,
    pub communities: Vec<AtlasCommunity>,
    pub total_nodes: usize,
    pub truncated: bool,
    pub min_sim: f32,
}

/// Input row for [`build_atlas`] — the fields the payload needs, already read
/// out of the tenant lock so the pure builder never touches runtime state.
pub struct AtlasInput {
    pub id: String,
    pub label: String,
    pub score: f32,
    pub asserted: Option<f32>,
    pub cell: Option<(String, String)>,
    pub pinned: bool,
    pub created_at: String,
    pub embedding: Vec<f32>,
}

/// Assemble the drawable graph. Pure: same inputs, same output, no clock, no
/// locks — which is what makes it testable and what makes two renders of an
/// unchanged graph identical instead of merely similar.
pub fn build_atlas(
    mut nodes: Vec<AtlasInput>,
    holarchy: Vec<(String, String, f32)>,
    total_nodes: usize,
    k: usize,
    min_sim: f32,
) -> AtlasGraph {
    // Highest coherence first, id as tiebreak: a stable order means the layout
    // seed is stable, so the picture does not reshuffle between refreshes.
    nodes.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });

    let present: std::collections::HashSet<&str> =
        nodes.iter().map(|n| n.id.as_str()).collect();

    let mut edges: Vec<AtlasEdge> = Vec::new();
    let mut seen: std::collections::HashSet<(String, String)> = Default::default();

    // Recorded structure first, so a real edge is never dropped in favour of a
    // derived one between the same pair.
    for (a, b, w) in holarchy {
        if a == b || !present.contains(a.as_str()) || !present.contains(b.as_str()) {
            continue;
        }
        let key = ordered(&a, &b);
        if seen.insert(key) {
            edges.push(AtlasEdge {
                source: a,
                target: b,
                weight: w,
                kind: "holarchy",
            });
        }
    }

    // Similarity layer: each node keeps its k nearest neighbours above the
    // floor. Symmetric duplicates collapse, so a mutual pair is one edge.
    for (i, n) in nodes.iter().enumerate() {
        if n.embedding.is_empty() {
            continue;
        }
        let mut near: Vec<(usize, f32)> = Vec::new();
        for (j, m) in nodes.iter().enumerate() {
            if i == j || m.embedding.is_empty() {
                continue;
            }
            let sim = crate::content::cosine(&n.embedding, &m.embedding);
            if sim >= min_sim {
                near.push((j, sim));
            }
        }
        near.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| nodes[a.0].id.cmp(&nodes[b.0].id))
        });
        for (j, sim) in near.into_iter().take(k) {
            let key = ordered(&n.id, &nodes[j].id);
            if seen.insert(key) {
                edges.push(AtlasEdge {
                    source: n.id.clone(),
                    target: nodes[j].id.clone(),
                    weight: sim,
                    kind: "similarity",
                });
            }
        }
    }

    // Communities over both layers together: clustering only the recorded
    // edges would leave every register-only node a singleton and the picture
    // would say "this graph has no structure" when it plainly does.
    let edge_list: crate::communities::EdgeList = edges
        .iter()
        .map(|e| (e.source.clone(), e.target.clone()))
        .collect();
    let report = crate::communities::detect_communities(&edge_list, 16);
    let assignment: std::collections::HashMap<&str, usize> = report
        .nodes
        .iter()
        .zip(report.assignment.iter())
        .map(|(n, &c)| (n.as_str(), c as usize))
        .collect();

    let mut degree: std::collections::HashMap<&str, usize> = Default::default();
    for e in &edges {
        *degree.entry(e.source.as_str()).or_insert(0) += 1;
        *degree.entry(e.target.as_str()).or_insert(0) += 1;
    }

    // Unconnected nodes land in one shared community rather than one each —
    // the same convention the communities rollup uses, so the two surfaces
    // count clusters the same way.
    let singleton = report.n_communities();
    let out_nodes: Vec<AtlasNode> = nodes
        .iter()
        .map(|n| AtlasNode {
            id: n.id.clone(),
            label: n.label.clone(),
            score: n.score,
            asserted: n.asserted,
            domain: n.cell.as_ref().map(|c| c.0.clone()),
            mode: n.cell.as_ref().map(|c| c.1.clone()),
            pinned: n.pinned,
            created_at: n.created_at.clone(),
            community: *assignment.get(n.id.as_str()).unwrap_or(&singleton),
            degree: *degree.get(n.id.as_str()).unwrap_or(&0),
        })
        .collect();

    let mut communities: Vec<AtlasCommunity> = Vec::new();
    let n_comms = singleton + 1;
    for cid in 0..n_comms {
        let members: Vec<&AtlasNode> = out_nodes.iter().filter(|n| n.community == cid).collect();
        if members.is_empty() {
            continue;
        }
        // Nodes are already sorted by coherence, so the first member is the
        // exemplar — the cluster's own strongest statement of what it is.
        let exemplar = members[0].label.clone();
        let mut cells: std::collections::HashMap<(String, String), usize> = Default::default();
        for m in &members {
            if let (Some(d), Some(md)) = (&m.domain, &m.mode) {
                *cells.entry((d.clone(), md.clone())).or_insert(0) += 1;
            }
        }
        let dominant = cells
            .into_iter()
            // Count desc, then the cell name, so a tie resolves the same way
            // every time instead of by hash order.
            .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
            .map(|(cell, _)| cell);
        communities.push(AtlasCommunity {
            id: cid,
            size: members.len(),
            cohesion: report.cohesion.get(cid).copied().unwrap_or(1.0),
            exemplar,
            domain: dominant.as_ref().map(|c| c.0.clone()),
            mode: dominant.as_ref().map(|c| c.1.clone()),
        });
    }
    communities.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.id.cmp(&b.id)));

    AtlasGraph {
        truncated: total_nodes > out_nodes.len(),
        nodes: out_nodes,
        edges,
        communities,
        total_nodes,
        min_sim,
    }
}

fn ordered(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

/// GET /api/v1/atlas/graph
pub async fn atlas_graph_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<AtlasParams>,
) -> Json<AtlasGraph> {
    let uid = user_key(&headers);
    let shared = state.shared.clone();
    let limit = params.limit.clamp(1, 2000);
    let k = params.k.clamp(1, 12);
    let min_sim = params.min_sim.clamp(0.0, 1.0);

    let (inputs, holarchy, total) = state.with_user(&uid, |s| {
        let total = s.core.nodes.len();
        let mut ranked: Vec<&crate::models::CoherenceNode> = s.core.nodes.values().collect();
        ranked.sort_by(|a, b| {
            b.coherence_score
                .partial_cmp(&a.coherence_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.id.cmp(&b.id))
        });
        ranked.truncate(limit);
        let inputs: Vec<AtlasInput> = ranked
            .into_iter()
            .map(|n| AtlasInput {
                id: n.id.clone(),
                // A bare-vector node has no label; showing its id is more
                // honest than showing an empty circle with no way to ask what
                // it is.
                label: n
                    .label
                    .clone()
                    .filter(|l| !l.trim().is_empty())
                    .unwrap_or_else(|| n.id.clone()),
                score: n.coherence_score,
                asserted: n.asserted,
                // The pin wins over the classifier — same precedence as
                // `NodeView::observable`, so the atlas and the node browser
                // never disagree about which cell a node is in.
                cell: n
                    .cell_pin
                    .clone()
                    .or_else(|| auto_cell(&shared, n)),
                pinned: n.cell_pin.is_some(),
                created_at: n.created_at.to_rfc3339(),
                embedding: n.embedding.clone(),
            })
            .collect();
        let holarchy: Vec<(String, String, f32)> = s
            .core
            .resolved_edges()
            .into_iter()
            .filter_map(|(a, b, w)| match (a, b) {
                (Some(a), Some(b)) => Some((a, b, w)),
                _ => None,
            })
            .collect();
        (inputs, holarchy, total)
    });

    Json(build_atlas(inputs, holarchy, total, k, min_sim))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, score: f32, emb: Vec<f32>) -> AtlasInput {
        AtlasInput {
            id: id.to_string(),
            label: id.to_string(),
            score,
            asserted: None,
            cell: Some(("FABRICATE".into(), "MAINTAIN".into())),
            pinned: false,
            created_at: "2026-08-29T00:00:00Z".into(),
            embedding: emb,
        }
    }

    #[test]
    fn similarity_edges_appear_for_a_graph_with_no_recorded_edges() {
        // The common case: nodes registered one by one, no holarchy edges at
        // all. A picture drawn from recorded edges alone would be a starfield.
        let g = build_atlas(
            vec![
                node("a", 0.9, vec![1.0, 0.0]),
                node("b", 0.8, vec![0.98, 0.2]),
                node("c", 0.7, vec![0.0, 1.0]),
            ],
            Vec::new(),
            3,
            3,
            0.55,
        );
        assert_eq!(g.edges.len(), 1, "{:?}", g.edges);
        assert_eq!(g.edges[0].kind, "similarity");
        let pair = ordered(&g.edges[0].source, &g.edges[0].target);
        assert_eq!(pair, ("a".to_string(), "b".to_string()));
    }

    #[test]
    fn a_recorded_edge_is_never_relabeled_as_similarity() {
        let g = build_atlas(
            vec![
                node("a", 0.9, vec![1.0, 0.0]),
                node("b", 0.8, vec![0.99, 0.1]),
            ],
            vec![("a".into(), "b".into(), 0.4)],
            2,
            3,
            0.55,
        );
        assert_eq!(g.edges.len(), 1, "the pair collapses: {:?}", g.edges);
        assert_eq!(
            g.edges[0].kind, "holarchy",
            "recorded structure outranks resemblance: {:?}", g.edges
        );
        assert_eq!(g.edges[0].weight, 0.4);
    }

    #[test]
    fn unconnected_nodes_share_one_community_and_are_not_lost() {
        let g = build_atlas(
            vec![
                node("a", 0.9, vec![1.0, 0.0]),
                node("b", 0.8, vec![0.99, 0.1]),
                node("lonely", 0.5, vec![0.0, 1.0]),
                node("lonelier", 0.4, vec![0.0, -1.0]),
            ],
            Vec::new(),
            4,
            3,
            0.55,
        );
        assert_eq!(g.nodes.len(), 4, "every node is drawn");
        let lonely: Vec<&AtlasNode> = g
            .nodes
            .iter()
            .filter(|n| n.label.starts_with("lonel"))
            .collect();
        assert_eq!(lonely[0].community, lonely[1].community);
        assert_eq!(lonely[0].degree, 0);
        let total: usize = g.communities.iter().map(|c| c.size).sum();
        assert_eq!(total, 4, "communities partition the nodes: {:?}", g.communities);
    }

    #[test]
    fn communities_report_their_exemplar_and_dominant_cell() {
        let mut a = node("alpha", 0.95, vec![1.0, 0.0]);
        a.label = "spindle bearing failure".into();
        let g = build_atlas(
            vec![a, node("b", 0.8, vec![0.99, 0.05])],
            Vec::new(),
            2,
            3,
            0.55,
        );
        let c = &g.communities[0];
        assert_eq!(c.exemplar, "spindle bearing failure", "{:?}", g.communities);
        assert_eq!(c.domain.as_deref(), Some("FABRICATE"));
        assert_eq!(c.mode.as_deref(), Some("MAINTAIN"));
    }

    #[test]
    fn output_is_stable_across_identical_builds() {
        // The layout is seeded from this order; an unstable sort would make the
        // graph reshuffle on every refresh for no reason the user can see.
        let build = || {
            build_atlas(
                vec![
                    node("b", 0.7, vec![1.0, 0.0]),
                    node("a", 0.7, vec![0.99, 0.1]),
                    node("c", 0.7, vec![0.98, 0.2]),
                ],
                Vec::new(),
                3,
                2,
                0.55,
            )
        };
        let one = build();
        let two = build();
        let ids = |g: &AtlasGraph| g.nodes.iter().map(|n| n.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&one), ids(&two));
        assert_eq!(
            one.edges
                .iter()
                .map(|e| (e.source.clone(), e.target.clone()))
                .collect::<Vec<_>>(),
            two.edges
                .iter()
                .map(|e| (e.source.clone(), e.target.clone()))
                .collect::<Vec<_>>()
        );
    }
}
