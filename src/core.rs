//! Central engine state — coherence graph nodes, certified/isolated branches,
//! dream archive, and PQ-accelerated nearest-neighbour search.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::models::*;
use crate::trie::DynamicVectorTrie;
use crate::quantize::ProductQuantizer;
use crate::graph::{HolonicGraph, NodePayload, RawNodeKey};

/// Bridges the two identity spaces: a coherence node's string id and its
/// holarchy `RawNodeKey` (a packed slotmap key). Without this, holarchy edges
/// (keyed by `RawNodeKey`) can't be mapped back to the labeled wiki nodes, so
/// edges can't be exported (Spanner) or browsed by label (TUI graph-walk).
/// Maps store `RawNodeKey.data` (u64) since `RawNodeKey` isn't `Hash`.
#[derive(Debug, Default)]
pub struct IdBridge {
    coh_to_holon: HashMap<String, u64>,
    holon_to_coh: HashMap<u64, String>,
}

impl IdBridge {
    pub fn link(&mut self, coh_id: &str, raw: RawNodeKey) {
        self.coh_to_holon.insert(coh_id.to_string(), raw.data);
        self.holon_to_coh.insert(raw.data, coh_id.to_string());
    }
    pub fn holon_for(&self, coh_id: &str) -> Option<RawNodeKey> {
        self.coh_to_holon.get(coh_id).map(|&data| RawNodeKey { data })
    }
    pub fn coh_for(&self, raw: RawNodeKey) -> Option<&str> {
        self.holon_to_coh.get(&raw.data).map(String::as_str)
    }
    pub fn len(&self) -> usize { self.coh_to_holon.len() }
    pub fn is_empty(&self) -> bool { self.coh_to_holon.is_empty() }
}

/// Central engine state — coherence graph, certified/isolated branches, dream archive,
/// and optional PQ-accelerated nearest-neighbour search.
#[derive(Debug)]
pub struct PhysisCore {
    /// Holonic decentralized graph (Phase I).
    pub holarchy: HolonicGraph,
    /// All coherence nodes keyed by ID.
    pub nodes: HashMap<String, CoherenceNode>,
    /// Dynamic vector trie for semantic search.
    pub wiki: DynamicVectorTrie,
    /// Branches that have been certified as coherent clusters.
    pub certified_branches: Vec<CertifiedBranch>,
    /// Branches flagged as low-coherence outliers.
    pub isolated_branches: Vec<IsolatedBranch>,
    /// Archive of dream evaluation results.
    pub dream_archive: Vec<DreamResult>,
    /// Optional product quantizer for ANN search.
    pub quantizer: Option<ProductQuantizer>,
    /// PQ-encoded nodes: node_id → codes.
    pub encoded_nodes: Vec<(String, Vec<u8>)>,
    /// Dimension used when training the quantizer.
    pub quantizer_dim: usize,
    /// label → node id, for O(1) dedup of labeled registrations (e.g. re-scanning
    /// the same dir must not create duplicate nodes). Only labeled nodes are
    /// indexed; unlabeled `register_node_vec` calls are never deduped.
    label_index: HashMap<String, String>,
    /// coherence id ↔ holarchy RawNodeKey. See `IdBridge`.
    pub bridge: IdBridge,
    /// When true, every newly registered node is auto-bridged into the holarchy
    /// (machines share one graph without manual `bridge_node`). Default false.
    pub auto_bridge: bool,
}

impl PhysisCore {
    /// Create an empty engine core with no nodes, no quantizer.
    pub fn new() -> Self {
        Self {
            holarchy: HolonicGraph::new(),
            nodes: HashMap::new(),
            wiki: DynamicVectorTrie::new(),
            certified_branches: vec![],
            isolated_branches: vec![],
            dream_archive: vec![],
            quantizer: None,
            encoded_nodes: vec![],
            quantizer_dim: 0,
            label_index: HashMap::new(),
            bridge: IdBridge::default(),
            auto_bridge: false,
        }
    }

