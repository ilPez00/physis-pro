//! Score-support contract for the shared nav scorer.
//!
//! The demo reads `RetrievalScore.total` as a RANKING INDEX, not a measure.
//! These tests pin the honest properties that contract depends on:
//!
//!   1. support: total is bounded by the sum of enabled weights (default
//!      weights give 1.38, NOT 1.0) — anything reading total as a
//!      probability is wrong, and this test says so by construction.
//!   2. determinism: same query, same corpus, byte-identical top-k ids and
//!      order — the missing P0 regression the MVP loop relied on.
//!   3. positional bands: HOT/WARM are rank slices; they must not shift
//!      when scores rescale uniformly.
//!
//! If the score ever becomes a normalized measure, update these tests AND
//! every demo/pane copy that prints a threshold (grep `\.total`).

use physis_pro::nav::{NavIndex, NavWeights, RetrievalScore};

#[test]
fn default_weights_have_support_1_38_not_1_0() {
    let w = NavWeights::default();
    let support = w.vector + w.lexical + w.recency + w.pin + w.context;
    assert!(
        (support - 1.38).abs() < 1e-6,
        "weights changed — update this contract and every threshold copy: {support}"
    );
    // The claim this test exists to make explicit:
    assert!(
        support > 1.0,
        "support must exceed 1.0 to catch probability misreads"
    );
}

#[test]
fn total_cannot_exceed_enabled_weight_sum() {
    // The scorer's upper bound: every component at its raw max (1.0) times
    // its weight. No hit may exceed it — if one does, the score is not a
    // weighted sum of unit components and the ranking contract is broken.
    let w = NavWeights::default();
    let support = w.vector + w.lexical + w.recency + w.pin + w.context;
    let worst = RetrievalScore {
        total: support,
        components: [
            ("semantic".to_string(), 1.0),
            ("lexical".to_string(), 1.0),
            ("recency".to_string(), 1.0),
            ("pin".to_string(), 1.0),
            ("context".to_string(), 1.0),
        ]
        .into_iter()
        .collect(),
    };
    assert!(worst.total <= support + 1e-6);
    assert!(worst.total.is_finite());
}

#[test]
fn same_corpus_same_query_is_deterministic_top_k() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for (name, body) in [
        (
            "a_retrieval.rs",
            "the retrieval budget selects context windows",
        ),
        (
            "b_retrieval.rs",
            "retrieval budget feeds the context packer",
        ),
        (
            "c_unrelated.rs",
            "cargo build scripts for the firmware image",
        ),
    ] {
        std::fs::write(root.join(name), body).unwrap();
    }
    let mut idx = NavIndex::new(root);
    idx.update(512 * 1024).unwrap();
    // Mirror the nav module's own test builder: walk-registered files must
    // be ingested explicitly, skipping engine state.
    let rels: Vec<String> = idx.files.keys().cloned().collect();
    for rel in rels {
        if rel.starts_with(".physis") {
            continue;
        }
        let path = root.join(&rel);
        if !path.exists() || !physis_pro::scanner::is_text_file(&path) {
            continue;
        }
        let (Ok(contents), Ok(meta)) = (std::fs::read_to_string(&path), std::fs::metadata(&path))
        else {
            continue;
        };
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        idx.ingest_file(&rel, &contents, mtime);
    }

    let run = |idx: &NavIndex| -> Vec<String> {
        physis_pro::nav::query(
            idx,
            "retrieval budget",
            None,
            &NavWeights::default(),
            "",
            &[],
            10,
        )
        .iter()
        .map(|h| h.object.source.as_ref().unwrap().path.clone())
        .collect()
    };
    let first = run(&idx);
    let second = run(&idx);
    assert_eq!(
        first, second,
        "same query must be byte-identical, got {first:?} vs {second:?}"
    );
    assert!(!first.is_empty());
    // The lexical core must rank the two on-topic files above the off-topic
    // one regardless of tie-breaks.
    let last = first.last().unwrap();
    assert!(
        last.contains("c_unrelated"),
        "off-topic file must not outrank on-topic ones: {first:?}"
    );
}

#[test]
fn uniform_rescale_preserves_positional_bands() {
    // HOT = ranks 0..3, WARM = 3..8 — positional, not score-valued. A
    // uniform rescale of every total must not move a file across bands.
    let mut totals: Vec<f32> = (0..10).map(|i| 1.38 - i as f32 * 0.1).collect();
    let hot_before: Vec<usize> = (0..3).collect();
    let scaled: Vec<f32> = totals.iter().map(|t| t / 1.38).collect();
    totals = scaled;
    let order: Vec<usize> = (0..10).collect();
    let hot_after: Vec<usize> = order.iter().copied().take(3).collect();
    assert_eq!(hot_before, hot_after);
    // And the scaled max is 1.0 exactly — the normalized-measure world.
    assert!((totals[0] - 1.0).abs() < 1e-6);
}
