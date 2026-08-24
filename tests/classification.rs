//! Classification harness tests (PREMORTEM.md P3).
//!
//! Two layers:
//!  - `pipeline_invariants` runs in CI with the deterministic random-projection
//!    embedder. It does NOT assert semantic accuracy (RP is non-semantic) — it
//!    guards the wiring: every cell scored, sorted, finite, deterministic,
//!    non-empty. This catches regressions like "classify returns []".
//!  - `accuracy_floor` is `#[ignore]` because it needs the real Jina model on
//!    disk. Run locally with `cargo test --features full -- --ignored`. It
//!    asserts top-1 DOMAIN accuracy on a curated fixture set stays above a
//!    floor, which also surfaces P0 (a degraded model scores ~random ≈ 0.2).

use physis_pro::classify::CellClassifier;
use physis_pro::config::{OntologyLoader, PhysisConfig};
use physis_pro::embed::{RandomProjectionEmbedder, VectorEmbed};

fn load_ontology() -> OntologyLoader {
    OntologyLoader::load_all(&PhysisConfig::default())
}

#[derive(serde::Deserialize)]
// `text`, `mode` and `tier` are only read by `accuracy_floor`, which is gated on
// `embed-onnx`; on the default build they deserialize but are never touched.
#[allow(dead_code)]
struct Fixture {
    text: String,
    domain: String,
    mode: String,
    /// "core" = unambiguous case (gated). "hard" = abstract/empty-cell concept
    /// where a weak embedder is expected to struggle (reported, not gated).
    #[serde(default = "default_tier")]
    tier: String,
}

fn default_tier() -> String {
    "core".to_string()
}

fn fixtures() -> Vec<Fixture> {
    serde_json::from_str(include_str!("classification_fixtures.json"))
        .expect("fixtures are valid JSON")
}

#[test]
fn pipeline_invariants() {
    let ontology = load_ontology();
    let embedder = RandomProjectionEmbedder::new(384);
    let clf = CellClassifier::build(&ontology, &embedder);

    // Centroids must actually be populated — the human grid alone has 22 cells.
    assert!(
        clf.cell_count() >= 20,
        "expected >=20 populated cells, got {}",
        clf.cell_count()
    );

    let results = clf.classify(&embedder.embed("lifting heavy weights at the gym"));

    // Every cell scored, never empty (regression guard for the {results} bug).
    assert_eq!(results.len(), clf.cell_count());
    assert!(!results.is_empty());

    // Sorted descending, scores finite and within cosine range.
    for w in results.windows(2) {
        assert!(w[0].score >= w[1].score, "results not sorted by score desc");
    }
    for r in &results {
        assert!(
            r.score.is_finite(),
            "non-finite score for {}/{}",
            r.domain,
            r.mode
        );
        assert!(
            r.score >= -1.01 && r.score <= 1.01,
            "score {} out of range",
            r.score
        );
        assert!(!r.domain.is_empty() && !r.mode.is_empty());
    }

    // Determinism: a fresh classifier + embed yields the same top cell.
    let clf2 = CellClassifier::build(&ontology, &embedder);
    let again = clf2.classify(&embedder.embed("lifting heavy weights at the gym"));
    assert_eq!(results[0].domain, again[0].domain);
    assert_eq!(results[0].mode, again[0].mode);
}

#[test]
fn fixtures_use_valid_vocabulary() {
    // Guard the fixture file itself: every label is a real grid token.
    // Validated against the canonical enums so this can't go stale when the
    // grid grows (as it did at 6 -> 14 modes).
    use physis_pro::models::{HumanDomain, HumanMode};
    let fx = fixtures();
    assert!(fx.len() >= 20, "expected a non-trivial fixture set");
    for f in &fx {
        assert!(
            HumanDomain::from_str(&f.domain).is_some(),
            "bad domain {}",
            f.domain
        );
        assert!(
            HumanMode::from_str(&f.mode).is_some(),
            "bad mode {}",
            f.mode
        );
    }
}

/// Semantic self-test — the engine's shared gate: a model that loads but emits
/// garbage (e.g. Jina fp16's float16 outputs read as f32 → random) is rejected.
#[cfg(feature = "embed-onnx")]
fn is_semantic(e: &dyn VectorEmbed) -> bool {
    physis_pro::embed::semantic_self_test(e)
}

