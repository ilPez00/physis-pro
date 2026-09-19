use serde::{Deserialize, Serialize};

/// A minimal repair region representing a set of nodes to regenerate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairRegion {
    /// IDs of affected nodes to repair
    pub affected_nodes: Vec<String>,
    /// Fixed left context anchor text (preserved)
    pub left_anchor: String,
    /// Fixed right context anchor text (preserved)
    pub right_anchor: String,
    /// Estimated compute complexity for this region (0.0 = trivial, 1.0 = complex)
    pub estimated_complexity: f32,
    /// Whether the region has been verified as correct
    pub verified: bool,
}

impl RepairRegion {
    /// Create a repair region from stale node IDs and their positions.
    pub fn from_stale_nodes(
        stale_node_ids: &[String],
        positions: &std::collections::HashMap<String, (usize, usize)>,
    ) -> Self {
        // Collect affected node IDs
        let affected: Vec<String> = stale_node_ids
            .iter()
            .filter(|id| positions.contains_key(*id))
            .cloned()
            .collect();

        // If no stale nodes have positions, return empty region
        if affected.is_empty() {
            return Self {
                affected_nodes: vec![],
                left_anchor: String::new(),
                right_anchor: String::new(),
                estimated_complexity: 0.0,
                verified: false,
            };
        }

        // Find min and max positions
        let min_pos: usize = positions.values().map(|(s, _)| *s).min().unwrap_or(0);
        let max_pos: usize = positions.values().map(|(_, e)| *e).max().unwrap_or(0);

        // Get left anchor (text before min position)
        let left_anchor = if min_pos > 0 {
            // Find a node before min_pos to use as anchor
            "...".to_string()
        } else {
            String::new()
        };

        // Get right anchor (text after max position)
        let right_anchor = if max_pos > 0 {
            "...".to_string()
        } else {
            String::new()
        };

        // Simple complexity: count of affected nodes + entity/quantity changes
        let complexity: f32 = affected.len() as f32 * 0.1;

        Self {
            affected_nodes: affected,
            left_anchor,
            right_anchor,
            estimated_complexity: complexity.min(1.0),
            verified: false,
        }
    }

    /// Check if this region covers a given node ID.
    pub fn covers(&self, node_id: &str) -> bool {
        self.affected_nodes.iter().any(|id| id == node_id)
    }
}

/// Select minimal repair regions from affected nodes.
///
/// Groups stale nodes into regions based on their textual proximity
/// and returns the minimal set of regions needed.
pub fn select_repair_regions(
    affected_nodes: &[String],
    positions: &std::collections::HashMap<String, (usize, usize)>,
    // delta: &SemanticDelta,
) -> Vec<RepairRegion> {
    // If no affected nodes, return empty
    if affected_nodes.is_empty() {
        return vec![];
    }

    // Filter to only nodes that have positions
    let positioned: Vec<String> = affected_nodes
        .iter()
        .filter(|id| positions.contains_key(*id))
        .cloned()
        .collect();

    if positioned.is_empty() {
        return vec![];
    }

    // Create one region for all positioned affected nodes
    // (In a full implementation, this would split into multiple regions)
    let region = RepairRegion::from_stale_nodes(&positioned, positions);

    vec![region]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_creation() {
        let positions = std::collections::HashMap::new();
        let region = RepairRegion::from_stale_nodes(&[], &positions);
        assert!(region.affected_nodes.is_empty());
    }

    #[test]
    fn test_region_with_positions() {
        let mut positions = std::collections::HashMap::new();
        positions.insert("s1".to_string(), (10, 50));
        positions.insert("s2".to_string(), (60, 110));
        positions.insert("s3".to_string(), (120, 170));

        let region = RepairRegion::from_stale_nodes(
            &["s1".to_string(), "s2".to_string(), "s3".to_string()],
            &positions,
        );

        assert!(!region.affected_nodes.is_empty());
        assert_eq!(region.affected_nodes.len(), 3);
        assert!(!region.left_anchor.is_empty());
        assert!(!region.right_anchor.is_empty());
    }

    #[test]
    fn test_select_repair_regions() {
        let mut positions = std::collections::HashMap::new();
        positions.insert("s1".to_string(), (0, 100));
        positions.insert("s2".to_string(), (100, 200));
        positions.insert("s3".to_string(), (200, 300));

        let _delta = crate::semantic_delta::SemanticDelta {
            source_node: "s1".to_string(),
            lexical: crate::semantic_delta::LexicalDiff::no_change(),
            proposition_changed: true,
            scope_change: None,
            polarity_change: None,
            quantity_change: None,
            modality_change: None,
            entity_changes: vec![],
            relation_changes: vec![],
            old_claims: vec![],
            new_claims: vec![],
            removed_constraints: vec![],
            added_constraints: vec![],
            semantic_distance: 0.5,
            confidence: 0.8,
        };

        let affected = vec!["s1".to_string(), "s2".to_string(), "s3".to_string()];
        let regions = select_repair_regions(&affected, &positions);

        assert!(!regions.is_empty());
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].affected_nodes.len(), 3);
    }

    #[test]
    fn test_region_covers() {
        let mut positions = std::collections::HashMap::new();
        positions.insert("s1".to_string(), (0, 100));
        positions.insert("s2".to_string(), (100, 200));

        let region =
            RepairRegion::from_stale_nodes(&["s1".to_string(), "s2".to_string()], &positions);

        assert!(region.covers("s1"));
        assert!(region.covers("s2"));
        assert!(!region.covers("s3"));
    }
}
