---
id: PH-018
track: physis-pro
phase: 0
status: todo
rung: 1
depends: [PH-017]
gate: "cd ~/dev/physis-pro && cargo test -p physis-core && cargo test --release && cargo clippy --all-targets -- -D warnings"
commit: "branch agent/PH-018; conventional commits; per PROTOCOL §6"
claimed_by:
claimed_at:
---

# Fitness reports two hardcoded constants as if they were measurements

## Why (the failure this prevents)

`predictive_success` carries 0.25 of the composite fitness weight — the single largest term
alongside `empirical_support`. In the product path it is the literal `0.70`. `outcome_success` is
the literal `0.70` too. A customer, a reviewer, or a future maintainer reading a fitness score has
no way to tell that a quarter of it is a number somebody typed.

This is worse than a missing feature. A missing feature is visible; a fabricated constant renders
every downstream comparison meaningless while looking like data. It also makes the engine's
headline claim — that it "preferentially retains interpretations that continue to work" —
literally false on the path customers run: whether an interpretation worked never enters its
score.

## Context (measured 2026-09-06 on physis-pro `13d8cdf` + physis-core `9d43821`; verify before relying on it)

`src/ai/harness.rs:290` — the only place in the product path that builds a full breakdown:

```rust
let breakdown = FitnessBreakdown {
    semantic_fit: semantic_score,
    ontological_fit: ontological_score,
    logical_consistency: logical_score,
    empirical_support: empirical_score,
    predictive_success: 0.70,      // literal
    outcome_success: 0.70,         // literal
    contradiction_penalty,
    failed_prediction_penalty: quality_penalty,
    composite_fitness,
};
```

The machinery to compute the real value **already exists and is already wired into the composite**:

- `physis-core/src/hypothesis.rs:386` — `recompute_fitness()` derives `predictive_success` from
  `Prediction.correct` over resolved predictions, plus a `failed_prediction_penalty` of
  `min(0.15 × failures, 0.5)`.
- `physis-core/src/hypothesis.rs:450` — `resolve_prediction(index, actual_outcome, correct)` is
  the one operation that writes `Prediction.correct`. Its doc comment already records that nothing
  could write that field for the whole life of the project, which pinned a quarter of fitness at
  its 0.5 default.
- Its **only** caller is the CLI at `physis-core/src/main.rs:1130`. The harness never calls it.

`outcome_success` is worse: `rg 'outcome_success'` finds one writer (`harness.rs:296`, the
literal), one default (`hypothesis.rs:195`), and one pass-through (`hypothesis.rs:426`). Nothing
computes it anywhere, and — note this before you spend time on it — `recompute_fitness()` does
**not** include `outcome_success` in the composite at all (`hypothesis.rs:412` weights semantic,
ontological, logical, empirical, predictive only). So `outcome_success` is a field that is
displayed, never computed, and never used. Decide deliberately between computing it from PDCA/task
outcomes and deleting it; do not leave it as a constant.

Two adjacent fabrications, in scope because they are the same defect:

- `physis-core/src/core.rs:750` — `dream_hypotheses()` sets
  `hyp.coherence_profile.empirical.score = 0.2` with the comment "Reflects failure observation".
- `physis-core/src/delta_engine.rs:407` — `hyp.coherence = new_semantic_fit` overwrites the
  composite with a single dimension.

`harness.rs:277` also defaults `empirical_score` to `0.60` when no asserted node is near enough;
`hypothesis.rs:378` uses `0.5` for the same "no evidence" case. Two different neutral values for
the same condition in the same pipeline.

## Steps

1. Delete `predictive_success: 0.70`. Source it from the hypothesis's resolved predictions via
   `recompute_fitness()`. If a candidate has no resolved predictions, the honest value is the
   existing 0.5 "unknown" default — **and the breakdown must be able to say so**. Add an explicit
   marker (e.g. `predictions_resolved: usize` on `FitnessBreakdown`, or `Option<Score>`) so a
   consumer can distinguish "measured 0.5" from "nothing measured yet". This distinction is the
   point of the packet; a bare 0.5 repeats the original sin more quietly.
