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
struct Fixture {
    text: String,
    domain: String,
    #[allow(dead_code)]
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
        assert!(r.score.is_finite(), "non-finite score for {}/{}", r.domain, r.mode);
        assert!(r.score >= -1.01 && r.score <= 1.01, "score {} out of range", r.score);
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
    let domains = ["HEAL", "CONSTRUCT", "FABRICATE", "BOND", "STUDY"];
    let modes = ["LIFT", "REST", "WALK", "WORK", "CREATE", "LEARN"];
    let fx = fixtures();
    assert!(fx.len() >= 20, "expected a non-trivial fixture set");
    for f in &fx {
        assert!(domains.contains(&f.domain.as_str()), "bad domain {}", f.domain);
        assert!(modes.contains(&f.mode.as_str()), "bad mode {}", f.mode);
    }
}

/// Best available real (semantic) embedder for local accuracy checks: Jina,
/// else MiniLM (`models/model.onnx`). `None` when no model is on disk.
#[cfg(feature = "embed-onnx")]
fn real_embedder() -> Option<Box<dyn VectorEmbed>> {
    use physis_pro::embed::jina::JinaEmbedder;
    use physis_pro::embed::onnx::OnnxEmbedder;

    let jina = JinaEmbedder::with_model_dir("models/jina-clip-v2");
    if jina.is_available() {
        eprintln!("using embedder: jina-clip-v2");
        return Some(Box::new(jina));
    }
    let onnx = OnnxEmbedder::with_config(&PhysisConfig::default().onnx);
    if onnx.is_available() {
        eprintln!("using embedder: minilm (models/model.onnx)");
        return Some(Box::new(onnx));
    }
    None
}

#[cfg(feature = "embed-onnx")]
#[test]
#[ignore = "needs a real model on disk; run with --features full -- --ignored"]
fn accuracy_floor() {
    let embedder = match real_embedder() {
        Some(e) => e,
        None => {
            eprintln!("SKIP accuracy_floor: no real model on disk (jina/minilm)");
            return;
        }
    };

    let ontology = load_ontology();
    let clf = CellClassifier::build(&ontology, embedder.as_ref());
    let fx = fixtures();

    // Tally per tier: "core" is gated, "hard" is reported only.
    let (mut core_n, mut core_dom, mut core_cell) = (0usize, 0usize, 0usize);
    let (mut hard_n, mut hard_dom) = (0usize, 0usize);
    for f in &fx {
        let top = &clf.classify_text(&f.text, embedder.as_ref())[0];
        let dom_ok = top.domain == f.domain;
        let cell_ok = dom_ok && top.mode == f.mode;
        if f.tier == "hard" {
            hard_n += 1;
            if dom_ok { hard_dom += 1; }
        } else {
            core_n += 1;
            if dom_ok { core_dom += 1; }
            if cell_ok { core_cell += 1; }
        }
        if !cell_ok {
            eprintln!("[{}] miss: {:?} → got {}/{} want {}/{}", f.tier, f.text, top.domain, top.mode, f.domain, f.mode);
        }
    }
    let core_dom_acc = core_dom as f32 / core_n.max(1) as f32;
    let core_cell_acc = core_cell as f32 / core_n.max(1) as f32;
    eprintln!("CORE top-1 domain: {core_dom_acc:.3} ({core_dom}/{core_n}); cell: {core_cell_acc:.3} ({core_cell}/{core_n})");
    if hard_n > 0 {
        eprintln!("HARD top-1 domain: {:.3} ({hard_dom}/{hard_n}) [informational — abstract/empty-cell concepts, model-limited]", hard_dom as f32 / hard_n as f32);
    }

    // Floor gates the CORE (unambiguous) tier: nearest-entry scoring on MiniLM
    // hits ≈0.85 there — well above random-projection degradation (P0, ~0.2) and
    // tight enough to catch a real regression (model swap, ontology drift, a
    // revert to the old mean-centroid scoring that scored ~0.56). The HARD tier
    // exercises the newly-filled empty cells; a weak embedder is expected to
    // miss many — those lift once a stronger model (Jina, P4) loads.
    assert!(
        core_dom_acc >= 0.80,
        "CORE top-1 domain accuracy {core_dom_acc:.3} below floor 0.80 — model degraded or scoring/ontology regression"
    );
    // Cell floor locks the nested domain→mode gain (0.795 → 0.821).
    assert!(
        core_cell_acc >= 0.78,
        "CORE top-1 cell accuracy {core_cell_acc:.3} below floor 0.78 — nested scoring or ontology regression"
    );
}
