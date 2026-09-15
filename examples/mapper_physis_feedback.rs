// provenance: moved from physis-core examples/mapper_physis_feedback.rs
// (core HEAD d65858b) by the 2026-09-14 Core/Product split: it drives the
// product's quality feedback loop, which left Core.
//! E5c — does physis's OWN feedback machinery make corrections propagate?
//!
//! E2 measured emendation with naive relabelling and got 0.05-0.31 held-out
//! entries repaired per human correction: corrections did not travel. That was
//! a test of a hand-rolled loop, not of physis. `QualityTracker` is the shipped
//! machinery for exactly this -- `penalize_cell`, `report_success`,
//! `adjust_score` -- so the honest version of the ablation runs the loop
//! through it.
//!
//! ## Protocol
//!
//! Classify the held-out entries with the shipped `CellClassifier`. A human
//! correction on a MISCLASSIFIED entry is one intervention:
//!
//!   penalize_cell(the cell physis wrongly chose, severity)
//!   report_success(the cell the human says is right)
//!
//! Then re-score the entries that were NOT corrected. Scoring the corrected
//! ones would be trivially perfect and would measure nothing.
//!
//! Three arms:
//!   A  no feedback                     -- the baseline
//!   B  naive re-seeding                -- E2's approach: the corrected entry
//!                                         joins its true cell as a new seed
//!   C  physis QualityTracker           -- the shipped mechanism
//!
//! Run:
//!   cargo run -p physis-core --features embed-onnx --release --example mapper_physis_feedback

use physis_core::classify::{Cell, CellClassifier};
use physis_core::ontology::OntologyLoader;
use physis_pro::ported::quality::QualityTracker;
use std::collections::{HashMap, HashSet};

fn key(d: &str, m: &str) -> String {
    format!("{d}\u{0}{m}")
}

