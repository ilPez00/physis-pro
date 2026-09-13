---
id: PH-019
track: physis-core
phase: 1
status: done
rung: 2
depends: []
gate: "cd ~/dev/physis-pro/physis-core && cargo test --lib temporal:: && cargo test --lib hypothesis:: && cargo clippy --lib --tests -- -D warnings"
commit: "branch agent/PH-019; conventional commits; per PROTOCOL §6"
claimed_by: claude-pansa-2026-09-13
claimed_at: 2026-09-13
---

# A validity window can be opened but never narrowed

## Why (the failure this prevents)

`TemporalValidity` carries both legs of a bi-temporal record —
`valid_from`/`valid_until` (when the claim was true in the world) and
`expired_at` (when the record was superseded). `is_valid_at` reads all three.
The schema is right and it is genuinely used.

What is missing is the path from **evidence** to the world-time leg. Nothing in
`physis-core` ever writes `valid_until` except a constructor:

```
$ grep -rn 'valid_until\s*[:=]' src/ --include='*.rs' | grep -v '^src/temporal.rs'
src/process.rs:69:    pub valid_until: Option<chrono::DateTime<chrono::Utc>>,
```

So a claim's world-time end is whatever the caller asserted when the claim was
created, and later evidence that the claim stopped being true earlier than
recorded cannot correct it. The ledger will happily carry a window it has
already been given the evidence to narrow.

This was found by testing graphiti, which has the same defect one step further
along (`computer-remake-research/experiments/graphiti/RESULTS.md`): graphiti
*does* write `invalid_at` once, from the first contradiction it sees, and then
never narrows it when better evidence arrives. Measured there: an employment
fact was recorded as ending 2026-02-20 while the record contained a document
saying it ended 2025-03-10, so a query for mid-2025 returned two employers.

Physis is exposed to the identical error and has no mechanism to avoid it,
which makes this a prerequisite for using the world-time leg for anything.

## Context (measured 2026-09-13, verify before relying on it)

- `src/temporal.rs` — `TemporalValidity { valid_from, valid_until, trigger,
  expired_at }`. Public API is `permanent()`, `from()`, `during()`,
  `is_valid_at()`, `is_current()`, `overlaps()`. **No mutator of any kind.**
- `src/hypothesis.rs:424` — `Hypothesis::is_valid_at` delegates to it.
- `Hypothesis::revise` already exists and appends to `revision_history`; every
  belief change in this crate is expected to leave a `Revision` behind. A
  narrowing that did not record one would be the first silent mutation in the
  ledger, which is the one thing this crate exists to prevent.

## Steps

1. Add `TemporalValidity::narrow_until(&mut self, when, trigger) -> bool`.
   - **Narrow only.** If `when` is at or after an existing `valid_until`, make
     no change and return `false`. Widening a window on new evidence is a
     different claim, not a correction, and must not be done silently here.
   - Refuse and return `false` if `when` precedes `valid_from` — that is a
     contradiction about the interval itself, not a narrowing, and it should
     surface rather than produce an empty window.
   - Set `trigger` when one is supplied, so the narrowing says what caused it.
2. Add `Hypothesis::narrow_validity(&mut self, when, trigger) -> bool` that
   calls it and, **only when the window actually changed**, records a
   `Revision` describing the correction. A no-op must not pollute the
   revision history.
3. Do not wire this into evidence ingestion in this packet. The mechanism and
   its record come first; deciding *which* evidence narrows a window is a
   separate question, owned by G2 in the integration plan (see below).

## Tests you must add

- **Unit (`temporal.rs`):** `narrow_until` narrows an open window; narrows a
  window that already had a later `valid_until`; returns `false` and changes
  nothing when given a later instant than the current `valid_until`; returns
  `false` when given an instant before `valid_from`.
- **Unit (`temporal.rs`):** the graphiti case, by name — a window ended at T2
  by a first contradiction is corrected to T1 when earlier evidence arrives,
  and `is_valid_at` between T1 and T2 flips from true to false. This is the
  test that fails before the change.
- **Unit (`hypothesis.rs`):** a successful narrowing appends exactly one
  `Revision`; a refused narrowing appends none.

## Gate

```bash
cd ~/dev/physis-pro/physis-core
cargo test --lib temporal::
cargo test --lib hypothesis::
cargo clippy --lib --tests -- -D warnings
```

Note: `cargo clippy --all-targets` in physis-core is red on 31 pre-existing
errors, all in `examples/`. Scope the gate to `--lib --tests` until that is
cleared separately.

## Commit strategy

Branch `agent/PH-019`. One commit for `temporal.rs` + its tests, one for
`hypothesis.rs` + its tests.

## Never

- Never let `narrow_until` widen a window. If evidence says a claim lasted
  *longer* than recorded, that is a new claim about the interval and belongs in
  the revision machinery as such, not in a setter.
- Never narrow without recording the revision. A silent correction to the
  world-time leg is indistinguishable from the record having always said that,
  which defeats replay.

## Relation to the existing plan

`docs/ATLAS_GRAPHITI_INTEGRATION_PLAN.md` already owns the follow-on. **G1**
(the `expired_at` system leg) has landed, and **G6** reads it. **G2** —
"(resolved, invalidated, new) ingest protocol … stamping invalid_at/expired_at"
— is the step that decides *which* evidence moves a world-time boundary, and it
is already scoped there with the right constraint:

> judgment needs a deterministic rule first (LLM judging NOT taken) — start
> with overlap+recency, log every call

and its probe, **Q3**: overlap+recency on 50 hand-built pairs, ship flag-only
if it is not clearly above chance.

This packet is the prerequisite G2 is missing: G2 stamps a boundary, and until
`narrow_until` exists there is no supported way to stamp one, nor to correct it
when the next contradiction is better than the first. The graphiti measurement
in `computer-remake-research/experiments/graphiti/RESULTS.md` is independent
support for G2's "LLM judging NOT taken" constraint — the extraction step there
dropped episodes, duplicated edges and rewrote facts into another language,
while every timestamp on top stayed correct.
