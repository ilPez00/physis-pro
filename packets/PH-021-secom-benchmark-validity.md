---
id: PH-021
track: physis-pro
phase: 1
status: todo
rung: 2
depends: []
gate: "cd ~/dev/physis-pro && cargo test --release --lib industrial_benchmark && cargo run --release --bin physis-pro -- benchmark secom --embedder onnx --output /tmp/secom && grep -q balanced_accuracy /tmp/secom/latest.md"
commit: "branch agent/PH-021; conventional commits; per PROTOCOL §6"
claimed_by:
claimed_at:
---

# The SECOM benchmark's correctness predicate does not measure classification

## Why (the failure this prevents)

SECOM is the only place in this repository where the engine meets a named public dataset, and it
is what a customer or a referee will look at first. Its accuracy number is currently not a
classification metric: it compares the classifier's *confidence* against 0.5 and never compares
the predicted class to the true class. A wafer that passed counts as "correct" when the classifier
was unconfident about a semiotic cell that was never checked.

Publishing or quoting that number is the fastest way to lose credibility on the whole engine,
because the defect is visible in one line to anyone who opens the file.

## Context (measured 2026-09-06 on physis-pro `13d8cdf`; verify before relying on it)

**What is right, and must be preserved.** The experimental skeleton is better than the rest of the
repo: seeded train/test split (`--seed`, `--train-ratio`), feedback derived from the **train** set
only (`src/industrial_benchmark/secom.rs:556`), evaluation on the **test** set, and a
`--no-feedback` ablation flag. There is no label leakage. Keep all of it.

**The defect** — `src/industrial_benchmark/secom.rs:541`:

```rust
correct: obs.outcome.map(|gt| if gt { top.score > 0.5 } else { top.score <= 0.5 }),
```

`top.score` is the classifier's confidence in whichever semiotic cell it happened to pick.
`predicted_cell` is written on the line above and never compared to anything. There is no decision
rule mapping a cell to pass/fail, so no classification is actually being evaluated.

**Measured consequence.** `physis-pro benchmark secom --embedder onnx`, N = 10 test rows:

```
accuracy before feedback  0.70
accuracy after  feedback  0.70     top1_changed = 0   topk_changed = 0
mean_adjusted_score       0.52 → 0.94
```

Feedback changed no decision; it only inflated confidence. The cells selected for semiconductor
observations were `BOND×PLAY`, `HEAL×PLAN`, `HEAL×WORK` — the 5×14 grid has no vocabulary for this
domain, which is a second finding worth reporting rather than hiding.

**Two further problems:**

- **No external baseline.** Real SECOM is ~1567 × 590 with ≈ 6.6 % fail rate, so predicting the
  majority class scores ≈ 0.934. Any accuracy figure without that number beside it is
  uninterpretable — and on the real dataset, the current pipeline would likely lose to it.
- **The dataset is not the dataset.** `secom.rs:4` documents "embedded sample + optional download".
  `rg -n 'download|uci|http' src/industrial_benchmark/secom.rs` finds no download code. The
  embedded `secom_data.csv` is **21 rows, 10 features** of synthetic data. Every SECOM number this
  project has ever produced came from 21 synthetic rows, with 10 in the test split.

## Steps

1. **Define the decision rule explicitly.** Choose and document how a semiotic classification
   becomes a binary pass/fail prediction. Options: a learned threshold on the score of a
   designated failure cell (threshold fit on **train** only), or a nearest-centroid decision
   between a pass-prototype and a fail-prototype. Whichever you pick, `predicted_cell` or the
   derived decision must appear in the correctness comparison — that is the whole packet.
2. Rewrite `correct` to compare the predicted class against `obs.outcome`. Keep the old field as
   `confidence_above_half` if anything depends on it, clearly named for what it is.
3. **Report metrics that survive class imbalance.** Accuracy alone is misleading at a 6.6 % base
   rate. Emit precision, recall, F1 for the minority (fail) class, balanced accuracy, and the
   confusion matrix. Extend the `Metrics` struct at `secom.rs:116` rather than adding a parallel
   one.
4. **Print the trivial baseline beside every figure**: majority-class accuracy and, if cheap, a
   logistic-regression reference on the raw numeric features. If the semiotic pipeline loses to
   majority class, the report must say so in the same table. That is a real result.
5. **Get the real dataset.** Implement the download that `secom.rs:4` promises: fetch UCI SECOM
   (`secom.data` + `secom_labels.data`) to a cache dir on `--download`, verify a checksum, keep
   the 21-row sample as the offline default. Never vendor a 1567 × 590 dataset into git — the
   repo's big-blob guard from PH-014 will reject it, correctly.
6. Handle the missing values the real dataset actually contains. `SecomObservation.features` is
   already `Vec<Option<f32>>` and `observation_to_text_rich` (`secom.rs:205`) has a
   `missing_features` bucket — verify it behaves on real data, where entire columns are empty.
7. Report the domain-mismatch finding honestly: the 5×14 grid maps fab observations onto
   `BOND×PLAY` / `HEAL×PLAN`. Either add a machine/process axis appropriate to the domain (the
   `config/machine_ontology.json` axis already exists — check whether the benchmark loads it) or
   state in `docs/benchmarks/` that SECOM is out of the shipped ontology's domain. Do not quietly
   leave it.
8. Re-run and record before/after for both `--no-feedback` and the feedback path, at real N.

## Tests you must add

- **Unit** (`src/industrial_benchmark/secom.rs`): `correctness_compares_predicted_class_not_confidence`
  — construct two results with identical `top.score` but different `predicted_cell`, one matching
  ground truth and one not, and assert they are scored differently. This fails before the fix and
  is the packet's core regression test.
- **Unit:** `metrics_report_minority_class` — a run where every prediction is the majority class
  must show recall 0.0 for the minority class while accuracy stays high. This is the property that
  makes the imbalance visible.
- **Unit:** `majority_baseline_is_reported` — the baseline appears in `Metrics` and in the rendered
  report.
- **Unit:** `missing_features_do_not_panic` — an observation whose features are entirely `None`
  produces text and classifies without panicking.
- **Integration:** the CLI writes a report containing `balanced_accuracy` and the confusion matrix;
  `--no-feedback` and the feedback path produce distinguishable output.
- A bug fix without a regression test is not done.

## Gate

```bash
cd ~/dev/physis-pro
cargo test --release --lib industrial_benchmark
cargo run --release --bin physis-pro -- benchmark secom --embedder onnx --output /tmp/secom
grep -q balanced_accuracy /tmp/secom/latest.md
cargo clippy --all-targets -- -D warnings
```

## Commit strategy

Land the metric fix (steps 1–4) before the dataset work (steps 5–6): the corrected metric on the
21-row sample is already a publishable correction, and it is what makes the real-data numbers
meaningful when they arrive. Separate commits.

The `--download` path must not run in CI or in a customer build by default. Network access at
benchmark time is opt-in and must fail closed with a clear message when the cache is absent.

## Report

Append `packets/reports/PH-021.md` per PROTOCOL §5. Must include the full metric table — accuracy,
balanced accuracy, minority-class P/R/F1, confusion matrix — for the old predicate and the new
one, with majority-class baseline in the same table, at both sample and (if step 5 lands) real N.

## Never

- Never report accuracy on an imbalanced dataset without the majority-class baseline in the same
  table.
- Never fit the decision threshold on the test split.
- Never commit the real SECOM dataset to git.
- Never quietly drop a metric because it looks bad. Losing to majority class is a result; hiding
  it is a defect.
