use crate::semantic_delta::{self, QuantityDirection, ScopeDelta, SemanticDelta};
use crate::staleness::StalenessClassifier;
use physis_core::relation::RelationType;
use serde::{Deserialize, Serialize};

/// Typed dependency edge between semantic nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyEdge {
    /// Source node ID
    pub source: String,
    /// Target node ID
    pub target: String,
    /// Relation type (Supports, Contradicts, Causes, etc.)
    pub relation: RelationType,
    /// Edge strength (0.0 = weak, 1.0 = strong)
    pub strength: f32,
    /// Edge confidence (0.0 = uncertain, 1.0 = certain)
    pub confidence: f32,
    /// Provenance of this edge
    pub provenance: String,
}

impl DependencyEdge {
    pub fn new(
        source: String,
        target: String,
        relation: RelationType,
        strength: f32,
        confidence: f32,
    ) -> Self {
        Self {
            source,
            target,
            relation,
            strength: strength.clamp(0.0, 1.0),
            confidence: confidence.clamp(0.0, 1.0),
            provenance: "auto".to_string(),
        }
    }
}

/// Propagate a semantic delta through the dependency graph.
///
/// For each affected node, computes an affectedness score based on:
/// - incoming affectedness from source
/// - edge strength
/// - relevance of delta type to edge relation
/// - scope overlap with target node
pub fn propagate_delta(
    graph: &DependencyGraph,
    source: &str,
    delta: &SemanticDelta,
    threshold: f32,
) -> Vec<(String, f32, PropagationTrace)> {
    // BFS propagation with score attenuation
    let mut queue: Vec<(String, f32)> = vec![(source.to_string(), 1.0)];
    let mut affected: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
    let mut traces: Vec<PropagationTrace> = vec![];

    while let Some((node_id, incoming_score)) = queue.pop() {
        // Get outgoing edges from this node
        let outgoing = graph.outgoing_edges(&node_id);

        for edge in outgoing {
            let target = &edge.target;

            // Compute relevance of this delta to this edge relation type
            let relation_relevance = delta_edge_relevance(delta, &edge.relation);

            // Compute scope overlap with target node's content
            let scope_overlap = compute_scope_overlap(delta, &target, source, graph);

            // Compute resulting affectedness score
            let score = incoming_score
                * edge.strength
                * relation_relevance
                * scope_overlap;

            // Decay for next level
            let decayed_score = score * 0.7; // 30% attenuation per hop

            // Only propagate if above threshold
            if score < threshold {
                continue;
            }

            // Record if this is a new/stronger affectedness
            let previous = affected.get(target).copied().unwrap_or(0.0);
            if score > previous {
                affected.insert(target.clone(), score);

                // Create trace
                let trace = PropagationTrace {
                    source: source.to_string(),
                    target: target.clone(),
                    relation: edge.relation.clone(),
                    incoming_score,
                    relation_relevance,
                    scope_overlap,
                    resulting_score: score,
                    decayed_score,
                };
                traces.push(trace);
            }

            // Add to queue for further propagation (with decay)
            queue.push((target.clone(), decayed_score));
        }
    }

    // Build result: (node_id, affectedness_score, trace)
    let result: Vec<(String, f32, PropagationTrace)> = traces
        .into_iter()
        .filter(|trace| {
            affected.contains_key(&trace.target)
        })
        .map(|trace| {
            let final_score = affected.get(&trace.target).copied().unwrap_or(0.0);
            let target = trace.target.clone();
            (target, final_score, trace)
        })
        .collect();

    result
}

/// Compute relevance of a semantic delta to a given edge relation type.
fn delta_edge_relevance(delta: &SemanticDelta, relation: &RelationType) -> f32 {
    let base_relevance = delta.semantic_distance.max(0.1); // minimum base relevance

    match relation {
        // Quantity changes affect everything that depends on support
        RelationType::Supports | RelationType::DerivedFrom if delta.quantity_change.is_some() => 0.9,
        // Scope changes propagate through derivations
        RelationType::DerivedFrom if delta.scope_change.is_some() => 0.8,
        // Polarity/reversal changes have very high relevance
        RelationType::Supports | RelationType::Contradicts | RelationType::Causes
            if delta.polarity_change.is_some() =>
        {
            1.0
        }
        // Entity substitutions propagate through references
        RelationType::DependsOn if delta.entity_changes.len() > 0 => 0.6,
        // Default: use base relevance
        _ => base_relevance,
    }
}

