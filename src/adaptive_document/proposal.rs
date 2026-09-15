//! Proposal and application logic for adaptive documents (Phase 26 §26.5).
//!
//! Reachability is exact for the *declared* graph; the graph itself may be
//! incomplete, which is why review links exist. Calculation edges are
//! evaluated in dependency order, not arbitrary BFS order.

use super::{
    ApplyError, BlockId, Change, Document, Link, LinkBehavior, LinkProvenance, Patch, PatchStatus,
    Replacement, ReviewNote, fnv1a,
};
use std::collections::BTreeMap;

/// Default cap on reachable blocks per request (Phase 26 §26.5).
pub const MAX_AFFECTED: usize = 256;

/// Record a human edit of one block: splice bytes, bump the revision, and
/// return the new document plus the observed change event. An empty/no-op
/// edit returns `None` and must create no semantic-change claim (Phase 26
/// §26.4).
pub fn apply_source_edit(
    doc: &Document,
    block_id: &str,
    new_bytes: &str,
    actor: &str,
    sequence: u64,
    recorded_at: u64,
) -> Option<(Document, Change)> {
    let block = doc.block(block_id)?;
    if block.text(&doc.raw) == new_bytes {
        return None;
    }
    let before_hash = block.content_hash;
    let (next, updated) = doc.splice_block(block_id, new_bytes)?;
    let change = Change {
        event_id: format!("evt-{}-{}", next.revision, fnv1a(new_bytes.as_bytes())),
        document_id: next.id.clone(),
        block_id: block_id.to_string(),
        before_revision: doc.revision,
        after_revision: next.revision,
        before_hash,
        after_hash: updated.content_hash,
        actor: actor.to_string(),
        operation: "source_edit".into(),
        sequence,
        recorded_at,
    };
    Some((next, change))
}

/// Build the patch proposal for a source edit (Phase 26 §26.5). Persisting
/// the proposal metadata does not change document bytes.
pub fn propose_after_source_edit(doc: &Document, change: &Change, patch_id: &str) -> Patch {
    let mut patch = Patch {
        id: patch_id.to_string(),
        document_id: doc.id.clone(),
        triggered_by_change_id: change.event_id.clone(),
        expected_document_revision: doc.revision,
        expected_graph_revision: doc.graph_revision,
        replacements: Vec::new(),
        review_only: Vec::new(),
        status: PatchStatus::Proposed,
        applied_revision: None,
    };

    let (affected, truncated) = doc.reachable_dependents(&change.block_id, MAX_AFFECTED);
    if truncated {
        patch.review_only.push(ReviewNote {
            block_id: change.block_id.clone(),
            reason: "traversal cap reached; affected set may be incomplete".into(),
            link_ids: Vec::new(),
        });
    }

    // Calculation edges need dependency order, not arbitrary BFS evaluation.
    let calc_order = match doc.topological_calculation_order(&affected) {
        Ok(order) => order,
        Err(cycle) => {
            for id in &cycle {
                patch.review_only.push(ReviewNote {
                    block_id: id.clone(),
                    reason: "cyclic calculation".into(),
                    link_ids: Vec::new(),
                });
            }
            // Non-cyclic members keep their order; cycle members review-only.
            affected.iter().filter(|id| !cycle.contains(id)).cloned().collect()
        }
    };

    // Map dependent block -> its confirmed inbound auto-propagation links.
    let mut inbound: BTreeMap<&BlockId, Vec<&Link>> = BTreeMap::new();
    for link in &doc.links {
        if link.active
            && link.provenance == LinkProvenance::HumanConfirmed
            && matches!(link.behavior, LinkBehavior::Copy | LinkBehavior::Calculate)
        {
            inbound.entry(&link.dependent_block_id).or_default().push(link);
        }
    }

    for block_id in calc_order {
        if block_id == change.block_id {
            continue;
        }
        let Some(block) = doc.block(&block_id) else {
            continue;
        };
        if block.locked {
            patch.review_only.push(ReviewNote {
                block_id: block_id.clone(),
                reason: "locked dependent".into(),
                link_ids: Vec::new(),
            });
            continue;
        }
        let links = inbound.get(&block_id).cloned().unwrap_or_default();
        if links.is_empty() {
            // Only confirmed copy/calculate links auto-propagate. A reachable
            // block with none of them is review-only coverage; the review-link
            // pass below emits its note.
            continue;
        }
        let current = block.text(&doc.raw).to_string();
        let mut computed: Option<String> = None;
        let mut reasons = Vec::new();
        let mut unresolved = false;
        for link in links {
            reasons.push(link.id.clone());
            let Some(src_block) = doc.block(&link.source_block_id) else {
                unresolved = true;
                break;
            };
            let source = src_block.text(&doc.raw);
            let Some(t) = &link.transform_id else {
                unresolved = true;
                break;
            };
            // `copy` transforms consume the source bytes directly; calculation
            // transforms rewrite the dependent text from its own bytes.
            let input = if link.behavior == LinkBehavior::Copy { source } else { &current };
            match super::apply_transform(t, source, input) {
                Some(next_bytes) => computed = Some(next_bytes),
                None => {
                    unresolved = true;
                    break;
                }
            }
        }
        if unresolved || computed.is_none() {
            patch.review_only.push(ReviewNote {
                block_id: block_id.clone(),
                reason: "unresolved dependency".into(),
                link_ids: reasons,
            });
            continue;
        }
        let new_bytes = computed.unwrap();
        if new_bytes != current {
            patch.replacements.push(Replacement {
                block_id: block_id.clone(),
                range: (0, current.len()),
                expected_old_bytes: current,
                new_bytes,
                reasons,
            });
        }
    }

    // Review-only notice for links that only mark prose for inspection.
    for link in &doc.links {
        if link.active
            && link.behavior == LinkBehavior::Review
            && affected.contains(&link.dependent_block_id)
        {
            patch.review_only.push(ReviewNote {
                block_id: link.dependent_block_id.clone(),
                reason: "review link: source content changed".into(),
                link_ids: vec![link.id.clone()],
            });
        }
    }
    patch
}
/// Apply a proposed patch to `doc`. Every check runs before a single byte
/// changes, so any failure rejects the whole patch atomically (AD05/AD07).
pub fn apply_patch(
    doc: &Document,
    patch: &Patch,
    actor: &str,
    sequence: u64,
    recorded_at: u64,
) -> Result<(Document, Change, Patch), ApplyError> {
    if patch.status != PatchStatus::Proposed {
        return Err(ApplyError::NotProposed);
    }
    if patch.expected_document_revision != doc.revision {
        return Err(ApplyError::StaleDocumentRevision);
    }
    if patch.expected_graph_revision != doc.graph_revision {
        return Err(ApplyError::StaleGraphRevision);
    }
    // Reject duplicate targets up front; replacement ranges within one block
    // are whole-block in this MVP, so block identity is the overlap unit.
    let mut seen = std::collections::BTreeSet::new();
    for r in &patch.replacements {
        if !seen.insert(r.block_id.clone()) {
            return Err(ApplyError::OverlappingRanges);
        }
    }
    // Verify every expected block and old bytes before mutating anything.
    for r in &patch.replacements {
        let block = doc.block(&r.block_id).ok_or(ApplyError::MissingBlock)?;
        if block.locked {
            return Err(ApplyError::ContentMismatch);
        }
        let current = block.text(&doc.raw);
        if current != r.expected_old_bytes {
            return Err(ApplyError::ContentMismatch);
        }
    }
    // Splice from the highest span so earlier offsets stay valid regardless
    // of how the caller ordered replacements (right-to-left rule, §26.4).
    let mut order: Vec<&Replacement> = patch.replacements.iter().collect();
    order.sort_by_key(|r| std::cmp::Reverse(doc.block(&r.block_id).map(|b| b.span.0)));
    let mut next = doc.clone();
    for r in order {
        let (n, _) = next
            .splice_block(&r.block_id, &r.new_bytes)
            .ok_or(ApplyError::ContentMismatch)?;
        next = n;
    }
    let change = Change {
        event_id: format!("evt-applied-{}-{}", patch.id, next.revision),
        document_id: doc.id.clone(),
        block_id: patch
            .replacements
            .first()
            .map(|r| r.block_id.clone())
            .unwrap_or_default(),
        before_revision: doc.revision,
        after_revision: next.revision,
        before_hash: fnv1a(doc.raw.as_bytes()),
        after_hash: fnv1a(next.raw.as_bytes()),
        actor: actor.to_string(),
        operation: format!("apply_patch:{}", patch.id),
        sequence,
        recorded_at,
    };
    let mut applied = patch.clone();
    applied.status = PatchStatus::Applied;
    applied.applied_revision = Some(next.revision);
    Ok((next, change, applied))
}

