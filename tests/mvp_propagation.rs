//! Propagation-fixture test: the committed demo repository must genuinely
//! propagate. `fixtures/mvp_propagation` defines the budget once in
//! `src/retrieval.rs` and uses it from `src/agent.rs`, `src/context.rs`, and
//! `README.md`. The shared dependents rule must return exactly that set.
//!
//! Read-only: never writes into the fixture. The demo script copies the
//! fixture to a temp dir before editing, so the committed budget (500) stays
//! stable for this test.

use std::path::PathBuf;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/mvp_propagation")
}

#[test]
fn fixture_carries_the_documented_budget() {
    let root = fixture_root();
    let text = std::fs::read_to_string(root.join("src/retrieval.rs")).unwrap();
    assert!(
        text.contains("500"),
        "fixture drifted — reset src/retrieval.rs to budget 500:\n{text}"
    );
    assert!(
        text.contains("retrieval_budget"),
        "fixture drifted — the symbol must stay retrieval_budget:\n{text}"
    );
}

#[test]
fn retrieval_change_propagates_to_agent_context_and_readme() {
    let root = fixture_root();
    let changed = vec!["src/retrieval.rs".to_string()];
    let syms = physis_pro::mvp::changed_symbols(&root, &changed);
    assert!(
        syms.contains(&"retrieval_budget".to_string()),
        "symbol extraction lost the fixture's only symbol: {syms:?}"
    );
    let deps = physis_pro::mvp::affected_dependents(&root, &changed, &syms);
    for d in ["src/agent.rs", "src/context.rs", "README.md"] {
        assert!(deps.contains(&d.to_string()), "missing {d}, got {deps:?}");
    }
    assert_eq!(
        deps.len(),
        3,
        "the demo promises a compact set — a fourth dependent voids the script: {deps:?}"
    );
}
