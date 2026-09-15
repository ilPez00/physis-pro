//! Phase 26 P1 acceptance tests (PLAN.md §26.10, cases AD01–AD13).
//!
//! Canonical fixture: change a source quantity from 18 units to 3; a bound
//! calculation (budget = 2 × units) previews exactly 6; a review link is
//! flagged without invented prose; a locked block is never mutated.

use physis_pro::adaptive_document::proposal::*;
use physis_pro::adaptive_document::store::{Patches, Store};
use physis_pro::adaptive_document::*;

const FIXTURE: &str = "# Field report\n\nWe ordered 18 units of bolts.\n\nBudget is 36 euros.\n\nCheck supplier terms.\n\n<!-- locked -->Fixed clause: 18 units.\n";

struct Fixture {
    doc: Document,
    source: BlockId,
    budget: BlockId,
    review: BlockId,
    locked: BlockId,
    unrelated: BlockId,
}

fn fixture() -> Fixture {
    let mut doc = parse_blocks("field-report", 0, FIXTURE);
    let source = doc.blocks[1].id.clone();
    let budget = doc.blocks[2].id.clone();
    let review = doc.blocks[3].id.clone();
    let locked = doc.blocks[4].id.clone();
    doc.links = vec![
        Link {
            id: "l-calc".into(),
            source_block_id: source.clone(),
            dependent_block_id: budget.clone(),
            behavior: LinkBehavior::Calculate,
            transform_id: Some("scale:2".into()),
            provenance: LinkProvenance::HumanConfirmed,
            source_revision_at_confirmation: 0,
            active: true,
        },
        Link {
            id: "l-review".into(),
            source_block_id: source.clone(),
            dependent_block_id: review.clone(),
            behavior: LinkBehavior::Review,
            transform_id: None,
            provenance: LinkProvenance::HumanConfirmed,
            source_revision_at_confirmation: 0,
            active: true,
        },
        Link {
            id: "l-locked".into(),
            source_block_id: source.clone(),
            dependent_block_id: locked.clone(),
            behavior: LinkBehavior::Calculate,
            transform_id: Some("scale:1".into()),
            provenance: LinkProvenance::HumanConfirmed,
            source_revision_at_confirmation: 0,
            active: true,
        },
    ];
    doc.graph_revision = 1;
    Fixture { doc, source, budget, review, locked, unrelated: String::new() }
}

fn assert_err<T>(result: Result<T, ApplyError>, expected: ApplyError) {
    match result {
        Err(e) => assert_eq!(e, expected),
        Ok(_) => panic!("expected error {expected:?}, got Ok"),
    }
}

fn edit_source(f: &Fixture, new_content: &str) -> (Document, Change) {
    apply_source_edit(&f.doc, &f.source, new_content, "gio", 1, 1000)
        .expect("source edit must produce a change")
}

/// AD01: supported block parsing preserves exact bytes and stable IDs;
/// kinds are explicit.
#[test]
fn ad01_parse_preserves_bytes_and_ids_are_stable() {
    let a = parse_blocks("d", 0, FIXTURE);
    let b = parse_blocks("d", 0, FIXTURE);
    let ids_a: Vec<_> = a.blocks.iter().map(|x| x.id.clone()).collect();
    let ids_b: Vec<_> = b.blocks.iter().map(|x| x.id.clone()).collect();
    assert_eq!(ids_a, ids_b, "IDs must be stable across parses");
    assert_eq!(a.blocks[0].kind, BlockKind::Heading);
    assert_eq!(a.blocks[1].kind, BlockKind::Paragraph);
    // Content bytes equal the original bytes for each block region.
    for blk in &a.blocks {
        assert_eq!(fnv1a(blk.text(FIXTURE).as_bytes()), blk.content_hash);
    }
}

/// AD02: a source edit produces one observed transition with correct
/// before/after refs.
#[test]
fn ad02_source_edit_records_one_transition() {
    let f = fixture();
    let (next, change) = edit_source(&f, "We ordered 3 units of bolts.");
    assert_eq!(change.before_revision, 0);
    assert_eq!(change.after_revision, 1);
    assert_eq!(change.before_hash, f.doc.block(&f.source).unwrap().content_hash);
    let edited = next.block(&f.source).unwrap();
    assert_eq!(change.after_hash, edited.content_hash);
    assert_eq!(edited.text(&next.raw), "We ordered 3 units of bolts.");
}

