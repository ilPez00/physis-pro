# Physis Pro — Resources Map: TMS / ATMS / Belief Revision / Temporal KG / Provenance

Date: 2026-09-09 (updated: Atlas + Graphiti local clones investigated).
Sources: `~/dev/physis_resources.txt` (422 lines, research map, 8 lineages);
`~/dev/ATMS-in-Python/atms_algo.py` (228 lines); `~/dev/NATMS/` (Ruggiero-Santo);
`~/dev/Truth-Maintenance-System/` (satvikshetty04, NCSU course);
`~/dev/atlas/` (RichSchefren, cloned 2026-09-09, README 556 lines);
`~/dev/graphiti/` (getzep, v0.30.2, cloned 2026-09-09);
`~/dev/physis-pro/physis-core/src/` (`contradiction.rs`, `hypothesis.rs`,
`epistemic.rs`, `provenance.rs`, `temporal.rs`, `delta_engine.rs`,
`explanation.rs`, `history.rs`, `propose.rs`).

Audit limits (do not forget while reading this): auto-classification audit
**FAILED** — plain cosine 0.519 ≈ chance, structural 0.536, ancestry 0.534
(see `physis-core/src/propose.rs` header + README status notice). `propose`
top-3 0.712 **proposes, it does not verify** — coverage filters and ranks.
Corpus regenerated 0.1.21 (513/661 moved, 35/70 → 57/70 cells occupied).
Nothing below upgrades those numbers by itself.

---

## §0 — Verdict: ALL relevant, priority-ranked

| Prio | Addition | Why now |
|------|----------|---------|
| P0 | NATMS (`~/dev/NATMS/`) | Only local impl with negated assumptions + `choose{A,B,C}`; maps directly onto `Contradiction` / `ContradictionParty` |
| P0 | ATMS-in-Python (`~/dev/ATMS-in-Python/`) | Cleanest PROPAGATE/WEAVE/UPDATE/NOGOOD reference; env/label mechanics to port |
| P1 | `physis_resources.txt` research map | Atlas + Graphiti + AGM survey are the three external baselines; monster benchmark is the right next experiment |
| P2 | Truth-Maintenance-System (`~/dev/Truth-Maintenance-System/`) | Doyle-style Tell/Retract + dependency-directed backtracking sketch; code itself is not reusable (Python2 `rU`, course-grade) |

---

## §1 — ATMS-in-Python (FellnerDotDev)

- **What:** minimal single-file ATMS: `PROPAGATE` / `WEAVE` / `UPDATE` / `NOGOOD` /
  `minimizeLabel`; environments as `frozenset`s; nogood marker `NoGood='⊥'`.
- **Where:** `~/dev/ATMS-in-Python/atms_algo.py` (228 lines).
- **Upstream:** <https://github.com/FellnerDotDev/ATMS-in-Python> — **MIT**,
  author warns implementation is incomplete.
- **Examples in-file:** `rules_ex1` (A→a, B∧b→e, …; `{'a','e'}→⊥`,
  `{'g','f'}→⊥`), `rules_nixon` (Nixon diamond: `q+Dq→p`, `r+Dr→np`,
  `{p,np}→⊥`), `rules_coffee` (request/water/beans/‑no_* variants, 4 nogoods).
- **Reusable in Rust:**
  - `WEAVE` incremental label construction (§“avoid computing the full label”,
    `atms_algo.py:68-104`) → `Hypothesis.assumptions: Vec<String>` in
    `physis-core/src/hypothesis.rs:234-235` — store assumption sets per
    hypothesis instead of a flat list; add `label: Vec<BTreeSet<String>>`.
  - `minimizeLabel` subsumption pruning (`atms_algo.py:106-121`: drop envs that
    are supersets of another) → new `Hypothesis::minimize_assumptions()`;
    call it from `add_supporting_evidence` / `add_contradicting_evidence`
    (`hypothesis.rs:312-335`).
  - `UPDATE` 2a/2b/2c minimality dance (`atms_algo.py:137-155`) →
    `Hypothesis::recompute_fitness` (`hypothesis.rs:371-433`) already recomputes;
    add the same add-only-if-not-subsumed guard before pushing `Evidence`.
  - `NOGOOD` superset eviction (`atms_algo.py:174-193`) → `Contradiction`
    creation path in `physis-core/src/contradiction.rs:51-63`
    (`Contradiction::new`); record evicted envs as `ContradictionParty.context`.
- **NOT to copy:** the `TODO: I'm not sure if this is okay!?` nogood-marking
  order (`atms_algo.py:181`); `verbose` print-driven control flow; the
  `nodes.get(h) == set()` uninitialised-label branch (`atms_algo.py:86-89`)
  — initialise labels explicitly in Rust; `processRules()` running at import
  (`atms_algo.py:228`).

## §2 — NATMS (Ruggiero-Santo)

- **What:** ATMS subclass supporting **negated assumptions in antecedents**;
  `choose{A,B,C}` expressed via `¬A, ¬B, ¬C` justifications, no hyperresolution
  rules needed.
- **Where:** `~/dev/NATMS/` — `NATMS.py` (28 lines, `class NATMS(ATMS)`),
  `ATMS.py` (181 lines), `Node.py` (15 lines), `HornClause.py` (55 lines,
  `NoGood='⊥'`, `Negation='¬'`), `test/test_ATMS.py`, `test/test_NATMS.py`.