/// Compute scope overlap between delta and target node's content.
fn compute_scope_overlap(delta: &SemanticDelta, target_id: &str, source: &str, graph: &DependencyGraph) -> f32 {
    // Get the target node's claims
    let target_claims = graph.target_claims(target_id);

    if target_claims.is_empty() {
        // No claims = default moderate overlap
        return 0.5;
    }

    // Compute Jaccard-like overlap between changed claims and target claims
    let changed_claims: Vec<&str> = if delta.scope_change.is_some() {
        // When scope changes, all previous claims may be affected
        vec!["changed"].iter().map(|s| *s).collect()
    } else {
        // Compare old vs new claims
        vec![]
    };

    if changed_claims.is_empty() {
        // No explicit scope change: check if target depends on source
        let depends = graph.node_depends_on(target_id, source);
        if depends {
            0.6
        } else {
            0.1
        }
    } else {
        // Scope change: moderate overlap
        0.5
    }
}

/// Propagation trace recording why a node was affected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropagationTrace {
    /// Source of the propagation
    pub source: String,
    /// Target node that was affected
    pub target: String,
    /// Relation type that propagated the effect
    pub relation: RelationType,
    /// Affectedness entering this edge
    pub incoming_score: f32,
    /// How relevant this delta type is to this edge relation
    pub relation_relevance: f32,
    /// Scope overlap with target node
    pub scope_overlap: f32,
    /// Final affectedness score at this node
    pub resulting_score: f32,
    /// Decayed score for further propagation
    pub decayed_score: f32,
}

/// Graph of dependency edges between semantic nodes.
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// Adjacency list: node_id -> outgoing edges
    outgoing: std::collections::HashMap<String, Vec<DependencyEdge>>,
    /// Reverse adjacency: node_id -> incoming edges
    incoming: std::collections::HashMap<String, Vec<DependencyEdge>>,
    /// Node claims (for scope overlap computation)
    claims: std::collections::HashMap<String, Vec<String>>,
    /// Node dependency info
    depends_on: std::collections::HashMap<String, Vec<String>>,
}

impl DependencyGraph {
    /// Create a new empty dependency graph.
    pub fn new() -> Self {
        Self {
            outgoing: std::collections::HashMap::new(),
            incoming: std::collections::HashMap::new(),
            claims: std::collections::HashMap::new(),
            depends_on: std::collections::HashMap::new(),
        }
    }

    /// Add a dependency edge between two nodes.
    pub fn add_edge(&mut self, edge: DependencyEdge) {
        let source = edge.source.clone();
        let target = edge.target.clone();

        self.outgoing
            .entry(source.clone())
            .or_default()
            .push(edge.clone());
        self.incoming
            .entry(target.clone())
            .or_default()
            .push(edge.clone());

        // Track claims
        self.claims
            .entry(target.clone())
            .or_default()
            .push(format!("claim from {}", source));

        // Track dependency
        self.depends_on
            .entry(target.clone())
            .or_default()
            .push(source.clone());
    }