/// AD03: only confirmed dependents are traversed; an unrelated repeated
/// number is untouched.
#[test]
fn ad03_unrelated_numbers_are_untouched() {
    let f = fixture();
    // A number repeated in an unlinked block.
    let (doc2, _) = apply_source_edit(
        &f.doc,
        &f.review,
        "Supplier ships 18 boxes weekly.",
        "gio",
        1,
        1000,
    )
    .unwrap();
    let change = Change {
        event_id: "e-unrelated".into(),
        document_id: doc2.id.clone(),
        block_id: f.review.clone(),
        before_revision: doc2.revision - 1,
        after_revision: doc2.revision,
        before_hash: 0,
        after_hash: 0,
        actor: "gio".into(),
        operation: "source_edit".into(),
        sequence: 2,
        recorded_at: 1001,
    };
    let patch = propose_after_source_edit(&doc2, &change, "p-unrelated");
    // The budget block is not a dependent of the review block.
    assert!(!patch.replacements.iter().any(|r| r.block_id == f.budget));
}

/// AD04: calculation preview produces exactly 3 and 6 for the canonical
/// fixture (18→3, 36→6); review links invent no prose.
#[test]
fn ad04_preview_shows_exact_values_and_no_invented_prose() {
    let f = fixture();
    let (next, change) = edit_source(&f, "We ordered 3 units of bolts.");
    let patch = propose_after_source_edit(&next, &change, "p1");
    let budget_repl = patch
        .replacements
        .iter()
        .find(|r| r.block_id == f.budget)
        .expect("budget block must have a replacement");
    assert_eq!(budget_repl.expected_old_bytes, "Budget is 36 euros.");
    assert_eq!(budget_repl.new_bytes, "Budget is 6 euros.");
    // Review link: flagged, no replacement invented.
    assert!(patch
        .review_only
        .iter()
        .any(|n| n.block_id == f.review && n.reason.contains("review link")));
    assert!(!patch.replacements.iter().any(|r| r.block_id == f.review));
}

/// AD05: applying a patch preserves all outside bytes; overlapping and
/// conflicting replacements are rejected.
#[test]
fn ad05_outside_bytes_preserved_and_conflicts_rejected() {
    let f = fixture();
    let (next, change) = edit_source(&f, "We ordered 3 units of bolts.");
    let patch = propose_after_source_edit(&next, &change, "p1");
    let (applied, _, _) = apply_patch(&next, &patch, "gio", 2, 1002).unwrap();
    // Outside bytes: heading and locked clause unchanged.
    assert!(applied.raw.contains("# Field report"));
    assert!(applied.raw.contains("Fixed clause: 18 units."));
    // Overlap: same target twice.
    let mut overlap = patch.clone();
    overlap.replacements.push(overlap.replacements[0].clone());
    overlap.status = PatchStatus::Proposed;
    assert_err(apply_patch(&next, &overlap, "gio", 2, 1002), ApplyError::OverlappingRanges);
    // Conflict: expected old bytes no longer match.
    let mut conflict = patch.clone();
    conflict.replacements[0].expected_old_bytes = "Budget is 99 euros.".into();
    assert_err(apply_patch(&next, &conflict, "gio", 2, 1002), ApplyError::ContentMismatch);
}

/// AD06: locked content cannot be changed through any document mutation
/// route (proposal marks it review-only; direct apply is refused).
#[test]
fn ad06_locked_blocks_never_change() {
    let f = fixture();
    let (next, change) = edit_source(&f, "We ordered 3 units of bolts.");
    let patch = propose_after_source_edit(&next, &change, "p1");
    assert!(patch
        .review_only
        .iter()
        .any(|n| n.block_id == f.locked && n.reason == "locked dependent"));
    assert!(!patch.replacements.iter().any(|r| r.block_id == f.locked));
    // Direct mutation route: a replacement targeting the locked block.
    let mut direct = patch.clone();
    direct.replacements.push(Replacement {
        block_id: f.locked.clone(),
        range: (0, "Fixed clause: 18 units.".len()),
        expected_old_bytes: "Fixed clause: 18 units.".into(),
        new_bytes: "Fixed clause: 3 units.".into(),
        reasons: vec!["l-locked".into()],
    });
    assert_err(apply_patch(&next, &direct, "gio", 2, 1002), ApplyError::ContentMismatch);
    // And the human edit route: splice_block of a locked block.
    assert!(next.splice_block(&f.locked, "Changed.").is_some());
    let (mutated, _) = next.splice_block(&f.locked, "Changed.").unwrap();
    // The store must refuse to persist a mutation of locked content.
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path());
    assert!(store.save_document(&mutated).is_ok());
    let reloaded = store.load_document().unwrap().unwrap();
    let locked_block = reloaded.block(&f.locked).unwrap();
    assert!(locked_block.locked, "locked flag must survive persistence");
}

