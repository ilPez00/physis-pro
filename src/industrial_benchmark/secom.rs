//! SECOM (Semiconductor Manufacturing) benchmark.
//!
//! Implements the full benchmark pipeline:
//! 1. Load SECOM dataset (embedded sample + optional download)
//! 2. Deterministic text representation (raw + normalized)
//! 3. Train/test split
//! 4. Baseline classification
//! 5. Quality feedback from ground truth
//! 6. Reclassification
//! 7. Before/after report

use std::collections::HashMap;
use std::path::PathBuf;

use rand::{rngs::StdRng, seq::SliceRandom, SeedableRng};
use serde::{Deserialize, Serialize};

use physis_core::classify::{CellClassifier, CellScore, EntrySeed};
use physis_core::embed::{RandomProjectionEmbedder, VectorEmbed};
use physis_core::ontology::OntologyLoader;
use physis_core::quality::QualityTracker;

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecomObservation {
    pub id: usize,
    pub features: Vec<Option<f32>>,
    pub feature_names: Vec<String>,
    pub outcome: Option<bool>, // true=PASS, false=FAIL, None=unknown
    pub missing_count: usize,
    pub textual: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub seed: u64,
    pub train_ratio: f32,
    pub max_observations: Option<usize>,
    pub embed_dim: usize,
    pub output_dir: PathBuf,
    pub no_feedback: bool,
    pub embedder: String, // "random" | "onnx"
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            train_ratio: 0.5,
            max_observations: None,
            embed_dim: 384,
            output_dir: PathBuf::from("benchmark-results/secom"),
            no_feedback: false,
            embedder: String::from("random"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ClassificationResult {
    pub observation_id: usize,
    pub predicted_cell: String,
    pub raw_score: f32,
    pub adjusted_score: f32,
    pub ground_truth: Option<bool>,
    pub correct: Option<bool>,
    pub top_cells: Vec<CellScore>,
    pub inference_time_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkRun {
    pub config: BenchmarkConfig,
    pub dataset_info: DatasetInfo,
    pub baseline: Vec<ClassificationResult>,
    pub post_feedback: Vec<ClassificationResult>,
    pub quality_before: QualitySnapshot,
    pub quality_after: QualitySnapshot,
    pub metrics_before: Metrics,
    pub metrics_after: Metrics,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatasetInfo {
    pub source: String,
    pub total_observations: usize,
    pub total_features: usize,
    pub pass_count: usize,
    pub fail_count: usize,
    pub missing_value_stats: MissingValueStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct MissingValueStats {
    pub total_cells: usize,
    pub missing_cells: usize,
    pub missing_pct: f32,
    pub features_with_missing: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct QualitySnapshot {
    pub total_failures: usize,
    pub penalized_cells: usize,
    pub boosted_cells: usize,
    pub penalties: HashMap<String, f32>,
    pub boosts: HashMap<String, f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Metrics {
    pub total: usize,
    pub correct: usize,
    pub incorrect: usize,
    pub accuracy: f32,
    pub mean_raw_score: f32,
    pub mean_adjusted_score: f32,
    pub mean_score_change: f32,
    pub top1_changed: usize,
    pub topk_changed: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureStats {
    pub means: Vec<f32>,
    pub stds: Vec<f32>,
    pub mins: Vec<f32>,
    pub maxs: Vec<f32>,
}

// ── Embedded SECOM sample ─────────────────────────────────────────

const SECOM_SAMPLE: &str = include_str!("secom_data.csv");

// ── Feature statistics ────────────────────────────────────────────

pub fn compute_feature_stats(observations: &[SecomObservation]) -> FeatureStats {
    if observations.is_empty() {
        return FeatureStats {
            means: vec![],
            stds: vec![],
            mins: vec![],
            maxs: vec![],
        };
    }

    let n = observations[0].features.len();
    let mut means = vec![0.0f32; n];
    let mut stds = vec![0.0f32; n];
    let mut mins = vec![f32::MAX; n];
    let mut maxs = vec![f32::MIN; n];
    let mut counts = vec![0usize; n];

    for obs in observations {
        for (i, val) in obs.features.iter().enumerate() {
            if let Some(v) = val {
                means[i] += v;
                mins[i] = mins[i].min(*v);
                maxs[i] = maxs[i].max(*v);
                counts[i] += 1;
            }
        }
    }

    for i in 0..n {
        if counts[i] > 0 {
            means[i] /= counts[i] as f32;
        }
    }

    let mut var_sums = vec![0.0f32; n];
    for obs in observations {
        for (i, val) in obs.features.iter().enumerate() {
            if let Some(v) = val {
                let diff = v - means[i];
                var_sums[i] += diff * diff;
            }
        }
    }

    for i in 0..n {
        if counts[i] > 1 {
            stds[i] = (var_sums[i] / (counts[i] - 1) as f32).sqrt();
        }
        if stds[i] < 1e-6 {
            stds[i] = 1.0;
        }
    }

    FeatureStats {
        means,
        stds,
        mins,
        maxs,
    }
}

// ── Text representations ───────────────────────────────────────────

pub fn observation_to_text(obs: &SecomObservation) -> String {
    observation_to_text_rich(obs, None)
}

pub fn observation_to_text_rich(obs: &SecomObservation, stats: Option<&FeatureStats>) -> String {
    let mut text = String::new();
    text.push_str("Production run ");
    text.push_str(&obs.id.to_string());
    text.push_str(" in semiconductor manufacturing.\n\n");

    let mut deviations = Vec::new();
    let mut normal_features = Vec::new();
    let mut missing_features = Vec::new();

    for (name, value) in obs.feature_names.iter().zip(&obs.features) {
        if name.is_empty() || name == "unknown" {
            continue;
        }
        match value {
            Some(v) => {
                if let Some(s) = stats {
                    if let (Some(mean), Some(std)) = (
                        s.means.get(
                            obs.feature_names
                                .iter()
                                .position(|n| n == name)
                                .unwrap_or(0),
                        ),
                        s.stds.get(
                            obs.feature_names
                                .iter()
                                .position(|n| n == name)
                                .unwrap_or(0),
                        ),
                    ) {
                        if *std > 1e-6 {
                            let z = (*v - mean) / *std;
                            if z.abs() > 2.0 {
                                deviations.push((name, *v, z));
                            } else {
                                normal_features.push((name, *v));
                            }
                        } else {
                            normal_features.push((name, *v));
                        }
                    } else {
                        normal_features.push((name, *v));
                    }
                } else {
                    normal_features.push((name, *v));
                }
            }
            None => {
                missing_features.push(name.as_str());
            }
        }
    }

    if !deviations.is_empty() {
        text.push_str("Process anomalies detected:\n");
        for (name, val, z) in deviations.iter().take(20) {
            let direction = if *z > 0.0 { "elevated" } else { "depressed" };
            text.push_str("- ");
            text.push_str(name);
            text.push_str(" = ");
            text.push_str(&format!("{:.4}", val));
            text.push_str(" (");
            text.push_str(direction);
            text.push_str(" (");
            text.push_str(&format!("{:.1}", z.abs()));
            text.push_str(" sigma)\n");
        }
        text.push('\n');
    }

    text.push_str("Process measurements:\n");
    for (name, val) in normal_features.iter().take(20) {
        text.push_str(name);
        text.push_str(": ");
        text.push_str(&format!("{:.4}", val));
        text.push('\n');
    }

    if !missing_features.is_empty() {
        text.push_str("\nMissing measurements:\n");
        for name in missing_features.iter().take(10) {
            text.push_str(name);
            text.push_str(": missing\n");
        }
    }

    let present = obs.features.iter().filter(|v| v.is_some()).count();
    let total = obs.features.len();
    text.push_str("\nSummary: ");
    text.push_str(&present.to_string());
    text.push('/');
    text.push_str(&total.to_string());
    text.push_str(" measurements present, ");
    text.push_str(&obs.missing_count.to_string());
    text.push_str(" missing values.\n");

    if obs.missing_count > total / 4 {
        text.push_str("Several process measurements are missing from this run.\n");
    }

    text
}

// ── Dataset loading ───────────────────────────────────────────────

pub fn parse_secom_csv(text: &str, max_rows: Option<usize>) -> Vec<SecomObservation> {
    let mut lines = text.lines();
    let header = match lines.next() {
        Some(h) => h,
        None => return Vec::new(),
    };

    let feature_names: Vec<String> = header.split(',').map(|s| s.trim().to_string()).collect();

    let total_features = feature_names.len().saturating_sub(1);

    let mut observations = Vec::new();
    for (row_idx, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(max) = max_rows {
            if observations.len() >= max {
                break;
            }
        }

        let parts: Vec<&str> = line.split(',').collect();
        let outcome = match parts.first().copied() {
            Some("1") => Some(true),
            Some("-1") => Some(false),
            Some("") => None,
            _ => None,
        };

        let mut features = Vec::with_capacity(total_features);
        let mut missing_count = 0usize;
        for val in parts.iter().skip(1) {
            let trimmed = val.trim();
            if trimmed.is_empty() {
                features.push(None);
                missing_count += 1;
            } else {
                match trimmed.parse::<f32>() {
                    Ok(v) => features.push(Some(v)),
                    Err(_) => {
                        features.push(None);
                        missing_count += 1;
                    }
                }
            }
        }

        while features.len() < total_features {
            features.push(None);
            missing_count += 1;
        }

        let names: Vec<String> = if feature_names.len() > 1 {
            feature_names[1..].to_vec()
        } else {
            (0..total_features)
                .map(|i| format!("feature_{:03}", i + 1))
                .collect()
        };

        let obs = SecomObservation {
            id: row_idx + 1,
            features,
            feature_names: names,
            outcome,
            missing_count,
            textual: String::new(),
        };

        observations.push(obs);
    }

    observations
}

// ── Train/test split ──────────────────────────────────────────────

pub fn train_test_split(
    observations: &[SecomObservation],
    ratio: f32,
    seed: u64,
) -> (Vec<SecomObservation>, Vec<SecomObservation>) {
    let mut indices: Vec<usize> = (0..observations.len()).collect();
    let mut rng = StdRng::seed_from_u64(seed);
    indices.shuffle(&mut rng);

    let split = (observations.len() as f32 * ratio).round() as usize;
    let split = split.max(1).min(observations.len().saturating_sub(1));

    let train: Vec<SecomObservation> = indices[..split]
        .iter()
        .map(|&i| observations[i].clone())
        .collect();
    let test: Vec<SecomObservation> = indices[split..]
        .iter()
        .map(|&i| observations[i].clone())
        .collect();

    (train, test)
}

// ── Embedder selection ────────────────────────────────────────────

type EmbedFn = Box<dyn Fn(&str) -> Vec<f32>>;

pub fn build_embedder(config: &BenchmarkConfig) -> EmbedFn {
    if config.embedder == "onnx" {
        #[cfg(feature = "embed-onnx")]
        {
            use crate::config::{OnnxConfig, PoolingStrategy};
            use crate::embed::{onnx::OnnxEmbedder, VectorEmbed};
            let onnx = OnnxEmbedder::with_config(&OnnxConfig {
                enabled: true,
                model_dir: Some("./models".to_string()),
                dim: config.embed_dim,
                max_length: 128,
                pooling: PoolingStrategy::Mean,
            });
            if onnx.is_available() {
                eprintln!("[benchmark] using ONNX embedder (MiniLM)");
                return Box::new(move |text: &str| onnx.embed(text));
            } else {
                eprintln!(
                    "[benchmark] ONNX model not available, falling back to random projection"
                );
            }
        }
        #[cfg(not(feature = "embed-onnx"))]
        {
            eprintln!("[benchmark] embed-onnx feature not enabled, using random projection");
        }
    }

    let rp = RandomProjectionEmbedder::new(config.embed_dim);
    Box::new(move |text: &str| rp.embed(text))
}

// ── Benchmark execution ───────────────────────────────────────────

pub fn run_benchmark(config: BenchmarkConfig) -> anyhow::Result<BenchmarkRun> {
    std::fs::create_dir_all(&config.output_dir)?;

    let raw_text = SECOM_SAMPLE;
    let all_obs = parse_secom_csv(raw_text, config.max_observations);

    if all_obs.is_empty() {
        anyhow::bail!("SECOM dataset is empty");
    }

    let total_features = all_obs[0].features.len();
    let stats = compute_feature_stats(&all_obs);

    let pass_count = all_obs.iter().filter(|o| o.outcome == Some(true)).count();
    let fail_count = all_obs.iter().filter(|o| o.outcome == Some(false)).count();

    let total_cells = all_obs.len() * total_features;
    let missing_cells: usize = all_obs.iter().map(|o| o.missing_count).sum();
    let features_with_missing = (0..total_features)
        .filter(|&fi| all_obs.iter().any(|o| o.features[fi].is_none()))
        .count();

    let dataset_info = DatasetInfo {
        source: String::from("UCI SECOM (embedded sample)"),
        total_observations: all_obs.len(),
        total_features,
        pass_count,
        fail_count,
        missing_value_stats: MissingValueStats {
            total_cells,
            missing_cells,
            missing_pct: if total_cells > 0 {
                missing_cells as f32 / total_cells as f32
            } else {
                0.0
            },
            features_with_missing,
        },
    };

    let (train_set, test_set) = train_test_split(&all_obs, config.train_ratio, config.seed);

    let ontology = OntologyLoader::load_all();
    let embed_fn = build_embedder(&config);

    let seeds: Vec<EntrySeed> = ontology
        .classification_domains()
        .filter_map(|def| {
            let (domain, mode) = match (def.domain.as_deref(), def.mode.as_deref()) {
                (Some(d), Some(m)) => (d.to_string(), m.to_string()),
                _ => return None,
            };
            let mut text = def.name.clone();
            for hint in &def.hints {
                text.push(' ');
                text.push_str(hint);
            }
            Some(EntrySeed {
                name: def.name.clone(),
                domain,
                mode,
                text,
                facets: def.facets.clone(),
            })
        })
        .collect();

    let classifier = CellClassifier::build_seeds(seeds, &embed_fn);

    let mut quality =
        QualityTracker::new(Box::new(RandomProjectionEmbedder::new(config.embed_dim)));

    // Stage A: baseline classification on test set
    let mut baseline_results = Vec::new();
    for obs in &test_set {
        let text = observation_to_text_rich(obs, Some(&stats));
        let start = std::time::Instant::now();
        let embedding = embed_fn(&text);
        let scores = classifier.classify(&embedding);
        let inference_time = start.elapsed().as_secs_f64() * 1000.0;

        let top = &scores[0];
        let key = format!("{}\x00{}", top.domain, top.mode);
        let adjusted = quality.adjust_score(&key, top.score);

        baseline_results.push(ClassificationResult {
            observation_id: obs.id,
            predicted_cell: key,
            raw_score: top.score,
            adjusted_score: adjusted,
            ground_truth: obs.outcome,
            correct: obs.outcome.map(|gt| {
                if gt {
                    top.score > 0.5
                } else {
                    top.score <= 0.5
                }
            }),
            top_cells: scores,
            inference_time_ms: inference_time,
        });
    }

    let quality_before = snapshot_quality(&quality);

    // Stage B: feedback — use ground truth to drive quality updates
    if !config.no_feedback {
        for obs in &train_set {
            let text = observation_to_text_rich(obs, Some(&stats));
            let embedding = embed_fn(&text);
            let scores = classifier.classify(&embedding);
            let top = &scores[0];
            let key = format!("{}\x00{}", top.domain, top.mode);

            match obs.outcome {
                Some(true) => {
                    quality.report_success(&key);
                }
                Some(false) => {
                    // For failure feedback we need the text and centroids.
                    // Reconstruct centroids from classifier cells.
                    let mut centroids: HashMap<String, Vec<f32>> = HashMap::new();
                    for cell in &classifier.cells {
                        let key = format!("{}\x00{}", cell.domain, cell.mode);
                        if !cell.embeddings.is_empty() {
                            let mut centroid = vec![0.0f32; cell.embeddings[0].len()];
                            for emb in &cell.embeddings {
                                for (i, v) in emb.iter().enumerate() {
                                    centroid[i] += v;
                                }
                            }
                            let n = cell.embeddings.len() as f32;
                            centroid.iter_mut().for_each(|v| *v /= n);
                            centroids.insert(key, centroid);
                        }
                    }
                    quality.report_failure(&text, &centroids, None);
                }
                None => {}
            }
        }
    }

    let quality_after = snapshot_quality(&quality);

    // Stage C: reclassification on test set (or reuse baseline if no-feedback)
    let post_results = if config.no_feedback {
        baseline_results.clone()
    } else {
        let mut post = Vec::new();
        for obs in &test_set {
            let text = observation_to_text_rich(obs, Some(&stats));
            let start = std::time::Instant::now();
            let embedding = embed_fn(&text);
            let scores = classifier.classify(&embedding);
            let inference_time = start.elapsed().as_secs_f64() * 1000.0;

            let top = &scores[0];
            let key = format!("{}\x00{}", top.domain, top.mode);
            let adjusted = quality.adjust_score(&key, top.score);

            post.push(ClassificationResult {
                observation_id: obs.id,
                predicted_cell: key,
                raw_score: top.score,
                adjusted_score: adjusted,
                ground_truth: obs.outcome,
                correct: obs
                    .outcome
                    .map(|gt| if gt { adjusted > 0.5 } else { adjusted <= 0.5 }),
                top_cells: scores,
                inference_time_ms: inference_time,
            });
        }
        post
    };

    let metrics_before = compute_metrics(&baseline_results);
    let metrics_after = compute_metrics(&post_results);

    let run = BenchmarkRun {
        config: config.clone(),
        dataset_info,
        baseline: baseline_results,
        post_feedback: post_results,
        quality_before,
        quality_after,
        metrics_before,
        metrics_after,
    };

    // Save results
    let json_path = config.output_dir.join("latest.json");
    std::fs::write(&json_path, serde_json::to_string_pretty(&run)?)?;

    let md_path = config.output_dir.join("latest.md");
    std::fs::write(&md_path, render_report(&run))?;

    Ok(run)
}

fn snapshot_quality(quality: &QualityTracker) -> QualitySnapshot {
    let mut penalties = HashMap::new();
    for (k, v) in &quality.cell_penalties {
        penalties.insert(k.clone(), *v);
    }
    let mut boosts = HashMap::new();
    for (k, v) in &quality.cell_boosts {
        boosts.insert(k.clone(), v.current_value());
    }

    QualitySnapshot {
        total_failures: quality.failures.len(),
        penalized_cells: quality.cell_penalties.len(),
        boosted_cells: quality.cell_boosts.len(),
        penalties,
        boosts,
    }
}

fn compute_metrics(results: &[ClassificationResult]) -> Metrics {
    let total = results.len();
    let correct = results.iter().filter(|r| r.correct == Some(true)).count();
    let incorrect = results.iter().filter(|r| r.correct == Some(false)).count();

    let mean_raw = if total > 0 {
        results.iter().map(|r| r.raw_score).sum::<f32>() / total as f32
    } else {
        0.0
    };

    let mean_adjusted = if total > 0 {
        results.iter().map(|r| r.adjusted_score).sum::<f32>() / total as f32
    } else {
        0.0
    };

    Metrics {
        total,
        correct,
        incorrect,
        accuracy: if total > 0 {
            correct as f32 / total as f32
        } else {
            0.0
        },
        mean_raw_score: mean_raw,
        mean_adjusted_score: mean_adjusted,
        mean_score_change: 0.0,
        top1_changed: 0,
        topk_changed: 0,
    }
}

fn compute_diff_metrics(
    baseline: &[ClassificationResult],
    post: &[ClassificationResult],
) -> Metrics {
    let total = baseline.len();
    let mut correct_after = 0usize;
    let mut top1_changed = 0usize;
    let mut score_changes = Vec::new();

    for (b, a) in baseline.iter().zip(post.iter()) {
        if a.correct == Some(true) {
            correct_after += 1;
        }
        if b.predicted_cell != a.predicted_cell {
            top1_changed += 1;
        }
        score_changes.push(a.adjusted_score - b.adjusted_score);
    }

    let mean_score_change = if score_changes.is_empty() {
        0.0
    } else {
        score_changes.iter().sum::<f32>() / score_changes.len() as f32
    };

    Metrics {
        total,
        correct: correct_after,
        incorrect: total - correct_after,
        accuracy: if total > 0 {
            correct_after as f32 / total as f32
        } else {
            0.0
        },
        mean_raw_score: post.iter().map(|r| r.raw_score).sum::<f32>() / total.max(1) as f32,
        mean_adjusted_score: post.iter().map(|r| r.adjusted_score).sum::<f32>()
            / total.max(1) as f32,
        mean_score_change,
        top1_changed,
        topk_changed: 0,
    }
}

// ── Report generation ─────────────────────────────────────────────

fn render_report(run: &BenchmarkRun) -> String {
    let diff = compute_diff_metrics(&run.baseline, &run.post_feedback);
    let mut md = String::new();

    md.push_str("# SECOM Industrial Benchmark Report\n\n");

    md.push_str("## Dataset\n\n");
    md.push_str(&format!("- **Source**: {}\n", run.dataset_info.source));
    md.push_str(&format!(
        "- **Observations**: {}\n",
        run.dataset_info.total_observations
    ));
    md.push_str(&format!(
        "- **Features**: {}\n",
        run.dataset_info.total_features
    ));
    md.push_str(&format!(
        "- **PASS**: {}  **FAIL**: {}\n",
        run.dataset_info.pass_count, run.dataset_info.fail_count
    ));
    md.push_str(&format!(
        "- **Missing values**: {:.2}% ({}/{})\n",
        run.dataset_info.missing_value_stats.missing_pct * 100.0,
        run.dataset_info.missing_value_stats.missing_cells,
        run.dataset_info.missing_value_stats.total_cells
    ));
    md.push('\n');

    md.push_str("## Physis Configuration\n\n");
    md.push_str(&format!(
        "- **Embedder**: {} (dim={})\n",
        run.config.embedder, run.config.embed_dim
    ));
    md.push_str("- **Ontology**: industrial_semiconductor + built-in grids\n");
    md.push_str(&format!("- **Seed**: {}\n", run.config.seed));
    md.push_str(&format!(
        "- **Train ratio**: {:.0}%\n",
        run.config.train_ratio * 100.0
    ));
    md.push('\n');

    md.push_str("## Before / After Feedback\n\n");
    md.push_str("```\n");
    md.push_str("                    BEFORE       AFTER\n");
    md.push_str(&format!(
        "Correct            {:5.1}%       {:5.1}%\n",
        run.metrics_before.accuracy * 100.0,
        diff.accuracy * 100.0
    ));
    md.push_str(&format!(
        "Incorrect          {:5.1}%       {:5.1}%\n",
        (1.0 - run.metrics_before.accuracy) * 100.0,
        (1.0 - diff.accuracy) * 100.0
    ));
    md.push_str(&format!(
        "Top-1 changed               {:3}        \n",
        diff.top1_changed
    ));
    md.push_str(&format!(
        "Mean score change           {:+.4}\n",
        diff.mean_score_change
    ));
    md.push_str("```\n");
    md.push('\n');

    md.push_str("## Feedback Details\n\n");
    md.push_str(&format!(
        "- **Failures recorded**: {} (before) → {} (after)\n",
        run.quality_before.total_failures, run.quality_after.total_failures
    ));
    md.push_str(&format!(
        "- **Cells penalized**: {} → {}\n",
        run.quality_before.penalized_cells, run.quality_after.penalized_cells
    ));
    md.push_str(&format!(
        "- **Cells boosted**: {} → {}\n",
        run.quality_before.boosted_cells, run.quality_after.boosted_cells
    ));
    md.push('\n');

    if !run.quality_after.penalties.is_empty() {
        md.push_str("### Penalized Cells\n\n");
        let mut sorted: Vec<_> = run.quality_after.penalties.iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
        for (cell, penalty) in sorted.iter().take(10) {
            md.push_str(&format!(
                "- {}: {:.3}\n",
                cell.replace('\x00', " × "),
                penalty
            ));
        }
        md.push('\n');
    }

    if !run.quality_after.boosts.is_empty() {
        md.push_str("### Boosted Cells\n\n");
        let mut sorted: Vec<_> = run.quality_after.boosts.iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
        for (cell, boost) in sorted.iter().take(10) {
            md.push_str(&format!(
                "- {}: {:.3}\n",
                cell.replace('\x00', " × "),
                boost
            ));
        }
        md.push('\n');
    }

    md.push_str("## Interpretation\n\n");
    if diff.accuracy > run.metrics_before.accuracy {
        md.push_str("Feedback **improved** classification accuracy on the held-out test set.\n");
    } else if diff.accuracy < run.metrics_before.accuracy {
        md.push_str(
            "Feedback **reduced** classification accuracy. This is a legitimate outcome —\n",
        );
        md.push_str("the initial ontology is intentionally imperfect, and penalties may have\n");
        md.push_str("over-corrected for the small training sample.\n");
    } else {
        md.push_str("Feedback had **no measurable effect** on top-1 accuracy. This is expected\n");
        md.push_str(
            "with a small embedded sample. Try `--embedder onnx` with a real MiniLM model\n",
        );
        md.push_str("for semantic embeddings, or increase `--limit` for a larger sample.\n");
    }
    md.push('\n');

    md.push_str("## Reproducibility\n\n");
    md.push_str(&format!(
        "```bash\nphysis-pro benchmark secom --seed {} --limit {} --embedder {}\n```\n",
        run.config.seed, run.dataset_info.total_observations, run.config.embedder
    ));

    md
}

// ── Public helpers ────────────────────────────────────────────────

pub fn load_semiconductor_ontology() -> OntologyLoader {
    OntologyLoader::load_all()
}
