# Brainstorm — Atlas, NATMS, TMS, ATMS, Graphiti → Physis (2026-09-09)

> Companion to `ATLAS_GRAPHITI_INTEGRATION_PLAN.md` (which covers Atlas A1–A7
> and Graphiti G1–G7 in normative detail). This document adds what that plan
> does not cover: the **TMS family** (`~/dev/ATMS-in-Python`, `~/dev/NATMS`,
> `~/dev/Truth-Maintenance-System`), the Atlas/Graphiti corners the A/G lists
> skipped (trust quarantine/promotion, communities, graph-distance search),
> and the cross-combinations between all of them. Discharges the PLAN.md todo
> "investigate tmg/ntmg/graphiti/atlas as codebases in /home/gio/dev".

## 0. Principle (unchanged)

Patterns, protocols, test shapes — never dependencies. All five codebases stay
untouched and are read as references. Everything below is stated as an
observable behavior of a named physis-core symbol plus a cargo test name; if
it cannot be, it is a study note, not a take. §2b applies throughout (coverage
filters and ranks, never verifies), and 22.5's honesty constraints carry
(cosine 0.519 / propose top-3 0.712 stand).

## 1. Inventory — what each codebase actually is (verified 2026-09-09)

| Codebase | Size / shape | What it genuinely offers |
|---|---|---|
| **Atlas** v0.1.0-alpha.1 (Python) | `atlas_core/ripple/` (engine, analyze_impact, adjudication, reassess, contradiction), `revision/agm.py` + `benchmarks/agm_compliance/` (49 scenarios), `trust/` (hash-chained `ledger.py`, **`promotion_policy.py`, `quarantine.py`, `sanitization.py`**), obsidian-plugin adjudication queue | A1–A7 cover ripple/AGM/ledger/adjudication. **Not yet covered: the trust pipeline — ingest lands quarantined and is promoted to the belief graph only by a stated policy.** |
| **Graphiti** v0.30.2 (Python) | `edges.py` (tri-temporal), `utils/maintenance/` (edge ops, dedupe, **`community_operations.py`**), `search/` (node/edge/community scopes, hybrid config, MMR) | G1–G7 cover temporal legs, ingest triple, watermark, hybrid BM25+RRF. **Not yet covered: community build/summaries as a search scope; graph-distance as a retrieval leg.** |
| **ATMS-in-Python** (MIT, 227 lines, author-flagged incomplete) | one file: environments as frozensets, `NoGood = '⊥'`, Horn-clause rules, PROPAGATE/WEAVE/UPDATE | the **label algorithm** in its smallest readable form; encodes the **Nixon Diamond** and a self-support loop as worked examples. Hyperresolution caveat already excluded (N4). |
| **NATMS** (MIT, de Kleer 1988 general labeling) | `ATMS.py` (PROPAGATE/WEAVE/UPDATE/NOGOOD) + `NATMS.py` (negation subclass) | **negation as first-class in justification antecedents**: `¬A` may appear directly; `choose{A,B,C}` is expressible as `¬A,¬B,¬C` without encoding three implications; author's label-completeness refinement treats denied assumptions as environment members. |
| **Truth-Maintenance-System** (JTMS, NCSU coursework, 174 lines) | `TMS.py`: add_rule / assess_rules / delete_literals + **recursive delete_rules** | the **retraction cascade**: retract a literal and statuses are re-derived from the remaining justifications, recursively. Simple, but it is exactly the shape Physis's evidence-retraction story needs. |

## 2. New takes (T-series) — each with symbol, behavior, gate

- **T1 — assumption-set labels (ATMS environments).** `hypothesis.rs`:
  a Hypothesis may carry the environments (assumption sets) under which it
  currently holds, beside the single `HypothesisStatus` — alternative worlds
  without a second status enum. Physis already keeps competing hypotheses
  alive (multi-membership: Iteration 8 / E10's Gap 2; E29's ranked cell
  scores at the scoring level); ATMS supplies the formal shape: labels are
  sets of *minimal* environments, NoGood environments excluded.
  Gate: `hypothesis_reports_minimal_environments` (minimality discipline —
  de Kleer's, not ours; a superset environment must never be reported when a
  subset is present) + a stated label-size cap, never silent.