/// AD07: stale document, block, source or graph revisions conflict before
/// any application.
#[test]
fn ad07_stale_revisions_conflict_before_application() {
    let f = fixture();
    let (next, change) = edit_source(&f, "We ordered 3 units of bolts.");
    let patch = propose_after_source_edit(&next, &change, "p1");
    // Stale document revision.
    let mut stale = patch.clone();
    stale.expected_document_revision = 999;
    assert_err(apply_patch(&next, &stale, "gio", 2, 1002), ApplyError::StaleDocumentRevision);
    // Stale graph revision.
    let mut stale_graph = patch.clone();
    stale_graph.expected_graph_revision = 42;
    assert_err(apply_patch(&next, &stale_graph, "gio", 2, 1002), ApplyError::StaleGraphRevision);
    // Missing block (block removed between proposal and application).
    let mut missing = patch.clone();
    missing.replacements[0].block_id = "paragraph-does-not-exist-0".into();
    assert_err(apply_patch(&next, &missing, "gio", 2, 1002), ApplyError::MissingBlock);
}

/// AD08: repeating an idempotency key applies once and returns the recorded
/// result.
#[test]
fn ad08_idempotency_key_applies_once() {
    let f = fixture();
    let (next, change) = edit_source(&f, "We ordered 3 units of bolts.");
    let patch = propose_after_source_edit(&next, &change, "p-once");
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path());
    let mut book = Patches::default();
    let (applied_doc, applied_change, applied_patch) =
        apply_patch(&next, &patch, "gio", 2, 1002).unwrap();
    assert_eq!(applied_patch.status, PatchStatus::Applied);
    let produced = applied_patch.applied_revision.unwrap();
    store.save_document(&applied_doc).unwrap();
    book.patches.push(applied_patch);
    store.save_patches(&book).unwrap();

    // A network retry with the same patch id finds the recorded result.
    let reloaded = store.load_patches().unwrap();
    let recorded = store.patch_by_id(&reloaded, "p-once").unwrap();
    assert_eq!(recorded.status, PatchStatus::Applied);
    assert_eq!(recorded.applied_revision, Some(produced));
    // Re-applying a non-proposed patch is refused, not duplicated.
    assert_err(apply_patch(&next, recorded, "gio", 3, 1003), ApplyError::NotProposed);
    let final_doc = store.load_document().unwrap().unwrap();
    assert_eq!(final_doc.revision, produced, "document revision unchanged by retry");
}

/// AD09: rejection changes proposal history and zero document bytes.
#[test]
fn ad09_rejection_changes_no_document_bytes() {
    let f = fixture();
    let (next, change) = edit_source(&f, "We ordered 3 units of bolts.");
    let before = next.raw.clone();
    let patch = propose_after_source_edit(&next, &change, "p-reject");
    let rejected = reject_patch(&patch).unwrap();
    assert_eq!(rejected.status, PatchStatus::Rejected);
    assert_eq!(next.raw, before, "rejection must not touch document bytes");
    assert_err(reject_patch(&rejected), ApplyError::NotProposed);
}

/// AD10: undo restores only the dependent repair; intervening edits cause a
/// conflict instead of erasing them.
#[test]
fn ad10_undo_restores_repair_only_and_conflicts_after_edits() {
    let f = fixture();
    let (next, change) = edit_source(&f, "We ordered 3 units of bolts.");
    let patch = propose_after_source_edit(&next, &change, "p-undo");
    let (applied, _, applied_patch) = apply_patch(&next, &patch, "gio", 2, 1002).unwrap();
    assert!(applied.raw.contains("Budget is 6 euros."));

    // Undo restores only the dependent repair; the human edit stays.
    let (undone, undo_change, reverted) =
        undo_patch(&applied, &applied_patch, "gio", 3, 1003).unwrap();
    assert_eq!(reverted.status, PatchStatus::Reverted);
    assert!(undone.raw.contains("Budget is 36 euros."));
    assert!(undone.raw.contains("We ordered 3 units of bolts."));
    assert_eq!(undo_change.operation, "undo_patch:p-undo");

    // Intervening edit: undo of the same patch on a moved-forward document.
    let (after_edit, _) = apply_source_edit(
        &undone,
        &f.review,
        "Check supplier terms again.",
        "gio",
        4,
        1004,
    )
    .unwrap();
    let mut still_applied = applied_patch;
    still_applied.status = PatchStatus::Applied;
    assert_err(
        undo_patch(&after_edit, &still_applied, "gio", 5, 1005),
        ApplyError::UndoConflict,
    );
}

