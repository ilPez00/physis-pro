# MS-002-harness — report

Claimed and executed 2026-09-20. Status: `review`. Deliverable:
`physis-core/tests/replay_contract.rs` (test-only; no production edit).

## 1. Oracle semantics (the decision the packet demanded first)

The packet's step 1 asked for this before any oracle code. **The wave is not
"the set of nodes whose coherence changed."** Two independent reasons:

- The DFS stops at `MAX_PROPAGATION_DEPTH` (5) and `MIN_IMPACT` (0.01), while
  `compute_coherence` scans the whole effective graph (`effective_nodes_all`).
  A mutation can change the coherence of a node the wave never visits.
- The wave's `previous_coherence` is the node's **stored** `coherence_score`,
  while `new_coherence` is a **recomputed** value. A delta is therefore
  `recomputed − stored`, not `recomputed − recomputed`.

So an equality oracle over the changed set fails on healthy input and proves
nothing. What is checked instead:

| id | property |
|----|----------|
| P1 | `previous_coherence` equals the base node's stored score |
| P2 | every `new_coherence` equals an independent full recompute, epsilon 1e-6 |
| P3 | `affected_nodes`, as a set sorted by `node_id`, equals an independent BFS over the documented neighbour rule |
| P7 | `net_coherence_delta` equals the sum of the reported deltas, epsilon 1e-6 |
| P8 | the hidden-global-work gap is pinned, not asserted away |

## 2. The packet's P1 was unverifiable, and that is the finding

The packet specified P1 as "every `affected_nodes` entry also changed under an
independent full recompute". Implemented literally, the fanout arm failed on
healthy input:

```
P1: n2 reports a change the full recompute does not see
```

`n2` carries the fixture score `0.5`, recomputes to `0.0` before and after the
mutation, and so reports a `−0.5` delta that no recomputed-vs-recomputed check
can see. The packet's P1 compared the stored score against a recomputed one.
Replaced with P1 as tabulated above. This is worth carrying forward: a
successor harness that checks "delta != 0 implies the recompute moved" will
reject correct runs.

## 3. The independent oracle

`neighbors_of` re-derives the documented rule (typed edges both directions,
shared `cell_pin`, `parent → ` label-prefix) and `expected_wave` re-derives the
DFS cutoffs. Neither calls `find_neighbors`, `build_adjacency` or `dfs_collect`.
The full path builds `EvaluationContext::from_base` over a clone of the base
nodes with the mutation applied, then reads `compute_coherence` for every node.

**Limitation, stated plainly:** the BFS reimplements the same *spec*. It
catches an implementation error in the wave, not an error in the rule itself.
Only an independently specified notion of "should be affected" could do that,
and no such specification exists yet.

## 4. Arms (7 tests, all green)

| test | arm | result |
|---|---|---|
| `no_op_shift_returns_an_empty_report` | no-op | empty report, `revision_walk.is_none()`, shadow frame untouched (A7) |
| `local_mutation_affects_only_the_target` | local | wave `{n0}` |
| `cell_pin_fanout_reaches_both_link_classes` | fanout | wave `{n0,n1,n2}` — the edge carries n1, the shared pin carries n2 |
| `depends_on_cycle_is_recorded_and_terminates` | cycle | `revision_walk.cycles` non-empty, not truncated |
| `disconnected_mutation_does_not_leak` | disconnected | wave `{orphan}` |
| `full_recompute_moves_a_node_the_wave_never_visits` | P8 | `n3` changes under the full recompute and is never visited |
| `oracle_rejects_a_corrupted_incremental_wave` | negative | corruption (n0–n1 edge dropped) rejected; the oracle names `n1` |

P8 is the measured "hidden global work" MS-002 flagged: `n3` is identical to
`n0` before the shift and linked to nothing, so the wave never reaches it, yet
its coherence moves because `compute_coherence` is global.

## 5. Gate

```
$ cargo test --test replay_contract          # run twice, per the packet
test result: ok. 7 passed; 0 failed   (run 1)
test result: ok. 7 passed; 0 failed   (run 2)

$ cargo clippy --lib --tests -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s)

$ cargo test --lib --tests
233 passed (lib) + 13 integration targets, 0 failed
```

**Caveat, with its date.** `cargo clippy --all-targets -- -D warnings` is red
in `physis-core`, and was red before this packet: 41 error lines, every one in
`examples/` (hex-literal grouping, unused imports in `mapper_docs_export.rs`,
unused vars in `experiment49/50`, and an unresolved
`physis_core::embed_onnx` import in `examples/embed_probe.rs`). That last file
is **untracked and not this packet's** — another session left it in the
submodule worktree, and it also makes plain `cargo test` in `physis-core` fail
to compile. It was not deleted. The packet's gate was therefore narrowed to
`--lib --tests`, which is what this packet can affect and what the sibling
PH-CORE packets already use. Do not read the narrowing as the examples being
green.

## 6. What the oracle still cannot see

- Timing, CPU and peak memory: `delta_engine.rs` has no instrumentation
  (`duration|Duration|elapsed|Instant` = 0 hits). Measuring them needs a
  production field, which is out of scope here.
- `RelationSevered` / `RelationCreated`: `mutated_nodes` only applies
  `EmbeddingShift`, because those two ops change edges rather than node state.
  An edge-mutating full path is a separate arm.
- The wave's `O(affected × n)` `compute_coherence` cost: counted nowhere.
- Spec-level correctness of the neighbour rule (see §3).

## 7. Decision

Deliverable complete, gate green, negative arm proven to fail. Status:
`review`. Follow-up worth a packet: instrument `OntologyDeltaReport` with an
optional `timing_ms` so the O(affected × n) cost becomes measurable, and add
the edge-mutation arms.