- **Upstream:** <https://github.com/Ruggiero-Santo/NATMS> — check LICENSE in
  repo before vendoring (not confirmed MIT here); algorithm from de Kleer,
  “A general labeling algorithm for assumption-based truth maintenance,”
  AAAI-88 (<https://www.aaai.org/Papers/AAAI/1988/AAAI88-034.pdf>).
- **Key mechanics (verified in source):**
  - `NATMS.NOGOOD` override (`NATMS.py:7-28`): after `super().NOGOOD(E)`, for
    each `A ∈ E` where `A` appears in a nogood antecedent, call
    `UPDATE({E−{A}}, ¬A)` — creates `¬A` nodes on demand.
  - `¬⊥` guard (README §“first refinement”): do **not** create `¬⊥` nodes when
    `⊥` lands inside an environment.
  - Completeness fix (README §“other change”, paper §8): negated assumptions
    are plain nodes, not assumptions, so `A→b, ¬A→b` yields label `{A},{¬A}`
    instead of a bogus singleton — `isAssumption()` (`HornClause.py:5-10`)
    skips the leading `¬` deliberately.
  - `choose{A,B,C}` encoding (README:4-10): `A∧B→¬C`, `A∧C→¬B`, `B∧C→¬A`
    materialised as node justifications.
- **Reusable in Rust:**
  - `¬A`-on-NOGOOD (`NATMS.py:24-27`) → `Contradiction::new(claim_a,
    claim_b)` (`contradiction.rs:52-63`): set `claim_b` = negated party with
    `source` = nogood env id, `context = E−{A}`, `resolution =
    ResolutionStatus::Open` (default, `contradiction.rs:18-29`).
  - Negated-assumption-is-node (not assumption) → `ContradictionParty`
    (`contradiction.rs:67-87`): `¬A` parties carry `authority`/`confidence`/
    `timestamp`/`context` like any claim; never enter the assumption-set type.
  - `choose{}` triple-implication pattern → three `Contradiction` rows sharing
    one `detected_at` + `temporal` window (`temporal.rs:15-23`), or one row
    with `resolution: Contextual`.
  - `isAssumption` leading-`¬` skip (`HornClause.py:8-9`) → any future
    `is_assumption()` helper in `hypothesis.rs` must do the same or envs
    pollute.
- **NOT to copy:** `hyperresolution()` (`ATMS.py:158-164`) mutates the nogood
  label in place and is flagged by the author as non-standard — keep it out;
  `console_log` print debugging; `Node.label = set(frozenset())` default
  (`Node.py:7`) which conflates “no label” with “empty env”.

## §3 — Truth-Maintenance-System (satvikshetty04, NCSU course)

- **What:** Doyle-style TMS: `Tell`/`Retract` over literals and `A+B→E` rules;
  `add_rule` / `assess_rules` / `delete_literals` / `delete_rules`; demo input
  `TMSInput.txt` (13 lines: `Tell:A+B->E`, `Retract:A`, `Tell:-F*H->M`, …).
- **Where:** `~/dev/Truth-Maintenance-System/` — `TMS.py` (175 lines),
  `TMSInput.txt`, `README.md` (11 lines, incl. plagiarism-avoidance note).
- **Upstream:** <https://github.com/satvikshetty04/Truth-Maintenance-System> —
  course project, no license confirmed; treat as **read-only reference**.
- **Reusable in Rust (pattern only, not code):**
  - `assess_rules` forward-chaining loop (`TMS.py:31-52`) → the shape of a
    future `delta_engine.rs` justification pass (today `delta_engine.rs:1-79`
    propagates cosine-shift waves with `GAMMA=0.85`, `MAX_PROPAGATION_DEPTH=5`
    — no logical justifications yet).
  - `delete_literals` cascade (`TMS.py:55-72`) → dependency-directed
    backtracking sketch for `OntologyDeltaReport` consumers.
  - `TMSInput.txt` Tell/Retract script → format for AGM/TMS conformance tests
    (§6, proposal 8).
- **NOT to copy:** literally broken on Python 3 — `open("TMSInput.txt",'rU')`
  (`TMS.py:3`) raises `ValueError: invalid mode: 'rU'`; module-level
  side effects on import (`TMS.py:111-175` run at import time); globals
  `TMS/rules/stack/status` with stringly-typed `'{antecedent,rule}'`
  justifications (`TMS.py:45-50`); single-char `*`→`,`, `+`→split encoding of
  conjunction/disjunction (`TMS.py:22-23`, `add_rule`) that cannot express
  negation scope; no tests.

## §4 — `physis_resources.txt` research map (all 8 lineages)

- **What:** 422-line map placing Physis at the intersection of TMS/ATMS, AGM,
  temporal KG, provenance semirings, paraconsistency, ontology embeddings,
  agent memory, plus Atlas-vs-Physis architecture and a monster benchmark.
- **Where:** `~/dev/physis_resources.txt`.
- **Reusable in Rust (patterns, not code):** justification graphs, assumption
  sets, dependency-directed revision, alternative worlds, explanation
  generation (TMS/ATMS); expansion/contraction/revision + entrenchment (AGM);
  temporal edges + validity intervals + incremental graph build (Graphiti);
  derivation lineage + immutable chains (provenance); `true/false/both/neither`
  states (paraconsistent); graph↔vector mappings (ontology embedding);
  Ripple-style downstream propagation + AGM compliance tests (Atlas);
  ingestion + episodic→semantic paths (agent memory).
- **NOT to copy:** wholesale code from any of them; the map's claim “very
  close to one slice” for Atlas is about belief-dependency propagation only —
  Physis's empirical loop (observation→embedding→hypotheses→outcome→revision→
  replay, `physis_resources.txt:227-244`) is the part Atlas does not cover.

---

## §5 — Wider lineages worth tracking

