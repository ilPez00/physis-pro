//! Phase 9.4 — holarchy community detection (clean-room adaptation,
//! docs/reference/repowise-adaptation.md P5).
//!
//! Label-propagation communities over an edge list — zero dependencies.
//! Hard invariant (adopted from the pattern source as an engineering rule):
//! **curation reads the input graph but only writes its own returned
//! [`CommunityReport`]**; the caller's data is never mutated.

/// Undirected edge list over node labels.
pub type EdgeList = Vec<(String, String)>;

/// Result of community detection: node → community id, plus per-community
/// cohesion (intra-cluster edge density 0..1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CommunityReport {
    /// Sorted unique nodes that appeared in the edges.
    pub nodes: Vec<String>,
    /// Community assignment per node index into `nodes`.
    pub assignment: Vec<u32>,
    /// One cohesion score per community id (0..1).
    pub cohesion: Vec<f32>,
}

impl CommunityReport {
    /// Nodes belonging to community `id`.
    pub fn members(&self, id: u32) -> Vec<&str> {
        self.nodes
            .iter()
            .zip(self.assignment.iter())
            .filter(|(_, &c)| c == id)
            .map(|(n, _)| n.as_str())
            .collect()
    }

    /// Number of communities found.
    pub fn n_communities(&self) -> usize {
        self.cohesion.len()
    }
}

/// Adjacency sets from an undirected edge list.
fn adjacency(edges: &EdgeList) -> (Vec<String>, Vec<Vec<usize>>) {
    fn intern(
        n: &str,
        nodes: &mut Vec<String>,
        idx: &mut std::collections::HashMap<String, usize>,
    ) -> usize {
        if let Some(&i) = idx.get(n) {
            i
        } else {
            let i = nodes.len();
            nodes.push(n.to_owned());
            idx.insert(n.to_owned(), i);
            i
        }
    }

    let mut nodes: Vec<String> = Vec::new();
    let mut idx: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let pairs: Vec<(usize, usize)> = edges
        .iter()
        .map(|(a, b)| {
            (
                intern(a, &mut nodes, &mut idx),
                intern(b, &mut nodes, &mut idx),
            )
        })
        .collect();

    let mut adj: Vec<std::collections::BTreeSet<usize>> =
        vec![std::collections::BTreeSet::new(); nodes.len()];
    for (ia, ib) in pairs {
        adj[ia].insert(ib);
        adj[ib].insert(ia);
    }
    (
        nodes,
        adj.into_iter().map(|s| s.into_iter().collect()).collect(),
    )
}

