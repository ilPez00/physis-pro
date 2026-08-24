//! Phase 9.5 — wiki generation over the holarchy: deterministic outline core
//! (clean-room adaptation, docs/reference/repowise-adaptation.md P1).
//!
//! Two-pass outline planning, coverage-first:
//! 1. **Grouping pass** — pure, deterministic. Nodes cluster by community
//!    (label-propagation over coherence edges, reusing `crate::communities`)
//!    with oversized clusters split by dominant label prefix. Coverage of the
//!    input node set is GUARANTEED here; nothing downstream can drop a node.
//! 2. **Naming pass** — the only LLM-touching step. The model receives opaque
//!    group ids and returns title+scope per id. Forgotten ids keep
//!    deterministic fallback names; INVENTED ids are discarded. Coverage is
//!    invariant to whatever the LLM does or fails to do.
//!
//! The LLM is injected as `&dyn Fn(&str) -> Result<String>` so every naming
//! path is testable with canned responses and no network.

use crate::communities::{detect_communities, EdgeList};

/// One holarchy node to be covered by the outline.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeInput {
    /// Stable node label (e.g. coherence node label).
    pub label: String,
}

/// A group of nodes that will become one wiki page.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    /// Opaque id handed to the naming pass ("g0", "g1", ...).
    pub id: String,
    /// Every input label in this group — never modified after grouping.
    pub members: Vec<String>,
}

/// Final outline entry.
#[derive(Debug, Clone, PartialEq)]
pub struct OutlinePage {
    pub title: String,
    pub scope: String,
    pub members: Vec<String>,
}

/// Pass 1: deterministic grouping. Communities from the edge list become
/// groups; communities larger than `max_group` are split by dominant first
/// path-segment of their labels (`/`-delimited), recursively until small
/// enough or un-splittable.
pub fn group_nodes(nodes: &[NodeInput], edges: &EdgeList, max_group: usize) -> Vec<Group> {
    let known: std::collections::HashSet<&str> =
        nodes.iter().map(|n| n.label.as_str()).collect();
    // Only edges between known nodes participate (holarchy may hold more).
    let filtered: EdgeList = edges
        .iter()
        .filter(|(a, b)| known.contains(a.as_str()) && known.contains(b.as_str()))
        .cloned()
        .collect();

    let report = detect_communities(&filtered, 16);
    let mut raw: Vec<Vec<String>> = vec![Vec::new(); report.n_communities().max(1)];
    if report.n_communities() == 0 && !nodes.is_empty() {
        raw.push(Vec::new());
    }
    for (i, label) in report.nodes.iter().enumerate() {
        raw[report.assignment[i] as usize].push(label.clone());
    }
    // Isolated nodes never entered the graph — singleton groups.
    let clustered: std::collections::HashSet<&str> = report
        .nodes
        .iter()
        .map(|s| s.as_str())
        .collect();
    for n in nodes {
        if !clustered.contains(n.label.as_str()) {
            raw.push(vec![n.label.clone()]);
        }
    }

    let mut out = Vec::new();
    for cluster in raw {
        split_oversized(cluster, max_group, &mut out);
    }
    out.sort_by_key(|g| g.id.clone());
    out
}

fn split_oversized(cluster: Vec<String>, max_group: usize, out: &mut Vec<Group>) {
    if cluster.len() <= max_group {
        out.push(Group {
            id: format!("g{}", out.len()),
            members: cluster,
        });
        return;
    }
    // Split by dominant leading segment; fall back to halving when all share
    // one segment (or have none).
    let seg = |l: &String| {
        l.split('/')
            .next()
            .unwrap_or("")
            .to_lowercase()
    };
    let mut buckets: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for l in cluster {
        buckets.entry(seg(&l)).or_default().push(l);
    }
    if buckets.len() == 1 {
        let mut members = buckets.into_values().next().unwrap();
        members.sort();
        let half = members.len() / 2;
        let tail = members.split_off(half);
        split_oversized(members, max_group, out);
        split_oversized(tail, max_group, out);
        return;
    }
    for (_, bucket) in buckets {
        split_oversized(bucket, max_group, out);
    }
}

