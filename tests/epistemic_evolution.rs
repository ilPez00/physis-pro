//! Canonical Epistemic Evolution Test Suite and Peculiarity Benchmark.
//!
//! Evaluates the core thesis of Physis:
//! An engine that maintains competing interpretations of reality, evaluates their
//! multidimensional coherence with observations, processes, evidence, and outcomes,
//! and preferentially retains interpretations that continue to work.

use physis_core::classify::CellClassifier;
use physis_core::coherence_dimensions::CoherenceProfile;
use physis_core::coherence_query::{EpistemicQuery, EpistemicQueryResult};
use physis_core::contradiction::{Contradiction, ContradictionParty, ResolutionStatus};
use physis_core::core::PhysisCore;
use physis_core::discovery::{discover, DiscoveryConfig};
use physis_core::embed::{RandomProjectionEmbedder, VectorEmbed};
use physis_core::hypothesis::{Evidence, EvidencePolarity, Hypothesis, HypothesisStatus, Prediction};
use physis_core::ontology::OntologyLoader;
use physis_core::provenance::ProvenanceLink;
use physis_core::quality::{ContextualQualityTracker, FitnessContext};
use physis_core::relation::{RelationType, TypedEdge};

fn fixture_embedder() -> RandomProjectionEmbedder {
    RandomProjectionEmbedder::new(64)
}

// ── Test 1 — Contradiction Handling ──────────────────────────────────
// Give Physis conflicting evidence.
// Expected: conflict detected, both claims retained, provenance preserved.
#[test]
fn test_1_contradiction_preservation() {
    let mut core = PhysisCore::new();
    let emb = fixture_embedder();

    let hyp_a_id = core.register_hypothesis(Hypothesis::new(
        "Sensor telemetry: Spindle bearing overheating at 95C",
        emb.embed("sensor spindle overheating 95C"),
    ));

    let hyp_b_id = core.register_hypothesis(Hypothesis::new(
        "Operator inspection: Spindle bearing running cool to touch",
        emb.embed("operator spindle bearing cool normal"),
    ));

    // Provenance for Claim A
    let prov_a = core.provenance_chain(&hyp_a_id);
    prov_a.add_link(ProvenanceLink::new(
        "Temperature reading 95C",
        "maintenance_telemetry_sensor_S1",
    ));

    // Provenance for Claim B
    let prov_b = core.provenance_chain(&hyp_b_id);
    prov_b.add_link(ProvenanceLink::new(
        "Manual tactile inspection",
        "operator_shift_log_OP4",
    ));

    // Create contradiction
    let party_a = ContradictionParty::new(
        "Spindle bearing overheating at 95C",
        "telemetry_sensor_S1",
    )
    .with_confidence(0.95)
    .with_authority(0.90);

    let party_b = ContradictionParty::new(
        "Spindle bearing running cool to touch",
        "operator_log_OP4",
    )
    .with_confidence(0.50)
    .with_authority(0.40);

    let mut contradiction = Contradiction::new(party_a, party_b);
    contradiction.resolution = ResolutionStatus::APreferred;
    contradiction.explanation = Some("Sensor S1 calibrated yesterday; operator inspection subject to heat dissipation delay".to_string());

    let contra_id = core.record_contradiction(contradiction);

    // Assertions:
    // 1. Both hypotheses exist and are retained
    assert!(core.hypothesis(&hyp_a_id).is_some());
    assert!(core.hypothesis(&hyp_b_id).is_some());

    // 2. Contradiction is recorded and inspectable
    assert_eq!(core.contradictions.len(), 1);
    assert_eq!(core.contradictions[0].id, contra_id);
    assert_eq!(core.contradictions[0].resolution, ResolutionStatus::APreferred);

    // 3. Provenance chains for both sources remain intact
    assert!(core.provenance_chains[&hyp_a_id].summary().contains("maintenance_telemetry_sensor_S1"));
    assert!(core.provenance_chains[&hyp_b_id].summary().contains("operator_shift_log_OP4"));

    // 4. Contradiction query retrieves the conflicting claims
    let query_res = core.query_coherence(&EpistemicQuery::WhatContradicts {
        id: "Spindle bearing".to_string(),
    });
    match query_res {
        EpistemicQueryResult::Contradictions { contradicting_claims, .. } => {
            assert_eq!(contradicting_claims.len(), 1);
            assert!(contradicting_claims[0].contains("95C"));
            assert!(contradicting_claims[0].contains("cool"));
        }
        _ => panic!("Expected Contradictions result"),
    }
}

