use llm_coherence_scan::capture::RunRecord;
use llm_coherence_scan::engine;
use llm_coherence_scan::risk;

fn load(name: &str) -> RunRecord {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    RunRecord::load(&p).expect("load fixture")
}

#[test]
fn clean_run_has_low_risk() {
    let r = load("clean_run.json");
    let s = engine::score(&r);
    let risk = risk::coherence_risk(&s);
    assert!(risk.total < 0.5, "clean risk={}", risk.total);
    assert!(risk.breakdown.semantic < 0.5);
}

#[test]
fn injected_run_has_higher_risk_than_clean() {
    let clean = risk::coherence_risk(&engine::score(&load("clean_run.json")));
    let injected = risk::coherence_risk(&engine::score(&load("injected_run.json")));
    assert!(
        injected.breakdown.semantic >= clean.breakdown.semantic,
        "injected.semantic={} should be >= clean.semantic={}",
        injected.breakdown.semantic,
        clean.breakdown.semantic
    );
    assert!(injected.total > clean.total);
}

#[test]
fn drift_run_has_high_risk() {
    let r = load("drift_run.json");
    let risk = risk::coherence_risk(&engine::score(&r));
    assert!(risk.total > 0.5, "drift risk={}", risk.total);
}