    /// Assert a relation between two coherence nodes. Both are bridged into the
    /// holarchy (idempotent), then a holarchy edge is added between their keys.
    /// This is how a machine's reducer composes a graph — not just nodes, but
    /// the relations between them. Returns false if either id is unknown.
    pub fn add_edge_between(
        &mut self,
        coh_a: &str,
        coh_b: &str,
        weight: f32,
        semantic_type: u32,
    ) -> bool {
        let (Some(a), Some(b)) = (self.bridge_node(coh_a), self.bridge_node(coh_b)) else {
            return false;
        };
        self.holarchy.edges.push(crate::graph::Edge {
            source: a,
            target: b,
            weight,
            semantic_type,
        });
        true
    }

    /// Ensure the coherence node `coh_id` has a corresponding holarchy node and
    /// is linked to it; returns the holarchy key. Idempotent — a second call
    /// returns the existing key without adding a duplicate holarchy node. This
    /// is the one place the two identity spaces get joined, so edges and durable
    /// rows can share a stable id. Returns `None` if `coh_id` is unknown.
    pub fn bridge_node(&mut self, coh_id: &str) -> Option<RawNodeKey> {
        if let Some(raw) = self.bridge.holon_for(coh_id) {
            return Some(raw);
        }
        let embedding = self.nodes.get(coh_id)?.embedding.clone();
        let payload = NodePayload {
            activation_energy: 1.0,
            decay_metrics: 0.0,
            prune_flag: 0,
            compress_flag: 0,
            _padding: [0; 2],
        };
        let key = self.holarchy.add_node(payload, embedding);
        let raw = RawNodeKey::from(key);
        self.bridge.link(coh_id, raw);
        Some(raw)
    }

    /// Holarchy edges mapped back to coherence ids via the bridge. Endpoints not
    /// yet bridged resolve to `None`. Lets clients export edges with stable ids
    /// (Spanner) or render a label-walkable graph instead of opaque keys.
    pub fn resolved_edges(&self) -> Vec<(Option<String>, Option<String>, f32)> {
        self.holarchy
            .edges
            .iter()
            .map(|e| {
                (
                    self.bridge.coh_for(e.source).map(str::to_string),
                    self.bridge.coh_for(e.target).map(str::to_string),
                    e.weight,
                )
            })
            .collect()
    }

    /// Create an engine core pre-populated with an existing wiki trie.
    pub fn with_wiki(wiki: DynamicVectorTrie) -> Self {
        let mut g = Self::new();
        g.wiki = wiki;
        g
    }

    /// Enable PQ compression for ANN search. Call once with the vector dimension.
    pub fn enable_quantizer(&mut self, dim: usize) {
        self.quantizer = Some(ProductQuantizer::new(dim));
        self.quantizer_dim = dim;
        self.encoded_nodes.clear();
    }

    /// Register a coherence node from a vector.
    pub fn register_node_vec(&mut self, embedding: Vec<f32>) -> String {
        self.register_node_vec_labeled(embedding, None)
    }

    /// Register a vector, optionally retaining the source text as a browsable label.
    /// Labeled registrations are deduped: re-registering the same label returns the
    /// existing node id instead of creating a duplicate (so re-scanning a dir is
    /// idempotent). Unlabeled registrations are always inserted.
    pub fn register_node_vec_labeled(&mut self, embedding: Vec<f32>, label: Option<String>) -> String {
        if let Some(ref l) = label {
            if let Some(existing) = self.label_index.get(l) {
                return existing.clone();
            }
        }
        let mut node = CoherenceNode::new(embedding.clone());
        node.label = label.clone();
        let id = node.id.clone();
        if let Some(l) = label {
            self.label_index.insert(l, id.clone());
        }
        // Train PQ quantizer if enabled
        if let Some(ref mut pq) = self.quantizer {
            pq.train_one(&embedding);
            let codes = pq.quantize(&embedding);
            self.encoded_nodes.push((id.clone(), codes));
        }
        self.update_coherence(&id);
        self.nodes.insert(id.clone(), node);
        // Auto-bridge: when enabled, every new node is joined into the holarchy
        // so machines populate one shared graph without calling bridge_node by
        // hand. Off by default (back-compat with the separate ingest pipeline).
        if self.auto_bridge {
            self.bridge_node(&id);
        }
        id
    }

    /// Register from text (embed first using an embedder). The text is kept as
    /// the node's label so it can be recalled by content later.
    pub fn register_node_from_text(&mut self, text: &str, embedder: &dyn crate::embed::VectorEmbed) -> String {
        let embedding = embedder.embed(text);
        self.register_node_vec_labeled(embedding, Some(text.to_string()))
    }