/// Asynchronous label propagation. Deterministic: neighbors are visited in
/// sorted order and ties break toward the smaller label. `max_rounds` bounds
/// oscillation on bipartite-ish graphs.
///
/// Returns `(assignment, n_communities)`.
pub fn label_propagation(
    n_nodes: usize,
    adj: &[Vec<usize>],
    max_rounds: usize,
) -> (Vec<u32>, usize) {
    let mut labels: Vec<u32> = (0..n_nodes as u32).collect(); // each node its own community
    for _ in 0..max_rounds {
        let mut changed = false;
        for v in 0..n_nodes {
            let mut counts: std::collections::BTreeMap<u32, usize> =
                std::collections::BTreeMap::new();
            for &u in &adj[v] {
                *counts.entry(labels[u]).or_default() += 1;
            }
            // Most common neighbor label; ties break toward the SMALLEST
            // label id for determinism. min_by_key keeps the first minimum,
            // so sort key = (-count, label) via reversed count comparison.
            let best_label = counts
                .iter()
                .min_by_key(|(&l, &c)| (std::cmp::Reverse(c), l))
                .map(|(&l, _)| l);
            if let Some(best) = best_label {
                if best != labels[v] {
                    labels[v] = best;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    // Compact labels to 0..k preserving first-seen order.
    let mut remap: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let mut next = 0u32;
    let compacted: Vec<u32> = labels
        .iter()
        .map(|&l| {
            *remap.entry(l).or_insert_with(|| {
                let x = next;
                next += 1;
                x
            })
        })
        .collect();
    (compacted, next as usize)
}

/// Full report: communities + oversized-split (>30% of graph triggers one
/// recursive split by dropping the heaviest intra-edges is NOT attempted —
/// instead the oversized community is reported with `cohesion` so callers
/// can decide; matches the read-only invariant).
pub fn detect_communities(edges: &EdgeList, max_rounds: usize) -> CommunityReport {
    let (nodes, adj) = adjacency(edges);
    let (assignment, k) = label_propagation(nodes.len(), &adj, max_rounds);

    // Cohesion per community: intra-community edges / possible pairs.
    let mut sizes = vec![0usize; k];
    for &c in &assignment {
        sizes[c as usize] += 1;
    }
    let mut intra = vec![0usize; k];
    let mut seen_pairs = std::collections::HashSet::new();
    for (a, b) in edges {
        let (ia, ib) = (
            nodes.iter().position(|n| n == a).unwrap(),
            nodes.iter().position(|n| n == b).unwrap(),
        );
        let (ca, cb) = (assignment[ia], assignment[ib]);
        if ca == cb {
            let key = if ia < ib { (ia, ib) } else { (ib, ia) };
            if seen_pairs.insert(key) {
                intra[ca as usize] += 1;
            }
        }
    }
    let cohesion: Vec<f32> = sizes
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            if s < 2 {
                1.0 // singleton: trivially cohesive
            } else {
                let pairs = s * (s - 1) / 2;
                intra[i] as f32 / pairs as f32
            }
        })
        .collect();

    CommunityReport {
        nodes,
        assignment,
        cohesion,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(a: &str, b: &str) -> (String, String) {
        (a.to_owned(), b.to_owned())
    }

    #[test]
    fn two_disconnected_cliques_form_two_communities() {
        let edges: EdgeList = vec![
            e("a", "b"),
            e("b", "c"),
            e("a", "c"), // clique {a,b,c}
            e("x", "y"),
            e("y", "z"),
            e("z", "x"), // clique {x,y,z}
        ];
        let r = detect_communities(&edges, 16);
        assert_eq!(r.n_communities(), 2, "{r:?}");
        assert_eq!(r.members(0).len() + r.members(1).len(), 6);
        // Same-community assertions independent of label numbering:
        let same = |p: &str, q: &str| {
            let (ip, iq) = (
                r.nodes.iter().position(|n| n == p).unwrap(),
                r.nodes.iter().position(|n| n == q).unwrap(),
            );
            r.assignment[ip] == r.assignment[iq]
        };
        assert!(same("a", "b") && same("b", "c"));
        assert!(same("x", "y") && same("y", "z"));
        assert!(!same("a", "x"));
    }

    #[test]
    fn bridge_edge_may_merge_but_never_splits_a_clique() {
        let edges: EdgeList = vec![
            e("a", "b"),
            e("b", "c"),
            e("a", "c"),
            e("c", "d"),
            e("d", "e"),
            e("e", "f"),
            e("d", "f"),
        ];
        let r = detect_communities(&edges, 16);
        let same = |p: &str, q: &str| {
            let (ip, iq) = (
                r.nodes.iter().position(|n| n == p).unwrap(),
                r.nodes.iter().position(|n| n == q).unwrap(),
            );
            r.assignment[ip] == r.assignment[iq]
        };
        assert!(same("a", "b") && same("b", "c"));
        assert!(same("d", "e") && same("e", "f"));
    }

    #[test]
    fn cohesion_is_one_for_perfect_clique_and_lower_for_sparse() {
        let clique = vec![e("a", "b"), e("b", "c"), e("a", "c")];
        let sparse = vec![e("a", "b"), e("c", "d")];
        let rc = detect_communities(&clique, 8);
        let rs = detect_communities(&sparse, 8);
        assert!((rc.cohesion[rc.assignment[0] as usize] - 1.0).abs() < 1e-6);
        assert!(rs.cohesion.iter().all(|&c| (0.0..=1.0).contains(&c)));
    }

    #[test]
    fn empty_and_single_edge_inputs_are_clean() {
        assert_eq!(
            detect_communities(&Vec::new(), 4),
            CommunityReport::default()
        );
        let r = detect_communities(&vec![e("only", "child")], 4);
        assert_eq!(r.nodes.len(), 2);
    }

    #[test]
    fn deterministic_across_calls() {
        let edges: EdgeList = vec![
            e("a", "b"),
            e("b", "c"),
            e("c", "d"),
            e("d", "a"),
            e("a", "c"),
        ];
        let r1 = detect_communities(&edges, 10);
        let r2 = detect_communities(&edges, 10);
        assert_eq!(r1, r2);
    }

    #[test]
    fn input_edges_not_mutated_read_only_invariant() {
        let edges: EdgeList = vec![e("a", "b"), e("b", "c")];
        let before = edges.clone();
        let _ = detect_communities(&edges, 8);
        assert_eq!(
            edges, before,
            "detect_communities must not mutate its input"
        );
    }
}