/// Best available real (semantic) embedder for local accuracy checks: Jina →
/// BGE-base → BGE-M3 → MiniLM, self-test-gated (mirrors the server cascade).
/// BGE-M3 ranks below BGE-base on measurement (2026-06-12: CORE 0.923/0.769 vs
/// 1.000/0.949; Italian ties at 0.733 domain). `None` when no model on disk.
/// Also returns the model kind so language-dependent floors can gate on it.
#[cfg(feature = "embed-onnx")]
fn real_embedder() -> Option<(Box<dyn VectorEmbed>, &'static str)> {
    use physis_pro::config::OnnxConfig;
    use physis_pro::embed::jina::JinaEmbedder;
    use physis_pro::embed::onnx::OnnxEmbedder;

    let jina = JinaEmbedder::with_model_dir("models/jina-clip-v2");
    if jina.is_available() && is_semantic(&jina) {
        eprintln!("using embedder: jina-clip-v2 (1024-d)");
        return Some((Box::new(jina), "jina-v2"));
    }
    let bge = OnnxEmbedder::with_config(&OnnxConfig {
        enabled: true,
        model_dir: Some("models/bge-base-en-v1.5".to_string()),
        dim: 768,
        max_length: 512,
        pooling: physis_pro::config::PoolingStrategy::Mean,
    });
    if bge.is_available() && is_semantic(&bge) {
        eprintln!("using embedder: bge-base-en-v1.5 (768-d)");
        return Some((Box::new(bge), "bge-base"));
    }
    let m3 = OnnxEmbedder::with_config(&OnnxConfig {
        enabled: true,
        model_dir: Some("models/bge-m3".to_string()),
        dim: 1024,
        max_length: 512,
        pooling: physis_pro::config::PoolingStrategy::Cls,
    });
    if m3.is_available() && is_semantic(&m3) {
        eprintln!("using embedder: bge-m3 (1024-d, multilingual)");
        return Some((Box::new(m3), "bge-m3"));
    }
    let onnx = OnnxEmbedder::with_config(&PhysisConfig::default().onnx);
    if onnx.is_available() && is_semantic(&onnx) {
        eprintln!("using embedder: minilm (models/model.onnx, 384-d)");
        return Some((Box::new(onnx), "minilm"));
    }
    None
}

#[cfg(feature = "embed-onnx")]
#[test]
#[ignore = "needs a real model on disk; run with --features full -- --ignored"]
fn accuracy_floor() {
    let (embedder, model_kind) = match real_embedder() {
        Some((e, k)) => (e, k),
        None => {
            eprintln!("SKIP accuracy_floor: no real model on disk (jina/minilm)");
            return;
        }
    };
    // Only multilingual models are held to the Italian floor — an
    // English-only encoder was never expected to carry cross-lingual queries
    // (the original design note said so; the assert just never implemented it).
    let multilingual = matches!(model_kind, "jina-v2" | "bge-m3");

    let ontology = load_ontology();
    let clf = CellClassifier::build(&ontology, embedder.as_ref());
    let fx = fixtures();

    // Tally per tier: "core" is gated, "hard" is reported only, "italian" is
    // reported per-model (gated only under a multilingual embedder below —
    // English-only models are expected to miss Italian).
    let (mut core_n, mut core_dom, mut core_cell) = (0usize, 0usize, 0usize);
    let (mut hard_n, mut hard_dom) = (0usize, 0usize);
    let (mut it_n, mut it_dom, mut it_cell) = (0usize, 0usize, 0usize);
    for f in &fx {
        let top = &clf.classify_text(&f.text, embedder.as_ref())[0];
        let dom_ok = top.domain == f.domain;
        let cell_ok = dom_ok && top.mode == f.mode;
        match f.tier.as_str() {
            "hard" => {
                hard_n += 1;
                if dom_ok {
                    hard_dom += 1;
                }
            }
            "italian" => {
                it_n += 1;
                if dom_ok {
                    it_dom += 1;
                }
                if cell_ok {
                    it_cell += 1;
                }
            }
            _ => {
                core_n += 1;
                if dom_ok {
                    core_dom += 1;
                }
                if cell_ok {
                    core_cell += 1;
                }
            }
        }
        if !cell_ok {
            eprintln!(
                "[{}] miss: {:?} → got {}/{} want {}/{}",
                f.tier, f.text, top.domain, top.mode, f.domain, f.mode
            );
        }
    }
    let core_dom_acc = core_dom as f32 / core_n.max(1) as f32;
    let core_cell_acc = core_cell as f32 / core_n.max(1) as f32;
    eprintln!("CORE top-1 domain: {core_dom_acc:.3} ({core_dom}/{core_n}); cell: {core_cell_acc:.3} ({core_cell}/{core_n})");
    if hard_n > 0 {
        eprintln!("HARD top-1 domain: {:.3} ({hard_dom}/{hard_n}) [informational — abstract/empty-cell concepts, model-limited]", hard_dom as f32 / hard_n as f32);
    }
    if it_n > 0 {
        let it_dom_acc = it_dom as f32 / it_n as f32;
        let it_cell_acc = it_cell as f32 / it_n as f32;
        eprintln!("ITALIAN top-1 domain: {it_dom_acc:.3} ({it_dom}/{it_n}); cell: {it_cell_acc:.3} ({it_cell}/{it_n}) [model={model_kind}]");
        // Cross-lingual queries are the multilingual models' job (jina/m3);
        // English-only encoders get a report card, not a gate — measured
        // bge-base-en history: 0.733 pre-anchors (2026-06-12), 0.867 after
        // Italian anchor entries (2026-06-12), 0.667 after the 2026-08-14
        // ontology expansion diluted nearest-entry scoring.
        if multilingual {
            assert!(
                it_dom_acc >= 0.80,
                "ITALIAN top-1 domain accuracy {it_dom_acc:.3} below floor 0.80 under {model_kind} — italian anchor regression"
            );
            assert!(
                it_cell_acc >= 0.80,
                "ITALIAN top-1 cell accuracy {it_cell_acc:.3} below floor 0.80 under {model_kind} — italian anchor regression"
            );
        }
    }

    // Floor gates the CORE (unambiguous) tier: nearest-entry scoring on MiniLM
    // hits ≈0.85 there — well above random-projection degradation (P0, ~0.2) and
    // tight enough to catch a real regression (model swap, ontology drift, a
    // revert to the old mean-centroid scoring that scored ~0.56). The HARD tier
    // exercises the newly-filled empty cells; a weak embedder is expected to
    // miss many — those lift once a stronger model (Jina, P4) loads.
    // Floors recalibrated 2026-08-24 under bge-base-en-v1.5 after switching
    // the shipped scoring to max (Fixed(1), PLAN 8.8 sweep result):
    // measured CORE 0.891 domain / 0.782 cell; ITALIAN 0.867 / 0.733.
    // History for the record: the 0.974/0.923 floors were calibrated
    // 2026-06-12 and invalidated by the 2026-08-14 ontology expansion;
    // under adaptive smoothing the same fixtures fell to 0.891/0.636 —
    // which is why max ships and smoothing stays behind TopKStrategy.
    assert!(
        core_dom_acc >= 0.85,
        "CORE top-1 domain accuracy {core_dom_acc:.3} below floor 0.85 — weak/degraded model or scoring/ontology regression"
    );
    assert!(
        core_cell_acc >= 0.72,
        "CORE top-1 cell accuracy {core_cell_acc:.3} below floor 0.72 — weak/degraded model or scoring/ontology regression"
    );
}

