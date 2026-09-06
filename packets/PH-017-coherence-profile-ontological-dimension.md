---
id: PH-017
track: physis-pro
phase: 0
status: todo
rung: 1
depends: []
gate: "cd ~/dev/physis-pro && cargo test -p physis-core && cargo test --release --lib && cargo clippy --all-targets -- -D warnings"
commit: "branch agent/PH-017; conventional commits; per PROTOCOL §6"
claimed_by:
claimed_at:
---

# The ontological coherence dimension is computed and then silently discarded

## Why (the failure this prevents)

Every coherence number this engine has ever reported is missing the one dimension that is not a
plain text-similarity measure — and nothing anywhere says so. `src/ai/harness.rs` computes an
ontological score, hands it to `CoherenceProfile::set("ontological", …)`, and that call is a
no-op because the struct has no such field and the `match` has no such arm. The value is dropped
without a warning, a log line, or a compile error.

The consequence is not a wrong number in one place. It means the composite coherence that drives
hypothesis ranking is (three cosine-derived values + three untouched 0.5 defaults) / 6, while the
docs, the thesis, and the module comment all claim a multidimensional evaluation. Any measurement
taken before this is fixed is measuring something other than what it is reported to be — which is
why this packet is rung 1 and everything in PH-020 depends on it.

## Context (measured 2026-09-06 on physis-pro `13d8cdf` + physis-core `9d43821`; verify before relying on it)

`physis-core/src/coherence_dimensions.rs:50` — the struct has six fields and no `ontological`:

```rust
pub struct CoherenceProfile {
    pub semantic: CoherenceDimension,
    pub logical: CoherenceDimension,
    pub temporal: CoherenceDimension,
    pub causal: CoherenceDimension,
    pub procedural: CoherenceDimension,
    pub empirical: CoherenceDimension,
}
```

`physis-core/src/coherence_dimensions.rs:96` — `set()` falls through silently:

```rust
pub fn set(&mut self, label: &str, score: Score, weight: Score) {
    let dim = CoherenceDimension::new("", score, weight);
    match label {
        "semantic" => self.semantic = dim,
        // ... five arms ...
        _ => {}                      // <— "ontological" lands here
    }
}
```

`src/ai/harness.rs:247` computes the score and `:252` throws it away:

```rust
let ontological_score = classifier.best_entry_sim(&embedding)
    .map(|(sim, _, _)| sim).unwrap_or(0.5).clamp(0.0, 1.0);
profile.set("ontological", ontological_score, 1.0);   // no-op
```

It does survive into `confidence` at `harness.rs:288`
(`ontological_score * 0.4 + semantic_score * 0.6`) and into `FitnessBreakdown.ontological_fit`
at `harness.rs:292` — so the score is not entirely unused, only absent from the profile and
therefore from `composite()`.

Related doc drift, in scope for this packet: `physis-core/src/lib.rs:20` documents **five**
dimensions (`Semantic, Ontological, Logical, Empirical, Process`); the implementation has six and
shares only three names with that list. `PHYSIS_THESIS.md` §4.1 repeats the five-name version.

Note the second, separate silent-drop hazard in the same function: `temporal`, `causal` and
`procedural` are never written by any caller in either crate, so they always contribute 0.5 at
weight 1.0. That is PH-018's territory for the fitness side; here, only make it *visible*.

## Steps

1. Add `pub ontological: CoherenceDimension` to `CoherenceProfile`. Initialise it in `new()` as
   `CoherenceDimension::neutral("ontological")` and include it in `composite()` and `summary()`.
   `Default` derives through, but check the `#[derive(Default)]` on the struct still does what the
   `new()` path does — they differ today (`Default` gives `label: "unspecified"`).
2. Make `set()` unable to drop a value again. Either return `bool` (and have every caller check),
   or `debug_assert!(false, "unknown coherence dimension: {label}")` in the `_` arm, or — best —
   replace the string API with an enum parameter so an unknown dimension cannot be spelled. Prefer
   the enum: `CoherenceDimension` already exists as a struct name, so use a distinct name such as
   `Dim` to avoid the collision. If you keep the string API for serde/HTTP compatibility, keep it
   as a thin wrapper over the enum version and mark it `#[deprecated]`.
3. Update `src/ai/harness.rs` to use the non-dropping API. The ontological value it already
   computes is correct; this is a plumbing fix, not a scoring change.
4. Recalibrate any threshold that was tuned against the old 6-value composite. Search for literal
   comparisons against `composite()` / `coherence` (`rg 'coherence\s*[<>]='`) and note in each
   comment that the divisor changed from 6 to 7. Do not silently retune: if a threshold moves,
   say in the comment what it was, what it is, and why.
5. Fix the doc drift: `physis-core/src/lib.rs:20` and `PHYSIS_THESIS.md` §4.1 must list the
   dimensions the code actually has. Per repo rule, do not delete the old comment text — rewrite
   it to describe the new reason.
6. Record the composite before/after for one fixed input in the report, so the reviewer can see
   exactly how much every existing score moves.

## Tests you must add

- **Unit** (`physis-core/src/coherence_dimensions.rs`, `mod tests`):
  - `ontological_dimension_is_set_and_reaches_composite` — set ontological to 1.0 with the other
    five at 0.0 and assert `composite()` changes; this fails before the fix.
  - `unknown_dimension_name_is_not_silently_dropped` — asserts the chosen mechanism from step 2
    (returns false / panics in debug / does not compile, whichever you implemented).
  - `composite_divides_by_seven_dimensions` — pin the arity so a future field addition that
    forgets `composite()` fails loudly.
- **Integration** (`tests/` in physis-pro): drive the harness path over one fixed input and assert
  `coherence_profile.ontological.score` is non-default and equals the `ontological_fit` in the
  same `FitnessBreakdown`. This is the end-to-end proof the value now arrives.
- A bug fix without a regression test is not done.

## Gate

```bash
cd ~/dev/physis-pro
cargo test -p physis-core
cargo test --release --lib
cargo clippy --all-targets -- -D warnings
```

## Commit strategy

`physis-core` is a submodule with its own remote and is published to crates.io. The struct change
is a **breaking change to a public type**: bump the minor version, commit in the submodule first,
push that branch, then commit the submodule pointer bump in physis-pro. Do not leave physis-pro
pointing at an unpushed submodule commit.

## Report

Append `packets/reports/PH-017.md` per PROTOCOL §5. Include the before/after composite for the
fixed input from step 6.

## Never

- Never "fix" this by deleting the `set("ontological", …)` call. The score is correct; the sink is
  missing.
- Never widen the fix into rescoring semantic/logical/empirical. That is PH-018 and PH-019.
- Never change `GAMMA`, `DEGRADATION_THRESHOLD`, or any delta-engine constant here.
