---
id: MS-006
track: multiscale-structure
phase: 3
status: blocked
rung: 2
depends: [MS-004]
gate: "Independent reviewer verifies the acceptance criteria below and reproduction commands in packets/reports/MS-006.md"
claimed_by:
claimed_at:
---

# MS-006 — Criticality, finite-size scaling and recurrence

## Scope and execution
Prepared only; no worker has been launched. Blocked until MS-004 reports are reviewed and their required artifacts are usable.
Read PLAN.md Phase 27, AGENTS.md, docs/WHAT_PHYSIS_IS.md and packets/PROTOCOL.md.
Work on pansa via randomesh/SSH. Confirm the exact checkout and Physis store;
the configured MCP may target another checkout. Preserve unrelated dirty work.
Own packets/reports/MS-006.md and research/multiscale/MS-006/ only.
No production changes, worker spawning, merge or deployment. Scope expansion
requires a reviewed amendment. Follow protocol branch/claim rules on execution.

## Context
Criticality is not implied by a heavy tail. Engine attenuation, thresholds and hop/node caps can manufacture cutoffs. Wall time and propagation generations are distinct duration definitions.

Evidence starting points (verify against current source): MS-002 trace schema; MS-004 distributions; examples/empedocles_criticality.rs; PAPER_1_Empedocles_Criticality.md (locate actual paths).

## Steps
Preregister an engine-specific control parameter and order parameter. Test dying versus sustained propagation, threshold/separation robustness, comparable system sizes, out-of-sample collapse goodness, and size-duration relations only under stated assumptions. Estimate branching only with meaningful parent/child ancestry and deduplicated descendants. Assess recurrent states with an explicit distance and temporal null; similar snapshots alone are not attractors.

Before analysis, freeze data manifests/revisions, splits, seeds, controls,
metrics and tolerances. Use existing primitives and benchmark helpers before
adding code. Do not consume the invalid research/criticality draft as evidence.

## Checks and acceptance
One reproducible control suite where independent light-tailed and imposed-cutoff processes do not automatically receive a criticality verdict.

Compare against degree-preserving/topology and temporal controls; report censoring and boundary effects. State whether evidence supports merely concentration, heavy tails, critical-like behavior or self-organization. If equivalent sizes or causal generations are unavailable, mark those tests NOT MEASURED and make no criticality claim. Negative results must not block independently useful sparse computation.

Record the exact runnable commands with nonzero data/test denominators.
The reviewer reruns them independently; a report file or exit zero alone
does not pass. Negative/inconclusive findings are valid research outcomes,
not permission to promote. Missing prerequisite data means blocked, not a
substitute experiment. For report-only packets, verify citations and decisions;
do not add artificial implementation tests.

## Handoff
Write packets/reports/MS-006.md with Evidence, Commands, Results, Limitations,
Decision. Include changed paths, revision, actual observations, controls,
failure cases, uncertainty and a clear release/block decision for successors.
Record checked outcome using Physis in the correct workspace. Set status
to review; only an independent reviewer marks done.