/// Diagnostic for PLAN 8.8 (ignored by default): sweep per-cell top-k
/// strategies over the fixture set and print the accuracy table. Run with
/// `cargo test --features embed-onnx --test classification -- --ignored
/// --nocapture scoring_sweep`. No assertions — this is the measurement tool
/// that picks the next default; `accuracy_floor` enforces the result.
#[cfg(feature = "embed-onnx")]
#[test]
#[ignore = "calibration diagnostic; needs a real model on disk"]
fn scoring_strategy_sweep() {
    use physis_pro::classify::TopKStrategy;

    let Some((embedder, model_kind)) = real_embedder() else {
        eprintln!("SKIP scoring_strategy_sweep: no real model on disk");
        return;
    };
    let ontology = load_ontology();
    let clf = CellClassifier::build(&ontology, embedder.as_ref());
    let fx = fixtures();

    let strategies: Vec<(&str, TopKStrategy)> = vec![
        ("adaptive", TopKStrategy::Adaptive),
        ("fixed1", TopKStrategy::Fixed(1)),
        ("fixed2", TopKStrategy::Fixed(2)),
        ("fixed3", TopKStrategy::Fixed(3)),
    ];

    eprintln!("strategy      CORE-dom  CORE-cell  IT-dom   IT-cell  (n={model_kind})");
    for (name, strat) in strategies {
        let (mut cd, mut cc, mut cn) = (0, 0, 0usize);
        let (mut idom, mut icell, mut inn) = (0, 0, 0usize);
        for f in &fx {
            let top = &clf.classify_with_topk(&embedder.embed(&f.text), strat)[0];
            let dom_ok = top.domain == f.domain;
            let cell_ok = dom_ok && top.mode == f.mode;
            match f.tier.as_str() {
                "italian" => {
                    inn += 1;
                    if dom_ok { idom += 1; }
                    if cell_ok { icell += 1; }
                }
                "hard" => {}
                _ => {
                    cn += 1;
                    if dom_ok { cd += 1; }
                    if cell_ok { cc += 1; }
                }
            }
        }
        eprintln!(
            "{name:<12}  {:.3}     {:.3}      {:.3}    {:.3}",
            cd as f32 / cn.max(1) as f32,
            cc as f32 / cn.max(1) as f32,
            idom as f32 / inn.max(1) as f32,
            icell as f32 / inn.max(1) as f32,
        );
    }
}
