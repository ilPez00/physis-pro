# MS-002 — Replay harness design (research stub)

> Report-only packet. This is a design stub, **not** an executable. The runnable
> test binary it specifies is deferred to a follow-up packet (see
> `packets/reports/MS-002.md` §Decision).

## Goal

Build a paired replay harness that applies identical `OntologyMutation`s to two
independent code paths — a **full** rebuild (re-evaluate the whole graph from
scratch) and an **incremental** path (`evaluate_mutation` shadow frame) — and
asserts they agree within Amendment A1 tolerances.

## Snapshot source

Freeze fixtures from `physis-core/tests/test_delta_engine.rs` (lines 38-495):
the helpers `unit_vector`, `make_node`, `chain_graph`, `single_node_mutation_…`
already construct `Vec<CoherenceNode>` + `Vec<TypedEdge>`. These are the
minimal reproducible snapshots MS-002 needs; no new snapshot format is required.

## Comparison adapter (implements Amendment A1)

```rust
// 1. node_deltas: compare as a SET, sorted by node_id
let mut a: Vec<_> = report_a.affected_nodes.clone();
let mut b: Vec<_> = report_b.affected_nodes.clone();
a.sort_by_key(|d| d.node_id.clone());
b.sort_by_key(|d| d.node_id.clone());
assert_eq!(a.len(), b.len());
for (x, y) in a.iter().zip(b.iter()) {
    assert_eq!(x.node_id, y.node_id);
    assert_eq!(x.delta_type, y.delta_type);
    assert_abs_diff_eq!(x.new_coherence, y.new_coherence, epsilon = 1e-6);
}

// 2. net_coherence_delta: epsilon comparison
assert_abs_diff_eq!(
    report_a.net_coherence_delta,
    report_b.net_coherence_delta,
    epsilon = 1e-6,
);
```

Positional fields (`revision_walk.steps`, `revised_hypotheses`,
`hypothesis_status_shifts`, `adjudications`) are compared positionally — they
are already sorted/deterministic per §Results.

## Perturbation arms (acceptance criterion #2)

| Arm | Mutation shape | Oracle expectation |
|-----|----------------|--------------------|
| no-op | `EmbeddingShift` old==new | empty report (A7 invariant, line 695) |
| local | single node shift, no neighbors | 1 `NodeDelta`, 0 transitions |
| fanout | node tied via `cell_pin` | ≥2 affected nodes, A1 set-compare |
| cycle | DependsOn edges form a loop | walk records `cycles`, terminates |
| disconnected | mutation targets orphan node | `revision_walk.fallback_breadth_used` may engage |
| corrupted-incremental (negative) | incremental path skips a dependent | oracle rejects (mismatch on revisions) |

## Deferred

- The companion test binary `physis-core/tests/replay_contract.rs` that
  actually runs the above. Blocked on a new packet so the binary lands with an
  independent reviewer rerun path (PROTOCOL §5 gate).
- Duration/CPU/peak-memory capture: `delta_engine.rs` has zero instrumentation
  (grep `duration|Duration|elapsed|Instant` = 0 hits). A metrics packet would
  add an optional `timing_ms` field to `OntologyDeltaReport`; out of scope here.
