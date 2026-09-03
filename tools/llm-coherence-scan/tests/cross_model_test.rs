use llm_coherence_scan::baseline;
use llm_coherence_scan::capture::RunRecord;
use llm_coherence_scan::drift;
use std::path::Path;

fn load(name: &str) -> RunRecord {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    RunRecord::load(&p).expect("load")
}

#[test]
fn cross_model_drift_picks_up_other_model_same_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let db = sled::open(dir.path().join("h")).unwrap();

    let mut a = load("clean_run.json");
    a.model = "model-A".into();
    baseline::record(&db, &a).unwrap();

    let mut b = load("injected_run.json");
    b.model = "model-B".into();
    baseline::record(&db, &b).unwrap();

    let d = drift::model_drift(&b, &db, None).unwrap();
    let cross = d.components.cross_model.expect("cross_model present");
    assert!(cross > 0.05, "cross={cross}");
}
