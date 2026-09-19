---
id: PH-009
track: physis
phase: 1
status: done
rung: 2
depends: [PH-000]
gate: "cargo test -- --list | wc -l && cargo test --test http_surface -- --test-threads=4"
commit: "branch agent/PH-009; instrumentation commit, then parallelisation commit"
---

# Test suite speed: 790 s is too slow to gate on

## Why
Measured 2026-09-01: full cargo test = 790 s (docs claimed 333 s). Inference
is Mutex-serialised, so threads do not help the embedder-critical sections.
A 13-minute gate cannot run per-iteration, so loops fly blind for most of the
time and the Tier-0/Tier-1 split is doing all the work.

## Steps
1. Instrument: `cargo test -- --nocapture` timing per suite OR add
   test-util timing output. Produce a table of the 10 slowest tests in the
   report.
2. Classify each slow test: needs embedder weights / needs HTTP server / pure.
3. Attack in this order: (a) share one server instance across tests where
   safe, (b) mock the embedder with a deterministic fake for tests that only
   exercise routing/logic, (c) keep a `#[ignore]`-marked heavy tier with an
   explicit `--include-ignored` Tier-2 gate.
4. Target: Tier-1 (default `cargo test`) under 300 s with NO coverage loss —
   the ignored set must enumerate everything it skips in its output.

## Tests you must add
- **Meta-test:** a test asserting the fast tier contains ≥ N tests and the
  ignored list is printed by `cargo test -- --ignored --list` — protects
  against silently burying coverage.
- Every test converted to a fake embedder keeps an `#[ignore]`d real-embedder
  twin if it exercised embedding semantics.

## Gate
```bash
time cargo test              # < 300 s on pansa
cargo test -- --ignored --list | tail -5   # the buried tier, enumerated
cargo clippy --all-targets -- -D warnings
```

## Never
- Delete or weaken tests to go faster. Speed comes from fakes and sharing,
  never from removal (gate: no-weakened-tests behaviour).