- **T2 — negation-aware justification parties (NATMS).**
  `contradiction.rs` (`ContradictionParty`): a party carries its polarity, so
  "h under A" and "h′ under ¬A" coexist as **alternative worlds**, not as a
  defect to erase — `choose{A,B,C}` families map to competing hypothesis
  sets. This is the structural answer to "why isn't this just paraconsistent
  KR?": contradiction is kept as a temporally and empirically situated
  relation between competing, environment-indexed hypotheses (23.0's
  non-claim unchanged — logic gives the shape, Physis keeps the *state*).
  Gate: `negated_justifications_coexist_without_collapse` — both parties
  queryable, the contradiction stays Open, no ⊥ ever enters a current view.
- **T3 — retraction cascade (JTMS recursive delete_rules).**
  `hypothesis.rs` (`remove_evidence`) + `delta_engine.rs`: retracting an
  evidence item recomputes the holder and its DependsOn dependents (A2 walk)
  and **re-derives status from the remaining justification** — a hypothesis
- **T4 — the Nixon Diamond as a seeded benchmark scenario.**
  `ATMS-in-Python` encodes it: q, r are premises; p, np are derivable under
  assumptions; {p, np} = ⊥. ATMS answers with environments; monotonic RAG
  answers with whichever passage ranks higher. **Physis's answer is the
  showcase**: both hypotheses live (Supported-in-context), the contradiction
  stays Open with provenance, temporal windows mark which side each source
  endorses, and the human adjudicates. Seed it into the 23.5 benchmark as a
  named scenario with a per-arm expectation table.
  Gate: `nixon_diamond_retains_both_sides` on the Physis arm; the ATMS arm
  must reproduce its environments on the same rules (cross-check).
- **T5 — quarantine → promotion (Atlas `trust/`).** Ingest path: unverified
  intake (unknown source, contradiction with a Certified belief, low
  corroboration) lands **quarantined** — present in the store, excluded from
  classification/proposal surfaces — and is promoted by a *deterministic*
  policy (corroboration count, authority match, temporal consistency).
  Distinct from G2 (which resolves *which existing belief an ingest
  contradicts*): T5 gates whether the ingest participates at all yet.
  Gate: `quarantined_ingest_absent_until_promoted`; promotion is a logged
  event; the policy is named constants (A6 discipline), no LLM judgment (N2).
- **T6 — graph-distance as a third RRF leg (Graphiti hybrid search).**
  `rag.rs` / `propose.rs`: beside cosine and BM25 (G5), a bounded boost for
  hits that are ≤2-hop DependsOn neighbors of already-selected hits —
  structural proximity as a *ranking* signal, in-process, offline.
  Gate: `graph_distance_leg_vs_g5_baseline` on the same frozen probe set as
  G5; a negative ships disabled with the negative recorded (G5's rule).
  Ranking only — never a soundness claim (§2b).
- **T7 — community rollups derived by construction (Graphiti
  `community_operations.py` ↔ physis `studio_communities` rollup).**
  Graphiti builds communities and summaries, then searches them as a scope.
  Physis already has the rollup (`studio_communities.rs`); the take is the
  discipline: a community summary must be **derived from its members by
  construction** (the interface-mimics-nature rule), searchable as a scope.
  Graphiti's LLM-driven community labeling is NOT taken (N2).
  Gate: `community_summary_derives_from_members` — a summary diverging from
  its members is a defect the test catches.

## 3. Cross-combinations (where the real brainstorm value sits)


## 4. Explicitly NOT taking

- **Full ATMS label semantics as the runtime default** — environment
  enumeration is exponential in the wrong graph; labels are capped, minimal,
  and per-hypothesis, or they are not shipped at all.
- **Hyperresolution / author-flagged inference code** (ATMS-in-Python
  hyperresolution mutating nogood labels; NATMS's NOGOOD deviation is
  *documented* by its author and cited as such, but is still a deviation) —
  study, never port. (Extends N4.)
- **LLM community labeling, LLM quarantine judgment, LLM rerankers** — N2
  unchanged across all five codebases.
- **A "worlds" second status enum or a belief-revision proof claim** —
  environments annotate; they do not replace `HypothesisStatus` (A3's gate
  stays the only revision-correctness surface), and "ATMS-compatible" is
  never claimed beyond the Nixon cross-check.
- **Python runtime, graph DB, phone-home** — §0 unchanged.

## 5. Effect on the plan

- 23.5's arm list becomes concrete: the **TMS/ATMS arm** is the label
  algorithm run on the benchmark's rules (ATMS-in-Python/NATMS semantics,
  re-implemented in the harness, not imported), with T4 as the named
  cross-check scenario.
