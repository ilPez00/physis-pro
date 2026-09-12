---
id: PH-017
track: physis
phase: 1
status: todo
rung: 2
depends: [PH-000]
gate: "cargo test --lib epistemic:: && cargo test --lib hypothesis:: && cargo run --bin physis-core --features cli -- replay --subject <id> | grep -q Supported"
commit: "branch agent/PH-017; failing-test commit, then the emit commit"
---

# The audit trail cannot see status changes it did not cause

## Why
`replay --subject <id>` reconstructs belief state from the epistemic event log.
Measured 2026-09-12 on the live graph:

```
$ physis-core hypothesis list
  [f8fcf49e] (Supported) fitness=0.603 — Retrieval, not the write path, is ...

$ physis-core replay --subject f8fcf49e-6cb7-4f8f-8b12-7f5502f71fe8
  Belief at 2026-09-12 16:14:22 UTC for [f8fcf49e...]: Candidate
```

`list` and `explain` say **Supported**. `replay` says **Candidate**. Two sources
of truth for one status, disagreeing, on the same read.

The cause: status shown by `list`/`explain` is *derived* from fitness at read
time, while `replay` walks the event log — and `EpistemicEventType::StatusTransition`
is emitted from exactly one place, `src/core.rs:575`, the explicit
`hypothesis transition` command. Adding evidence moves fitness (measured:
0.587 → 0.603, empirical 0.85 → 0.91) and therefore moves the derived status,
and emits nothing.

`FitnessShifted` and `OutcomeObserved` are declared in the event enum and
emitted by **no code path at all** — the same shape as `OnnxEmbedder` before
PH-004's sibling fix: defined, exported, called by nothing.

This is worse than a missing feature. A truth-maintenance system's entire claim
is that the record is authoritative. A record that silently omits the transitions
it did not personally perform is a record that lies by omission, and
point-in-time replay — the thing that makes the ledger worth more than a
retriever — returns a belief the system did not hold.

## Steps
1. Write the failing test FIRST: register a hypothesis, add supporting evidence
   sufficient to move the derived status, then assert `replay` at `now` agrees
   with `hypothesis list`. It must fail before the fix.
2. Emit `FitnessShifted` wherever fitness is recomputed, carrying the before and
   after value so the delta is reconstructible rather than merely announced.
3. Emit `StatusTransition` whenever the DERIVED status changes, not only on the
   explicit command. The emit belongs next to the recomputation, so no future
   caller can move fitness without moving the log.
4. Emit `OutcomeObserved` from `hypothesis resolve`.
5. Decide and document which of the two is authoritative. Recommended: the log.
   Derived status becomes a projection of the log rather than a parallel
   computation, which removes the disagreement by construction instead of
   keeping two answers in sync by discipline.

## Tests you must add
- **Unit:** evidence that moves fitness across a status boundary emits exactly
  one `StatusTransition` and one `FitnessShifted`.
- **Regression:** `replay --at T` for a T between two evidence additions returns
  the status that held between them, not the latest.
- **Invariant:** for every hypothesis in the store, `replay` at `now` equals the
  status `list` prints. This is the assertion that would have caught the defect.
- **Negative:** no event is emitted when a recomputation leaves the status
  unchanged (the log records transitions, not heartbeats).

## Gate
```bash
cargo test --lib epistemic::
cargo test --lib hypothesis::
cargo clippy --lib --bins --all-features -- -D warnings
```

## Never
- Do not fix the disagreement by making `replay` derive from fitness. That
  deletes the audit trail's independence, which is the only reason it exists.
- Do not backfill synthetic events for history that was never recorded. A gap
  in the record is information; a fabricated record is not.
