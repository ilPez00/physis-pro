---
id: PH-020
track: physis-core
phase: 0
status: review
rung: 2
depends: []
gate: "cd ~/dev/physis-pro/physis-core && for i in 1 2 3 4 5; do cargo test --lib direction:: || exit 1; done && for i in 1 2 3; do cargo test --lib || exit 1; done"
commit: "branch agent/PH-020; conventional commits; per PROTOCOL §6"
claimed_by: claude-pansa-2026-09-13
claimed_at: 2026-09-13
---

# One test sets a process-global env var and the rest of `direction` fails around it

## Why (the failure this prevents)

`direction.rs` is the only implementation anywhere of scoring a transformation
against a null that achieves the same magnitude the stupid way — the operation
`computer-remake-research/gaps.md` records as **Gap 9**, which no other system
in a twenty-one system survey performs. Its own measurement (separation
+0.1964, accuracy 27/30) is the sole evidence that the operation is even
well-defined.

Its tests currently fail one or two at a time, nondeterministically, whenever
they run concurrently. That is worse here than anywhere else in the crate: a
flaky gate on the module that exists to make claims falsifiable trains a
reviewer to disbelieve the one number this project uniquely has.

## Context (measured 2026-09-13, verify before relying on it)

Root cause is identified, so this packet is a fix rather than an investigation.

`src/direction.rs:516` — `the_cosine_escape_produces_a_different_statistic`:

```rust
// Set, measure, restore — the variable is process-global.
unsafe { std::env::set_var("PHYSIS_DIRECTION_METRIC", "cosine") };
let broken = worst_covered(LONG, &cut, &e);
unsafe { std::env::remove_var("PHYSIS_DIRECTION_METRIC") };
```

The comment identifies the hazard and then addresses only half of it. The value
*is* restored — but `cargo test` runs tests on many threads, and for the
duration of that window **every other test in the process** that calls
`worst_covered` reads `PHYSIS_DIRECTION_METRIC=cosine` (`direction.rs:300`) and
silently computes with the known-broken metric.

Observed consequences, both from the full suite:

```
truncation_gets_shorter_and_still_fails
  "truncation is the null, so the margin must be ~0, got -0.16491014"

truncation_scores_exactly_zero_margin_against_its_own_null
  assertion `left == right` failed: left: 0.796537, right: 0.9627942
```

Both are what cosine does — it rewards copying, which is why it is the
benchmark's designated bad arm in the first place.

Evidence it is the concurrency and nothing else:

| run | result |
|---|---|
| `cargo test --lib -- --test-threads=1` | **260 passed, 0 failed** |
| `cargo test --lib` (default threads), 3 runs | 2 failed, 1 failed, 1 failed |
| `cargo test --lib direction::` alone, 3 runs | ok, ok, **1 failed** |

It fails even when only the ten `direction` tests run, which rules out
interference from the rest of the crate. It is **not** caused by the presence
of `models/bge-base-en-v1.5/onnx/model.onnx` (verified: removing the weights
does not fix it) and **not** by PH-019 (verified: the failures reproduce with
`src/temporal.rs` and `src/hypothesis.rs` stashed).

## Steps

1. Reproduce: `cargo test --lib direction::` in a loop until it fails. It is
   cheap — under 0.1 s per run — so do this before touching anything.
2. Decide the fix. Two candidates, in order of preference:
   - **Thread the metric through as a parameter.** `worst_covered` takes the
     metric explicitly; the env var is read **once**, at the CLI/benchmark
     boundary, and passed down. This removes the global entirely rather than
     synchronising around it, and it makes the benchmark's two arms explicit at
     the call site instead of implicit in process state.
   - **A test-only mutex.** A `static METRIC_LOCK: Mutex<()>` taken by every
     test that reads or writes the metric. Cheaper to write, but it leaves the
     global in place, serialises the module's tests, and silently breaks again
     the next time someone adds a test that forgets the lock.
3. Whichever is chosen, say in the comment why the previous shape failed. The
   existing comment is *correct about the hazard and wrong about the remedy* —
   per the repo rule, update it to describe the new reason rather than deleting
   it.
4. Re-run the gate below. A single green run is not evidence for a flake.

## Tests you must add

- **Unit:** a test that fails before the fix and passes after. With the
  parameter fix this is direct — assert that `worst_covered` with the default
  metric is unaffected by `PHYSIS_DIRECTION_METRIC` being set in the
  environment, which is false today and true after.
- Keep `the_cosine_escape_produces_a_different_statistic`. Its assertion is
  right and worth having: the benchmark's control must genuinely differ from
  the metric it grades. Only its mechanism is wrong.

## Gate

```bash
cd ~/dev/physis-pro/physis-core
for i in 1 2 3 4 5; do cargo test --lib direction:: || exit 1; done
for i in 1 2 3; do cargo test --lib || exit 1; done
```

Five and three, because one pass proves nothing about a race.

## Never

- Never fix this by adding `--test-threads=1` to CI. That hides the defect,
  costs the whole crate its parallel test run (9.7 s vs 2.7 s), and leaves the
  global hazard in place for any non-test caller that ever runs two directions
  concurrently.
- Never fix it by loosening the assertions. `-0.165` is not a tolerance
  problem; it is the wrong metric being used.

## Relation to other packets

Same shape as **PH-015** (content-search flake under full-suite concurrency) and
**PH-018** (two `http_surface` reds) — shared mutable state crossing test
boundaries. Unlike those two, this one is already root-caused, so it should be
cheaper than either.