| # | Lineage | Pointer | Why (one line) | Next step |
|---|---------|---------|----------------|-----------|
| 1 | Atlas — AGM property-graph memory, Ripple reassessment, 49/49 AGM scenarios | <https://github.com/RichSchefren/atlas> | Closest external baseline for revision cascades | Clone; run monster benchmark §“A–E” against it |
| 2 | Graphiti — temporal validity, provenance, incremental updates, hybrid retrieval | <https://github.com/getzep/graphiti> | Production-grade validity intervals + invalidate-don't-delete | Compare `TemporalValidity` (`temporal.rs`) semantics with Graphiti edges |
| 3 | AGM survey 2026 (Doyle→AGM bridge) | <https://doi.org/10.1177/30504554261426170> | Paper-ready historical framing for Physis | Read; lift expansion/contraction/revision invariants into tests |
| 4 | Provenance semirings (Green–Karvounarakis–Tannen 2007) | <https://doi.org/10.1145/1265530.1265535> | Formal vocabulary for “why does this hypothesis exist” | Map `ProvenanceLink{claim,source,timestamp,confidence,raw_reference,method}` (`provenance.rs:15-30`) onto semiring annotations |
| 5 | Paraconsistent logic | std. references (Priest; Belnap 4-valued) | Lets `P ∧ ¬P` coexist — the theory behind keeping `ResolutionStatus::Open` | Encode Belnap values into `ResolutionStatus` (§6.9); answer “why not just paraconsistent KR?” |
| 6 | Ontology embeddings (OWL2Vec/DL2Vec/RDF2Vec/PyKEEN) | <https://github.com/bio-ontology-research-group/machine-learning-with-ontologies> | Only honest path to “symbolic scaffold × continuous space constrain each other” given the 0.519 audit | Prototype ontology-aware embedding; re-run audit before claiming anything |
| 7 | Agent memory (MemGPT/Mem0/Zep episodic→semantic map) + Nous epistemic substrate | <https://github.com/NirDiamant/Agent_Memory_Techniques>, <https://github.com/base76-research-lab/Nous> | Where the LLM world is rediscovering this problem; ingestion + benchmarking infra to borrow | Survey ingestion paths; adopt benchmark harness ideas, not their memory model |

Foundational DOIs for the wall chart: Doyle 1979
<https://doi.org/10.1016/0004-3702(79)90008-0>; de Kleer 1986 ATMS
<https://doi.org/10.1016/0004-3702(86)90080-9>; de Kleer problem-solving
<https://doi.org/10.1016/0004-3702(86)90082-2>; TKG survey IJCAI-2023
<https://doi.org/10.24963/ijcai.2023/734>.

---

## §6 — Concrete integration proposals (smallest useful slice first)

1. **Justification graphs → `hypothesis.evidence` + `provenance`.**
   Attach `ProvenanceLink{claim,source,timestamp,confidence,raw_reference,method}`
   (`provenance.rs:15-30`) to every `Evidence{source,polarity,confidence,claim,
   observed_at,embedding,context}` (`hypothesis.rs:64-81`); surface the chain
   in `ExplanationReport.provenance_chain` (`explanation.rs:57`) and
   `human_readable_summary` (`explanation.rs:59`).
2. **Environments/labels → assumption sets on `Hypothesis`.**
   Add `label: Vec<BTreeSet<String>>` beside `assumptions` (`hypothesis.rs:234`);
   port `WEAVE` incremental construction + `minimizeLabel` subsumption
   (`atms_algo.py:68-121`); alternative worlds = hypotheses sharing an
   observation with disjoint envs.
3. **NOGOOD pruning → `Contradiction` lifecycle.**
   Nogood discovery (`atms_algo.py:174-193`, `ATMS.py:142-156`) creates
   `Contradiction::new` (`contradiction.rs:52`) with `resolution: Open`;
   evicted superset envs land in `ContradictionParty.context`; resolution
   transitions emit `EpistemicEventType::ContradictionDetected` /
   `StatusTransition` (`epistemic.rs:21-31`).
4. **Dependency-directed backtracking → `delta_engine`.**
   Today `delta_engine.rs` attenuates cosine shifts (`GAMMA=0.85`,
   `DEGRADATION_THRESHOLD=0.25`, `MAX_PROPAGATION_DEPTH=5`); add a
   justification pass shaped like `assess_rules`/`delete_literals`
   (`TMS.py:31-72`) so a retracted premise revises exactly its dependents and
   emits `OntologyDeltaReport` instead of a blind wave.
5. **`¬A` nodes → `ContradictionParty`.**
   Port `NATMS.NOGOOD` (`NATMS.py:7-28`) + the `¬⊥` guard: negated parties are
   first-class claims (`contradiction.rs:67-87`), never assumptions; `choose{}`
   becomes three `Contradiction` rows sharing `detected_at`/`temporal`.
6. **Validity intervals → `TemporalValidity` query semantics.**
   `TemporalValidity{valid_from,valid_until,trigger}` with
   `permanent/from/during/is_valid_at` (`temporal.rs:15-69`) already covers
   Graphiti-style invalidate-don't-delete; add “what did we believe at T?”
   via `EpistemicAuditTrail::reconstruct_status_at` (`epistemic.rs:128-156`).