/// Reject a proposal: recorded in history, zero document bytes change (AD09).
pub fn reject_patch(patch: &Patch) -> Result<Patch, ApplyError> {
    if patch.status != PatchStatus::Proposed {
        return Err(ApplyError::NotProposed);
    }
    let mut rejected = patch.clone();
    rejected.status = PatchStatus::Rejected;
    Ok(rejected)
}

/// Undo an applied patch: restore only the dependent replacements, and only
/// when the document is still at the revision the patch produced (AD10).
pub fn undo_patch(
    doc: &Document,
    patch: &Patch,
    actor: &str,
    sequence: u64,
    recorded_at: u64,
) -> Result<(Document, Change, Patch), ApplyError> {
    if patch.status != PatchStatus::Applied {
        return Err(ApplyError::NotProposed);
    }
    let produced = patch.applied_revision.ok_or(ApplyError::UndoConflict)?;
    if doc.revision != produced {
        // Intervening edits exist; undo would erase them.
        return Err(ApplyError::UndoConflict);
    }
    let mut next = doc.clone();
    let mut order: Vec<&Replacement> = patch.replacements.iter().collect();
    order.sort_by_key(|r| std::cmp::Reverse(doc.block(&r.block_id).map(|b| b.span.0)));
    for r in order {
        let (n, _) = next
            .splice_block(&r.block_id, &r.expected_old_bytes)
            .ok_or(ApplyError::UndoConflict)?;
        next = n;
    }
    let change = Change {
        event_id: format!("evt-undo-{}-{}", patch.id, next.revision),
        document_id: doc.id.clone(),
        block_id: patch
            .replacements
            .first()
            .map(|r| r.block_id.clone())
            .unwrap_or_default(),
        before_revision: doc.revision,
        after_revision: next.revision,
        before_hash: fnv1a(doc.raw.as_bytes()),
        after_hash: fnv1a(next.raw.as_bytes()),
        actor: actor.to_string(),
        operation: format!("undo_patch:{}", patch.id),
        sequence,
        recorded_at,
    };
    let mut reverted = patch.clone();
    reverted.status = PatchStatus::Reverted;
    Ok((next, change, reverted))
}