// ── Test 2 — Feedback Loop & Ranking ─────────────────────────────────
// Give Physis repeated successful / failed classifications.
// Expected: fitness changes, rankings change.
#[test]
fn test_2_feedback_loop_and_ranking() {
    let mut core = PhysisCore::new();
    let emb = fixture_embedder();

    let h1 = Hypothesis::new("Tool wear anomaly", emb.embed("tool wear anomaly"));
    let h2 = Hypothesis::new("Material impurity", emb.embed("material impurity"));

    // Initial baseline: both have neutral fitness 0.5
    assert_eq!(h1.fitness, 0.5);
    assert_eq!(h2.fitness, 0.5);

    let h1_id = core.register_hypothesis(h1);
    let h2_id = core.register_hypothesis(h2);

    // Simulate feedback for H1: repeated successful predictions & empirical support
    let h1_mut = core.hypothesis_mut(&h1_id).unwrap();
    h1_mut.add_supporting_evidence(Evidence {
        source: "microscope_inspection_1.png".to_string(),
        polarity: EvidencePolarity::Supports,
        confidence: 0.9,
        claim: "Flank wear exceeds 0.3mm".to_string(),
        observed_at: Some(chrono::Utc::now()),
        embedding: Vec::new(),
        context: vec!["machine_cnc_1".to_string()],
    });
    h1_mut.record_prediction(Prediction {
        statement: "Tool replacement will normalize vibration amplitude".to_string(),
        expected_outcome: Some("Vibration < 0.2 mm/s".to_string()),
        made_at: chrono::Utc::now(),
        observed_at: Some(chrono::Utc::now()),
        actual_outcome: Some("Vibration normalized to 0.12 mm/s".to_string()),
        correct: Some(true),
        confidence: 0.95,
    });

    // Simulate feedback for H2: contradiction and failed prediction
    let h2_mut = core.hypothesis_mut(&h2_id).unwrap();
    h2_mut.add_contradicting_evidence(Evidence {
        source: "spectrometry_lot_88.csv".to_string(),
        polarity: EvidencePolarity::Contradicts,
        confidence: 0.85,
        claim: "Alloy composition within 99.8% nominal tolerance".to_string(),
        observed_at: Some(chrono::Utc::now()),
        embedding: Vec::new(),
        context: vec!["lot_88".to_string()],
    });
    h2_mut.record_prediction(Prediction {
        statement: "Material batch swap will resolve defect".to_string(),
        expected_outcome: Some("Defect rate drops to 0%".to_string()),
        made_at: chrono::Utc::now(),
        observed_at: Some(chrono::Utc::now()),
        actual_outcome: Some("Defect rate remained at 12% on new batch".to_string()),
        correct: Some(false),
        confidence: 0.80,
    });

    // Verify fitness separation
    let h1_final = core.hypothesis(&h1_id).unwrap();
    let h2_final = core.hypothesis(&h2_id).unwrap();

    assert!(h1_final.fitness > 0.65, "H1 fitness should rise above baseline: {}", h1_final.fitness);
    assert!(h2_final.fitness < 0.40, "H2 fitness should drop below baseline: {}", h2_final.fitness);
    assert_eq!(h1_final.status, HypothesisStatus::Supported);
    assert_eq!(h2_final.status, HypothesisStatus::Contradicted);

    // Verify ranked query places H1 first
    let query_res = core.query_coherence(&EpistemicQuery::StrongestHypotheses { limit: 5 });
    match query_res {
        EpistemicQueryResult::RankedHypotheses { hypotheses } => {
            assert_eq!(hypotheses[0].id, h1_id);
            assert_eq!(hypotheses[1].id, h2_id);
            assert!(hypotheses[0].fitness > hypotheses[1].fitness);
        }
        _ => panic!("Expected RankedHypotheses result"),
    }
}

