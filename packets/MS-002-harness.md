---
id: MS-002-harness
track: multiscale-structure
phase: 1
status: review
rung: 2
depends: [MS-002]
gate: "cd physis-core && cargo test --test replay_contract && cargo clippy --lib --tests -- -D warnings"
commit: "branch agent/MS-002-harness; conventional commits; per PROTOCOL §6"
claimed_by: opencode
claimed_at: 2026-09-20
---

# MS-002-harness — the runnable replay contract MS-002 left undelivered

## Why (the failure this prevents)

MS-002's acceptance criterion — "one runnable check with no-op, local, fanout,
cycle and disconnected mutations" plus "a corrupted incremental arm that the
oracle rejects" — was deferred, not met. Its review note records that. Until
this packet lands, MS-004 through MS-008 rest on verified prose rather than an
executable oracle, and the A1 comparison adapter in
`research/multiscale/MS-002/harness_design.md` has never been compiled. A
contract that has never run is the failure shape PH-004 was opened for.

## Context

Measured facts, all from `packets/reports/MS-002.md` and re-checked by its
review note (2026-09-20, `status: done`):

- `evaluate_mutation` (`physis-core/src/delta_engine.rs:689`) has **zero
  production callers**; every existing call is in `#[test]` bodies. There is
  no production baseline to compare against, so the oracle must be built here.
- **A1 is binding.** `find_neighbors` ends `neighbors.into_iter().collect()`
  (line 297) with no sort, so `OntologyDeltaReport.affected_nodes` order is
  unspecified and `net_coherence_delta` is an order-dependent f32 sum. Compare
  `affected_nodes` as a **set sorted by `node_id`** and `net_coherence_delta`
  to **epsilon 1e-6**. Never positionally, never bit-exact.
- The struct field is `pub affected_nodes: Vec<NodeDelta>`
  (`delta_engine.rs:393`); `node_deltas` is only a local builder name.
- `depends_on_walk` is sorted and deterministic given its edges; positionally
  comparable fields are `revision_walk.steps`, `revised_hypotheses`,
  `hypothesis_status_shifts`, `adjudications`.
- **No timing instrumentation exists.** `duration|Duration|elapsed|Instant`
  over `delta_engine.rs` returns 0 hits. Duration/CPU/peak-memory capture
  therefore needs production edits and is **out of scope** here.
- Fixtures already exist and are reusable: `unit_vector`, `make_node`,
  `chain_graph` in `physis-core/tests/test_delta_engine.rs:13-58`.

## Steps

1. Settle the oracle semantics **before writing the oracle**, and write the
   decision into the report. The wave deliberately stops at
   `MAX_PROPAGATION_DEPTH` (5) and `MIN_IMPACT` (0.01), so a full recompute is a
   **superset** of the wave by construction: a naive "incremental == full
   changed set" equality oracle fails on healthy input and proves nothing.
2. Build an **independent** expected-affected-set function: its own BFS over
   the documented neighbour rule (typed edges both directions, shared
   `cell_pin`, `parent → ` label prefix) with the same depth/impact cutoffs.
   It must not call `find_neighbors`, `build_adjacency` or `dfs_collect` —
   reusing the code under test makes the oracle a tautology.
3. Add `physis-core/tests/replay_contract.rs` (test-only; no production edits)
   with these arms, each a `#[test]`:

   | arm | mutation shape | oracle expectation |
   |---|---|---|
   | no-op | `EmbeddingShift` with `old == new` | empty report (A7, line 695) |
   | local | single-node shift, no neighbours | 1 delta, 0 transitions |
   | fanout | nodes tied by shared `cell_pin` | ≥2 affected, set-compared |
   | cycle | `DependsOn` loop | `revision_walk.cycles` non-empty, terminates |
   | disconnected | mutation on an orphan node | wave does not leak |
   | corrupted-incremental | incremental path drops a dependency edge | oracle **rejects** and names the omitted node |

4. Assert at least these properties:
   - **P1** — `previous_coherence` equals the base node's stored
     `coherence_score`. **Corrected during execution:** the packet first asked
     for "every affected entry also changed under a full recompute", which
     compares a *stored* score against a *recomputed* one and fails on healthy
     input. See `packets/reports/MS-002-harness.md` §2.
   - **P2 value agreement** — each `NodeDelta.new_coherence` equals the full
     recompute for that node, epsilon 1e-6.
   - **P3 expected set** — on the five non-negative arms,
     `affected_nodes` as a set sorted by `node_id` equals the independent
     expected set from step 2.
   - **P4 the oracle can fail** — the corrupted arm must make P3 fail. Assert
     the mismatch is reported and that the omitted `node_id` is named. If the
     corrupted arm passes, the oracle is broken, not the engine.
   - **P5 A1 discipline** — no positional comparison of `affected_nodes`, no
     bit-exact float comparison anywhere in the file.

## Tests you must add

The test binary **is** the deliverable. Six `#[test]` arms above, each with a
nonzero assertion count; P4 must fail if the corruption is removed from the
test setup.

## Gate

```bash
cd physis-core
cargo test --test replay_contract
cargo clippy --lib --tests -- -D warnings
```

Run the gate twice in a row: A1 means an ordering-sensitive assertion can pass
by luck on one run.

`--all-targets` is deliberately not used: `physis-core/examples/` is red under
`-D warnings` for pre-existing reasons (41 error lines, including an untracked
`examples/embed_probe.rs` left by another session), none of which this packet
owns or may fix. See `packets/reports/MS-002-harness.md` §5.

## Report

Append `packets/reports/MS-002-harness.md` per PROTOCOL §5: the oracle-semantics
decision from step 1, the arm results, and an explicit statement of what the
oracle still cannot see (e.g. the wave's O(affected × n) `compute_coherence`
cost, which remains uninstrumented).

## Never

- No production source edits. `delta_engine.rs` is not touched by this packet.
- No positional comparison of `affected_nodes`; no bit-exact float equality.
- No reusing `find_neighbors`/`dfs_collect` inside the oracle.
- No speedup, complexity or O(|A|) claim from closure counts.
- No sorting change to the wave — that is a separate core packet and is not
  this one.
- No new snapshot format; reuse the `test_delta_engine.rs` helpers.
- Do not delete or "fix" the untracked `physis-core/examples/embed_probe.rs`:
  it belongs to another session, and removing it would hide the fact that
  `cargo test` in `physis-core` currently cannot compile.
