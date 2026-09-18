---
id: MS-008
track: multiscale-structure
phase: 5
status: blocked
rung: 2
depends: [MS-005, MS-006, MS-007]
gate: "Independent reviewer verifies the acceptance criteria below and reproduction commands in packets/reports/MS-008.md"
claimed_by:
claimed_at:
---

# MS-008 — Review evidence before minimal production promotion

## Scope and execution
Prepared only; no worker has been launched. Blocked until MS-005, MS-006, MS-007 reports are reviewed and their required artifacts are usable.
Read PLAN.md Phase 27, AGENTS.md, docs/WHAT_PHYSIS_IS.md and packets/PROTOCOL.md.
Work on pansa via randomesh/SSH. Confirm the exact checkout and Physis store;
the configured MCP may target another checkout. Preserve unrelated dirty work.
Own packets/reports/MS-008.md only.
No production changes, worker spawning, merge or deployment. Scope expansion
requires a reviewed amendment. Follow protocol branch/claim rules on execution.

## Context
This packet is conditional review, not permission for a rewrite. Sparse computation can be valuable without criticality. Routing and self-organization are independent optional branches.

Evidence starting points (verify against current source): MS-001 through MS-007 reports; PLAN.md Phase 27.

## Steps
Review all reports and select at most one smallest mechanism with measured benefit. Record its validity domain, correctness contract, maintenance cost, fallback and rollback. If MS-005 passes before later research concludes, a reviewer may propose an earlier promotion by explicitly revising this dependency list; never silently bypass it. Any adaptive-threshold experiment must be separately bounded, logged, off by default and compared with a static control.

Before analysis, freeze data manifests/revisions, splits, seeds, controls,
metrics and tolerances. Use existing primitives and benchmark helpers before
adding code. Do not consume the invalid research/criticality draft as evidence.

## Checks and acceptance
No implementation in this packet. Reviewer validates evidence, scope and rollback before proposing the focused production regression check.

Deliver a concrete promotion proposal first. Production implementation requires a separately authorized bounded packet. No positive evidence means no production change. Require held-out benefit after maintenance overhead and independent correctness review; criticality must not become an optimization target by default.

Record the exact runnable commands with nonzero data/test denominators.
The reviewer reruns them independently; a report file or exit zero alone
does not pass. Negative/inconclusive findings are valid research outcomes,
not permission to promote. Missing prerequisite data means blocked, not a
substitute experiment. For report-only packets, verify citations and decisions;
do not add artificial implementation tests.

## Handoff
Write packets/reports/MS-008.md with Evidence, Commands, Results, Limitations,
Decision. Include changed paths, revision, actual observations, controls,
failure cases, uncertainty and a clear release/block decision for successors.
Record checked outcome using Physis in the correct workspace. Set status
to review; only an independent reviewer marks done.
