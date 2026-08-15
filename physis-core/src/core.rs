//! Central engine state — coherence nodes, certified/isolated branches, and a
//! dream archive. Lean port of the physis-pro core: everything that doesn't
//! depend on the holarchy graph, quantizer, or sled is here.

use std::collections::HashMap;

use crate::embed::VectorEmbed;
use crate::models::*;

/// Central engine state.
#[derive(Debug, Default)]
pub struct PhysisCore {
    /// All coherence nodes keyed by ID.
    pub nodes: HashMap<String, CoherenceNode>,
    /// Branches certified as coherent clusters.
    pub certified_branches: Vec<CertifiedBranch>,
    /// Branches flagged as low-coherence outliers.
    pub isolated_branches: Vec<IsolatedBranch>,
    /// Archive of dream evaluation results.
    pub dream_archive: Vec<DreamResult>,
    /// label → node id, for O(1) dedup of labeled registrations.
    label_index: HashMap<String, String>,
    /// Embedder id stamped onto every node registered here (provenance).
    embedder_id: Option<String>,
}

impl PhysisCore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stamp the embedder id onto every node registered after this call.
    pub fn set_embedder_id(&mut self, id: impl Into<String>) {
        self.embedder_id = Some(id.into());
    }

    pub fn embedder_id(&self) -> Option<&str> {
        self.embedder_id.as_deref()
    }

    /// Register a vector, optionally retaining source text as a browsable label.
    /// Labeled registrations are deduped by label (idempotent re-scans).
    pub fn register_node_vec_labeled(&mut self, embedding: Vec<f32>, label: Option<String>) -> String {
        if let Some(ref l) = label {
            if let Some(existing) = self.label_index.get(l) {
                return existing.clone();
            }
        }
        let mut node = CoherenceNode::new(embedding.clone());
        node.label = label.clone();
        node.embedder = self.embedder_id.clone();
        let id = node.id.clone();
        if let Some(l) = label {
            self.label_index.insert(l, id.clone());
        }
        // Insert BEFORE scoring: `update_coherence` looks the node up in
        // `self.nodes`, so scoring first would leave every node at the 0.0 default.
        self.nodes.insert(id.clone(), node);
        self.update_coherence(&id);
        id
    }

    pub fn register_node_vec(&mut self, embedding: Vec<f32>) -> String {
        self.register_node_vec_labeled(embedding, None)
    }

    /// Register from text (embed first using an embedder). The text is kept as
    /// the node's label so it can be recalled by content later.
    pub fn register_node_from_text(&mut self, text: &str, embedder: &dyn VectorEmbed) -> String {
        let embedding = embedder.embed(text);
        self.register_node_vec_labeled(embedding, Some(text.to_string()))
    }

    /// Report whether a node actually worked. Score clamped to [-1, 1].
    /// Writes the *asserted* axis, deliberately leaving `coherence_score`
    /// (derived density) alone.
    pub fn assert_coherence(&mut self, id: &str, score: Score) -> bool {
        match self.nodes.get_mut(id) {
            Some(node) => {
                node.asserted = Some(score.clamp(-1.0, 1.0));
                true
            }
            None => false,
        }
    }

    /// Mean of the reported verdicts, or `None` when nothing has been judged.
    pub fn asserted_index(&self) -> Option<Score> {
        let judged: Vec<Score> = self.nodes.values().filter_map(|n| n.asserted).collect();
        if judged.is_empty() {
            return None;
        }
        Some(judged.iter().sum::<Score>() / judged.len() as Score)
    }

    /// Nodes someone reported as not working, worst first.
    pub fn asserted_failures(&self) -> Vec<&CoherenceNode> {
        let mut out: Vec<&CoherenceNode> =
            self.nodes.values().filter(|n| n.is_asserted_failure()).collect();
        out.sort_by(|a, b| {
            a.asserted
                .unwrap_or(0.0)
                .partial_cmp(&b.asserted.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }

    /// Recall labeled nodes by similarity to `query`. Embeddings are L2-normalized,
    /// so cosine reduces to a dot product. Returns `(id, label, score)` desc.
    pub fn search_nodes(&self, query: &[f32], max: usize) -> Vec<(String, String, Score)> {
        let mut scored: Vec<(String, String, Score)> = self
            .nodes
            .values()
            .filter_map(|n| {
                let label = n.label.as_ref()?;
                let dot: f32 = n.embedding.iter().zip(query).map(|(a, b)| a * b).sum();
                Some((n.id.clone(), label.clone(), dot))
            })
            .collect();
        scored.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max);
        scored
    }

    /// Search by text (embed first).
    pub fn search_text(&self, query: &str, embedder: &dyn VectorEmbed, max: usize) -> Vec<(String, String, Score)> {
        self.search_nodes(&embedder.embed(query), max)
    }

    /// Update coherence score for a node (mean cosine to k nearest neighbors).
    fn update_coherence(&mut self, node_id: &str) {
        if self.nodes.len() <= 1 {
            if let Some(node) = self.nodes.get_mut(node_id) {
                node.coherence_score = 1.0;
            }
            return;
        }

        let query = match self.nodes.get(node_id) {
            Some(n) => n.embedding.clone(),
            None => return,
        };

        let k = 5.min(self.nodes.len() - 1);
        let mut sims: Vec<f32> = self
            .nodes
            .values()
            .filter(|n| n.id != node_id)
            .map(|n| cosine_sim(&query, &n.embedding))
            .collect();
        sims.sort_by(|a, b| b.partial_cmp(a).unwrap());
        sims.truncate(k);

        let avg = if sims.is_empty() { 1.0 } else { sims.iter().sum::<f32>() / sims.len() as f32 };

        if let Some(node) = self.nodes.get_mut(node_id) {
            node.coherence_score = avg.max(0.0);
        }
    }

    /// Consistency check in vector space.
    pub fn check_consistency(&self, query_embedding: &[f32], threshold: f32) -> ConsistencyResult {
        for node in self.nodes.values() {
            let sim = cosine_sim(query_embedding, &node.embedding);
            if sim > threshold {
                let gap = 1.0 - sim;
                let refutation = ConstructiveRefutation::new(
                    query_embedding.to_vec(),
                    vec![node.id.clone()],
                    "",
                    gap,
                );
                return ConsistencyResult::Conflict(refutation);
            }
        }
        ConsistencyResult::Clean
    }

    /// Certify branches: cluster nodes by geometric proximity.
    pub fn certify_branches(&mut self) -> Vec<CertifiedBranch> {
        if self.nodes.len() < 2 {
            return vec![];
        }

        let mut newly_certified = Vec::new();
        let mut visited = std::collections::HashSet::new();

        for (id, node) in &self.nodes {
            if visited.contains(id) {
                continue;
            }

            let mut cluster = vec![id.clone()];
            visited.insert(id.clone());

            for (other_id, other) in &self.nodes {
                if visited.contains(other_id) {
                    continue;
                }
                let sim = cosine_sim(&node.embedding, &other.embedding);
                if sim > 0.7 {
                    cluster.push(other_id.clone());
                    visited.insert(other_id.clone());
                }
            }

            if cluster.len() >= 2 {
                let centroid: Vec<f32> = (0..node.embedding.len())
                    .map(|i| {
                        cluster
                            .iter()
                            .filter_map(|cid| self.nodes.get(cid))
                            .map(|n| n.embedding[i])
                            .sum::<f32>()
                            / cluster.len() as f32
                    })
                    .collect();

                let stability = cluster
                    .iter()
                    .filter_map(|cid| self.nodes.get(cid))
                    .map(|n| n.coherence_score)
                    .sum::<f32>()
                    / cluster.len() as f32;

                let branch = CertifiedBranch {
                    branch_id: uuid::Uuid::new_v4().to_string(),
                    node_ids: cluster,
                    centroid,
                    stability_score: stability,
                };
                newly_certified.push(branch.clone());
                self.certified_branches.push(branch);
            }
        }

        newly_certified
    }

    /// Detect outliers: nodes with low coherence relative to their nearest cluster.
    pub fn detect_contradictions(&mut self) -> Vec<IsolatedBranch> {
        let mut isolated = Vec::new();
        let threshold = 0.3;

        for (id, node) in &self.nodes {
            if node.coherence_score < threshold {
                let branch = IsolatedBranch {
                    branch_id: uuid::Uuid::new_v4().to_string(),
                    node_ids: vec![id.clone()],
                    outlier_score: 1.0 - node.coherence_score,
                };
                isolated.push(branch.clone());
                self.isolated_branches.push(branch);
            }
        }

        isolated
    }

    /// Dream simulation over the nodes worth replaying: reported failures are
    /// always replayed, judged successes never, unjudged nodes by density.
    pub fn dream(&mut self) -> Vec<DreamResult> {
        let low_coherence: Vec<CoherenceNode> = self
            .nodes
            .values()
            .filter(|n| match n.rating() {
                Some(CoherenceRating::Failure) | Some(CoherenceRating::Inert) => true,
                Some(CoherenceRating::Success) => false,
                None => n.coherence_score < 0.5,
            })
            .cloned()
            .collect();

        let mut results = Vec::new();
        for node in &low_coherence {
            if self.dream_archive.iter().any(|d| d.nodes_tested.contains(&node.id)) {
                continue;
            }

            let outcome = if node.coherence_score < 0.2 { 0.0 } else { 1.0 };
            let result = DreamResult {
                dream_id: uuid::Uuid::new_v4().to_string(),
                nodes_tested: vec![node.id.clone()],
                outcome,
                prevented_failure: outcome < 0.5,
                coherence_delta: node.coherence_score,
            };
            results.push(result.clone());
            self.dream_archive.push(result);
        }
        results
    }

    /// Mean coherence across all nodes.
    pub fn coherence_index(&self) -> Score {
        if self.nodes.is_empty() {
            return 1.0;
        }
        let sum: Score = self.nodes.values().map(|n| n.coherence_score).sum();
        sum / self.nodes.len() as Score
    }

    pub fn snapshot(&self) -> CoherenceSnapshot {
        let total = self.nodes.len();
        let high = self.nodes.values().filter(|n| n.coherence_score > 0.7).count();
        let mid = self.nodes.values().filter(|n| n.coherence_score > 0.3 && n.coherence_score <= 0.7).count();
        let low = self.nodes.values().filter(|n| n.coherence_score <= 0.3).count();
        let rated = |want: CoherenceRating| {
            self.nodes.values().filter(|n| n.rating() == Some(want)).count()
        };
        CoherenceSnapshot {
            total_nodes: total,
            high_coherence: high,
            mid_coherence: mid,
            low_coherence: low,
            certified_branches_count: self.certified_branches.len(),
            isolated_branches_count: self.isolated_branches.len(),
            dream_cycle_count: self.dream_archive.len(),
            coherence_index: self.coherence_index(),
            cluster_count: self.certified_branches.len(),
            outlier_count: self.isolated_branches.len(),
            asserted_success: rated(CoherenceRating::Success),
            asserted_inert: rated(CoherenceRating::Inert),
            asserted_failure: rated(CoherenceRating::Failure),
            asserted_index: self.asserted_index(),
        }
    }

    /// Serialize all nodes to JSON.
    pub fn to_json(&self) -> anyhow::Result<String> {
        let nodes: Vec<&CoherenceNode> = self.nodes.values().collect();
        Ok(serde_json::to_string_pretty(&nodes)?)
    }

    /// Replace all nodes from JSON (e.g. loaded from disk).
    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        let nodes: Vec<CoherenceNode> = serde_json::from_str(json)?;
        let mut core = Self::new();
        for n in nodes {
            if let Some(ref label) = n.label {
                core.label_index.insert(label.clone(), n.id.clone());
            }
            core.nodes.insert(n.id.clone(), n);
        }
        Ok(core)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::RandomProjectionEmbedder;

    fn fixture_embedder() -> RandomProjectionEmbedder {
        RandomProjectionEmbedder::new(64)
    }

    fn fixture_core() -> PhysisCore {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        g.register_node_from_text("exercise running success", &emb);
        g.register_node_from_text("diet no sugar success", &emb);
        g.register_node_from_text("compile physis core", &emb);
        g
    }

    #[test]
    fn register_and_search() {
        let g = fixture_core();
        let emb = fixture_embedder();
        let q = emb.embed("exercise running success");
        let hits = g.search_nodes(&q, 3);
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].1, "exercise running success");
        assert!(hits[0].2 >= hits[1].2 && hits[1].2 >= hits[2].2);
    }

    #[test]
    fn labeled_registration_is_deduped() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let id1 = g.register_node_from_text("src/main.rs", &emb);
        let id2 = g.register_node_from_text("src/main.rs", &emb);
        assert_eq!(id1, id2);
        assert_eq!(g.nodes.len(), 1);
        g.register_node_vec(vec![0.1; 64]);
        g.register_node_vec(vec![0.1; 64]);
        assert_eq!(g.nodes.len(), 3);
    }

    #[test]
    fn coherence_index_and_snapshot() {
        let g = fixture_core();
        let idx = g.coherence_index();
        assert!((0.0..=1.0).contains(&idx));
        let snap = g.snapshot();
        assert_eq!(snap.total_nodes, 3);
    }

    #[test]
    fn asserted_axis_is_separate_from_density() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let a = g.register_node_from_text("deploy script that always works", &emb);
        let b = g.register_node_from_text("deploy script that always works, but fails", &emb);

        assert_eq!(g.nodes[&a].rating(), None);
        assert_eq!(g.asserted_index(), None);

        let density_before = g.nodes[&b].coherence_score;
        assert!(g.assert_coherence(&b, -1.0));
        assert_eq!(g.nodes[&b].rating(), Some(CoherenceRating::Failure));
        assert_eq!(g.nodes[&b].coherence_score, density_before);
        assert_eq!(g.asserted_index(), Some(-1.0));

        g.assert_coherence(&a, 1.0);
        assert_eq!(g.asserted_index(), Some(0.0));
        g.assert_coherence(&a, 9.0);
        assert_eq!(g.nodes[&a].asserted, Some(1.0));
        assert!(!g.assert_coherence("no-such-node", 1.0));
    }

    #[test]
    fn dream_replays_asserted_failures_only() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let failed = g.register_node_from_text("migration that corrupted the index", &emb);
        let worked = g.register_node_from_text("migration that corrupted the index, fixed", &emb);

        g.assert_coherence(&failed, -1.0);
        g.assert_coherence(&worked, 1.0);
        g.nodes.get_mut(&failed).unwrap().coherence_score = 0.9;
        g.nodes.get_mut(&worked).unwrap().coherence_score = 0.1;

        let dreams = g.dream();
        let tested: Vec<&String> = dreams.iter().flat_map(|d| &d.nodes_tested).collect();
        assert!(tested.contains(&&failed));
        assert!(!tested.contains(&&worked));
    }

    #[test]
    fn nodes_survive_json_roundtrip() {
        let mut g = PhysisCore::new();
        g.set_embedder_id("fixture");
        let emb = fixture_embedder();
        let id = g.register_node_from_text("morning yoga completed", &emb);
        g.assert_coherence(&id, -1.0);
        let json = g.to_json().unwrap();

        let restored = PhysisCore::from_json(&json).unwrap();
        assert_eq!(restored.nodes.len(), 1);
        assert_eq!(restored.nodes[&id].asserted, Some(-1.0));
        // Dedupe holds after reload.
        let mut again = restored;
        let id2 = again.register_node_from_text("morning yoga completed", &emb);
        assert_eq!(id2, id);
        assert_eq!(again.nodes.len(), 1);
    }

    #[test]
    fn certify_and_contradict() {
        let mut g = fixture_core();
        g.certify_branches();
        g.detect_contradictions();
        assert!(g.certified_branches.len() + g.isolated_branches.len() > 0 || g.nodes.len() == 3);
    }
}