    /// Get outgoing edges from a node.
    pub fn outgoing_edges(&self, node_id: &str) -> &[DependencyEdge] {
        self.outgoing.get(node_id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get incoming edges to a node.
    pub fn incoming_edges(&self, node_id: &str) -> &[DependencyEdge] {
        self.incoming.get(node_id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Check if target depends on source.
    pub fn node_depends_on(&self, target: &str, source: &str) -> bool {
        self.depends_on
            .get(target)
            .map(|deps| deps.iter().any(|d| d == source))
            .unwrap_or(false)
    }

    /// Get claims associated with a node.
    pub fn target_claims(&self, node_id: &str) -> &[String] {
        self.claims.get(node_id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get all node IDs in the graph.
    pub fn node_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.outgoing.keys().cloned().collect();
        ids.extend(self.incoming.keys().cloned());
        ids.dedup();
        ids
    }
}

/// Classify a node's staleness based on its affectedness score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StalenessState {
    /// Node is unaffected by the edit
    Unaffected,
    /// Node is stale but can be verified without repair
    VerifyOnly,
    /// Node is stale and needs repair, but cheap model suffices
    RepairableCheap,
    /// Node is stale and requires strong model
    RequiresStrongModel,
    /// Node is stale and requires human review
    RequiresHumanReview,
    /// Node is human-authored and locked; changes require review
    LockedConflict,
}

impl StalenessState {
    /// Classify staleness based on affectedness score and node properties.
    pub fn classify(
        affectedness: f32,
        provenance: &str,
        locked: bool,
        delta: &semantic_delta::SemanticDelta,
    ) -> Self {
        if affectedness < 0.15 {
            return StalenessState::Unaffected;
        }

        // Human-authored locked nodes: changes require review
        if locked && provenance == "human" {
            return StalenessState::LockedConflict;
        }

        if affectedness < 0.40 {
            return StalenessState::VerifyOnly;
        }

        // Check if this is a cheap repair candidate
        if affectedness < 0.70 {
            if delta.entity_changes.is_empty()
                && delta.quantity_change.as_ref().map_or(true, |q| {
                    matches!(q.direction, QuantityDirection::Unknown)
                })
                && delta.scope_change.as_ref().map_or(true, |s| matches!(s, ScopeDelta::Unchanged))
            {
                return StalenessState::RepairableCheap;
            }
        }

        // Check for human anchor conflicts
        if affectedness >= 0.70 {
            if delta.entity_changes.iter().any(|e| {
                let privileged_prefixes = ["The ", "A ", "An ", "The", "A", "An"];
                privileged_prefixes.iter().any(|p| e.old_value.starts_with(p))
                    && e.change_type == semantic_delta::EntityChangeType::Substitution
            }) {
                return StalenessState::RequiresHumanReview;
            }
        }

        // Default: requires strong model repair
        StalenessState::RequiresStrongModel
    }
}

/// Classify staleness for a collection of affected nodes.
pub fn classify_affected_nodes(
    graph: &DependencyGraph,
    affected: &[(String, f32, PropagationTrace)],
    node_provenance: &std::collections::HashMap<String, String>,
    node_locked: &std::collections::HashMap<String, bool>,
    delta: &semantic_delta::SemanticDelta,
) -> std::collections::HashMap<String, StalenessState> {
    let mut results = std::collections::HashMap::new();

    for (node_id, affectedness, _) in affected {
        let provenance = node_provenance
            .get(node_id)
            .map(|s| s.as_str())
            .unwrap_or("machine");
        let locked = node_locked.get(node_id).copied().unwrap_or(false);

        let state =
            StalenessClassifier::classify(*affectedness, provenance, locked, delta);

        results.insert(node_id.clone(), state);
    }

    results
}

/// Generate a human-readable explanation for why a node was marked with a
/// particular staleness state.
pub fn explain_staleness(
    node_id: &str,
    state: &StalenessState,
    affectedness: f32,
    delta: &semantic_delta::SemanticDelta,
) -> String {
    match state {
        StalenessState::Unaffected => {
            format!("Node {} is UNAFFECTED by the edit (affectedness: {:.2})", node_id, affectedness)
        }
        StalenessState::VerifyOnly => {
            format!(
                "Node {} is VERIFY_ONLY - stale but may be coherent (affectedness: {:.2})",
                node_id, affectedness
            )
        }
        StalenessState::RepairableCheap => {
            format!(
                "Node {} requires REPAIRABLE_CHEAP - can be fixed with cheap model (affectedness: {:.2})",
                node_id, affectedness
            )
        }
        StalenessState::RequiresStrongModel => {
            format!(
                "Node {} requires STRONG_MODEL repair (affectedness: {:.2})",
                node_id, affectedness
            )
        }
        StalenessState::RequiresHumanReview => {
            format!(
                "Node {} requires HUMAN_REVIEW - conflicts with privileged anchor",
                node_id
            )
        }
        StalenessState::LockedConflict => {
            format!(
                "Node {} is LOCKED_CONFLICT - human-authored, changes require review (affectedness: {:.2})",
                node_id, affectedness
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_delta::{self, QuantityDirection};

    #[test]
    fn test_dependency_graph_creation() {
        let mut graph = DependencyGraph::new();
        graph.add_edge(DependencyEdge::new(
            "s1".to_string(),
            "s2".to_string(),
            RelationType::DependsOn,
            0.8,
            0.9,
        ));

        let edges = graph.outgoing_edges("s1");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].relation, RelationType::DependsOn);
    }

    #[test]
    fn test_node_depends_on() {
        let mut graph = DependencyGraph::new();
        graph.add_edge(DependencyEdge::new(
            "s1".to_string(),
            "s2".to_string(),
            RelationType::DependsOn,
            0.8,
            0.9,
        ));

        assert!(graph.node_depends_on("s2", "s1"));
        assert!(!graph.node_depends_on("s1", "s2"));
    }

    #[test]
    fn test_propagate_delta_basic() {
        let mut graph = DependencyGraph::new();
        graph.add_edge(DependencyEdge::new(
            "s1".to_string(),
            "s2".to_string(),
            RelationType::DependsOn,
            0.8,
            0.9,
        ));
        graph.add_edge(DependencyEdge::new(
            "s2".to_string(),
            "s3".to_string(),
            RelationType::Supports,
            0.7,
            0.8,
        ));

        let delta = SemanticDelta {
            source_node: "s1".to_string(),
            lexical: semantic_delta::LexicalDiff::no_change(),
            proposition_changed: true,
            scope_change: None,
            polarity_change: None,
            quantity_change: None,
            modality_change: None,
            entity_changes: vec![],
            relation_changes: vec![],
            old_claims: vec!["inference happens locally".to_string()],
            new_claims: vec!["generation is remote".to_string()],
            removed_constraints: vec![],
            added_constraints: vec![],
            semantic_distance: 0.8,
            confidence: 0.9,
        };

        let results = propagate_delta(&graph, "s1", &delta, 0.1);
        // s3 should be affected (s2 depends on s1, s3 depends on s2 via Supports)
        assert!(!results.is_empty());
    }

    #[test]
    fn test_staleness_classification() {
        // Low affectedness → Unaffected
        assert_eq!(
            StalenessState::classify(0.1, "machine", false, &semantic_delta::SemanticDelta::no_semantic_change(
                "s1".to_string(),
                "old text",
                "old text"
            )),
            StalenessState::Unaffected
        );

        // Human + locked → LockedConflict
        assert_eq!(
            StalenessState::classify(0.8, "human", true, &semantic_delta::SemanticDelta::no_semantic_change(
                "s1".to_string(),
                "old text",
                "new text"
            )),
            StalenessState::LockedConflict
        );

        // Moderate affectedness → VerifyOnly
        assert_eq!(
            StalenessState::classify(0.3, "machine", false, &semantic_delta::SemanticDelta::no_semantic_change(
                "s1".to_string(),
                "old text",
                "old text"
            )),
            StalenessState::VerifyOnly
        );
    }

    #[test]
    fn test_delta_edge_relevance() {
        let delta = SemanticDelta {
            source_node: "s1".to_string(),
            lexical: semantic_delta::LexicalDiff::no_change(),
            proposition_changed: true,
            scope_change: None,
            polarity_change: Some(semantic_delta::PolarityDelta::Flipped),
            quantity_change: None,
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

        // Polarity change with Supports should have high relevance
        let relevance = delta_edge_relevance(&delta, &RelationType::Supports);
        assert!(relevance > 0.5);

        // No change should have base relevance
        let delta_no_change = semantic_delta::SemanticDelta::no_semantic_change(
            "s1".to_string(),
            "old text",
            "old text",
        );
        let relevance2 = delta_edge_relevance(&delta_no_change, &RelationType::Supports);
        assert!(relevance2 > 0.0);
    }
}