2. Wire `resolve_prediction` into the product path so the value can ever become non-default.
   Minimum: an HTTP endpoint and a service call that resolves a prediction by
   `(hypothesis_id, index, actual_outcome, correct)`. Re-resolution is already refused by
   `hypothesis.rs:456` — preserve that, and surface the `false` return as a 409, not a silent no-op.
3. Resolve `outcome_success`: either compute it from real PDCA/task outcomes and add it to the
   composite in `recompute_fitness()` with a stated weight, or delete the field. Do not ship a
   third option. Whichever you choose, say why in the commit message and in `docs/FITNESS.md`.
4. Fix `core.rs:750`: the dream loop's empirical score must come from the failure evidence
   attached to the node, not from `0.2`. If the node carries no evidence, use the same "unknown"
   representation chosen in step 1.
5. Fix `delta_engine.rs:407`: assign the semantic dimension, then set
   `hyp.coherence = hyp.coherence_profile.composite()`. Do not overwrite the composite with one
   dimension. Note this changes delta-engine output — capture before/after for the existing
   `physis-core/tests/test_delta_engine.rs` cases in the report.
6. Unify the "no evidence" neutral between `harness.rs:277` (0.60) and `hypothesis.rs:378` (0.5).
   Pick one, name it as a `const` in physis-core, use it in both places, and record in the comment
   which value was chosen and what the other one was.
7. Grep for any remaining fitness/coherence literal before declaring done:
   `rg -n '(fit|success|coherence|score)\s*[:=]\s*0\.[0-9]' src physis-core/src`. Every hit is
   either a documented neutral default, a tuned threshold with a comment explaining the tuning, or
   a bug. Enumerate all three categories in the report.

## Tests you must add

- **Unit** (`physis-core/src/hypothesis.rs`):
  - `unresolved_predictions_are_reported_as_unmeasured` — a hypothesis with predictions but none
    resolved is distinguishable from one with a genuine 0.5; fails before step 1.
  - `resolving_a_prediction_moves_predictive_success` — resolve true, assert the term rises;
    resolve false, assert it falls and the penalty applies.
  - `composite_has_no_literal_terms` — construct a hypothesis where every measurable input is 0.0
    and assert composite is 0.0. Any surviving constant makes this fail. This is the packet's
    real gate.
- **Unit** (`physis-core/src/delta_engine.rs`): `mutation_sets_coherence_from_composite_not_semantic`
  — set the non-semantic dimensions to values that would change the composite, run
  `evaluate_mutation`, assert `hyp.coherence == hyp.coherence_profile.composite()`.
- **Unit** (`physis-core/src/core.rs`): `dream_candidate_empirical_comes_from_evidence` — two
  failure nodes with different attached evidence must not produce the same empirical score.
- **Integration** (physis-pro): resolve a prediction over HTTP and assert the hypothesis's
  `predictive_success` changed in a subsequent read. Assert a second resolve of the same index
  returns 409 and does not change the value.
- A bug fix without a regression test is not done.

## Gate

```bash
cd ~/dev/physis-pro
cargo test -p physis-core
cargo test --release
cargo clippy --all-targets -- -D warnings
```

`cargo test --release` was measured at 790 s on omo and ~630 s here — Tier-1, run it once before
declaring done, not per iteration. Tier-0 is `cargo check --all-targets` + clippy.

## Commit strategy

`FitnessBreakdown` is a public physis-core type and is serialised into persisted state and HTTP
responses. Adding a field is a breaking change for `serde` consumers that deny unknown fields, and
changes stored JSON: add `#[serde(default)]` on any new field so old snapshots still load, and
say in the report what an old `~/.physis` state directory does after the change. Bump physis-core,
push the submodule branch, then bump the pointer in physis-pro.

## Report

Append `packets/reports/PH-018.md` per PROTOCOL §5. Include: the step-7 literal census (all three
categories), the delta-engine before/after from step 5, and the `outcome_success` decision with
its reasoning.

## Never

- Never replace one constant with a different constant. If a value cannot be measured yet, it must
  be *representable as unmeasured*, not guessed.
- Never make `composite_fitness` depend on a field no code path writes.
- Never change the composite weights (0.20/0.15/0.15/0.25/0.25) in this packet. Reweighting is a
  scoring decision and needs an experiment behind it — see PH-020 and the series it records.
