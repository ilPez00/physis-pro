//! Phase 26 live demo: the adaptive document workflow from PLAN.md §26.1.
//!
//! Change a source quantity from 18 units to 3. Preview linked values and
//! exact calculations changing. See related prose flagged for review. Accept
//! the patch. Undo it. Reload the store and show the same history.
//!
//! Run: cargo run --offline --example adaptive_doc_demo [-- /tmp/physis-doc-demo]

use physis_pro::adaptive_document::proposal::*;
use physis_pro::adaptive_document::store::{Patches, Store};
use physis_pro::adaptive_document::*;

const REPORT: &str = "# Field report\n\nWe ordered 18 units of bolts.\n\nBudget is 36 euros.\n\nCheck supplier terms.\n";

fn banner(what: &str) {
    println!("\n=== {what} ===");
}

fn main() {
    let root = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/physis-doc-demo".to_string());
    let store = Store::open(std::path::Path::new(&root));
    std::fs::remove_dir_all(&root).ok();

    banner("1. Document parsed (blocks + confirmed links)");
    let mut doc = parse_blocks("field-report", 0, REPORT);
    let source = doc.blocks[1].id.clone();
    let budget = doc.blocks[2].id.clone();
    let review = doc.blocks[3].id.clone();
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
    ];
    doc.graph_revision = 1;
    for b in &doc.blocks {
        println!("  [{:?}] {:?}", b.kind, b.text(REPORT));
    }

    banner("2. Human edit: source quantity 18 -> 3");
    let (next, change) = apply_source_edit(&doc, &source, "We ordered 3 units of bolts.", "gio", 1, 1000)
        .expect("edit must produce a change");
    println!("  event {} : rev {} -> {}", change.event_id, change.before_revision, change.after_revision);
    store.save_document(&next).unwrap();
    store.append_event(&change).unwrap();

    banner("3. Proposal: preview linked changes");
    let patch = propose_after_source_edit(&next, &change, "p1");
    for r in &patch.replacements {
        println!("  [calc] {:?} : {:?} -> {:?}  (reasons {:?})", r.block_id, r.expected_old_bytes, r.new_bytes, r.reasons);
    }
    for n in &patch.review_only {
        println!("  [review] {:?} : {}", n.block_id, n.reason);
    }
    store.save_patches(&Patches { patches: vec![patch.clone()] }).unwrap();

    banner("4. Accept the patch");
    let (applied, applied_change, applied_patch) = apply_patch(&next, &patch, "gio", 2, 1002).unwrap();
    println!("{}", applied.raw);
    store.save_document(&applied).unwrap();
    store.append_event(&applied_change).unwrap();
    store.save_patches(&Patches { patches: vec![applied_patch.clone()] }).unwrap();

    banner("5. Undo the repair (human edit survives)");
    let (undone, undo_change, reverted) = undo_patch(&applied, &applied_patch, "gio", 3, 1003).unwrap();
    println!("  {}", undone.raw.trim());
    println!("  event {} ({})", undo_change.event_id, undo_change.operation);
    store.save_document(&undone).unwrap();
    store.append_event(&undo_change).unwrap();
    store.save_patches(&Patches { patches: vec![reverted] }).unwrap();

    banner("6. Reload the store: same history, replayed from disk");
    let store2 = Store::open(std::path::Path::new(&root));
    let doc2 = store2.load_document().unwrap().unwrap();
    let events = store2.load_events().unwrap();
    let patches = store2.load_patches().unwrap();
    println!("  document at revision {}, raw bytes preserved: {}", doc2.revision, doc2.raw == undone.raw);
    println!("  event log (store sequence): {}", events.iter().map(|e| e.operation.as_str()).collect::<Vec<_>>().join(" -> "));
    println!("  patch p1 status: {:?}", patches.patches[0].status);
}
