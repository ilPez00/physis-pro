# SECOM Industrial Benchmark

Reproducible benchmark for the Physis semantic engine using the UCI SECOM
semiconductor manufacturing dataset.

## Quick start

```bash
physis-pro benchmark secom --limit 20
```

This runs the full pipeline on the embedded sample (20 observations):
1. Parse the dataset
2. Deterministic text representation (raw + normalized with z-scores)
3. Train/test split (seed=42 by default)
4. Baseline classification
5. Quality feedback from ground truth
6. Reclassification
7. Before/after report

Outputs:
- `benchmark-results/secom/latest.json` — machine-readable results
- `benchmark-results/secom/latest.md` — human-readable report

## Options

```
--limit <N>         Max observations to load (default: all embedded)
--seed <N>          Random seed for train/test split (default: 42)
--train-ratio <F>   Fraction for training (default: 0.5)
--output <PATH>     Output directory (default: benchmark-results/secom)
--no-feedback       Skip the quality feedback loop (baseline only)
--embedder <KIND>   Embedder kind: random (default) or onnx
                    Requires --features embed-onnx for ONNX MiniLM.
```

## Embedding modes

### Random projection (default)
Deterministic, model-free, zero dependencies. Fast but coarse semantic signal.

```bash
physis-pro benchmark secom --limit 20 --embedder random
```

### ONNX MiniLM (semantic)
Requires the `embed-onnx` feature and a MiniLM model in `./models/`:
- `./models/model.onnx`
- `./models/tokenizer.json`

```bash
cargo run --features embed-onnx -- benchmark secom --limit 20 --embedder onnx
```

If the model is missing, the benchmark logs a warning and falls back to random
projection automatically.

## Text representation

Each observation is converted to a deterministic textual description:

```
Production run 1042 in semiconductor manufacturing.

Process anomalies detected:
- temp: 218.4 (elevated 5.2 sigma)
- pressure: 0.91 (elevated 2.0 sigma)

Process measurements:
- flow: 31.4

Summary: 10/10 measurements present, 0 missing values.
```

The rich representation uses dataset-wide z-scores to describe deviations
(elevated/depressed) and groups anomalies separately from normal measurements.

## Ontology

The benchmark uses an expanded semiconductor manufacturing ontology with
specific entries for:
- Normal process operation
- Elevated / depressed process parameters
- Multi-parameter anomalies
- Sensor data gaps
- Process drift
- Equipment performance anomalies
- Manufacturing defects
- Yield loss

This makes classification targets more specific than the generic grid,
allowing the feedback loop to refine narrower cells.

## Downloading the full dataset

The embedded sample contains 20 rows for fast CI/testing. To run on the full
UCI SECOM dataset (~1567 rows, 591 features):

```bash
# Download from UCI
curl -o secom.zip https://archive.ics.uci.edu/ml/machine-learning-databases/secom/secom.zip
unzip secom.zip

# Run benchmark on the full dataset
physis-pro benchmark secom --limit 1567
```

> Note: The `--data` flag accepts a path to a SECOM-format CSV. If the file
> is not present, the benchmark falls back to the embedded sample.

## Dataset

- **Source**: UCI Machine Learning Repository — SECOM Dataset
- **URL**: https://archive.ics.uci.edu/dataset/291/secom
- **License**: Public domain / UCI ML Repository license
- **Size**: 1567 observations, 591 features, binary outcome (PASS/FAIL)
- **Missing values**: ~1.5% (encoded as empty cells)

## Scientific constraint

The benchmark preserves the unidirectional information flow:

```
observation
    ↓
embedding
    ↓
classification
    ↓
prediction
    ↓
ground truth revealed
    ↓
feedback
    ↓
future classification
```

Ground-truth labels are **never** supplied to the classifier during prediction.

## Reproducibility

All randomness is seeded. The same seed + limit + train-ratio always produces
the same train/test split and the same benchmark results.

```bash
physis-pro benchmark secom --seed 42 --limit 100 --train-ratio 0.6 --embedder random
```
