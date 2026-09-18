---
id: MS-004
track: multiscale-structure
phase: 2
status: blocked
rung: 2
depends: [MS-002, MS-003]
gate: "Independent reviewer verifies the acceptance criteria below and reproduction commands in packets/reports/MS-004.md"
claimed_by:
claimed_at:
---

# MS-004 — Distribution fitting and concentration

## Scope and execution
Prepared only; no worker has been launched. Blocked until MS-002, MS-003 reports are reviewed and their required artifacts are usable.
Read PLAN.md Phase 27, AGENTS.md, docs/WHAT_PHYSIS_IS.md and packets/PROTOCOL.md.
Work on pansa via randomesh/SSH. Confirm the exact checkout and Physis store;
the configured MCP may target another checkout. Preserve unrelated dirty work.
Own packets/reports/MS-004.md and research/multiscale/MS-004/ only.
No production changes, worker spawning, merge or deployment. Scope expansion
requires a reviewed amendment. Follow protocol branch/claim rules on execution.

## Context
Consume actual replay measurements from MS-002 and linguistic/relation measurements from MS-003; keep populations and relation types separate. Existing research/criticality/run.py and results.json are invalid drafts, never baselines.

Evidence starting points (verify against current source): research/criticality/ (read-only invalid draft); outputs/reports from MS-002 and MS-003.

## Steps
Use a mature research statistics implementation when available. Compare power law, truncated power law, lognormal, exponential and appropriate stretched exponential on the SAME support and discrete/continuous measure. Report xmin selection, tail count, MLE parameters, KS, likelihood ratios and assumptions, uncertainty, refitted bootstrap diagnostics, censoring and sample adequacy. Produce CDF/CCDF, rank-frequency, Lorenz, size/duration and size-duration plots. Quantify timescales and participation concentration without fixed 80/20 assumptions.

Before analysis, freeze data manifests/revisions, splits, seeds, controls,
metrics and tolerances. Use existing primitives and benchmark helpers before
adding code. Do not consume the invalid research/criticality draft as evidence.

## Checks and acceptance
One reproducible validation command with synthetic positive/negative controls and support-consistency assertions. Record numerical dependency versions and seeds.

Validate fitting against seeded known distributions, light-tailed controls and censored data; no naive log-log regression as primary evidence. Bootstrap must repeat fitting/xmin selection where the claimed test requires it. Insufficient data yields inconclusive, not power-law-like. No criticality verdict in this packet.

Record the exact runnable commands with nonzero data/test denominators.
The reviewer reruns them independently; a report file or exit zero alone
does not pass. Negative/inconclusive findings are valid research outcomes,
not permission to promote. Missing prerequisite data means blocked, not a
substitute experiment. For report-only packets, verify citations and decisions;
do not add artificial implementation tests.

## Handoff
Write packets/reports/MS-004.md with Evidence, Commands, Results, Limitations,
Decision. Include changed paths, revision, actual observations, controls,
failure cases, uncertainty and a clear release/block decision for successors.
Record checked outcome using Physis in the correct workspace. Set status
to review; only an independent reviewer marks done.
