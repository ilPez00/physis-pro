// Repair routing pipeline below is scaffolding kept for the repair phase:
// only `RepairResult`/`RepairRegion` types are consumed today. Reuse or
// delete when the repair worker lands.
#![allow(dead_code)]
use crate::semantic_delta::SemanticDelta;
use crate::dependency::{PropagationTrace};
use crate::repair_regions::RepairRegion;
use serde::{Deserialize, Serialize};

/// Worker capability profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerCapabilities {
    /// Worker identifier
    pub worker_id: String,
    /// Model family (e.g. "tiny_ar", "small_ar", "strong", "deterministic")
    pub model_family: String,
    /// Supported modalities
    pub modalities: Vec<String>,
    /// Maximum context window
    pub max_context: usize,
    /// Estimated quality for this type of region
    pub estimated_quality: f32,
    /// Estimated latency in milliseconds
    pub estimated_latency_ms: u64,
    /// Estimated cost (arbitrary units)
    pub estimated_cost: f32,
    /// Supports infilling/span editing
    pub supports_infilling: bool,
    /// Supports structured/sparse output
    pub supports_structured_output: bool,
    /// Semantic interpolation radius (how far context can extend)
    pub semantic_interpolation_radius: usize,
}

/// Trait for repair workers.
pub trait RepairWorker: Send + Sync {
    /// Return this worker's capabilities.
    fn capabilities(&self) -> WorkerCapabilities;

    /// Attempt to repair a region.
    fn repair(
        &self,
        region: &RepairRegion,
        context: &RepairContext,
    ) -> anyhow::Result<RepairResult>;
}

/// Context for repair operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairContext {
    /// The full document text
    pub document: String,
    /// The region being repaired
    pub region: RepairRegion,
    /// The semantic delta that triggered the repair
    pub delta: SemanticDelta,
    /// Previously preserved anchors
    pub preserved_anchors: Vec<String>,
    /// Graph constraints
    pub constraints: RepairConstraints,
}

/// Constraints for repair operations.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepairConstraints {
    /// Human-authored text that must not be altered
    pub locked_anchors: Vec<String>,
    /// Claims that must be preserved
    pub preserve_claims: Vec<String>,
    /// Claims that must not appear
    pub prohibit_claims: Vec<String>,
    /// Scope constraints
    pub scope_constraints: Vec<String>,
}

/// Result of a repair operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairResult {
    /// The repaired text span
    pub repaired_text: String,
    /// Which worker handled this region
    pub worker_id: String,
    /// Confidence in the repair
    pub confidence: f32,
    /// Whether verification passed
    pub verification_passed: bool,
    /// Propagation trace for debugging
    pub propagation_trace: Option<PropagationTrace>,
}

/// Compute a heaviness score for a repair region, determining which worker
/// should handle it.
///
/// Higher score → more expensive/complex repair needed.
fn compute_heaviness(region: &RepairRegion, delta: &SemanticDelta) -> f32 {
    let mut score = 0.0f32;

    // Concept density: fraction of content words in affected nodes
    let node_text_len: usize = region
        .affected_nodes
        .iter()
        .map(|_id| {
            // Rough estimate based on position
            100 // placeholder
        })
        .sum();
    let density = if node_text_len > 0 { 0.5 } else { 0.1 };
    score += density * 0.2;

    // Uncertainty: semantic distance from delta
    let uncertainty = delta.semantic_distance;
    score += uncertainty * 0.25;

    // Dependency centrality: how many other nodes depend on this region
    let centrality: f32 = region
        .affected_nodes
        .iter()
        .map(|_id| {
            // Count outgoing edges to non-region nodes
            0.0 // placeholder
        })
        .sum();
    score += centrality * 0.2;

    // Downstream impact: how many nodes are affected downstream
    let downstream = region.affected_nodes.len() as f32;
    score += (downstream / 10.0f32.min(downstream)) * 0.15;

    // Reconstruction difficulty: entity changes, quantity changes, etc.
    let reconstruction = if !delta.entity_changes.is_empty() {
        0.6
    } else if delta.quantity_change.is_some() {
        0.4
    } else {
        0.2
    };
    score += reconstruction * 0.15;

    // Failure history: if previous repairs of this type failed
    let failure_penalty = 0.0; // placeholder
    score += failure_penalty * 0.05;

    score.min(1.0)
}

/// Choose the appropriate worker for a repair region.
fn choose_worker(region: &RepairRegion, delta: &SemanticDelta) -> (Box<dyn RepairWorker>, f32) {
    let heaviness = compute_heaviness(region, delta);

    // Simple two-worker routing for demo
    // In production, this would consider multiple worker types

    if heaviness < 0.4 {
        // Easy region → cheap worker
        (Box::new(CheapWorker), heaviness)
    } else {
        // Hard region → strong model worker
        (Box::new(StrongWorker), heaviness)
    }
}

/// Cheap worker for simple repairs (rephrasing, minor updates).
struct CheapWorker;