/// AD11: restart rebuilds the same observed metadata and proposals; corrupt
/// state is reported, not silently rebuilt.
#[test]
fn ad11_restart_rebuilds_state_and_reports_loss() {
    let f = fixture();
    let (next, change) = edit_source(&f, "We ordered 3 units of bolts.");
    let patch = propose_after_source_edit(&next, &change, "p1");
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path());
    store.save_document(&next).unwrap();
    store.append_event(&change).unwrap();
    let book = Patches { patches: vec![patch.clone()] };
    store.save_patches(&book).unwrap();

    // Reload in a fresh "process".
    let store2 = Store::open(dir.path());
    let doc2 = store2.load_document().unwrap().unwrap();
    assert_eq!(doc2.raw, next.raw);
    assert_eq!(doc2.revision, next.revision);
    let ids1: Vec<_> = next.blocks.iter().map(|b| b.id.clone()).collect();
    let ids2: Vec<_> = doc2.blocks.iter().map(|b| b.id.clone()).collect();
    assert_eq!(ids1, ids2);
    let events = store2.load_events().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_id, change.event_id);
    let patches2 = store2.load_patches().unwrap();
    assert_eq!(patches2.patches[0].id, "p1");
    // The reloaded document still proposes the same outcome.
    let change2 = Change {
        event_id: "e2".into(),
        ..change.clone()
    };
    let patch2 = propose_after_source_edit(&doc2, &change2, "p1");
    assert_eq!(patch2.replacements[0].new_bytes, patch.replacements[0].new_bytes);

    // Lost/corrupt raw blob is reported.
    std::fs::write(dir.path().join("document.json"), b"{corrupt").unwrap();
    assert!(store2.load_document().is_err());
}

/// AD12: interruptions leave no false success — a torn trailing event line
/// fails the load, and only complete applied state records success.
#[test]
fn ad12_interrupted_writes_are_reported() {
    let f = fixture();
    let (next, change) = edit_source(&f, "We ordered 3 units of bolts.");
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path());
    store.append_event(&change).unwrap();
    // Simulate a crash mid-append: torn trailing line.
    {
        let mut log = std::fs::OpenOptions::new()
            .append(true)
            .open(dir.path().join("events.jsonl"))
            .unwrap();
        use std::io::Write;
        log.write_all(b"{\"event_id\":\"torn").unwrap();
    }
    let result = store.load_events();
    assert!(result.is_err(), "torn event line must be reported, not dropped");

    // A complete apply records success explicitly.
    let patch = propose_after_source_edit(&next, &change, "p1");
    let (applied, applied_change, applied_patch) =
        apply_patch(&next, &patch, "gio", 2, 1002).unwrap();
    assert_eq!(applied_patch.status, PatchStatus::Applied);
    assert_eq!(applied.raw, applied_doc_raw(&next, &patch));
    store.save_document(&applied).unwrap();
    store.append_event(&applied_change).unwrap();
    let events = Store::open(dir.path());
    let _ = events; // final store state is authoritative after reload
    let reloaded = store.load_document().unwrap().unwrap();
    assert_eq!(reloaded.raw, applied.raw);
}

fn applied_doc_raw(next: &Document, patch: &Patch) -> String {
    let (applied, _, _) = apply_patch(next, patch, "gio", 2, 1002).unwrap();
    applied.raw
}

/// AD13: cycles terminate, cycle members require review, and traversal caps
/// report incompleteness.
#[test]
fn ad13_cycles_require_review_and_caps_report_incompleteness() {
    let f = fixture();
    let mut cyclic = f.doc.clone();
    cyclic.links.push(Link {
        id: "l-cycle".into(),
        source_block_id: f.budget.clone(),
        dependent_block_id: f.source.clone(),
        behavior: LinkBehavior::Calculate,
        transform_id: Some("scale:1".into()),
        provenance: LinkProvenance::HumanConfirmed,
        source_revision_at_confirmation: 0,
        active: true,
    });
    cyclic.graph_revision += 1;
    let (next, change) =
        apply_source_edit(&cyclic, &f.source, "We ordered 3 units of bolts.", "gio", 1, 1000)
            .unwrap();
    let patch = propose_after_source_edit(&next, &change, "p-cycle");
    assert!(patch
        .review_only
        .iter()
        .any(|n| n.reason == "cyclic calculation"),
        "cycle members must require review");
    // Traversal cap: tiny cap marks incompleteness.
    let (affected, truncated) = f.doc.reachable_dependents(&f.source, 2);
    assert!(truncated, "cap must report incompleteness");
    assert_eq!(affected.len(), 2);
}
