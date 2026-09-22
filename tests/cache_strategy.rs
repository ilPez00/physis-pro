//! Cache-strategy probe: stale-serve (SWR) vs rebuild (ISR) on the web content index.
//!
//! `#[ignore]`d: it builds a synthetic corpus and times both legs of the
//! `ensure_index` decision in `src/web/content.rs` — the metadata-only
//! `scan_signature` freshness check (serve-stale leg) against a full
//! `ingest_all` + `ContentIndex::build` (rebuild leg). Run with:
//!
//! ```bash
//! cargo test --test cache_strategy -- --ignored --nocapture
//! ```
//!
//! This is a measurement, not an assertion suite: it prints both timings and
//! their ratio, so the "is the freshness check actually cheap" claim stays
//! measured rather than remembered. One smoke assertion holds the floor: the
//! signature check must beat the rebuild by at least 2x on the fixture, or
//! the SWR-shaped fast path is not earning its keep.

use std::time::Instant;

use physis_pro::content::{scan_signature, ContentIndex};
use physis_pro::ingest::ingest_all;

fn fixture_corpus(dir: &std::path::Path, files: usize) {
    for i in 0..files {
        let body = format!(
            "# note {i}\n\n{}\n",
            (0..20)
                .map(|j| format!("line {j} of note {i} with some searchable words"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        std::fs::write(dir.join(format!("note-{i:04}.md")), body).unwrap();
    }
}

#[test]
#[ignore]
fn stale_check_cost_vs_rebuild_cost() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fixture_corpus(root, 200);

    // Warm the filesystem once so cold page cache does not decide the race.
    let _ = scan_signature(root);

    let t0 = Instant::now();
    let sig = scan_signature(root);
    let check_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let t0 = Instant::now();
    let docs = ingest_all(std::slice::from_ref(&root.to_path_buf()));
    let addresses = vec![None; docs.len()];
    let index = ContentIndex::build(root, &docs, &addresses, Vec::new());
    let build_ms = t0.elapsed().as_secs_f64() * 1000.0;

    assert_eq!(index.signature, sig, "rebuilt index signs itself");
    println!("files: {}", docs.len());
    println!("signature check (serve-stale leg): {check_ms:.2} ms");
    println!("full ingest+build (rebuild leg):   {build_ms:.2} ms");
    println!("ratio rebuild/check: {:.1}x", build_ms / check_ms.max(1e-9));
    assert!(
        build_ms > 2.0 * check_ms,
        "freshness check must stay much cheaper than a rebuild"
    );
}
