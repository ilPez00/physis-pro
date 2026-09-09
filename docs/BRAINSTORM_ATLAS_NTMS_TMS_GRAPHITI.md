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
  alive (E29 multi-membership); ATMS supplies the formal shape: labels are
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

