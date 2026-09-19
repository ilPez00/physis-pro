# Typed temporal fixture suite — methodology and results (development-cycle-2026-09-15)

## What this is and is not

This is a **fixture-and-oracle validation exercise**, not a measurement of
physis-core. Per the task's explicit instruction ("Do not fake the
not-yet-implemented production interface"), no claim here is about the real
CLI/engine's behaviour on these cases — `state(valid_at, known_at)` does not
exist in physis-core yet (verified by reading `physis-core/src/hypothesis.rs`,
`physis-core/src/temporal.rs`, `physis-core/src/epistemic.rs` and
`physis-core/src/main.rs::run_replay` on 2026-09-15; see
`benchmarks/conatus/development-cycle-2026-09-15/ADAPTER_CONTRACT.md` for the
exact grep trail). What exists and is real:

1. A frozen, typed fixture file distinguishing `valid_at` (world time) from
   `known_at` (write/ingestion time) across five required categories: late
   arrivals, corrections, unknown time, interval endpoints, and forward/
   reverse final-evidence arrival order.
2. A small, dependency-free Python reference oracle implementing the
   semantics the fixtures declare, which recomputes every fixture's declared
   answer from raw evidence and flags any mismatch — this catches authoring
   bugs in the fixture file itself, independent of any production code.
3. A known-bad control reproducing the exact defect
   `benchmarks/conatus/e77_temporal.py` measured (replay indexed by
   ingestion/write time, not by the time evidence is about), used to prove
   the fixture suite actually discriminates the two axes rather than being
   vacuously satisfiable by a timestamp-only engine.
4. A self-test of the validator itself (an induced, in-memory fixture error
   that must be caught) -- a known-bad control on the checker, not just on
   the fixtures.

## Commands run and results

```
$ python3 fixture_validator.py
  cases: 10  queries: 22
  categories present: correction, forward_reverse_final_evidence,
    interval_endpoint, late_arrival, unknown_time
  known-bad control disagreed with the typed oracle on 10/22 queries (45%)
  ... (22/22 PASS)
All fixtures self-consistent; known-bad control diverges where expected.
exit code: 0
```

```
$ python3 fixture_validator.py --self-test
SELF-TEST PASS: an induced fixture error (T02-e77-typed-replication/q_jun
expected_status flipped) was correctly flagged by the validator:
  - T02-e77-typed-replication/q_jun: fixture declares SUPPORTED ['e_jun'],
    oracle computes CONTRADICTED ['e_jun']
  - T03-arrival-order-invariance/q_final_jun: cross-case invariance with
    T02-e77-typed-replication/q_jun violated
exit code: 0
```

```
$ python3 check_contradiction_disjointness.py   # secondary deliverable, see below
existing sentences: 128, existing vocabulary: 277 words
new sentences: 32, new vocabulary: 107 words
exact sentence duplicates: 0
word-level vocabulary overlap: 44 words shared (41% of the new fixture's vocabulary)
PASS: no exact sentence duplicates.
exit code: 0
```

All three scripts are self-contained (Python 3 standard library only, no
`.venv` required) and re-runnable from
`benchmarks/conatus/development-cycle-2026-09-15/`.

## Reading the 45% known-bad-disagreement number correctly

This is **not** a benchmark accuracy figure and must not be reported as one.
It is a sanity check that the fixture suite is not vacuous: on 10 of 22
queries, an engine that only tracks *when it learned something* would give a
provably wrong answer relative to an engine that also tracks *when the claim
was true*. The other 12 queries are cases where both oracles happen to agree
(e.g. `T06`'s pre-report query, where there is genuinely no evidence yet
under either reading) -- agreement there is expected and correct, not a gap
in the suite.

## Category-to-requirement mapping (assignment: "late arrivals, corrections,
unknown time, interval endpoints, forward/reverse final evidence")

| required category | fixture case(s) |
|---|---|
| late arrivals | T01 (literal Day-4 gate sentence), T02 (typed E77 replication) |
| corrections | T04 (interval-narrowing correction, matches `temporal.rs`'s own `narrow_until` test), T05 (polarity-reversing correction) |
| unknown time | T06 (unknown-time-only subject), T07 (unknown-time vs. a dated record) |
| interval endpoints | T08 (closed start), T09 (open/exclusive end), T10 (open-ended `valid_until=null`) |
| forward/reverse final evidence | T03 (same final evidence as T02, reverse arrival order; final-`known_at` answers must match T02 exactly, intermediate answers may legitimately diverge) |

## Known limitations (do not overstate this deliverable)

- **10 cases / 22 queries, one author.** This is a schema-and-oracle exercise,
  not a large or independently authored benchmark.
- **The oracle is a specification, not a verified match to any future Core
  implementation.** Core may reasonably choose different precedence rules
  (e.g. supporting-over-contradicting, or a confidence-weighted resolution)
  than this oracle's "contradicting wins" default; `ADAPTER_CONTRACT.md`
  names this as an explicit design choice, not a hidden assumption.
- **`known_bad_state` models one specific wrong design** (pure visibility-by-
  `known_at`, ignoring `valid_at` entirely) because that is what E77 actually
  measured in this codebase. It is not a survey of every possible wrong
  design.
- **No production code was touched or executed against these fixtures.**
  Runtime integration against the real CLI/engine remains explicitly
  NOT MEASURED, per the task's instruction. See `ADAPTER_CONTRACT.md`'s
  proposed CLI/JSON surface for what that integration should look like when
  it exists, and `REQUESTS.md` (report directory) for the concrete ask to
  Core.

## Secondary deliverable status: contradiction fixtures + routed protocol

Prepared, **not run**, per the task's "if time allows" framing and the
explicit scope boundary (this worker does not own
`benchmarks/conatus/e86_participant_binding.py`, `e89_nli_arm.py`, or the
`.venv-nli` environment):

- `fixtures/contradiction_holdout_disjoint_2026-09-15.json` — 20 new pairs
  (10 dev, 10 heldout) across participant/object/time/negation_scope/
  unit_conversion/paraphrase, in logistics/facilities vocabulary chosen to
  minimize lexical overlap with the existing E83-E86 industrial-machine
  fixtures. Verified disjoint from those fixtures by
  `check_contradiction_disjointness.py` (0 exact-sentence duplicates, run
  above). **Human-authored by this worker — explicitly not independent human
  validation**, per the task's own caution against a worker's self-authored
  fixtures counting as a fresh blind holdout in place of a second author.
- `ROUTED_COMPARISON_PROTOCOL.md` — a frozen, predeclared comparison spec
  (arms, primary metric, denominators, stopping rule) for RULES_ONLY /
  NLI_ONLY / NAIVE_UNION / ROUTED on the new held-out split, written and
  frozen *before* any arm has been run against it. Execution is out of this
  task's scope and ownership; handed off in `REQUESTS.md`.
