# PH-104 — SECOM benchmark verdict

**Packet:** `PH-104-secom-wire-or-decide`
**Branch:** `agent/PH-102-generic-extract-guard`
**Box:** pansa (CPU-only, no leptonica)
**Date:** 2026-09-22
**Verdict:** research-only — de-cited from customer-facing claims.

## Attempt 1: full-data run

```
cargo run --features full --bin physis -- benchmark secom --embedder onnx --limit 0 --seed 42 --no-feedback
```
Fails at build: `leptonica-sys` requires system `lept.pc` (not installed:
`Package 'lept' not found`). `leptonica`/`tesseract` are pulled by the `ocr`
feature, which `--features full` enables. The ONNX embedder path lives behind
`embed-onnx` but the build break blocks the whole `--features full` gate.

## Attempt 2: embedded sample only

```
cargo run --bin physis -- benchmark secom --limit 0 --seed 42 --no-feedback
```
Reproduces the PH-104 diagnosis exactly:
- 21 observations / 10 features (embedded sample `secom_data.csv`)
- 10-row test split (seed=42)
- 60% accuracy (random embedder, `dim=768`)
- 61.9% majority-class baseline

The ONNX 70% figure (PLAN.md H5) cannot be reproduced without the full build
and the real UCI dataset (1567 × 590), for which no download seam exists in
`src/industrial_benchmark/secom.rs` (line 451: `let raw_text = SECOM_SAMPLE;`).

## Decision: research-only

Per PH-104 Step 3 ("If full data cannot run: remove SECOM from customer-facing
claims, mark the module research-only in place"), the following changes ship:

1. `src/industrial_benchmark/secom.rs` doc comment — marked **research-only**,
   pointing to this verdict.
2. `PLAN.md` H5 section — verdict recorded; 0.523→0.939 / 0.700 figures
   re-tagged as sample-only shape indicators.
3. `docs/ARCHITECTURAL_REPORT.md` Claim 5 — SECOM citation qualified
   "research-only".

## Gate

```
cargo test --lib
cargo clippy --all-targets -- -D warnings
! grep -rn -i "secom benchmark" docs/ARCHITECTURAL_REPORT.md | grep -v "research-only"
```