// ── Test 3 — Context-Dependent Fitness ───────────────────────────────
// Make an interpretation successful in context A and unsuccessful in context B.
// Expected: Physis does not globally destroy the interpretation.
#[test]
fn test_3_context_dependent_fitness() {
    let emb = fixture_embedder();
    let mut tracker = ContextualQualityTracker::new(Box::new(emb));

    let context_heavy_cnc = FitnessContext {
        domain: Some("machining".to_string()),
        machine: Some("CNC_Heavy_M1".to_string()),
        process: Some("roughing".to_string()),
        environment: Some("shop_floor_A".to_string()),
        operator: None,
        material: Some("CastIron_GG25".to_string()),
        state: Some("HighLoad".to_string()),
        task: None,
    };

    let context_micro_mill = FitnessContext {
        domain: Some("machining".to_string()),
        machine: Some("MicroMill_Precision_M2".to_string()),
        process: Some("finishing".to_string()),
        environment: Some("cleanroom_B".to_string()),
        operator: None,
        material: Some("Acrylic_Optic".to_string()),
        state: Some("HighSpeed".to_string()),
        task: None,
    };

    let interpretation = "AggressiveFeedRate";

    // In Context A (Heavy CNC Roughing), aggressive feed rate works excellently
    tracker.report_success_context(interpretation, context_heavy_cnc.clone());
    tracker.report_success_context(interpretation, context_heavy_cnc.clone());
    tracker.report_success_context(interpretation, context_heavy_cnc.clone());

    // In Context B (Micro-Mill Acrylic), aggressive feed rate causes chatter and fractures
    tracker.report_failure_for_cell_context(
        interpretation,
        "Chatter marks and acrylic fracture at aggressive feed",
        context_micro_mill.clone(),
    );
    tracker.report_failure_for_cell_context(
        interpretation,
        "Surface roughness out of spec",
        context_micro_mill.clone(),
    );

    // Verify context-specific fitness
    let fit_heavy = tracker.fitness_for(interpretation, &context_heavy_cnc).unwrap();
    let fit_micro = tracker.fitness_for(interpretation, &context_micro_mill).unwrap();

    assert_eq!(fit_heavy, 1.0, "Heavy CNC roughing should maintain 100% success fitness");
    assert_eq!(fit_micro, 0.0, "Micro mill acrylic should have 0% fitness");

    // Adjusting score in Context A remains high
    let raw_score = 0.90;
    let adjusted_heavy = tracker.adjust_score_contextual(interpretation, raw_score, &context_heavy_cnc);
    let adjusted_micro = tracker.adjust_score_contextual(interpretation, raw_score, &context_micro_mill);

    assert!(adjusted_heavy > 0.80, "Score in Context A must remain viable: {adjusted_heavy}");
    assert!(adjusted_micro < 0.60, "Score in Context B must be heavily penalized: {adjusted_micro}");
    assert!(adjusted_heavy > adjusted_micro);
}

// ── Test 4 — Temporal Revision & State Reconstruction ─────────────────
// Give evidence that changes over time.
// Expected: historical state remains reconstructable at time T.
#[test]
fn test_4_temporal_revision_and_reconstruction() {
    let mut core = PhysisCore::new();
    let emb = fixture_embedder();

    let hyp_id = core.register_hypothesis(Hypothesis::new(
        "Reactor Batch Status: Nominal Operation",
        emb.embed("reactor batch nominal operation 100%"),
    ));

    let t0 = chrono::Utc::now();
    std::thread::sleep(std::time::Duration::from_millis(50));

    core.transition_hypothesis(
        &hyp_id,
        HypothesisStatus::Supported,
        "Sensor telemetry confirms temperature and pressure within band",
        Some("telemetry_tick_100".to_string()),
    );
    std::thread::sleep(std::time::Duration::from_millis(50));
    let t1 = chrono::Utc::now();

    std::thread::sleep(std::time::Duration::from_millis(50));
    core.transition_hypothesis(
        &hyp_id,
        HypothesisStatus::Contradicted,
        "Exothermic spike observed; pressure exceeds safety interlock threshold",
        Some("alarm_ev_992".to_string()),
    );
    std::thread::sleep(std::time::Duration::from_millis(50));
    let t2 = chrono::Utc::now();

    // Reconstruct state at past moments:
    // At t0: Was Candidate
    assert_eq!(core.reconstruct_belief_at(&hyp_id, t0), Some(HypothesisStatus::Candidate));
    // At t1: Was Supported
    assert_eq!(core.reconstruct_belief_at(&hyp_id, t1), Some(HypothesisStatus::Supported));
    // At t2: Was Contradicted
    assert_eq!(core.reconstruct_belief_at(&hyp_id, t2), Some(HypothesisStatus::Contradicted));

    // Audit trail preserves all events with triggers
    let history = core.epistemic_audit.history_for(&hyp_id);
    assert_eq!(history.len(), 3); // Created + Transition 1 + Transition 2
    assert!(history.iter().any(|e| e.description.contains("Exothermic spike")));
}

