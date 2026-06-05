//! Vector-space nearest-neighbor reconstruction and LLM-assisted interpretation.
use std::cmp::Ordering;

use serde::Serialize;

use crate::core::PhysisCore;
use crate::embed::VectorEmbed;
use crate::models::{cosine_sim, Goal};

/// A single vector-space neighbor with cosine similarity and coherence score.
#[derive(Debug, Clone, Serialize)]
pub struct Neighbor {
    pub id: String,
    pub embedding: Vec<f32>,
    pub cosine_similarity: f32,
    pub coherence_score: f32,
    /// Source text the vector came from, when retained — lets clients show
    /// real content instead of an opaque id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// The result of a nearest-neighbor query: query embedding plus ranked neighbors.
#[derive(Debug, Clone, Serialize)]
pub struct Reconstruction {
    pub query_embedding: Vec<f32>,
    pub neighbors: Vec<Neighbor>,
    pub count: usize,
}

impl Reconstruction {
    /// Returns the first N neighbors (or fewer if insufficient results).
    pub fn top(&self, n: usize) -> &[Neighbor] {
        let end = self.neighbors.len().min(n);
        &self.neighbors[..end]
    }
}

/// Finds the k nearest neighbors to a query vector using PQ-accelerated or brute-force search.
pub fn find_neighbors(
    query: &[f32],
    core: &PhysisCore,
    k: usize,
) -> Vec<Neighbor> {
    // Try PQ-accelerated search first
    let pq_results = core.pq_find_neighbors(query, k);
    if !pq_results.is_empty() {
        return pq_results
            .iter()
            .filter_map(|(id, _)| {
                core.nodes.get(id).map(|n| Neighbor {
                    id: n.id.clone(),
                    embedding: n.embedding.clone(),
                    cosine_similarity: cosine_sim(query, &n.embedding),
                    coherence_score: n.coherence_score,
                    label: n.label.clone(),
                })
            })
            .collect();
    }

    // Fall back to brute-force
    let mut scored: Vec<Neighbor> = core
        .nodes
        .values()
        .map(|n| Neighbor {
            id: n.id.clone(),
            embedding: n.embedding.clone(),
            cosine_similarity: cosine_sim(query, &n.embedding),
            coherence_score: n.coherence_score,
            label: n.label.clone(),
        })
        .collect();

    scored.sort_by(|a, b| {
        b.cosine_similarity
            .partial_cmp(&a.cosine_similarity)
            .unwrap_or(Ordering::Equal)
    });

    scored.truncate(k);
    scored
}

/// Returns the k closest goals to a query vector, sorted by cosine similarity.
pub fn find_nearest_goals(
    query: &[f32],
    goals: &[Goal],
    k: usize,
) -> Vec<Neighbor> {
    let mut scored: Vec<Neighbor> = goals
        .iter()
        .map(|g| Neighbor {
            id: g.id.clone(),
            embedding: g.embedding.clone(),
            cosine_similarity: cosine_sim(query, &g.embedding),
            coherence_score: g.progress,
            label: Some(g.domain_name.clone()),
        })
        .collect();

    scored.sort_by(|a, b| {
        b.cosine_similarity
            .partial_cmp(&a.cosine_similarity)
            .unwrap_or(Ordering::Equal)
    });

    scored.truncate(k);
    scored
}

/// Embeds a text query and finds nearest neighbors from the core index.
pub fn reconstruct(
    input: &str,
    embedder: &dyn VectorEmbed,
    core: &PhysisCore,
    k: usize,
) -> Reconstruction {
    let query_embedding = embedder.embed(input);
    let neighbors = find_neighbors(&query_embedding, core, k);
    let count = neighbors.len();
    Reconstruction {
        query_embedding,
        neighbors,
        count,
    }
}

/// Embeds a text query and finds nearest neighbors from a slice of Goals.
pub fn reconstruct_from_goals(
    input: &str,
    embedder: &dyn VectorEmbed,
    goals: &[Goal],
    k: usize,
) -> Reconstruction {
    let query_embedding = embedder.embed(input);
    let neighbors = find_nearest_goals(&query_embedding, goals, k);
    let count = neighbors.len();
    Reconstruction {
        query_embedding,
        neighbors,
        count,
    }
}

/// Embeds a query, finds neighbors, and uses an LLM cascade to produce a semantic summary.
pub async fn reconstruct_with_llm(
    input: &str,
    embedder: &dyn VectorEmbed,
    core: &PhysisCore,
    cascade: &crate::ai::provider::ProviderCascade,
    k: usize,
) -> (Reconstruction, Option<String>) {
    use crate::ai::provider::{Content, Message, RouteHint};

    let rec = reconstruct(input, embedder, core, k);

    let prompt = build_reconstruction_prompt(input, &rec);
    let messages = vec![
        Message {
            role: "system".into(),
            content: Content::Text(
                "You are Physis Vector Reconstruction Engine. \
                 Given a user query and its nearest neighbors in vector space, \
                 generate a concise semantic summary (2-3 sentences) describing \
                 what the vector neighborhood represents. \
                 Only output the summary, no preamble."
                    .into(),
            ),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
        Message {
            role: "user".into(),
            content: Content::Text(prompt),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
    ];

    match cascade.complete(&messages, None, &RouteHint { task_type: "fast".into(), has_image: false }, Some(256)).await {
        Ok((resp, _provider)) => (rec, resp.content),
        Err(_) => (rec, None),
    }
}

fn build_reconstruction_prompt(query: &str, rec: &Reconstruction) -> String {
    let mut lines = vec![
        format!("Query: \"{}\"", query),
        format!("Found {} neighbor vectors:", rec.count),
        String::new(),
    ];

    for (i, n) in rec.neighbors.iter().enumerate() {
        let emb_preview: String = n
            .embedding
            .iter()
            .take(4)
            .map(|v| format!("{:.4}", v))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!(
            "Neighbor {}: id={}, similarity={:.4}, coherence={:.4}, vec=[{}...]",
            i + 1,
            &n.id[..n.id.len().min(8)],
            n.cosine_similarity,
            n.coherence_score,
            emb_preview
        ));
    }

    lines.push(String::new());
    lines.push("Generate a concise semantic reconstruction of this vector neighborhood.".into());

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::PhysisCore;
    use crate::embed::RandomProjectionEmbedder;

    fn fixture_core() -> PhysisCore {
        let mut g = PhysisCore::new();
        let emb = RandomProjectionEmbedder::new(32);
        g.register_node_from_text("exercise running fitness", &emb);
        g.register_node_from_text("diet nutrition health", &emb);
        g.register_node_from_text("coding rust programming", &emb);
        g
    }

    #[test]
    fn test_reconstruct_returns_neighbors() {
        let core = fixture_core();
        let embedder = RandomProjectionEmbedder::new(32);
        let rec = reconstruct("morning run", &embedder, &core, 3);
        assert_eq!(rec.neighbors.len(), 3);
        assert!(!rec.query_embedding.is_empty());
    }

    #[test]
    fn test_reconstruct_fitness_query() {
        let core = fixture_core();
        let embedder = RandomProjectionEmbedder::new(32);
        let rec = reconstruct("going for a swim", &embedder, &core, 2);
        assert_eq!(rec.neighbors.len(), 2);
    }

    #[test]
    fn test_reconstruct_empty_core() {
        let core = PhysisCore::new();
        let embedder = RandomProjectionEmbedder::new(32);
        let rec = reconstruct("anything", &embedder, &core, 5);
        assert!(rec.neighbors.is_empty());
    }

    #[test]
    fn test_find_neighbors_returns_sorted() {
        let mut core = PhysisCore::new();
        let embedder = RandomProjectionEmbedder::new(32);
        core.register_node_from_text("aaa", &embedder);
        core.register_node_from_text("zzz", &embedder);
        core.register_node_from_text("mmm", &embedder);

        let query = embedder.embed("test query for sorting");
        let neighbors = find_neighbors(&query, &core, 3);
        assert_eq!(neighbors.len(), 3);
        // Results should be sorted descending by similarity
        for i in 1..neighbors.len() {
            assert!(
                neighbors[i - 1].cosine_similarity >= neighbors[i].cosine_similarity,
                "neighbors should be sorted descending by cosine_similarity"
            );
        }
    }

    #[test]
    fn test_top_n() {
        let core = fixture_core();
        let embedder = RandomProjectionEmbedder::new(32);
        let rec = reconstruct("running", &embedder, &core, 3);
        assert_eq!(rec.top(1).len(), 1);
    }

    #[test]
    fn test_build_reconstruction_prompt() {
        let core = fixture_core();
        let embedder = RandomProjectionEmbedder::new(32);
        let rec = reconstruct("test query", &embedder, &core, 2);
        let prompt = build_reconstruction_prompt("test query", &rec);
        assert!(prompt.contains("test query"));
        assert!(prompt.contains("Neighbor"));
    }
}
