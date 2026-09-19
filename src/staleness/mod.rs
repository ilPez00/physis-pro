use crate::dependency::StalenessState;
use crate::semantic_delta::{self, EntityChangeType, QuantityDirection, ScopeDelta};

/// Maps propagated affectedness into operational states.
pub struct StalenessClassifier;

impl StalenessClassifier {
    /// Classify a node's staleness based on affectedness score and properties.
    pub fn classify(
        affectedness: f32,
        provenance: &str,
        locked: bool,
        delta: &semantic_delta::SemanticDelta,
    ) -> StalenessState {
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
        if affectedness < 0.70
            && delta.entity_changes.is_empty()
            && delta
                .quantity_change
                .as_ref()
                .is_none_or(|q| matches!(q.direction, QuantityDirection::Unknown))
            && delta
                .scope_change
                .as_ref()
                .is_none_or(|s| matches!(s, ScopeDelta::Unchanged))
        {
            return StalenessState::RepairableCheap;
        }

        // Check for human anchor conflicts
        if affectedness >= 0.70
            && delta.entity_changes.iter().any(|e| {
                let privileged_prefixes = ["The ", "A ", "An ", "The", "A", "An"];
                privileged_prefixes
                    .iter()
                    .any(|p| e.old_value.starts_with(p))
                    && e.change_type == EntityChangeType::Substitution
            })
        {
            return StalenessState::RequiresHumanReview;
        }

        // Default: requires strong model repair
        StalenessState::RequiresStrongModel
    }
}

impl Default for StalenessClassifier {
    fn default() -> Self {
        StalenessClassifier
    }
}
