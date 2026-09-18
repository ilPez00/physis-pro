---
id: MS-007
track: multiscale-structure
phase: 4
status: blocked
rung: 2
depends: [MS-005, MS-006]
gate: "Independent reviewer verifies the acceptance criteria below and reproduction commands in packets/reports/MS-007.md"
claimed_by:
claimed_at:
---

# MS-007 — Outcome-grounded compute and model routing

## Scope and execution
Prepared only; no worker has been launched. Blocked until MS-005, MS-006 reports are reviewed and their required artifacts are usable.
Read PLAN.md Phase 27, AGENTS.md, docs/WHAT_PHYSIS_IS.md and packets/PROTOCOL.md.
Work on pansa via randomesh/SSH. Confirm the exact checkout and Physis store;
the configured MCP may target another checkout. Preserve unrelated dirty work.
Own packets/reports/MS-007.md and research/multiscale/MS-007/ only.
No production changes, worker spawning, merge or deployment. Scope expansion
requires a reviewed amendment. Follow protocol branch/claim rules on execution.

## Context
Existing orchestration code contains placeholder workers/signals; verify live model execution before benchmarking. Next-action diversity is not task success or avoided cost.

Evidence starting points (verify against current source): src/orchestration/mod.rs; clients/opencode-orchestration/; docs/plans/2026-09-14-agent-delegation.md.

## Steps
On frozen tasks compare strong everywhere, cheap everywhere, static roles, impact routing, impact-plus-avalanche and invariant-aware routing. Preserve equal context/tool budgets and test criteria. Calibrate solely on training tasks; report success, failures, rework, corrections, escalations, tokens, cost and wall time. Treat criticality as an optional ablation only if MS-006 supports a usable indicator.

Before analysis, freeze data manifests/revisions, splits, seeds, controls,
metrics and tolerances. Use existing primitives and benchmark helpers before
adding code. Do not consume the invalid research/criticality draft as evidence.

## Checks and acceptance
One routing check with a cheap-worker failure requiring escalation; independent task acceptance must reject a worker's false success.

Requires executable workers, independent task tests and recorded provider/model provenance. Never simulate successful repair with placeholder text. No paid/remote calls without existing explicit authorization. If outcome labels, budget or workers are unavailable, report blocked; do not invent proxy economics.

Record the exact runnable commands with nonzero data/test denominators.
The reviewer reruns them independently; a report file or exit zero alone
does not pass. Negative/inconclusive findings are valid research outcomes,
not permission to promote. Missing prerequisite data means blocked, not a
substitute experiment. For report-only packets, verify citations and decisions;
do not add artificial implementation tests.

## Handoff
Write packets/reports/MS-007.md with Evidence, Commands, Results, Limitations,
Decision. Include changed paths, revision, actual observations, controls,
failure cases, uncertainty and a clear release/block decision for successors.
Record checked outcome using Physis in the correct workspace. Set status
to review; only an independent reviewer marks done.
