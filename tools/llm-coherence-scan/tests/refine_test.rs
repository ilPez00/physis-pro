use llm_coherence_scan::capture::RunRecord;
use llm_coherence_scan::refine;
use std::path::Path;

fn load(name: &str) -> RunRecord {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    RunRecord::load(&p).expect("load")
}

#[test]
fn repeated_edit_grows_salience() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = refine::SalienceStore::default();
    let now = chrono::Utc::now();
    let rec = load("clean_run.json");
    for _ in 0..3 {
        refine::update_outcome_salience(&rec, &mut store, now).unwrap();
    }
    let path = refine::salience_path(dir.path());
    store.save(&path).unwrap();
    let loaded = refine::SalienceStore::load(&path).unwrap();
    let cell = refine::cell_for(&rec.output.edited_files[0].path);
    let score = loaded
        .entries
        .get(&cell)
        .map(|e| e.raw_score)
        .unwrap_or(0.0);
    assert!(
        score >= 3.0,
        "salience should grow with repeated edits, got {score}"
    );
}

#[test]
fn edit_then_delete_offsets_salience() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = refine::SalienceStore::default();
    let now = chrono::Utc::now();
    let mut rec = load("clean_run.json");
    refine::update_outcome_salience(&rec, &mut store, now).unwrap();
    rec.output.edited_files.clear();
    rec.output.deleted_files = vec![rec.input.rag_files[0].path.clone()];
    refine::update_outcome_salience(&rec, &mut store, now).unwrap();
    let path = refine::salience_path(dir.path());
    store.save(&path).unwrap();
    let loaded = refine::SalienceStore::load(&path).unwrap();
    let cell = refine::cell_for(&rec.output.deleted_files[0]);
    let score = loaded
        .entries
        .get(&cell)
        .map(|e| e.raw_score)
        .unwrap_or(0.0);
    assert!(
        (score - 0.0).abs() < 1e-3,
        "edit+delete should cancel, got {score}"
    );
}

#[test]
fn dream_flags_silent_failure_on_high_risk_no_asserted() {
    let rec = load("drift_run.json");
    let dream = refine::run_dream_pass(&rec, 0.9, &[]);
    assert!(!dream.silent_failures.is_empty());
}

#[test]
fn discovery_emits_proposal_for_unseen_cell() {
    let rec = load("drift_run.json");
    let store = refine::SalienceStore::default();
    let proposals = refine::discover_proposals(&rec, &store, chrono::Utc::now());
    assert!(!proposals.is_empty(), "expected at least one proposal");
}