impl RepairWorker for CheapWorker {
    fn capabilities(&self) -> WorkerCapabilities {
        WorkerCapabilities {
            worker_id: "cheap_ar".to_string(),
            model_family: "tiny_ar".to_string(),
            modalities: vec!["text".to_string()],
            max_context: 2048,
            estimated_quality: 0.7,
            estimated_latency_ms: 50,
            estimated_cost: 0.1,
            supports_infilling: true,
            supports_structured_output: false,
            semantic_interpolation_radius: 50,
        }
    }

    fn repair(
        &self,
        _region: &RepairRegion,
        _context: &RepairContext,
    ) -> anyhow::Result<RepairResult> {
        Ok(RepairResult {
            repaired_text: "CHEAP: minor rephrasing applied".to_string(),
            worker_id: "cheap_ar".to_string(),
            confidence: 0.7,
            verification_passed: true,
            propagation_trace: None,
        })
    }
}

/// Strong model worker for complex repairs (structural changes, polarity reversals).
struct StrongWorker;

impl RepairWorker for StrongWorker {
    fn capabilities(&self) -> WorkerCapabilities {
        WorkerCapabilities {
            worker_id: "strong_model".to_string(),
            model_family: "large_model".to_string(),
            modalities: vec!["text".to_string()],
            max_context: 8192,
            estimated_quality: 0.95,
            estimated_latency_ms: 2000,
            estimated_cost: 1.0,
            supports_infilling: true,
            supports_structured_output: true,
            semantic_interpolation_radius: 500,
        }
    }

    fn repair(
        &self,
        _region: &RepairRegion,
        _context: &RepairContext,
    ) -> anyhow::Result<RepairResult> {
        Ok(RepairResult {
            repaired_text: "STRONG: semantically consistent repair".to_string(),
            worker_id: "strong_model".to_string(),
            confidence: 0.95,
            verification_passed: true,
            propagation_trace: None,
        })
    }
}

/// CHEAP_WORKER_EXAMPLE and STRONG_WORKER_EXAMPLE are referenced but not defined
/// at the module level here; in a full implementation they'd be instantiated
/// or passed in. For now, we define the routing logic that would use them.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_delta;
    use crate::semantic_delta::QuantityDirection;

    #[test]
    fn test_heaviness_computation() {
        let mut graph = crate::dependency::DependencyGraph::new();
        graph.add_edge(crate::dependency::DependencyEdge::new(
            "s1".to_string(),
            "s2".to_string(),
            physis_core::relation::RelationType::DependsOn,
            0.8,
            0.9,
        ));

        let region = RepairRegion {
            affected_nodes: vec!["s1".to_string(), "s2".to_string()],
            left_anchor: String::new(),
            right_anchor: String::new(),
            estimated_complexity: 0.0,
            verified: false,
        };

        let delta = semantic_delta::SemanticDelta {
            source_node: "s1".to_string(),
            lexical: semantic_delta::LexicalDiff::no_change(),
            proposition_changed: true,
            scope_change: Some(crate::semantic_delta::ScopeDelta::Narrowed),
            polarity_change: None,
            quantity_change: Some(semantic_delta::QuantityDelta {
                old_value: 18.0,
                new_value: 3.0,
                direction: QuantityDirection::Decrease,
            }),
            modality_change: None,
            entity_changes: vec![],
            relation_changes: vec![],
            old_claims: vec![],
            new_claims: vec![],
            removed_constraints: vec![],
            added_constraints: vec![],
            semantic_distance: 0.8,
            confidence: 0.9,
        };

        let heaviness = compute_heaviness(&region, &delta);
        // Should be moderately high due to quantity + scope changes
        assert!(heaviness > 0.3);
    }

    #[test]
    fn test_worker_choosing() {
        let mut graph = crate::dependency::DependencyGraph::new();
        graph.add_edge(crate::dependency::DependencyEdge::new(
            "s1".to_string(),
            "s2".to_string(),
            physis_core::relation::RelationType::DependsOn,
            0.8,
            0.9,
        ));

        let region = RepairRegion {
            affected_nodes: vec!["s1".to_string(), "s2".to_string()],
            left_anchor: String::new(),
            right_anchor: String::new(),
            estimated_complexity: 0.0,
            verified: false,
        };

        let delta = semantic_delta::SemanticDelta {
            source_node: "s1".to_string(),
            lexical: semantic_delta::LexicalDiff::no_change(),
            proposition_changed: true,
            scope_change: None,
            polarity_change: None,
            quantity_change: None, // No quantity change → lower heaviness
            modality_change: None,
            entity_changes: vec![],
            relation_changes: vec![],
            old_claims: vec![],
            new_claims: vec![],
            removed_constraints: vec![],
            added_constraints: vec![],
            semantic_distance: 0.3, // Lower distance → cheaper
            confidence: 0.5,
        };

        // Low heaviness should route to cheap worker
        let heaviness = compute_heaviness(&region, &delta);
        assert!(heaviness < 0.5);
    }
}