fn main() {
    #[cfg(feature = "embed-onnx")]
    {
        use physis_core::embed::VectorEmbed;
        use physis_core::embed_onnx::{OnnxConfig, OnnxEmbedder, PoolingStrategy};

        let Some(dir) = ["models/bge-base-en-v1.5", "../models/bge-base-en-v1.5"]
            .iter()
            .find(|d| std::path::Path::new(d).join("onnx/model.onnx").exists())
        else {
            println!("WARNING: BGE not available — aborting.");
            return;
        };
        let mk = || {
            OnnxEmbedder::with_config(&OnnxConfig {
                dim: 768,
                model_dir: Some(dir.to_string()),
                pooling: PoolingStrategy::Mean,
                ..OnnxConfig::default()
            })
        };
        let embedder = mk();
        if !embedder.is_available() {
            println!("WARNING: embedder unavailable — aborting.");
            return;
        }

        // the split E5 used, so both experiments score the same held-out entries
        let sp = [
            "research/mapper/split.json",
            "../research/mapper/split.json",
        ]
        .iter()
        .map(std::path::Path::new)
        .find(|p| p.exists())
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        let Some(sp) = sp else {
            println!("WARNING: research/mapper/split.json missing — run run_e5.py first.");
            return;
        };
        let test: HashSet<usize> = sp["test"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_u64().map(|x| x as usize))
            .collect();

        // corpus, loaded as every other mapper experiment loads it
        let anchor_path = ["physis-core/config", "config", "../config"]
            .iter()
            .map(|d| std::path::Path::new(d).join("mode_anchors_ontology.json"))
            .find(|p| p.exists());
        let mut anchor_names: HashSet<String> = HashSet::new();
        if let Some(ap) = &anchor_path {
            if let Ok(txt) = std::fs::read_to_string(ap) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                    for a in v["domains"].as_array().into_iter().flatten() {
                        if let Some(nm) = a["name"].as_str() {
                            anchor_names.insert(nm.to_string());
                        }
                    }
                }
            }
        }
        let ontology = OntologyLoader::load_all();
        let (mut texts, mut cells) = (Vec::new(), Vec::new());
        for def in ontology.classification_domains() {
            let (Some(d), Some(m)) = (&def.domain, &def.mode) else {
                continue;
            };
            if anchor_names.contains(&def.name) {
                continue;
            }
            let mut t = def.name.clone();
            for h in &def.hints {
                t.push(' ');
                t.push_str(h);
            }
            texts.push(t);
            cells.push((d.clone(), m.clone()));
        }
        let n = texts.len();
        println!("E5c — physis's own feedback machinery in the correction loop\n");
        println!("  {n} entries, {} held out (the E5 split)", test.len());
        let emb: Vec<Vec<f32>> = texts.iter().map(|t| embedder.embed(t)).collect();

        // classifier built from TRAIN entries only — a test entry must never
        // seed the cell it will be scored against
        let train_cells: Vec<Cell> = {
            let mut acc: HashMap<(String, String), Vec<Vec<f32>>> = HashMap::new();
            for i in 0..n {
                if test.contains(&i) {
                    continue;
                }
                acc.entry(cells[i].clone())
                    .or_default()
                    .push(emb[i].clone());
            }
            acc.into_iter()
                .map(|((domain, mode), embeddings)| Cell {
                    entries: vec![String::new(); embeddings.len()],
                    facets: vec![Default::default(); embeddings.len()],
                    domain,
                    mode,
                    embeddings,
                })
                .collect()
        };
        let clf = CellClassifier::from_cells(train_cells.clone());
        println!(
            "  classifier built from TRAIN only: {} populated cells\n",
            clf.cell_count()
        );

        let test_idx: Vec<usize> = (0..n).filter(|i| test.contains(i)).collect();
        let baseline: Vec<Option<(String, String)>> = test_idx
            .iter()
            .map(|&i| {
                clf.classify(&emb[i])
                    .first()
                    .map(|c| (c.domain.clone(), c.mode.clone()))
            })
            .collect();
        let correct0: Vec<bool> = test_idx
            .iter()
            .zip(&baseline)
            .map(|(&i, p)| p.as_ref() == Some(&cells[i]))
            .collect();
        let acc0 = correct0.iter().filter(|x| **x).count() as f64 / test_idx.len() as f64;
        println!("  A. no feedback — held-out cell accuracy {acc0:.3}\n");

        // the entries a human would be handed: the ones physis got wrong
        let wrong: Vec<usize> = (0..test_idx.len()).filter(|&k| !correct0[k]).collect();
        println!(
            "  {} of {} held-out entries are misclassified — the correction pool\n",
            wrong.len(),
            test_idx.len()
        );

        println!(
            "  {:<34}{:>10}{:>12}{:>14}",
            "arm / corrections", "accuracy", "delta", "fixed/corr"
        );

        for &n_fix in &[0usize, 10, 25, 50, 100] {
            if n_fix > wrong.len() {
                continue;
            }
            // deterministic: the first n_fix misclassified entries in index order
            let fixed: HashSet<usize> = wrong[..n_fix].iter().copied().collect();
            let eval: Vec<usize> = (0..test_idx.len()).filter(|k| !fixed.contains(k)).collect();
            if eval.is_empty() {
                continue;
            }
            let base_on_eval =
                eval.iter().filter(|&&k| correct0[k]).count() as f64 / eval.len() as f64;

            // ── arm B: naive re-seeding (E2's approach) ──
            let mut seeded = train_cells.clone();
            for &k in &fixed {
                let i = test_idx[k];
                if let Some(c) = seeded
                    .iter_mut()
                    .find(|c| (c.domain.clone(), c.mode.clone()) == cells[i])
                {
                    c.embeddings.push(emb[i].clone());
                    c.entries.push(String::new());
                    c.facets.push(Default::default());
                }
            }
            let clf_b = CellClassifier::from_cells(seeded);
            let acc_b = eval
                .iter()
                .filter(|&&k| {
                    let i = test_idx[k];
                    clf_b
                        .classify(&emb[i])
                        .first()
                        .map(|c| (c.domain.clone(), c.mode.clone()))
                        == Some(cells[i].clone())
                })
                .count() as f64
                / eval.len() as f64;

            // ── arm C: physis QualityTracker ──
            // The production path, copied from src/web/coherence.rs: a pin
            // disagreement penalizes the cell physis chose at
            // PIN_DISSENT_SEVERITY and reports success on the human's cell;
            // scoring then applies adjust_score to the CellScore that
            // `classify` produced. The first version of this arm scored
            // adjust_score over cell_centroids instead -- a MEAN-of-entries
            // scorer, where `classify` uses MAX -- so arm C at zero
            // corrections read 0.431 against arm A's 0.350 and the arms were
            // not comparable. Fixed: same base scorer, feedback on top.
            const PIN_DISSENT_SEVERITY: f32 = 0.25; // src/web/mod.rs:1352
            let mut qt = QualityTracker::new(Box::new(mk()));
            for &k in &fixed {
                let i = test_idx[k];
                if let Some(w) = &baseline[k] {
                    qt.penalize_cell(&key(&w.0, &w.1), PIN_DISSENT_SEVERITY);
                }
                qt.report_success(&key(&cells[i].0, &cells[i].1));
            }
            let acc_c = eval
                .iter()
                .filter(|&&k| {
                    let i = test_idx[k];
                    let mut best: Option<(f32, (String, String))> = None;
                    for c in clf.classify(&emb[i]) {
                        let adj = qt.adjust_score(&key(&c.domain, &c.mode), c.score);
                        if best.as_ref().map(|b| adj > b.0).unwrap_or(true) {
                            best = Some((adj, (c.domain.clone(), c.mode.clone())));
                        }
                    }
                    best.map(|b| b.1) == Some(cells[i].clone())
                })
                .count() as f64
                / eval.len() as f64;

            let eff = |a: f64| {
                if n_fix == 0 {
                    f64::NAN
                } else {
                    (a - base_on_eval) * eval.len() as f64 / n_fix as f64
                }
            };
            if n_fix == 0 {
                println!(
                    "  {:<34}{base_on_eval:>10.3}{:>12}{:>14}",
                    "A  no feedback", "—", "—"
                );
            }
            println!(
                "  {:<34}{acc_b:>10.3}{:>+12.3}{:>14.2}",
                format!("B  naive re-seed, {n_fix} corr"),
                acc_b - base_on_eval,
                eff(acc_b)
            );
            println!(
                "  {:<34}{acc_c:>10.3}{:>+12.3}{:>14.2}",
                format!("C  physis QualityTracker, {n_fix}"),
                acc_c - base_on_eval,
                eff(acc_c)
            );
        }
        println!("\n  'fixed/corr' is held-out entries repaired per human correction, scored");
        println!("  only on entries that were NOT themselves corrected. E2's naive loop");
        println!("  managed 0.05-0.31; anything at or below that is corrections failing to");
        println!("  travel, whichever machinery carries them.");
    }
    #[cfg(not(feature = "embed-onnx"))]
    println!("Built without --features embed-onnx; aborting.");
}