/// Pass 2 result quality gate: invented ids dropped, missing ids get fallbacks.
///
/// `naming_fn` receives a JSON payload `{id, sample_labels}` per group and
/// must return JSON `{id, title, scope}` lines. Any group absent from the
/// response keeps its deterministic name; any response id without a group is
/// silently discarded. Malformed output for one group never poisons others.
pub fn name_groups(
    groups: &[Group],
    naming_fn: &dyn Fn(&str) -> anyhow::Result<String>,
) -> Vec<OutlinePage> {
    use serde_json::Value;

    let payload: Vec<Value> = groups
        .iter()
        .map(|g| {
            serde_json::json!({
                "id": g.id,
                "sample_labels": g.members.iter().take(5).collect::<Vec<_>>(),
            })
        })
        .collect();
    let raw = match naming_fn(&serde_json::to_string(&payload).unwrap()) {
        Ok(s) => s,
        Err(_) => return fallback_all(groups), // total LLM failure still yields full coverage
    };

    let parsed: std::collections::HashMap<String, (String, String)> = serde_json::from_str::<Value>(&raw)
        .ok()
        .and_then(|v| v.get("pages").cloned())
        .and_then(|p| p.as_array().cloned())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| {
                    let id = entry.get("id")?.as_str()?.to_owned();
                    let title = entry.get("title")?.as_str()?.trim().to_owned();
                    let scope = entry
                        .get("scope")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned();
                    if title.is_empty() {
                        None
                    } else {
                        Some((id, (title, scope)))
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    groups
        .iter()
        .map(|g| match parsed.get(&g.id) {
            Some((title, scope)) => OutlinePage {
                title: title.clone(),
                scope: scope.clone(),
                members: g.members.clone(),
            },
            None => OutlinePage {
                title: deterministic_title(g),
                scope: format!("Auto-grouped cluster {}.", g.id),
                members: g.members.clone(),
            },
        })
        .collect()
}

fn fallback_all(groups: &[Group]) -> Vec<OutlinePage> {
    groups
        .iter()
        .map(|g| OutlinePage {
            title: deterministic_title(g),
            scope: "Named without model assistance.".into(),
            members: g.members.clone(),
        })
        .collect()
}

/// Deterministic fallback: dominant leading segment + size tag.
fn deterministic_title(g: &Group) -> String {
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for l in &g.members {
        *counts
            .entry(l.split('/').next().unwrap_or("").to_lowercase())
            .or_default() += 1;
    }
    let best = counts
        .into_iter()
        .max_by_key(|(name, c)| (*c, std::cmp::Reverse(name.clone())))
        .map(|(name, _)| name)
        .unwrap_or_else(|| "misc".into());
    format!(
        "{} cluster ({} nodes)",
        if best.is_empty() { "misc" } else { &best },
        g.members.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(l: &str) -> NodeInput {
        NodeInput {
            label: l.to_owned(),
        }
    }
    fn e(a: &str, b: &str) -> (String, String) {
        (a.to_owned(), b.to_owned())
    }

    #[test]
    fn grouping_covers_every_input_node_exactly_once() {
        let nodes: Vec<NodeInput> = ["a/x", "b/y", "a/z", "lonely"].iter().map(|s| n(s)).collect();
        let edges: EdgeList = vec![e("a/x", "a/z"), e("b/y", "b/y"), e("ghost", "a/x")];
        let groups = group_nodes(&nodes, &edges, 10);
        let mut covered: Vec<&str> =
            groups.iter().flat_map(|g| g.members.iter()).map(|s| s.as_str()).collect();
        covered.sort_unstable();
        assert_eq!(covered, vec!["a/x", "a/z", "b/y", "lonely"], "{groups:?}");
        // no duplicates
        assert_eq!(covered.len(), covered.iter().collect::<std::collections::HashSet<_>>().len());
        // ghost edge ignored
        assert!(!groups.iter().any(|g| g.members.iter().any(|m| m == "ghost")));
    }

    #[test]
    fn oversized_cluster_splits_by_leading_segment() {
        let labels: Vec<NodeInput> = [
            "infra/one", "infra/two", "infra/three", "ui/one", "ui/two", "docs/one",
        ]
        .iter()
        .map(|s| n(s))
        .collect();
        // All one community via chain, but cap at 3 forces splits.
        let edges: EdgeList = vec![
            e("infra/one", "infra/two"),
            e("infra/two", "infra/three"),
            e("infra/three", "ui/one"),
            e("ui/one", "ui/two"),
            e("ui/two", "docs/one"),
        ];
        let groups = group_nodes(&labels, &edges, 3);
        assert!(groups.iter().all(|g| g.members.len() <= 3), "{groups:?}");
        let total: usize = groups.iter().map(|g| g.members.len()).sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn unsplittable_same_segment_cluster_halves_recursively() {
        let labels: Vec<NodeInput> = (0..7).map(|i| n(&format!("mono/item{i}"))).collect();
        let edges: EdgeList = (1..7)
            .map(|i| e(&format!("mono/item{}", i - 1), &format!("mono/item{i}")))
            .collect();
        let groups = group_nodes(&labels, &edges, 2);
        assert!(groups.iter().all(|g| g.members.len() <= 2), "{groups:?}");
        assert_eq!(groups.iter().map(|g| g.members.len()).sum::<usize>(), 7);
    }

    fn canned(resp: &str) -> impl Fn(&str) -> anyhow::Result<String> + '_ {
        move |_req: &str| Ok(resp.to_owned())
    }

    const GOOD_RESPONSE: &str = r#"{"pages":[
        {"id":"g0","title":"Infrastructure","scope":"Shared plumbing components."},
        {"id":"g999","title":"Invented","scope":"Should be discarded."}
    ]}"#;

    #[test]
    fn naming_drops_invented_ids_keeps_fallback_for_forgotten() {
        let groups = vec![
            Group { id: "g0".into(), members: vec!["a/1".into()] },
            Group { id: "g1".into(), members: vec!["b/2".into()] },
        ];
        let pages = name_groups(&groups, &canned(GOOD_RESPONSE));
        assert_eq!(pages.len(), 2, "coverage invariant");
        assert_eq!(pages[0].title, "Infrastructure");
        // g1 was forgotten → deterministic fallback, NOT dropped.
        assert!(pages[1].title.contains("cluster"));
        assert_eq!(pages[1].members, vec!["b/2"]);
    }

    #[test]
    fn total_llm_failure_still_yields_full_outline() {
        let groups = vec![Group { id: "g0".into(), members: vec!["x/1".into(), "x/2".into()] }];
        let fail = |_req: &str| -> anyhow::Result<String> { Err(anyhow::anyhow!("api down")) };
        let pages = name_groups(&groups, &fail);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].members.len(), 2);
        assert!(pages[0].title.contains("x cluster"));
    }

    #[test]
    fn garbage_response_is_tolerated_like_failure() {
        let groups = vec![Group { id: "g0".into(), members: vec!["q/1".into()] }];
        let pages = name_groups(&groups, &canned("this is not json"));
        assert_eq!(pages.len(), 1);
        assert!(pages[0].title.contains("q cluster"));
    }

    #[test]
    fn empty_titles_in_response_get_fallback_not_empty_page() {
        let resp = r#"{"pages":[{"id":"g0","title":"","scope":""}]}"#;
        let groups = vec![Group { id: "g0".into(), members: vec!["w/1".into()] }];
        let pages = name_groups(&groups, &canned(resp));
        assert!(!pages[0].title.is_empty());
    }
}