- Order slots: T3 after G3 (P2); T5 beside G2 (P1/P2 boundary); T6 beside
  G5 (P2); T7 beside the studio rollup (whenever studio surfaces change);
  T1/T2 are new P3 items alongside 23.1–23.3 (they want the G3/G6 vocabulary
  too); T4 is built first regardless of order — it is one seeded test.
- `ATLAS_GRAPHITI_INTEGRATION_PLAN.md` stays normative for A/G items; this
  document is normative for T items. PLAN.md Phase 22.0/23 carry the pointer.

## 6. Open questions (each with a probe)

- Q1. Do environment labels earn their complexity at current corpus scale?
  Probe: T1 behind a flag on the 658-entry mapper corpus; count hypotheses
  with >1 minimal environment. If ≈0, the take is a study note.
- Q2. Does T3's retraction cascade beat simple count-decrement on recovery
  after false evidence? Probe: seeded chains, cascade vs decrement, A5/G7
  falsifier metric; ship the winner, keep the loser as a named flag.
- Q3. Can quarantine promotion stay deterministic without starving the graph?
  Probe: T5 on the ingest replay corpus; measure time-to-promotion and
  false-quarantine rate; a starving policy is a defect, not a virtue.
- Q4. Does the graph-distance leg add anything G5's fusion misses? Probe:
  T6 vs G5 baseline on the frozen probe set before any propose surface
  changes; negative = recorded finding.
- Q5. Is the Nixon Diamond still interesting at scale, or is it a toy?
  Probe: 3 scaled variants (10/50/200 competing pairs) through the same
  arms; if Physis's advantage decays with scale, say so in the paper.

- **T1 + T2 = alternative worlds with negation.** Environment labels
  (ATMS) × negated justifications (NATMS) give Physis a *state-level*
  paraconsistency: worlds where A and ¬A each support different hypothesis
  families, queryable per world — without adopting a paraconsistent *logic*.
  This is the precise answer to the resources file's reviewer question.
- **T3 + G3 = two mirrors.** G3 replays out-of-order *arrivals* to the same
  state; T3 replays a *retraction* to the state without the evidence. One
  trail, two directions; ship together or not at all.
- **T4 + 23.5 = the demo.** The Nixon Diamond is the smallest scenario where
  monotonic RAG visibly fails, ATMS visibly answers (environments), and
  Physis's answer is a *third thing* (retained contradiction + temporal
  validity + human routing). Build it first; it calibrates every arm.
- **T5 + A5 = one routing table.** Quarantine promotion and adjudication
  routes are the same decision shape (AUTO_APPLY / STRATEGIC_REVIEW /
  CORE_PROTECTED, Atlas `CONFIDENCE_DELTA_STRATEGIC_FLOOR = 0.15`); Certified
  beliefs are core-protected — never auto-invalidated by an ingest (G2's
  deterministic judgment proposes, A5 resolves).

  whose only support was retracted falls back toward Candidate/Inert, with
  every step on the trail. The JTMS file is 174 lines; the *shape* is the
  take, not the code. Needs G3 (ordered replay) to be honest.
  Gate: `evidence_retraction_replays_to_state_without_it` — the same
  falsifier 22.5 already assigns A5/G7 ("recovery after false evidence"),
  now with a concrete mechanism.


## 7. Second pass — Atlas subsystems the A-list never touched (2026-09-09)

- **T8 — decision lineage + foundation-weakening alerts**
  (`lineage/walker.py`, `lineage/contradiction.py`). Walk a *decision* back
  through its SUPPORTS chain (max depth 5) to answer "why did we decide X?"
  and — the part worth taking — **flag the decision when a supporting
  belief's standing weakens**. Physis mapping: `explanation.rs`
  (`provenance_chain`) already surfaces the chain; the new behavior is the
  alert: an A2 walk *inverted* (justification edges, upward) whose report
  flags downstream decisions whose foundation dropped.
  Gate: `weakened_foundation_flags_dependent_decision` — degrading a chain
  member flags the decision; a naive implementation that only checks the
  mutated node fails it.
- **T9 — alias registry, tier-1 entity resolution**
  (`resolution/aliases.py`, `fuzzy.py`; `llm_fallback.py` NOT taken). Surface
  forms ("Sarah", "@sarah") → canonical node id, resolved by a human-edited
  YAML first, fuzzy match second, never an LLM. Physis ingest has no identity
  resolution — every surface form risks a duplicate node, which then dilutes
  the DependsOn graph T3/A2 walk on.
  Gate: `alias_resolution_precedes_fuzzy_never_merges` — aliases merge
  deterministically; fuzzy matches *propose* merges, G2 discipline.