    /// Recall labeled coherence nodes by similarity to `query`. Embeddings are
    /// L2-normalized, so cosine reduces to a dot product. Returns up to `max`
    /// matches as `(id, label, score)`, highest score first. Unlabeled nodes are
    /// skipped. This is how a machine recalls what the shared graph already holds
    /// — across every domain that registered into it.
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
        let mut sims: Vec<f32> = self.nodes.values()
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

    /// High-level holarchy orchestration.
    pub fn tick_holarchy(
        &mut self,
        onnx: &crate::ai::onnx_worker::OnnxHolon,
        rachmaninov: &mut crate::rachmaninov::RachmaninovHolon
    ) {
        // 1. Dreaming/Pruning
        self.holarchy.dream_cycle(0.85);

        // 2. GNN Context Extraction
        let (reply_tx, reply_rx) = crossbeam_channel::unbounded();
        
        let mut nodes_for_gnn = Vec::new();
        for (key, payload) in self.holarchy.nodes.iter() {
            if let Some(emb) = self.holarchy.embeddings.get(key) {
                nodes_for_gnn.push((crate::graph::RawNodeKey::from(key).data, emb.clone(), *payload));
            }
        }
        
        onnx.send(crate::ai::onnx_worker::OnnxRequest::GnnContext {
            nodes: nodes_for_gnn,
            edges: self.holarchy.edges.clone(),
            reply_to: reply_tx,
        });

        if let Ok(crate::ai::onnx_worker::OnnxResponse::GnnResult { context_vector }) = reply_rx.recv() {
            // 3. Feed to Rachmaninov
            rachmaninov.tick(&context_vector);
        }
    }

