//! Quality feedback penalty e2e - moved from physis-core
//! tests/e2e_verification.rs section 5 by the 2026-09-14 Core/Product split
//! (the tracker is product; the penalty measurement is documented as research).

use physis_core::classify::CellClassifier;
use physis_core::embed::RandomProjectionEmbedder;
use physis_core::ontology::OntologyLoader;
use physis_pro::ported::quality::QualityTracker;

#[test]
fn quality_feedback_penalty_end_to_end() {
    let embedder = RandomProjectionEmbedder::new(64);
    let ontology = OntologyLoader::load_all();
    let classifier = CellClassifier::build(&ontology, &embedder);
    let sample_text = "Spindle bearing thermal runaway caused by lubricant breakdown";
    let scores = classifier.classify_text(sample_text, &embedder);
    let top_cell = &scores[0];

    let mut quality = QualityTracker::new(Box::new(RandomProjectionEmbedder::new(64)));
    let cell_key = format!("{}\x00{}", top_cell.domain, top_cell.mode);
    let initial_score = top_cell.score;

    quality.penalize_cell(&cell_key, 0.35);
    let penalty = quality
        .cell_penalties
        .get(&cell_key)
        .copied()
        .unwrap_or(0.0);
    assert!(penalty > 0.0, "Cell must receive quality penalty");

    let adjusted_score = quality.adjust_score(&cell_key, initial_score);
    assert!(
        adjusted_score < initial_score,
        "Adjusted score must be penalized"
    );
}