- **T10 — per-stream confidence floors**
  (`ingestion/confidence.py`: `STREAM_CONFIDENCE_FLOORS`). Noisy streams
  (OCR) start at 0.30, deliberate edits at 0.70 — starting confidence is a
  property of the *source*, named constants. Physis mapping: `DocKind`
  (Invoice/Email/VoiceTranscript/Report/Unknown) gets a floor per kind,
  feeding `Evidence.confidence` at intake and T5's quarantine threshold.
  Gate: `stream_floor_lowers_initial_confidence` + A6 discipline (frozen
  named constants, tuning as scored configs).
- **T11 — authority as a registry, not a field**
  (`people/registry.py`, tiered `people.yaml`). A5 says high-authority
  parties never auto-resolve; Atlas makes authority a *registry* with tiers
  that `route_proposal` consults. Physis mapping: `contradiction.rs` parties
  gain an authority tier from a config registry; CORE_PROTECTED semantics for
  Certified beliefs ride the same registry.
  Gate: `authority_tier_blocks_auto_resolve` — a Contradicted belief with a
  tier-1 counterparty stays Open regardless of Δ.
- **T12 — federated adjudication**
  (`multi_tenant/federated.py`). When one tenant's assertion contradicts
  another tenant's belief, the adjudication entry routes to **both** queues;
  resolution needs both to agree, or an explicit override with audit.
  Physis already scopes tenants (`X-Physis-User`) — which makes
  cross-tenant contradictions currently *invisible*. This take is the
  exception path, not shared state: tenants stay isolated, only the
  contradiction entry is federated.
  Gate: `cross_tenant_contradiction_routes_to_both_queues`; no belief
  content crosses the boundary, ever.
- **T13 — activity stream derived from the trail**
  (`api/events.py`: `EventBroadcaster`, replay-on-connect buffer of the last
  200 events). Ripple cascades, adjudication changes, ledger SUPERSEDE
  events are *pushed* to subscribers (Obsidian plugin, live visualization);

## 8. Second pass — Graphiti beyond the temporal legs (2026-09-09)

- **T16 — group-id namespaces + fan-out** (`namespaces/` +
  `decorators.py: handle_multiple_group_ids`). Every operation is
  group-scoped by default; one call fans deterministically across group ids
  where a caller explicitly asks. Physis mapping: physis-core is
  tenant-agnostic today (scoping lives in physis-pro handlers); the take is
  **operation-level scoping inside the crate** — tenant id on the sled key
  prefix, with an explicit fan-out used by T12's federated contradiction
  routing. Default-deny reads across tenants; fan-out is the only door.
  Gate: `operations_are_group_scoped_by_default` — a query without an
  explicit scope answers from one group only; the fan-out enumerates what it
  touched.
- **T17 — driver operation-trait layering** (`driver/driver.py`,
  `query_executor.py`, per-store operation interfaces). Graphiti separates
  *what* graph operations exist from *how* a store executes them. NOT taken:
  pluggable backends (N1 stands — sled-only by contract). Taken: the
  **internal trait boundary** — store operations behind one trait so tests
  run against an in-memory double and the sled implementation stays swappable
  without touching call sites. This is the testability pattern, not a
  portability bet.
  Gate: `store_ops_trait_serves_in_memory_double` — the existing epistemic
  gate tests run unchanged against the in-memory double, same results.
- **T18 — the two-clock watermark, on the summary object**
  (`nodes.py: SagaNode`). Graphiti's saga summaries carry **both** clocks:
  `last_summarized_at` (wall clock / transaction time) and
  `last_summarized_episode_valid_at` (episode reference time / valid time).
  This is G1's backfill-vs-arrival distinction, already living on the object
  that needs it — the summary watermark. Physis mapping: G3's per-stream
  high-water mark carries both clocks from day one, and the existing gate
  gains the assertion.
  Gate: extend `late_evidence_replays_to_same_state` — a backfilled episode
  older than the watermark replays to the same state while *both* recorded
  clocks advance correctly. Cost: S, folded into G3.

## 9. Inspection — opencode-orchestrator (2026-09-09)

What it is: a mature multi-agent mission control for OpenCode (TS + Rust;
Commander/Planner/Worker/Reviewer, verification gates before concluding,
session recovery, task queues, 119 test files / 1109 tests, strict coverage
ratchets). Read as a peer system, not a dependency source. Four things:

