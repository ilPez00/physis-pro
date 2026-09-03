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

fn tmp_db() -> (tempfile::TempDir, sled::Db) {
    let dir = tempfile::tempdir().unwrap();
    let db = sled::open(dir.path().join("h")).unwrap();
    (dir, db)
}

#[test]
fn first_run_drift_is_null() {
    let (_dir, db) = tmp_db();
    let r = load("clean_run.json");
    let d = drift::model_drift(&r, &db, None).unwrap();
    assert!(d.components.local.is_none());
    assert!(d.components.reference.is_none());
    assert!(d.components.cross_model.is_none());
    assert!(d.total.is_none());
}

#[test]
fn local_drift_grows_after_history() {
    let (_dir, db) = tmp_db();
    let mut clean = load("clean_run.json");
    clean.created_at = Some(chrono::Utc::now() - chrono::Duration::seconds(2));
    baseline::record(&db, &clean).unwrap();
    let mut drift_r = load("drift_run.json");
    drift_r.model = clean.model.clone();
    drift_r.created_at = Some(chrono::Utc::now() - chrono::Duration::seconds(1));
    baseline::record(&db, &drift_r).unwrap();
    drift_r.created_at = Some(chrono::Utc::now());
    baseline::record(&db, &drift_r).unwrap();
    let d = drift::model_drift(&drift_r, &db, None).unwrap();
    assert!(d.components.local.unwrap() > 0.0);
}