    /// Register a behavioural vector (convenience: same as register_node_vec).
    pub fn register_behavioural_vector(&mut self, embedding: Vec<f32>) -> String {
        self.register_node_vec(embedding)
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

    /// filter_context: input text → vector. No more qualia text output.
    pub fn filter_context(
        &self,
        raw_input: &str,
        embedder: &dyn crate::embed::VectorEmbed,
    ) -> FilteredContext {
        let embedding = embedder.embed(raw_input);
        let consistency = self.check_consistency(&embedding, 0.85);
        let valid = matches!(consistency, ConsistencyResult::Clean);
        FilteredContext {
            embedding,
            valid,
            token_estimate: raw_input.split_whitespace().count(),
        }
    }

    /// Certify branches: cluster nodes by geometric proximity.
    pub fn certify_branches(&mut self) -> Vec<CertifiedBranch> {
        if self.nodes.len() < 2 {
            return vec![];
        }

        let mut newly_certified = Vec::new();
        let mut visited = std::collections::HashSet::new();

        for (id, node) in &self.nodes {
            if visited.contains(id) { continue; }

            let mut cluster = vec![id.clone()];
            visited.insert(id.clone());

            for (other_id, other) in &self.nodes {
                if visited.contains(other_id) { continue; }
                let sim = cosine_sim(&node.embedding, &other.embedding);
                if sim > 0.7 {
                    cluster.push(other_id.clone());
                    visited.insert(other_id.clone());
                }
            }

            if cluster.len() >= 2 {
                let centroid: Vec<f32> = (0..node.embedding.len())
                    .map(|i| cluster.iter()
                        .filter_map(|cid| self.nodes.get(cid))
                        .map(|n| n.embedding[i])
                        .sum::<f32>() / cluster.len() as f32)
                    .collect();

                let stability = cluster.iter()
                    .filter_map(|cid| self.nodes.get(cid))
                    .map(|n| n.coherence_score)
                    .sum::<f32>() / cluster.len() as f32;

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

    /// PQ-based approximate nearest neighbors.
    /// Returns (node_id, approximate_squared_distance) sorted ascending.
    /// Falls back to empty vec if quantizer is not enabled or not trained.
    pub fn pq_find_neighbors(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        let pq = match &self.quantizer {
            Some(pq) if pq.is_trained() && !self.encoded_nodes.is_empty() => pq,
            _ => return vec![],
        };
        let mut results = pq.adc_search(query, &self.encoded_nodes);
        results.truncate(k);
        results
    }

    /// Compress logs into dense causal rules (unchanged).
    pub fn compress_logs(&self, raw_logs: &[String]) -> String {
        let mut rules = Vec::new();
        for log in raw_logs {
            let cleaned: Vec<&str> = log.split_whitespace()
                .filter(|w| w.len() > 2)
                .collect();
            if !cleaned.is_empty() {
                rules.push(cleaned.join(" "));
            }
        }
        let mut seen = std::collections::HashSet::new();
        rules.retain(|r| seen.insert(r.clone()));
        rules.truncate(200);
        rules.join("│")
    }

    /// Dream simulation on low-coherence nodes.
    pub fn dream(&mut self) -> Vec<DreamResult> {
        let low_coherence: Vec<CoherenceNode> = self.nodes.values()
            .filter(|n| n.coherence_score < 0.5)
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
        if self.nodes.is_empty() { return 1.0; }
        let sum: Score = self.nodes.values().map(|n| n.coherence_score).sum();
        sum / self.nodes.len() as Score
    }

    pub fn snapshot(&self) -> CoherenceSnapshot {
        let total = self.nodes.len();
        let high = self.nodes.values().filter(|n| n.coherence_score > 0.7).count();
        let mid = self.nodes.values().filter(|n| n.coherence_score > 0.3 && n.coherence_score <= 0.7).count();
        let low = self.nodes.values().filter(|n| n.coherence_score <= 0.3).count();
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
        }
    }
}

impl Default for PhysisCore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
/// Result of a consistency check: no conflict, or a constructive refutation.
pub enum ConsistencyResult {
    /// No conflicting nodes found above the threshold.
    Clean,
    /// A conflict was detected with a suggested refutation.
    Conflict(ConstructiveRefutation),
}

/// Snapshot of coherence health across the node graph, broken into bands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceSnapshot {
    pub total_nodes: usize,
    pub high_coherence: usize,
    pub mid_coherence: usize,
    pub low_coherence: usize,
    pub certified_branches_count: usize,
    pub isolated_branches_count: usize,
    pub dream_cycle_count: usize,
    pub coherence_index: Score,
    pub cluster_count: usize,
    pub outlier_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::{RandomProjectionEmbedder, VectorEmbed};

    fn fixture_embedder() -> RandomProjectionEmbedder {
        RandomProjectionEmbedder::new(32)
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
    fn test_filter_context() {
        let g = fixture_core();
        let emb = fixture_embedder();
        let result = g.filter_context("swimming is good exercise", &emb);
        assert!(result.valid);
        assert_eq!(result.embedding.len(), 32);
    }

    #[test]
    fn test_register_node_returns_id() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let id = g.register_node_from_text("test node", &emb);
        assert!(g.nodes.contains_key(&id));
    }

    #[test]
    fn test_search_nodes_ranks_by_similarity() {
        let g = fixture_core();
        let emb = fixture_embedder();
        let q = emb.embed("exercise running success");
        let hits = g.search_nodes(&q, 3);
        assert_eq!(hits.len(), 3, "all labeled nodes returned, capped at max");
        // Exact-text node ranks first (cosine 1.0 to itself).
        assert_eq!(hits[0].1, "exercise running success");
        // Sorted descending by score.
        assert!(hits[0].2 >= hits[1].2 && hits[1].2 >= hits[2].2);
        // max caps the result count.
        assert_eq!(g.search_nodes(&q, 1).len(), 1);
    }

    #[test]
    fn test_labeled_registration_is_deduped() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        // Same label twice (e.g. re-scanning the same file) → one node, same id.
        let id1 = g.register_node_from_text("src/main.rs", &emb);
        let id2 = g.register_node_from_text("src/main.rs", &emb);
        assert_eq!(id1, id2);
        assert_eq!(g.nodes.len(), 1);
        // A different label adds a distinct node.
        g.register_node_from_text("src/lib.rs", &emb);
        assert_eq!(g.nodes.len(), 2);
        // Unlabeled registrations are never deduped.
        g.register_node_vec(vec![0.1; 32]);
        g.register_node_vec(vec![0.1; 32]);
        assert_eq!(g.nodes.len(), 4);
    }

    #[test]
    fn test_id_bridge_links_and_resolves_edges() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let a = g.register_node_from_text("alpha", &emb);
        let b = g.register_node_from_text("beta", &emb);

        let ka = g.bridge_node(&a).expect("alpha bridged");
        let kb = g.bridge_node(&b).expect("beta bridged");
        // Idempotent: re-bridging returns the same holarchy key, no new node.
        assert_eq!(g.bridge_node(&a).unwrap().data, ka.data);
        assert_eq!(g.bridge.len(), 2);
        assert_eq!(g.holarchy.nodes.len(), 2);
        assert!(g.bridge_node("does-not-exist").is_none());

        // A holarchy edge between two bridged nodes resolves back to coherence ids.
        g.holarchy.edges.push(crate::graph::Edge {
            source: ka, target: kb, weight: 0.7, semantic_type: 0,
        });
        let resolved = g.resolved_edges();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0.as_deref(), Some(a.as_str()));
        assert_eq!(resolved[0].1.as_deref(), Some(b.as_str()));
        assert_eq!(resolved[0].2, 0.7);
    }

    #[test]
    fn test_auto_bridge_and_edge_write() {
        let mut g = PhysisCore::new();
        g.auto_bridge = true;
        let emb = fixture_embedder();
        let a = g.register_node_from_text("alpha", &emb);
        let b = g.register_node_from_text("beta", &emb);
        // auto-bridged at registration time
        assert!(g.bridge.holon_for(&a).is_some());
        assert!(g.bridge.holon_for(&b).is_some());
        assert_eq!(g.holarchy.nodes.len(), 2);

        // edge-write API composes a relation
        assert!(g.add_edge_between(&a, &b, 0.5, 1));
        assert!(!g.add_edge_between(&a, "missing", 0.5, 1));
        let resolved = g.resolved_edges();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0.as_deref(), Some(a.as_str()));
        assert_eq!(resolved[0].1.as_deref(), Some(b.as_str()));
    }

    #[test]
    fn test_coherence_index_empty() {
        let g = PhysisCore::new();
        assert!((g.coherence_index() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_coherence_index_with_nodes() {
        let g = fixture_core();
        let idx = g.coherence_index();
        assert!(idx >= 0.0);
        assert!(idx <= 1.0);
    }

    #[test]
    fn test_snapshot() {
        let g = fixture_core();
        let snap = g.snapshot();
        assert_eq!(snap.total_nodes, 3);
        assert!(snap.high_coherence > 0 || snap.mid_coherence > 0 || snap.low_coherence > 0);
    }

    #[test]
    fn test_detect_contradictions() {
        let mut g = fixture_core();
        g.detect_contradictions();
    }

    #[test]
    fn test_compress_logs() {
        let g = fixture_core();
        let logs = vec![
            "went for run today felt good".to_string(),
            "studied three hours".to_string(),
        ];
        let compressed = g.compress_logs(&logs);
        assert!(!compressed.is_empty());
        assert!(compressed.contains("went"));
    }

    #[test]
    fn test_register_behavioural_vector() {
        let mut g = PhysisCore::new();
        let emb = fixture_embedder();
        let vec = emb.embed("morning yoga completed");
        let id = g.register_behavioural_vector(vec);
        assert!(g.nodes.contains_key(&id));
    }

    #[test]
    fn test_certify_branches() {
        let mut g = fixture_core();
        let certified = g.certify_branches();
        // Should create clusters of similar nodes
        assert!(certified.len() <= 3);
    }

    #[test]
    fn test_pq_integration() {
        let mut g = PhysisCore::new();
        g.enable_quantizer(32);
        let emb = fixture_embedder();
        g.register_node_from_text("exercise running fitness", &emb);
        g.register_node_from_text("diet nutrition health", &emb);
        g.register_node_from_text("coding rust programming", &emb);
        assert!(g.quantizer.as_ref().unwrap().is_trained());
        assert_eq!(g.encoded_nodes.len(), 3);
        let query = emb.embed("morning run");
        let pq_results = g.pq_find_neighbors(&query, 2);
        assert_eq!(pq_results.len(), 2);
        assert!(pq_results[0].1 <= pq_results[1].1); // sorted by distance asc
    }
}