7. **Ripple → downstream reassessment pass.**
   After any `transition_to` (`hypothesis.rs:352-368`), walk dependents
   (Atlas Ripple pattern) and re-run `recompute_fitness` (`hypothesis.rs:371);
   record each cascade step as `FitnessShifted` (`epistemic.rs:27`).
8. **AGM invariants → `StatusTransition` conformance tests.**
   Encode expansion/contraction/revision + entrenchment as property tests over
   `HypothesisStatus` (`hypothesis.rs:25-44`) using `TMSInput.txt`-style
   Tell/Retract scripts; target an Atlas-like “N/N scenarios” gate.
9. **Belnap 4-values → `ResolutionStatus`.**
   Map `APreferred/BPreferred/Open/Contextual/Superseded`
   (`contradiction.rs:18-29`) onto true/false/both/neither; `Open` and
   `Contextual` are the both/neither slots that keep contradiction
   first-class instead of collapsing the system.

---

## §7 — What NOT to do

- Do not vendor the NCSU `TMS.py` — it fails on Python 3 (`'rU'` mode),
  runs on import, and its stringly-typed rules cannot express negation scope.
- Do not port `hyperresolution()` (`ATMS.py:158-164`) or the FellnerDotDev
  `TODO`-flagged nogood ordering (`atms_algo.py:181`) — both are
  author-flagged as suspect.
- Do not create `¬⊥` nodes (NATMS README refinement 1) and do not conflate
  “no label” with “empty environment” (`Node.py:7`).
- Do not treat `propose` top-3 0.712 as certification — same geometry scored
  0.519 top-1 as a judge; shortlists rank, they do not verify
  (`propose.rs:1-53`).
- Do not copy Atlas/Graphiti/Nous wholesale — use them as baselines in the
  monster benchmark (RAG vs Graphiti vs Atlas vs TMS/ATMS vs Physis on:
  contradiction retention, stale-belief rate, recovery after false evidence,
  provenance completeness, temporal accuracy, downstream revision accuracy,
  hypothesis diversity, outcome prediction, catastrophic forgetting, cost).
- Never `git add -A` in `~/dev/aion` (unrelated repo, stray untracked work +
  past SSH-key leak); stage explicit paths everywhere.

---

## §8 — Sources & paths

| Source | Local path | Upstream / DOI | License / status |
|--------|-----------|----------------|------------------|
| Research map (8 lineages + benchmark) | `~/dev/physis_resources.txt` | — (generated map) | local reference |
| ATMS-in-Python | `~/dev/ATMS-in-Python/atms_algo.py` | <https://github.com/FellnerDotDev/ATMS-in-Python> | MIT; author warns incomplete |
| NATMS (+tests) | `~/dev/NATMS/{NATMS,ATMS,Node,HornClause}.py`, `~/dev/NATMS/test/{test_ATMS,test_NATMS}.py` | <https://github.com/Ruggiero-Santo/NATMS> | verify LICENSE before vendoring |
| TMS (course) | `~/dev/Truth-Maintenance-System/{TMS.py,TMSInput.txt,README.md}` | <https://github.com/satvikshetty04/Truth-Maintenance-System> | course code; read-only; broken on py3 |
| Contradiction | `~/dev/physis-pro/physis-core/src/contradiction.rs` | — (`Contradiction`, `ContradictionParty`, `ResolutionStatus::Open` default) | workspace |
| Hypothesis | `~/dev/physis-pro/physis-core/src/hypothesis.rs` | — (`HypothesisStatus` ×9, `Evidence`, `resolve_prediction`, fitness 0.25 predictive weight) | workspace |
| Epistemic trail | `~/dev/physis-pro/physis-core/src/epistemic.rs` | — (`EpistemicEventType` ×10, `reconstruct_status_at`) | workspace |
| Provenance | `~/dev/physis-pro/physis-core/src/provenance.rs` | — (`ProvenanceLink`, `ProvenanceChain::summary`) | workspace |
| Temporal | `~/dev/physis-pro/physis-core/src/temporal.rs` | — (`TemporalValidity::permanent/from/during/is_valid_at/overlaps`) | workspace |
| Delta engine | `~/dev/physis-pro/physis-core/src/delta_engine.rs` | — (`GAMMA=0.85`, `DEGRADATION_THRESHOLD=0.25`, `MAX_PROPAGATION_DEPTH=5`) | workspace |
| Explanation | `~/dev/physis-pro/physis-core/src/explanation.rs` | — (`ExplanationReport`, `HistoricalPrecedent`) | workspace |
| Propose (audit) | `~/dev/physis-pro/physis-core/src/propose.rs` | — (cosine 0.519 fail; top-3 0.712 propose-don't-verify) | workspace |
| Atlas (local clone) | `~/dev/atlas/{atlas_core/{ripple/{engine,analyze_impact,reassess,contradiction,adjudication}.py,revision/agm.py,ontology/edges.py},benchmarks/agm_compliance/{runner,scenarios}.py,tests/}` | <https://github.com/RichSchefren/atlas> | Apache-2.0 (LICENSE verified, 201 lines); v0.1.0-alpha.1; `make test` = `PYTHONPATH=. pytest tests/ -v` (532 tests, needs Neo4j 5.26 via `docker compose up -d`) |
| Graphiti (local clone) | `~/dev/graphiti/{graphiti_core/{edges.py:263-285,nodes.py,graphiti.py:1043-1816,search/search{,_config,_filters,_utils}.py,utils/maintenance/edge_operations.py,prompts/extract_edges.py},docker-compose.yml,Makefile}` | <https://github.com/getzep/graphiti> | Apache-2.0 (LICENSE verified, 201 lines); v0.30.2; `make test` = unit-only (`-m "not integration"`, non-Neo4j drivers disabled); integration needs Neo4j 5.26 or FalkorDB via `docker-compose.test.yml` |
| Agent memory map | — (to clone) | <https://github.com/NirDiamant/Agent_Memory_Techniques> | check repo license |
| Nous | — (to clone) | <https://github.com/base76-research-lab/Nous> | check repo license |
| Onto embeddings | — (to clone) | <https://github.com/bio-ontology-research-group/machine-learning-with-ontologies> | check repo license |
| Doyle 1979 | — (paper) | <https://doi.org/10.1016/0004-3702(79)90008-0> | — |
| de Kleer 1986 ATMS | — (paper) | <https://doi.org/10.1016/0004-3702(86)90080-9> | — |
| AGM survey 2026 | — (paper) | <https://doi.org/10.1177/30504554261426170> | — |
| Provenance semirings | — (paper) | <https://doi.org/10.1145/1265530.1265535> | — |
| TKG survey IJCAI-23 | — (paper) | <https://doi.org/10.24963/ijcai.2023/734> | — |

---

## §9 — ATLAS deep dive (local clone `~/dev/atlas`, v0.1.0-alpha.1)

- **What:** local-first cognitive memory: AGM-compliant belief revision on a
  property graph + **Ripple**, a propagation engine that re-evaluates downstream
  beliefs when an upstream fact changes. Self-describes as alpha; propagation
  loop proven by `./demo.sh` (~12 s: plant 3 nodes + 2 `Depends_On` edges,
  change coffee price $89→$129, `RippleEngine.propagate()`, proposals,
  `adjudication.resolve()`, SHA-256 ledger verify). **Forks its storage
  substrate from Graphiti** (README:554: `atlas_core/graphiti.py`,
  dep `graphiti-core>=0.28.2` in `pyproject.toml:40`).
- **Where:** `~/dev/atlas/` — `README.md` (556 lines), `atlas_core/`
  (21 entries incl. `ripple/`, `revision/`, `trust/`, `lineage/`, `ontology/`),
  `benchmarks/{agm_compliance,business_mem_bench,locomo,longmemeval}/`,
  532 claimed tests, `paper/{atlas.md,appendix-a-agm-compliance.md}`,
  `TESTING.md`, `Makefile`, `docker-compose.yml` (Neo4j 5.26 + APOC).
- **Upstream:** <https://github.com/RichSchefren/atlas> — **Apache-2.0**
  (LICENSE verified, 201 lines). Implements correspondence proofs from
  Park, *Graph-Native Cognitive Memory* (arxiv:2603.17244).
- **Architecture (README:355-387):** facts → quarantine (corroboration by
  `revision/agm.py`: `revise`/`contract`/`expand` in one Cypher transaction,
  content-hash, `TAG_CURRENT`/`TAG_INITIAL`) → materialization triggers Ripple
  once per ledger event → downstream reassessment → contradictions →
  adjudication routing (`AUTO_APPLY` / `STRATEGIC_REVIEW` Obsidian queue /
  `CORE_PROTECTED`) → `adjudication.resolve()` = real AGM revise.
- **AGM coverage — what "49/49" actually covers:**
  `benchmarks/agm_compliance/{runner.py,scenarios.py}` (743 lines) +
  `paper/appendix-a-agm-compliance.md` + `tests/integration/
  test_agm_compliance.py` (`make bench-agm`). Operational, not symbolic:
  every assertion runs live Cypher against Neo4j 5.26. K\*2 Success (12) /
  K\*3 Inclusion (8) / K\*4 Vacuity (**1** — thinnest guarantee) / K\*5
  Consistency (9) / K\*6 Extensionality (**3**) + Hansson Relevance (7) +
  Core-Retainment (9). Five shapes: simple (10), multi_item (8), chain (8,
  depth 3-7 `SUPERSEDES` chains), temporal (8, bitemporal `valid_at` windows),
  adversarial (15: cycles, conflicting tags, revision races).
- **Ripple mechanics (module paths):**
  - `ripple/analyze_impact.py` — recursive `DEPENDS_ON` BFS with visited-set +
    `MAX_DEPTH_DEFAULT=10` + `MAX_NODES_DEFAULT=5000` guards; structural
    traversal only, outputs are proposals, never belief-base writes (Kumiho
    §7.6 invariant). BFS order = depth ascending.
  - `ripple/reassess.py` — `ReassessmentProposal` via additive-with-inertia
    formula `new = current + (1-α)·perturbation`, `perturbation =
    β·strength·Δupstream + γ·llm_delta + δ·(decay−0.5)`; defaults α=0.50,
    β=0.30, γ=0.15, δ=0.05 (`ReassessWeights`). `HeuristicReassessor` default
    is closed-form, $0/month; `LLMReassessor` opt-in. No-perturbation ⇒
    no-change invariant (fixes the v1 weighted-average decay-to-zero bug).
  - `ripple/contradiction.py` — type-aware detector, NOT generic:
    `ContradictionCategory` = strategic_belief_conflict (both >0.70,
    high-severity ≥0.85) / decision_unsupported (`SUPPORTS` belief <0.50) /
    financial_tier_conflict / commitment_deadline_conflict; per-entity rules
    over `CONTRADICTS` edges (`STRATEGIC_BELIEF_CONFIDENCE_FLOOR`,
    `DECISION_SUPPORT_FLOOR` constants).
  - `ripple/adjudication.py` — `route_proposal()`: `is_core_conviction` →
    `CORE_PROTECTED`; high/critical stakes OR |Δconf| ≥ 0.15
    (`CONFIDENCE_DELTA_STRATEGIC_FLOOR`) OR contradictions → `STRATEGIC_REVIEW`
    (Obsidian markdown queue); else `AUTO_APPLY` via AGM `revise()`.
    Every `RoutingDecision` carries rationale + inputs (audit-friendly).
  - `ripple/engine.py` — `RippleEngine.propagate()` orchestrates the four
    stages, returns auditable `CascadeResult{origin,impact,proposals,
    contradictions,routing,error}`; optional SSE `/events` notification.
  - Edge taxonomy `ontology/edges.py`: 6 structural (`DEPENDS_ON` — the only
    edge Ripple walks — `DERIVED_FROM`, `SUPERSEDES`, `REFERENCED`, `CONTAINS`,
    `CREATED_FROM`) + 10 domain (`CONTRADICTS`, `SUPPORTS`, …).
- **Reusable in Rust (concrete physis-core file+symbol mappings):**
  - `analyze_impact` BFS + depth/node caps → `delta_engine.rs`: replace the
    blind cosine wave (`GAMMA=0.85`, `MAX_PROPAGATION_DEPTH=5`) with a
    justification walk; keep the cap pattern but count *justification hops*,
    not embedding hops. BFS order (shallow first) prevents low-confidence
    cascades contaminating downstream — same guard `recompute_fitness`
    (`hypothesis.rs:371-433`) needs.
  - Reassess additive-with-inertia formula → `Hypothesis::recompute_fitness`:
    the no-perturbation ⇒ no-change invariant is exactly what a fitness update
    must satisfy (today a wave always shifts scores); adopt
    `new = current + (1-α)·perturbation` with perturbation gated on upstream
    Δ, not on wave arrival.
  - `route_proposal` three-bucket routing → `Contradiction` lifecycle
    (`contradiction.rs:51-63` + `ResolutionStatus::Open` default): small-Δ
    auto-recompute vs large-Δ/contradiction → `Open` + `EpistemicEventType::
    ContradictionDetected` (`epistemic.rs:21-31`); core-conviction analogue =
    high-`authority` parties never auto-resolved.
  - Type-aware contradiction categories → `ContradictionParty.context`:
    record *which rule fired* (support-floor vs deadline-clash vs tier-clash),
    not just the pair — physis `Contradiction` currently stores parties
    without a detection-rule tag.
  - AGM scenario shapes (simple/multi/chain/temporal/adversarial) → §6.8
    conformance tests over `HypothesisStatus` (×9): copy the *shape
    taxonomy*, not the Cypher; temporal-bucket scenarios map onto
    `TemporalValidity::is_valid_at` (`temporal.rs:57-69`).
- **NOT to copy:** the Neo4j/Cypher substrate (physis is sled + in-process
  Rust; no graph DB); `LLMReassessor` as default (cost + non-determinism —
  Atlas itself keeps it opt-in); Kumiho-gRPC scaffold (10/51 wired,
  41 UNIMPLEMENTED — dead surface); the BMB 1.000-vs-0.711 score as evidence
  (self-authored, 149 synthetic questions; Mem0/Letta/Memori/Kumiho columns
  SKIPPED for missing keys — README:451 admits it; the honest cell is the
  structural gap on contradiction/cross-stream/forget columns, not the 1.000).

---

## §10 — GRAPHITI deep dive (local clone `~/dev/graphiti`, v0.30.2)

- **What:** framework for building/querying **temporal context graphs** for
  agents: entities + fact-edges with validity windows + episodes (raw ingested
  data as ground-truth provenance) + prescribed-or-learned ontology + hybrid
  retrieval. Incremental ingestion, no batch recomputation. Open-source core of
  Zep's managed context-graph infrastructure (paper: arxiv:2501.13956).
- **Where:** `~/dev/graphiti/` — `README.md` (711 lines), `AGENTS.md`,
  `graphiti_core/` (22 entries), `server/graph_service/`, `mcp_server/`,
  `docker-compose.yml`, `docker-compose.test.yml`, `Makefile`.
- **Upstream:** <https://github.com/getzep/graphiti> — **Apache-2.0**
  (LICENSE verified, 201 lines). `pyproject.toml`: package `graphiti-core`,
  deps `pydantic/openai/tenacity/numpy/neo4j/posthog`; drivers for
  Neo4j + FalkorDB + Neptune (Kuzu deprecated); extras `anthropic/groq/
  google-genai/voyageai/sentence-transformers`.
- **Temporal edge model (`graphiti_core/edges.py:263-285`, `EntityEdge`):**
  `fact: str` + `fact_embedding` + `episodes: list[str]` (referencing episode
  ids) + **`expired_at`** (when invalidated) + **`valid_at`** (when fact
  became true) + **`invalid_at`** (when fact stopped being true) +
  `reference_time` + `attributes`. Three timestamps, not two: `valid_at` is
  *assertion/record time from the originating episode* (`graphiti.py:768-771`,
  episodes carry `valid_at` = reference time, `created_at` = now),
  `invalid_at` is *domain-truth end*, `expired_at` is *system invalidation
  time*. Bi-temporal in the TKG-survey sense (valid time vs transaction time),
  with the third leg (`expired_at`) recording when the pipeline noticed.
  vs physis `TemporalValidity{valid_from,valid_until,trigger}`
  (`temporal.rs:15-23` + `is_valid_at`, `temporal.rs:57-69`): physis has the
  valid-time interval but no `expired_at`/transaction-time leg and no
  backfill-vs-arrival distinction — `graphiti.py:459-574` watermark logic
  (`last_summarized_episode_valid_at`, `ORDER BY valid_at ASC/DESC`) is the
  pattern to study for late-arriving/out-of-order evidence.
- **Invalidate-don't-delete (`graphiti_core/utils/maintenance/
  edge_operations.py:339-847`, `search/search_utils.py:1587+`):**
  `resolve_extracted_edges()` returns `(resolved, invalidated, new)`; new edge
  → LLM-judged contradiction check (`prompts/dedupe_edges.py`, idx-offset
  candidate context) → `resolve_edge_contradictions()` sets `invalid_at`/
  `expired_at` on superseded edges, keeps them queryable for point-in-time
  history. `add_episode()` (`graphiti.py:1043-1290`) and bulk
  (`graphiti.py:1290+`) both funnel through this; `search_()` surfaces
  `invalidation_edges_dict` alongside hits. Nothing is `DETACH DELETE`d on
  revision — the only `delete` path is explicit `Edge.delete()`.
  vs physis: matches the §6.6 intent (`reconstruct_status_at`,
  `epistemic.rs:128-156`) but physis has no invalidation *protocol* — no
  (resolved, invalidated, new) triple, no contradiction-judgment step between
  ingest and fitness update.
- **Provenance (`nodes.py`: `EpisodeType`, episodic nodes; `HasEpisodeEdge`,
  `NextEpisodeEdge`):** every entity/edge traces to episode ids; episodes chain
  (`NEXT_EPISODE` sorted by `valid_at`); summaries evolve with watermarks.
  vs physis `ProvenanceLink{claim,source,timestamp,confidence,raw_reference,
  method}` + `ProvenanceChain` (`provenance.rs:15-30`): physis links are
  flat per-claim annotations; Graphiti's episode chain gives *ingest-order
  replay* (what arrived when, in what batch) that physis `history.rs` does
  not currently model.
- **Ontology handling:** prescribed via Pydantic entity/edge models, learned
  by letting structure emerge; custom types per deployment.
  vs physis ontology grid (75 office + 23 machine + 21 agent domains +
  9 built-ins): physis prescribes *content* (a fixed domain×mode lattice for
  classification), Graphiti prescribes *schema* (node/edge types for
  extraction). Orthogonal — Graphiti answers "what shape is this fact",
  physis answers "which cell does this observation belong to". Given the
  0.519 audit, the lesson is negative: a prescribed schema does not fix a
  broken embedding geometry either.
- **Hybrid retrieval (`graphiti_core/search/search.py`,
  `search_config.py`, `search_utils.py`):** cosine + BM25 + BFS traversal per
  layer (edges/nodes/episodes/communities), fused by RRF default (also MMR,
  cross-encoder via BGE/OpenAI/Gemini, episode-mentions, node-distance
  rerankers), `reranker_min_score` cutoff; `search_filters.py` exposes
  `valid_at/invalid_at/expired_at` range filters for point-in-time queries.
  vs physis `propose` (`propose.rs`): physis retrieves-then-ranks with plain
  cosine only — no BM25 leg, no RRF fusion, no temporal filter pushdown.
  The 0.519 top-1 says cosine alone does not carry the judgment; Graphiti's
  cheapest portable win is BM25+RRF fusion, not the LLM reranker.
- **Infra cost — what needs a graph DB:** everything durable. Default backend
  Neo4j 5.26 (`docker-compose.yml`: `neo4j:5.26.2`, bolt 7687, APOC for
  test env); FalkorDB profile available (`falkordb/falkordb:latest`, port
  6379) but AGENTS.md warns the async driver drops concurrent queries
  ("Connection closed by server" in `Graphiti.search` gather — prefer Neo4j
  for hybrid search). Real ingest/search needs `OPENAI_API_KEY` (LLM
  extraction + embeddings; startup/health works with a placeholder).
  Test: `make test` = unit-only (`DISABLE_FALKORDB=1 DISABLE_KUZU=1
  DISABLE_NEPTUNE=1 pytest -m "not integration"`); DB-backed suites need
  Neo4j+FalkorDB up (`.github/workflows/unit_tests.yml`); `mcp_server/` +
  `server/` suites self-skip (exit 5) without a key. Physis implication: no
  part of Graphiti runs inside physis-core's offline Rust process — anything
  borrowed must be a reimplementation of the *protocol* (valid/invalid/
  expired triple, invalidate-don't-delete, episode chaining), never the code.
- **NOT to copy:** the LLM extraction stack (entity/edge prompts,
  `extract_edges.py` `valid_at/invalid_at` date-guessing — nondeterministic,
  key-gated, the exact failure mode behind the physis 0.519 honesty notice);
  FalkorDB for concurrent search (known-broken per AGENTS.md); the
  cross-encoder reranker as a first step (paid, late-stage); telemetry
  default-on (`posthog` dep — set `GRAPHITI_TELEMETRY_ENABLED=false`).

---

## §11 — Atlas vs Graphiti vs Physis (where each wins + the decider)

| Axis | Atlas (`~/dev/atlas`) | Graphiti (`~/dev/graphiti`) | Physis (`physis-core/src/`) |
|------|----------------------|----------------------------|----------------------------|
| Core primitive | **Belief-dependency cascade**: `DEPENDS_ON` walk → reassess → route (`ripple/engine.py`) | **Temporal-fact lifecycle**: episodes → `valid_at/invalid_at/expired_at` edges → invalidate-don't-delete (`edges.py:263-285`, `edge_operations.py`) | **Empirical hypothesis loop**: observation → embedding → hypotheses → outcome → revision → replay (`hypothesis.rs`, `delta_engine.rs`) |
| Change trigger | Upstream belief revision fires downstream re-evaluation (Ripple) | New episode fires contradiction/invalidation judgment on overlapping edges | Outcome observation fires `recompute_fitness` (`hypothesis.rs:371-433`) + cosine wave (`delta_engine.rs`) |
| Contradiction model | Type-aware rules + `CONTRADICTS` edges + 3-bucket adjudication (`contradiction.py`, `adjudication.py`) | LLM-judged edge contradiction → timestamp invalidation, history preserved | First-class `Contradiction{parties, resolution: Open}` (`contradiction.rs:18-87`); detection rules not yet typed |
| Time model | Bitemporal `valid_at` windows on revisions + tag history (AGM temporal bucket, 8 scenarios) | Tri-temporal per edge (valid/invalid/expired) + episode watermarks + point-in-time filters | Valid-time interval only (`TemporalValidity`, `is_valid_at`); no transaction-time leg, no watermarks |
| Provenance model | `DERIVED_FROM` edges + SHA-256 ledger (`trust/ledger.py`, `verify_chain()`) | Episode-id chains (`NEXT_EPISODE` by `valid_at`) + per-edge `episodes[]` | Flat `ProvenanceLink` annotations + `ProvenanceChain::summary` (`provenance.rs`) |
| Where each wins | **Revision cascades**: only system with downstream reassessment as a primitive + 49/49 AGM gate | **Temporal QA + ingest**: point-in-time queries, incremental episode intake, hybrid retrieval, production-hardened | **Closed-loop empirics**: outcome-scored hypotheses + fitness + replay — neither Atlas nor Graphiti scores predictions against outcomes |
| What each lacks | Empirical loop (no outcome scoring; BMB 1.000 is retrieval/revision QA, not prediction) | Revision cascades (flag-only per Atlas's table; no dependent re-evaluation) + no AGM gate | Typed contradiction rules, transaction-time leg, (resolved, invalidated, new) ingest protocol, BM25+RRF retrieval |
| Infra price | Neo4j 5.26 + Docker; deterministic core $0/mo, LLM extraction $50-125/mo | Neo4j/FalkorDB + `OPENAI_API_KEY`; telemetry on by default | Offline Rust, sled, zero external services |

- **The decider — monster benchmark (from `physis_resources.txt`, §7 last
  bullet):** RAG vs Graphiti vs Atlas vs TMS/ATMS vs Physis on contradiction
  retention, stale-belief rate, recovery after false evidence, provenance
  completeness, temporal accuracy, downstream revision accuracy, hypothesis
  diversity, outcome prediction, catastrophic forgetting, cost. Neither clone
  changes physis's audit numbers (cosine 0.519, top-3 0.712 propose-don't-
  verify) — the benchmark is the only honest arbiter.
- **Concrete first experiment (smallest slice):** port Atlas's *scenario
  shapes* (simple/multi/chain/temporal/adversarial, `scenarios.py:1-11`) as
  property tests over `HypothesisStatus` transitions + `reconstruct_status_at`
  (`epistemic.rs:128-156`), seeded with Graphiti-style tri-temporal edges
  (add `invalidated_at`/`expired_at` beside `TemporalValidity.valid_until`)
  and Tell/Retract scripts in `TMSInput.txt` format (§6.8). Score: stale-belief
  rate after a mid-chain revision + temporal accuracy at a past T. No Neo4j,
  no API key, runs in `cargo test`. If physis can't beat plain cosine on
  *that*, no substrate borrowing will save it.

---

## §12 — llm_wiki (nashsu, v0.6.11) — the LLM-built persistent wiki

Added 2026-09-10. **Different project from the vendored `vendor/llmwiki`
crate** (`krakiun/llmwiki`, Tantivy BM25 binary/lib already linked in-process
via `wiki-embedded`). This is `~/dev/llm_wiki` — **nashsu/llm_wiki**, a Tauri
desktop app implementing Karpathy's LLM-wiki pattern end to end: the LLM
reads sources, incrementally builds/maintains a wiki, keeps it current, and
flags items for human review. MIT.

- **Backend (Rust, `src-tauri/`):** hybrid search (`search_project` — token +
  vector legs fused by RRF with **k = 60**, `score: 1/61` in the WebView
  tests), a tool-using chat agent runtime (`lib.rs::agent_start_turn`), and a
  local HTTP API on `127.0.0.1:19828/api/v1` (`api_server.rs`) with
  project-scoped routes, CORS and rate limiting (checked by tests). The MCP
  server (`mcp-server/`) is a *thin* wrapper over that API — it never scans
  the wiki itself, which is exactly Physis's own discipline.
- **Frontend (TypeScript, `src/lib/`):** `ingest-queue.ts` (serial ingest
  queue with crash recovery, retry/pause/cancel/discard — the production
  face of "human curates, LLM maintains"); `ingest.ts` (two-step
  chain-of-thought ingest: *analyze first, generate pages second* with source
  traceability); `lint.ts` + `lint-structural-core.ts` (the Karpathy Lint op,
  content + structural, with fixes); `sweep-reviews.ts` + `review-utils.ts`
  (Review system: the LLM flags items for *human judgment*; resolving a
  review re-sweeps and rebuilds the wiki index); `deep-research.ts` +
  `web-search.ts` (multi-query web research that auto-ingests the results);
  `graph-relevance.ts` (4-signal retrieval graph: **directLink 3.0 /
  sourceOverlap 4.0 / commonNeighbor 1.5 / typeAffinity 1.0**); `wiki-graph.ts`,
  `enrich-wikilinks.ts`, `vision-caption.ts` (multimodal: extract embedded
  source images → vision-LLM captions → image-aware search), `text-chunker.ts`,
  `dedup.ts`. Obsidian-compatible wiki directories, `[[wikilink]]`, YAML
  frontmatter, `index.md` + `log.md` kept as the compilation surface.

### Verdict: read it, do not port it wholesale — it is the *maintenance loop*
Physis's `ingest`/`review`/`dream` machinery already sketches, shipped as a
product. Three gates below are the honest slices; everything else is
STUDY-ONLY so far. Standing rules unchanged: an item without a named symbol
plus a cargo test name does not enter the schedule; read, never depend on.

| Item | What llm_wiki does | Physis landing | Gate test | Status |
|------|--------------------|----------------|-----------|--------|
| L1 | Serial ingest queue with crash recovery, retry, per-source discard (`ingest-queue.ts`) | `ingest.rs` queue controller over the G2 ingest triple | `ingest_queue_crash_recovery_retries_then_persists` | SCHEDULED W4 |
| L2 | Review system: LLM flags candidate items, human resolves, resolving re-sweeps + rebuilds index (`sweep-reviews.ts`, `review-utils.ts`) | `review_desk.rs` route on top of A5 adjudication (a queue surface at last — P1 shipped the rationale record only) | `review_resolution_refreshes_derived_index` | SCHEDULED W4 (after A5 queue surface) |
| L3 | 4-signal graph relevance (direct/overlap/neighbor/type affinity) fused into retrieval (`graph-relevance.ts`) | third RRF leg in `rag.rs::rank_hybrid` beside G5's cosine+BM25 | `graph_affinity_leg_vs_g5_baseline` | SCHEDULED W4 (after G5 measurement lands) |
| L4 | Two-step chain-of-thought ingest: analyze first, generate pages second, source-tagged (`ingest.ts`) | `doctree.rs`/`wiki_gen.rs` split the analysis from the page write (proposals are not writes) | `ingest_analysis_precedes_page_write` | CONVENTION (already Physis's propose-don't-write) |
| L5 | Deep research: multi-query web search → auto-ingest (`deep-research.ts`) | `dream.rs::dream_over_history` counterfactual search already covers the *propose* half; the scheduling half is new | `deep_research_proposals_are_reviewed_not_applied` | SCHEDULED W4 (dream 23.6 first) |
| L6 | Multimodal source images → vision-LLM captions → image-aware retrieval (`vision-caption.ts`) | `sensory.rs` `VisionFeature` arm of the intake bus | `vision_caption_intake_labels_sensory_frame` | SCHEDULED W4 |
| — | RRF k=60, hybrid token+vector (`search_project`, Rust backend) | Same constant the G5 `rank_hybrid` already ships (k = 60) | `hybrid_fusion_vs_cosine_baseline` | **CONVERGED / DONE** (W1) |
| — | index.md as compiled navigation layer (Karpathy pattern) | Physis's `wiki.rs`/`wiki-export` already produce the uberwiki surface | — | STUDY-ONLY (llm_wiki uses an index *and* search; Physis kept grep/Tantivy) |
| — | Obsidian vault compatibility, frontmatter schema | Physis `vault.rs` Obsidian import already exists | — | STUDY-ONLY |