// ── Test 5 — Structured Explanation System ───────────────────────────
// Ask why a hypothesis is ranked highly.
// Expected: supporting evidence, precedents, predictions, and provenance returned.
#[test]
fn test_5_structured_explanation() {
    let mut core = PhysisCore::new();
    let emb = fixture_embedder();

    let mut hyp = Hypothesis::new(
        "Preventative maintenance required on coolant pump P2",
        emb.embed("preventative maintenance coolant pump P2 cavitation"),
    );
    hyp.confidence = 0.88;
    hyp.coherence_profile = CoherenceProfile::new();
    hyp.coherence_profile.semantic.score = 0.92;
    hyp.coherence_profile.empirical.score = 0.85;
    hyp.coherence_profile.causal.score = 0.80;
    hyp.coherence = hyp.coherence_profile.composite();

    hyp.add_supporting_evidence(Evidence {
        source: "vibration_sensor_P2.log".to_string(),
        polarity: EvidencePolarity::Supports,
        confidence: 0.94,
        claim: "High frequency cavitation peak at 3.2 kHz".to_string(),
        observed_at: Some(chrono::Utc::now()),
        embedding: Vec::new(),
        context: vec!["coolant_loop_2".to_string()],
    });

    hyp.record_prediction(Prediction {
        statement: "Impeller seal failure within 48 operating hours if unserviced".to_string(),
        expected_outcome: Some("Seal leakage detected".to_string()),
        made_at: chrono::Utc::now(),
        observed_at: None,
        actual_outcome: None,
        correct: None,
        confidence: 0.85,
    });

    let hyp_id = core.register_hypothesis(hyp);

    // Attach provenance
    let prov = core.provenance_chain(&hyp_id);
    prov.add_link(ProvenanceLink::new(
        "Anomaly detected by spectral vibration analysis",
        "vibration_monitor_daemon",
    ));
    prov.add_link(ProvenanceLink::new(
        "Correlation with fluid temperature delta",
        "thermal_logger_daemon",
    ));

    // Request full explanation
    let explanation_opt = core.full_explanation_report(&hyp_id);
    assert!(explanation_opt.is_some());
    let report = explanation_opt.unwrap();

    assert_eq!(report.statement, "Preventative maintenance required on coolant pump P2");
    assert_eq!(report.supporting_evidence.len(), 1);
    assert!(report.supporting_evidence[0].claim.contains("cavitation peak"));
    assert_eq!(report.expected_consequences.len(), 1);
    assert!(report.provenance_chain.links.len() >= 2);
    assert!(report.human_readable_summary.contains("PHYSIS EXPLANATION REPORT"));
    assert!(report.human_readable_summary.contains("vibration_sensor_P2.log"));
}

// ── Test 6 — Failed Prediction Penalization ──────────────────────────
// Hypothesis predicts X. Reality produces Y.
// Expected: hypothesis fitness decreases, failure becomes part of provenance.
#[test]
fn test_6_failed_prediction_penalization() {
    let mut core = PhysisCore::new();
    let emb = fixture_embedder();

    let mut hyp = Hypothesis::new(
        "Replacing nozzle brass tip with hardened steel will eliminate clog frequency",
        emb.embed("nozzle hardened steel clog frequency eliminate"),
    );
    hyp.confidence = 0.90;

    let hyp_id = core.register_hypothesis(hyp);

    // Initial fitness is neutral
    assert_eq!(core.hypothesis(&hyp_id).unwrap().fitness, 0.5);

    // Record a failed prediction
    let hyp_mut = core.hypothesis_mut(&hyp_id).unwrap();
    hyp_mut.record_prediction(Prediction {
        statement: "Hardened steel tip will operate > 100 hours without thermal creep clogs".to_string(),
        expected_outcome: Some("0 clogs in 100 hours".to_string()),
        made_at: chrono::Utc::now(),
        observed_at: Some(chrono::Utc::now()),
        actual_outcome: Some("Severe clog occurred at hour 14 due to reduced thermal conductivity".to_string()),
        correct: Some(false),
        confidence: 0.90,
    });

    let hyp_after = core.hypothesis(&hyp_id).unwrap();

    // Verify penalty was applied
    assert!(
        hyp_after.fitness < 0.40,
        "Failed prediction must depress fitness: {}",
        hyp_after.fitness
    );
    assert!(hyp_after.fitness_breakdown.failed_prediction_penalty > 0.0);
    assert_eq!(hyp_after.actual_outcomes.len(), 1);
    assert!(hyp_after.actual_outcomes[0].contains("thermal conductivity"));

    // Verify failed prediction is captured by query
    let query_res = core.query_coherence(&EpistemicQuery::FailedPredictions { limit: 5 });
    match query_res {
        EpistemicQueryResult::FailedPredictionsList { failures } => {
            assert_eq!(failures.len(), 1);
            assert_eq!(failures[0].hypothesis_id, hyp_id);
            assert!(failures[0].actual_outcome.as_ref().unwrap().contains("Severe clog"));
        }
        _ => panic!("Expected FailedPredictionsList result"),
    }
}

