---
id: MS-005
track: multiscale-structure
phase: 2
status: blocked
rung: 2
depends: [MS-002, MS-003]
gate: "Independent reviewer verifies the acceptance criteria below and reproduction commands in packets/reports/MS-005.md"
claimed_by:
claimed_at:
---

# MS-005 — Invariant barriers and sparse retrieval

## Scope and execution
Prepared only; no worker has been launched. Blocked until MS-002, MS-003 reports are reviewed and their required artifacts are usable.
Read PLAN.md Phase 27, AGENTS.md, docs/WHAT_PHYSIS_IS.md and packets/PROTOCOL.md.
Work on pansa via randomesh/SSH. Confirm the exact checkout and Physis store;
the configured MCP may target another checkout. Preserve unrelated dirty work.
Own packets/reports/MS-005.md and research/multiscale/MS-005/ only.
No production changes, worker spawning, merge or deployment. Scope expansion
requires a reviewed amendment. Follow protocol branch/claim rules on execution.

## Context
Reuse existing retrieval, dependency and transform paths. Frequency and stability do not prove safe stopping. A barrier is valid only when every downstream consumer depends exclusively on the preserved interface/properties.

Evidence starting points (verify against current source): src/rag.rs; src/pack.rs; src/nav/; physis-core/src/delta_engine.rs; benchmarks/behaviour/e67.py.

## Steps
Compare full, local, top-importance, invariant-plus-delta and invariant-plus-delta-plus-exploration arms. Select candidates on training history only. Compare raw/z/robust-z/rank/ECDF/RRF fusion, calibrating on training data. Sweep mass budgets, account for estimating total mass, retain cold-tail access. Test historical propagation priors and cross-context survival; measure prediction, retrieval quality, changed-output recall and total compute.

Before analysis, freeze data manifests/revisions, splits, seeds, controls,
metrics and tolerances. Use existing primitives and benchmark helpers before
adding code. Do not consume the invalid research/criticality draft as evidence.

## Checks and acceptance
One runnable adversarial barrier check plus held-out paired benchmark; learned priors cannot omit correctness-required dependencies.

Use the independent full oracle from MS-002. Report quality versus evaluated fraction and measured runtime with uncertainty. Include rare/new/contradictory changes and a case where a preserved property hides another changed dependency. No false negatives for exact barrier claims. Approximate policies need a declared error budget and fallback. Stop if overhead erases gains.

Record the exact runnable commands with nonzero data/test denominators.
The reviewer reruns them independently; a report file or exit zero alone
does not pass. Negative/inconclusive findings are valid research outcomes,
not permission to promote. Missing prerequisite data means blocked, not a
substitute experiment. For report-only packets, verify citations and decisions;
do not add artificial implementation tests.

## Handoff
Write packets/reports/MS-005.md with Evidence, Commands, Results, Limitations,
Decision. Include changed paths, revision, actual observations, controls,
failure cases, uncertainty and a clear release/block decision for successors.
Record checked outcome using Physis in the correct workspace. Set status
to review; only an independent reviewer marks done.
