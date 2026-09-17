---
id: MS-001
track: multiscale-structure
phase: 0
status: review
rung: 2
depends: []
gate: "Independent reviewer verifies the acceptance criteria below and reproduction commands in packets/reports/MS-001.md"
claimed_by: muse-spark
claimed_at: 2026-09-17
---

# MS-001 — Archaeology and measurement contract

## Scope and execution
Prepared only; no worker has been launched. First eligible packet in this track.
Read PLAN.md Phase 27, AGENTS.md, docs/WHAT_PHYSIS_IS.md and packets/PROTOCOL.md.
Work on pansa via randomesh/SSH. Confirm the exact checkout and Physis store;
the configured MCP may target another checkout. Preserve unrelated dirty work.
Own packets/reports/MS-001.md only.
No production changes, worker spawning, merge or deployment. Scope expansion
requires a reviewed amendment. Follow protocol branch/claim rules on execution.

## Context
Inspect implementations, external callers, tests and existing experiments for transform/transplant, order/ngram tables, becoming, hypothesis/epistemic, delta_engine, relation, map/structsim, quality, ledger/observe, ontology/discovery, retrieval/pack/nav, adaptive_document, dependency and orchestration. Verify actual deployed versus research-only paths; do not infer reachability from exports.

Evidence starting points (verify against current source): physis-core/src/transform.rs; physis-core/src/delta_engine.rs; research/E91_ARCHAEOLOGY.md; research/E91_CONCEPT_MAP.md.

## Steps
Produce a module/caller/test map, observable-to-source table and frozen measurement contract. Define evaluated versus changed state, causal ancestry, output tolerances, clocks, censoring, missing data, corpus revisions/hashes, split policy and cost accounting. Review existing E91 notes rather than replacing them. Inventory current data and missing instrumentation.

Before analysis, freeze data manifests/revisions, splits, seeds, controls,
metrics and tolerances. Use existing primitives and benchmark helpers before
adding code. Do not consume the invalid research/criticality draft as evidence.

## Checks and acceptance
Report only. No implementation tests; reviewer checks cited source and callers.

Source-cited evidence for every capability; unknowns explicitly NOT MEASURED. Identify an independent full-recomputation oracle and the narrowest replayable update path. If no oracle exists, specify the missing seam and block MS-002. No production edits.

Record the exact runnable commands with nonzero data/test denominators.
The reviewer reruns them independently; a report file or exit zero alone
does not pass. Negative/inconclusive findings are valid research outcomes,
not permission to promote. Missing prerequisite data means blocked, not a
substitute experiment. For report-only packets, verify citations and decisions;
do not add artificial implementation tests.

## Handoff
Write packets/reports/MS-001.md with Evidence, Commands, Results, Limitations,
Decision. Include changed paths, revision, actual observations, controls,
failure cases, uncertainty and a clear release/block decision for successors.
Record checked outcome using Physis in the correct workspace. Set status
to review; only an independent reviewer marks done.
