# autoresearch — clippy-discovery

## Goal
Minimize `warnings` on `src/discovery.rs`. **Lower is better.**
Baseline (measured on a clean checkout, 2026-07-12): `warnings: 11`
(clippy::pedantic warnings attributed to discovery.rs; 962 lib-wide)

Best so far: `warnings: 0` (commit on 2026-07-12) — loop driven to its floor.
The 6 remaining `usize as f32` casts from the previous best got explicit,
justified `#[allow]`:
- `cast_precision_loss` on the three `len() as f32` count→mean conversions
  (corpus length is small → no precision loss);
- `cast_precision_loss` + `cast_possible_truncation` + `cast_sign_loss` on the
  `tail_threshold` quantile index (`len * frac` ≤ len with `frac ∈ [0,1]`, then
  `.min` bounds it — a valid non-negative floor).
All 10/10 discovery tests still pass, `compiles: 1`. Target file is now pedantic-clean..

Metric is produced by `scripts/autoresearch/evaluate_clippy.py src/discovery.rs`:
- `warnings` = count of `clippy::pedantic` warnings whose diagnostic path is
  `src/discovery.rs`. This is the number to drive toward 0.
- `compiles` is a hard gate: if the crate does not build (a real `error[...]`,
  not just lints), the metric is a large sentinel (100000) so the loop can never
  reward code that doesn't compile.
- `total_warnings` (whole-lib pedantic count) is reported for information — use
  it to confirm you are actually *fixing* warnings, not just pushing them into
  another file.

## What the Agent Can Change
- Anything inside `src/discovery.rs`: cast styles (`as f32` → `f32::from` /
  `try_into` where lossless, or an explicit `#[allow]` with a justifying comment
  only where the cast is provably safe), redundant closures, `# Panics` /
  `# Errors` doc sections, `#[must_use]`, `BTreeMap::default()` vs `::new()`.
- Behaviour MUST be preserved: the 10 discovery tests are the correctness gate.

## What the Agent Cannot Change
- The evaluator (`scripts/autoresearch/evaluate_clippy.py`) — read-only.
- Any file other than `src/discovery.rs`.
- Dependencies — do not add crates.
- The public API of the module (`discover`, `DiscoveryConfig`, `DiscoveryReport`,
  `ProposedDomain`) — callers and tests must still compile and pass.
- The P10 auto-retune semantics shipped in commit ebb7846 — the diffuse-corpus
  guarantee and the singleton guard must both still hold.

## Verification (the agent must run before claiming an improvement)
    cargo test --lib discovery          # all 10 tests still pass (correctness)
    python3 scripts/autoresearch/evaluate_clippy.py src/discovery.rs   # metric

A run only counts if `compiles: 1`, every discovery test passes, AND `warnings`
is strictly below the previous best. Blindly `#[allow]`-ing the whole file is
against the spirit of the task — prefer real fixes; use `#[allow]` only with a
one-line justification for a provably-safe cast.
