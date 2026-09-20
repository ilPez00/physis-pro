---
id: MS-002
track: multiscale-structure
phase: 1
status: review
rung: 2
depends: [MS-001]
gate: "Independent reviewer verifies the acceptance criteria below and reproduction commands in packets/reports/MS-002.md"
claimed_by: agent/MS-002
claimed_at: 2026-09-20
---

> **Released 2026-09-20** by the MS-001 reviewer pass (MS-001 now `done`).
> **Read amendment A1 first** (`packets/reports/MS-001.md` §Verification note
> 2026-09-20): the oracle's "deterministic" attribute is false for ORDER.
> `find_neighbors` (`physis-core/src/delta_engine.rs:252-298`) returns
> HashSet-iteration order with no sort, so `OntologyDeltaReport.node_deltas`
> order is unspecified and `net_coherence_delta` is order-dependent float
> summation. Compare `node_deltas` as a set sorted by `node_id`, and
> `net_coherence_delta` to epsilon 1e-6 — never as a positional vector and
> never bit-exact. Sorting the wave is a core source change and belongs in
> its own packet, not here.

# MS-002 — Actual mutation replay and incremental correctness

## Scope and execution
Prepared only; no worker has been launched. Blocked until MS-001 reports are reviewed and their required artifacts are usable.
Read PLAN.md Phase 27, AGENTS.md, docs/WHAT_PHYSIS_IS.md and packets/PROTOCOL.md.
Work on pansa via randomesh/SSH. Confirm the exact checkout and Physis store;
the configured MCP may target another checkout. Preserve unrelated dirty work.
Own packets/reports/MS-002.md and research/multiscale/MS-002/ only.
No production changes, worker spawning, merge or deployment. Scope expansion
requires a reviewed amendment. Follow protocol branch/claim rules on execution.

## Context
Reuse the update path and independent oracle selected by MS-001. Existing shadow delta evaluation and reverse DependsOn walks are starting points, not evidence of material propagation. compute_coherence scans the effective graph; include this hidden global work.

Evidence starting points (verify against current source): physis-core/src/delta_engine.rs; physis-core/tests/test_delta_engine.rs; physis-core/tests/epistemic_p1.rs; src/adaptive_document/.

## Steps
Build a research-only paired replay harness from frozen snapshots. Apply identical edits to independent full and incremental paths. Record changed nodes/edges/outputs separately from inspected nodes/edges, ancestry, generation depth/breadth, duration, CPU, peak memory, invariant violations and censored events. Sweep perturbation sizes and retain zero-change cases separately.

Before analysis, freeze data manifests/revisions, splits, seeds, controls,
metrics and tolerances. Use existing primitives and benchmark helpers before
adding code. Do not consume the invalid research/criticality draft as evidence.

## Checks and acceptance
One runnable check with no-op, local, fanout, cycle and disconnected mutations; include a corrupted incremental arm that the oracle rejects.

Exact agreement for discrete outputs and preregistered tolerances for floating outputs on held-out mutations. Measure equivalent completed work, including indexing/cache maintenance. A deliberate missed dependency must fail the oracle. No O(|A|) or speedup claim from closure counts. Stop on mismatch before performance promotion.

Record the exact runnable commands with nonzero data/test denominators.
The reviewer reruns them independently; a report file or exit zero alone
does not pass. Negative/inconclusive findings are valid research outcomes,
not permission to promote. Missing prerequisite data means blocked, not a
substitute experiment. For report-only packets, verify citations and decisions;
do not add artificial implementation tests.

## Handoff
Write packets/reports/MS-002.md with Evidence, Commands, Results, Limitations,
Decision. Include changed paths, revision, actual observations, controls,
failure cases, uncertainty and a clear release/block decision for successors.
Record checked outcome using Physis in the correct workspace. Set status
to review; only an independent reviewer marks done.
