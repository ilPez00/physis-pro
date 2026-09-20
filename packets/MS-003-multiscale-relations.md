---
id: MS-003
track: multiscale-structure
phase: 1
status: review
rung: 2
depends: [MS-001]
gate: "Independent reviewer verifies the acceptance criteria below and reproduction commands in packets/reports/MS-003.md"
claimed_by: agent/MS-003
claimed_at: 2026-09-20
---

> **Released 2026-09-20** by the MS-001 reviewer pass (MS-001 now `done`).
> Amendment A1 (`packets/reports/MS-001.md` §Verification note 2026-09-20)
> applies to any comparison you build on the delta engine: compare
> order-unspecified collections as sorted sets, floats to epsilon 1e-6.


# MS-003 — Linguistic scales and relation survival

## Scope and execution
Prepared only; no worker has been launched. Blocked until MS-001 reports are reviewed and their required artifacts are usable.
Read PLAN.md Phase 27, AGENTS.md, docs/WHAT_PHYSIS_IS.md and packets/PROTOCOL.md.
Work on pansa via randomesh/SSH. Confirm the exact checkout and Physis store;
the configured MCP may target another checkout. Preserve unrelated dirty work.
Own packets/reports/MS-003.md and research/multiscale/MS-003/ only.
No production changes, worker spawning, merge or deployment. Scope expansion
requires a reviewed amendment. Follow protocol branch/claim rules on execution.

## Context
Reuse symbolic Triple/Predicate/WorldState, structural transforms, existing ordered traces and typed relations. substitution_invariants currently excludes triples mentioning substituted tokens; this is syntactic preservation, not semantic discovery. Prior PH-051 failed representation transfer; inspect its limits before generalizing.

Evidence starting points (verify against current source): physis-core/src/transform.rs; physis-core/src/becoming.rs; physis-core/src/relation.rs; benchmarks/behaviour/; packets/PH-051-representation-covariant-probability-laws.md.

## Steps
Freeze explicit coarse-graining maps at supported token, phrase, sentence, paragraph and document scales; add concept/ontology scales only with traceable mappings. Measure Zipf/Heaps, transitions, recurrence, motifs and dependence versus distance. Preserve document/session boundaries. Separately measure relation-family degree, weight, survival and predictive value. Test renaming/refactor/paraphrase where ground truth exists.

Before analysis, freeze data manifests/revisions, splits, seeds, controls,
metrics and tolerances. Use existing primitives and benchmark helpers before
adding code. Do not consume the invalid research/criticality draft as evidence.

## Checks and acceptance
One runnable check where valid renaming preserves a relation and a meaning-changing transformation breaks it; verify boundaries and shuffled controls.

Hold out documents/projects AND transformations. Compare relation preservation with lexical/frequency/degree baselines and shuffled or mapping-destroyed controls. Use bias controls for mutual information and blocked uncertainty estimates. Preserve CAUSES versus CO-OCCURS provenance. Unsupported scales are NOT MEASURED. Freeze collapse normalization before holdout; descriptive curves alone cannot pass.

Record the exact runnable commands with nonzero data/test denominators.
The reviewer reruns them independently; a report file or exit zero alone
does not pass. Negative/inconclusive findings are valid research outcomes,
not permission to promote. Missing prerequisite data means blocked, not a
substitute experiment. For report-only packets, verify citations and decisions;
do not add artificial implementation tests.

## Handoff
Write packets/reports/MS-003.md with Evidence, Commands, Results, Limitations,
Decision. Include changed paths, revision, actual observations, controls,
failure cases, uncertainty and a clear release/block decision for successors.
Record checked outcome using Physis in the correct workspace. Set status
to review; only an independent reviewer marks done.