- **O1 — the honest headline: it *retired* its own hybrid-RAG memory.**
  ADR-0019 decommissioned the Knowledge RAG subsystem — 14 modules including
  `hybrid-search.ts`, `memory-scoring.ts`, `memory-promotion.ts`,
  `retrieval-weights.ts`, and the **Ebbinghaus decay model** — ~2,000 lines
  deleted after a successful migration to markdown-native, self-contained
  mission memory (`scratchpad.md`, `knowledge-map.canvas`, episodic notes).
  Reading: the parts of that architecture that did not survive contact are
  precisely the ones Physis is most suspicious of (retrieval weights as
  context curation, decay curves as relevance, scoring as verification).
  **Do not import what the source project itself deleted.** This is external
  evidence for §2b's posture, worth citing in the paper's related-work.
- **O2 — ADR supersession chains** (`docs/adr/README.md`, statuses
  Promoted/Superseded, decision records 0001–0020). Documentation-level
  invalidate-don't-delete: superseded ADRs stay readable with forward
  pointers; the index is the query surface. Take for physis-pro: adopt ADRs
  for standing architectural decisions (0001 no-LLM-in-verdicts, 0002
  sled-only, 0003 patterns-not-dependencies would formalize what AGENTS.md
  and the plans state in prose). Gate: `superseded_adrs_stay_readable` —
  trivially, by convention.
- **O3 — structured session handoff** (`AGENT_MEMORY.md`: current task /
  last completed step / next exact step / key decisions / known risks /
  files to open first). The shape works — this session and the research
  track already approximate it via `RESEARCH_STATUS.md`. Take: adopt the
  shape explicitly for physis-pro/physis-core handoffs (a fixed template,
  updated at session end), not a new tool.
- **O4 — verification gates before task conclusion + risk-graded coverage
  ratchets** (`docs/adr/0020`, thresholds 85/72/88/85 in CI). Same family as
  A5 (human/verification gates before state change) and A3's
  mutation-testing discipline. Study note: a coverage ratchet for
  physis-core CI is cheap and in character; the agent-council roles are NOT
  a take (physis's dream loop already has its own loop shape, and adding
  LLM roles would violate N2).

## 10. Effect on the plan (wave 2)

- Order slots: T8 after A1/A2 (walk inversion); T9/T10 beside G2; T11 with
  A5; T12/T16 together (both are tenant work, P2); T13 whenever the web/MCP
  surface is touched next (it is presentation-layer over existing events);
  T14 with the physis-pro ingest path; T15 with 23.4's replay hook; T17
  opportunistic refactor behind the existing gates; T18 folded into G3.
- The 23.5 arm list gains nothing new — but the *paper* gains two citations
  from O1: an independent system that converged on, then retracted,
  decay-scored retrieval memory, and the ADR/agent-memory pattern as
  operational evidence that handoff state wants to be small and honest.

  new subscribers replay the last N. Physis mapping: the web UI and MCP
  surface already sit on `epistemic.rs` events — the take is the **transport
  discipline**: the stream is the trail being written, 1:1, with a replay
  buffer; it is not a second reporting state assembled alongside it
  (interface-mimics-nature rule).
  Gate: `sse_stream_is_the_trail_with_replay` — every stream event carries
  the epistemic event id; a subscriber reconnect replays exactly the last N
  trail events.
- **T14 — bounded ingest cycle + health rows**
  (`daemon/cycle.py`, `daemon/health.py`). Ingest runs as one bounded cycle
  (wire extractors → orchestrate → write a health row → exit, exit code
  0/≠0); the scheduler re-fires. Crash-isolated by construction; the health
  row lands on failure too. Physis's interest daemon is long-running; the
  take is the *cycle shape* for the batch ingest path (physis-pro ingest
  currently runs per-invocation with no health record).
  Gate: `ingest_cycle_writes_health_row_on_failure`.
- **T15 — working memory with summarization pressure**
  (`working/blocks.py`: `MemoryBlock.estimated_tokens`, `utilization()`,
  `needs_summarization()` + `auto_summarizer.py`). A working set with an
  explicit token capacity and a pressure signal when summarization is due.
  Physis mapping: the dream loop gets a trigger — consolidation runs when
  the propose/working context exceeds capacity, replaying Inert/Failed
  structures (23.4's replay hook) into summaries derived from members.
  Gate: `working_set_pressure_fires_consolidation_at_capacity`; summaries
  cite the blocks they compress.