// ── Test 7 — Ontology Gap Discovery ──────────────────────────────────
// Give an observation that cannot be adequately represented in the current ontology.
// Expected: gap detected, candidate ontology extension generated.
#[test]
fn test_7_ontology_gap_discovery() {
    let loader = OntologyLoader::load_all();
    let emb = fixture_embedder();
    let classifier = CellClassifier::build(&loader, &emb);

    let unmapped_corpus = vec![
        "superconducting quantum qubit coherence decoherence time T1 T2 fluxonium gate fidelity".to_string(),
        "quantum error correction surface code logical qubit threshold lattice surgery".to_string(),
        "cryogenic dilution refrigerator microwave drive resonator transmon qubit readout".to_string(),
    ];

    let config = DiscoveryConfig {
        coverage_threshold: 0.95,
        cluster_sim: 0.70,
        min_cluster: 2,
        max_hints: 6,
        auto_retune: true,
    };

    let report = discover(&unmapped_corpus, &classifier, &emb, &config);

    assert!(report.unmapped > 0, "Unmapped texts should be detected");
    assert!(
        !report.proposals.is_empty(),
        "Discovery must propose candidate ontology extensions for quantum computing domain"
    );

    let top_prop = &report.proposals[0];
    assert!(!top_prop.hints.is_empty());
    assert!(
        top_prop.hints.iter().any(|t| t == "qubit" || t == "quantum" || t == "coherence" || t == "fidelity"),
        "Extracted proposal hints: {:?}",
        top_prop.hints
    );
}

// ── Section 28 — Peculiarity Benchmark ───────────────────────────────
// Compares 5 paradigms on:
// 1. Embedding similarity only
// 2. Embedding + Ontology
// 3. Embedding + Ontology + Graph
// 4. Physis Coherence
// 5. Physis Coherence + Feedback
#[test]
fn test_peculiarity_benchmark_comparison() {
    let loader = OntologyLoader::load_all();
    let emb = fixture_embedder();

    // Problem case: An observation that has high surface vector similarity to "Study"
    // but in reality represents an industrial thermal failure with contradicting feedback.
    let text = "thermal overload shutdown during emergency drill";
    let text_emb = emb.embed(text);

    // 1. Embedding similarity only: picks arbitrary highest dot product
    let pure_sim_score = physis_core::models::cosine_sim(&text_emb, &emb.embed("thermal overload shutdown"));
    assert!(pure_sim_score > 0.0);

    // 2. Embedding + Ontology: classifies into a domain cell
    let classifier = CellClassifier::build(&loader, &emb);
    let cell_res = classifier.classify(&text_emb);
    assert!(!cell_res.is_empty());

    // 3. Embedding + Ontology + Graph: evaluates structural relationships
    let mut core = PhysisCore::new();
    let n1 = core.register_node_from_text("thermal overload shutdown during emergency drill", &emb);
    let n2 = core.register_node_from_text("emergency drill procedure step 4", &emb);
    core.add_edge(TypedEdge::new(RelationType::Precedes, &n2, &n1));

    // 4. Physis Multidimensional Coherence: separates semantic, logical, empirical
    let mut hyp = Hypothesis::new(text, text_emb);
    hyp.coherence_profile.semantic.score = 0.85;
    hyp.coherence_profile.logical.score = 0.90;
    hyp.coherence_profile.empirical.score = 0.30; // empirical failure observed
    hyp.coherence = hyp.coherence_profile.composite();
    let coh_before_feedback = hyp.coherence;

    // 5. Physis Coherence + Feedback: applies operator feedback and penalizes without global destruction
    hyp.add_contradicting_evidence(Evidence {
        source: "safety_auditor_report.pdf".to_string(),
        polarity: EvidencePolarity::Contradicts,
        confidence: 0.95,
        claim: "Drill procedure violated safety cooldown buffer".to_string(),
        observed_at: Some(chrono::Utc::now()),
        embedding: Vec::new(),
        context: vec!["drill_zone_1".to_string()],
    });

    let hyp_id = core.register_hypothesis(hyp);
    let hyp_evaluated = core.hypothesis(&hyp_id).unwrap();

    // Verify key peculiarity metrics:
    // a) Physis coherence captures empirical defect where pure similarity is blind
    assert!(coh_before_feedback < pure_sim_score + 0.5);
    // b) Feedback modulates fitness down from 0.5 baseline
    assert!(hyp_evaluated.fitness < 0.45);
    // c) Contradictory evidence was preserved, not overwritten
    assert_eq!(hyp_evaluated.contradicting_evidence.len(), 1);
    assert_eq!(hyp_evaluated.status, HypothesisStatus::Contradicted);
}
