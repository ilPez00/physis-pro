//! Tests for the SECOM industrial benchmark pipeline.

use std::collections::HashMap;
use std::path::PathBuf;

use physis_core::embed::RandomProjectionEmbedder;
use physis_pro::ported::quality::QualityTracker;
use physis_pro::industrial_benchmark::secom::*;

#[test]
fn parse_secom_csv_counts_rows_and_features() {
    let csv = "\
outcome,f1,f2,f3
1,1.0,2.0,3.0
-1,4.0,,6.0
1,7.0,8.0,9.0
";
    let obs = parse_secom_csv(csv, None);
    assert_eq!(obs.len(), 3);
    assert_eq!(obs[0].features.len(), 3);
    assert_eq!(obs[0].outcome, Some(true));
    assert_eq!(obs[1].outcome, Some(false));
    assert_eq!(obs[1].missing_count, 1);
}

#[test]
fn parse_secom_csv_respects_max_rows() {
    let csv = "\
outcome,f1
1,1.0
-1,2.0
1,3.0
-1,4.0
1,5.0
";
    let obs = parse_secom_csv(csv, Some(3));
    assert_eq!(obs.len(), 3);
}

#[test]
fn observation_to_text_is_deterministic() {
    let obs = SecomObservation {
        id: 1,
        features: vec![Some(1.0), None, Some(3.0)],
        feature_names: vec!["temp".into(), "press".into(), "flow".into()],
        outcome: Some(false),
        missing_count: 1,
        textual: String::new(),
    };
    let t1 = observation_to_text(&obs);
    let t2 = observation_to_text(&obs);
    assert_eq!(t1, t2);
    assert!(t1.contains("Production run 1"));
    assert!(t1.contains("temp: 1.0000"));
    assert!(t1.contains("press: missing"));
}

#[test]
fn train_test_split_is_reproducible_with_seed() {
    let obs = vec![
        SecomObservation {
            id: 1,
            features: vec![],
            feature_names: vec![],
            outcome: Some(true),
            missing_count: 0,
            textual: String::new(),
        },
        SecomObservation {
            id: 2,
            features: vec![],
            feature_names: vec![],
            outcome: Some(false),
            missing_count: 0,
            textual: String::new(),
        },
        SecomObservation {
            id: 3,
            features: vec![],
            feature_names: vec![],
            outcome: Some(true),
            missing_count: 0,
            textual: String::new(),
        },
        SecomObservation {
            id: 4,
            features: vec![],
            feature_names: vec![],
            outcome: Some(false),
            missing_count: 0,
            textual: String::new(),
        },
    ];
    let (a1, b1) = train_test_split(&obs, 0.5, 42);
    let (a2, b2) = train_test_split(&obs, 0.5, 42);
    assert_eq!(a1.len(), a2.len());
    assert_eq!(b1.len(), b2.len());
    let ids1: Vec<usize> = a1.iter().chain(b1.iter()).map(|o| o.id).collect();
    let ids2: Vec<usize> = a2.iter().chain(b2.iter()).map(|o| o.id).collect();
    assert_eq!(ids1, ids2);
}

#[test]
fn benchmark_runs_end_to_end_with_sample() {
    let config = BenchmarkConfig {
        seed: 42,
        train_ratio: 0.5,
        max_observations: Some(10),
        embed_dim: 64,
        output_dir: std::env::temp_dir().join("physis-bench-test"),
        no_feedback: false,
        embedder: String::from("random"),
    };
    let run = run_benchmark(config).expect("benchmark must run");
    assert!(!run.baseline.is_empty());
    assert_eq!(run.baseline.len(), run.post_feedback.len());
    assert!(run.dataset_info.total_observations > 0);
}

#[test]
fn observation_to_text_rich_includes_deviations() {
    let obs = SecomObservation {
        id: 1,
        features: vec![Some(10.0), Some(2.0), Some(3.0)],
        feature_names: vec!["temp".into(), "press".into(), "flow".into()],
        outcome: Some(false),
        missing_count: 0,
        textual: String::new(),
    };
    let stats = FeatureStats {
        means: vec![5.0, 1.0, 3.0],
        stds: vec![1.0, 0.5, 1.0],
        mins: vec![0.0, 0.0, 0.0],
        maxs: vec![10.0, 2.0, 5.0],
    };
    let t = observation_to_text_rich(&obs, Some(&stats));
    assert!(
        t.contains("elevated"),
        "should describe elevated params: {t}"
    );
    assert!(
        t.contains("Process anomalies detected"),
        "should have anomalies section: {t}"
    );
}

#[test]
fn build_embedder_returns_closure() {
    let config = BenchmarkConfig {
        seed: 42,
        train_ratio: 0.5,
        max_observations: None,
        embed_dim: 64,
        output_dir: PathBuf::from("."),
        no_feedback: true,
        embedder: String::from("random"),
    };
    let embed_fn = build_embedder(&config);
    let v1 = embed_fn("hello");
    let v2 = embed_fn("hello");
    assert_eq!(v1, v2, "embedder must be deterministic");
    assert_eq!(v1.len(), 64);
}

#[test]
fn quality_feedback_modifies_scores() {
    let mut quality = QualityTracker::new(Box::new(RandomProjectionEmbedder::new(64)));
    let mut centroids = HashMap::new();
    centroids.insert("FABRICATE\x00WORK".to_string(), vec![0.5; 64]);
    let before = quality.adjust_score("FABRICATE\x00WORK", 0.8);
    quality.report_failure("bad classification", &centroids, None);
    let after = quality.adjust_score("FABRICATE\x00WORK", 0.8);
    assert!(after < before, "penalty must reduce adjusted score");
}

#[test]
fn result_serializes_to_json() {
    let result = ClassificationResult {
        observation_id: 1,
        predicted_cell: "FABRICATE\x00WORK".to_string(),
        raw_score: 0.8,
        adjusted_score: 0.7,
        ground_truth: Some(true),
        correct: Some(true),
        top_cells: vec![],
        inference_time_ms: 1.5,
    };
    let json = serde_json::to_string(&result).expect("must serialize");
    assert!(json.contains("\"observation_id\":1"));
}
