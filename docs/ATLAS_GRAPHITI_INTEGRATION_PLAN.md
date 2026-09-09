# Atlas + Graphiti → Physis: integration plan (2026-09-09, codebases untouched)

Sources: docs/resources.md §§9–11; ~/dev/atlas v0.1.0-alpha.1 (Apache-2.0);
~/dev/graphiti v0.30.2 (Apache-2.0); physis-core/src; ~/dev/physis_resources.txt
monster benchmark + "What I would copy". Nothing below edits any codebase.
Behavior + tests described, not implemented.

## 0. Guiding principle

Take patterns, protocols, and test shapes — never dependencies. No graph DB, no
LLM service, no Python runtime in physis-core. Everything borrowed is re-expressed
as offline Rust over sled + in-process embeddings. If an item cannot be stated as
an observable behavior of a named physis-core symbol plus a cargo test name, it
is not in this plan.

## 1. From Atlas — TAKE list

A1. Ripple downstream reassessment pass. Atlas: ripple/engine.py
(RippleEngine.propagate, CascadeResult) staging impact → reassess → contradiction
→ routing. Physis: delta_engine.rs (evaluate_mutation, OntologyDeltaReport) +
hypothesis.rs (transition_to, recompute_fitness) + epistemic.rs (FitnessShifted,
StatusTransition). Change: after any transition_to, walk dependents and recompute
fitness per dependent with one epistemic event per step; proposals first, writes
only via the shadow-frame commit. Pays: downstream revision accuracy (today a flip
revises one hypothesis, dependents go stale). Effort M. Risk: churn — needs the
A6 governor or one revision degrades fitness everywhere.

A2. DEPENDS_ON-only walk + edge taxonomy discipline. Atlas:
ripple/analyze_impact.py (analyze_impact, ImpactNode, MAX_DEPTH_DEFAULT=10,
MAX_NODES_DEFAULT=5000, visited-set + cycle list, BFS shallow-first, Kumiho §7.6
structural-not-inference invariant) + ontology/edges.py (StructuralEdgeType ×6,
DomainEdgeType ×10; Ripple walks DEPENDS_ON only). Physis: relation.rs (TypedEdge,
RelationType) + delta_engine.rs (find_neighbors, build_adjacency, dfs_collect,
MAX_PROPAGATION_DEPTH=5). Change: walk declared DependsOn edges only (not cell-pin
or label-prefix adjacency), count justification hops, keep caps + cycle recording
+ proposals-are-not-writes. Pays: kills the blind GAMMA=0.85 cosine wave that
shifts scores on mere arrival. Effort M. Risk: DependsOn-sparse graphs yield empty
walks — needs a logged fallback breadth rule.

A3. AGM 49-scenario gate adapted to HypothesisStatus transitions. Atlas:
benchmarks/agm_compliance/runner.py (Scenario, run_suite, fresh-namespace
isolation) + scenarios.py (simple 10 / multi_item 8 / chain 8 / temporal 8 /
adversarial 15; K*2×12 / K*3×8 / K*4×1 / K*5×9 / K*6×3 + Relevance 7 +
Core-Retainment 9) + revision/agm.py (revise/contract/expand, _content_hash,
TAG_CURRENT/TAG_INITIAL). Physis: hypothesis.rs (HypothesisStatus ×9,
transition_to) + epistemic.rs (StatusTransition). Change: port shape taxonomy +
postulate assertions as tests over status transitions (no Cypher). Pays: the only
external N/N revision-correctness gate in the neighborhood. Effort M. Risk: K*4
(1) and K*6 (3) are thin — ship as weak-form tests, never proof claims.

A4. DERIVED_FROM + SHA-256 ledger → provenance hardening. Atlas:
trust/ledger.py (HashChainedLedger, append_event, verify_chain,
ChainVerificationResult, EventType ×8, canonical-JSON envelope hashing) +
DERIVED_FROM edge (ontology/edges.py) + _content_hash (revision/agm.py). Physis:
provenance.rs (ProvenanceLink, ProvenanceChain::summary) + explanation.rs
(provenance_chain, human_readable_summary). Change: each link carries a content
hash + previous-link hash; a verify walk reports the first break with sequence
coordinates; explanation surfaces the chain, not just the last link. Pays:
provenance completeness on the monster benchmark. Effort M. Risk: detects local
edits but stops no local rewriter — corruption-detection, never a signature.

A5. Adjudication proposal/approve flow → human-in-loop resolution. Atlas:
ripple/adjudication.py (route_proposal, RoutingDecision with rationale+inputs,
AdjudicationRoute AUTO_APPLY/STRATEGIC_REVIEW/CORE_PROTECTED,
CONFIDENCE_DELTA_STRATEGIC_FLOOR=0.15, Obsidian queue Accept/Reject/Adjust/
Demote/Resolve surface). Physis: contradiction.rs (Contradiction::new,
ResolutionStatus::Open default) + epistemic.rs (ContradictionDetected). Change:
small-Δ auto-recomputes; large-Δ/contradiction/high-authority stays Open with a
recorded rationale and resolves only via explicit approve/reject/adjust/
synthesize; high-authority parties never auto-resolve. Pays: contradiction
retention + recovery after false evidence. Effort S. Risk: queue UX without a
consumer rots — ship the rationale record before any file/CLI surface.

A6. ReassessWeights-style tunable propagation params. Atlas: ripple/reassess.py
(ReassessWeights α=0.50/β=0.30/γ=0.15/δ=0.05, HeuristicReassessor default,
reassess_cascade, HALF_LIFE_DAYS=90, per-component breakdown on
ReassessmentProposal). Physis: delta_engine.rs (GAMMA, DEGRADATION_THRESHOLD,
MIN_IMPACT, MAX_PROPAGATION_DEPTH) + hypothesis.rs (recompute_fitness weights
0.20/0.15/0.15/0.25/0.25 + penalties). Change: one named documented parameter
set + per-term contribution record on each recompute. Pays: reproducibility +
ablation (each benchmark run names its weights). Effort S. Risk: overfitting to
one seed — freeze defaults, report tuning as separate scored configs.

A7. No-perturbation invariant as regression test. Atlas: reassess.py
(additive-with-inertia new = current + (1−α)·perturbation; zero upstream change
⇒ zero confidence change; fixes the v1 decay-to-zero bug). Physis:
delta_engine.rs (evaluate_mutation, classify_delta) + hypothesis.rs
(recompute_fitness). Change: tests asserting a zero-shift wave leaves fitness
bit-identical and emits no HypothesisTransition. Pays: cheapest guard against
today's always-shifts-on-arrival failure. Effort S. Risk: ~zero, pure tests.

## 2. From Graphiti — TAKE list

G1. expired_at transaction-time leg → TemporalValidity extension. Graphiti:
edges.py EntityEdge (valid_at = truth start, invalid_at = truth end, expired_at =
system invalidation time, reference_time, episodes[]) + graphiti.py watermark logic
(last_summarized_episode_valid_at, ORDER BY valid_at). Physis: temporal.rs
(TemporalValidity, permanent/from/during/is_valid_at/overlaps). Change: add the
system-recorded invalidation leg + backfill-vs-arrival distinction for
late/out-of-order evidence. Pays: temporal accuracy ("true then" vs "recorded
then"). Effort S–M. Risk: dead weight until queries filter on it — pair with G6.

G2. (resolved, invalidated, new) ingest protocol → delta path. Graphiti:
utils/maintenance/edge_operations.py (resolve_extracted_edges triple;
resolve_edge_contradictions stamping invalid_at/expired_at; dedupe_edges.py;
add_episode funnels). Physis: delta_engine.rs (evaluate_mutation,
OntologyMutation, OntologyDeltaReport) + hypothesis.rs
(add_supporting/add_contradicting_evidence). Change: ingest returns the triple
with a deterministic contradiction-judgment step before fitness update; superseded
items keep timestamps and stay queryable. Pays: stale-belief rate + recovery
after false evidence. Effort M. Risk: judgment needs a deterministic rule first
(LLM judging NOT taken) — start with overlap+recency, log every call.

G3. Episode + watermark model → epistemic trail. Graphiti: nodes.py (EpisodeType
message/json/text/fact_triple; EpisodicNode content/valid_at/created_at;
NEXT_EPISODE chaining; HasEpisodeEdge) + dual saga watermarks (wall-clock filter
vs temporal public watermark). Physis: epistemic.rs (EpistemicAuditTrail,
EpistemicEvent, history_for) + history.rs. Change: raw intake kept as ordered
replayable records with arrival-vs-assertion times + per-stream high-water mark,
so late evidence replays correctly. Pays: forgetting-resistance + ingest-order
replay (missing from history.rs today). Effort M. Risk: two watermarks confuse —
encode the split in types, not comments.

G4. Per-edge episodes[] → provenance chains. Graphiti: EntityEdge.episodes (ids
of referencing episodes) + MENTIONS edges (build_episodic_edges). Physis:
provenance.rs (ProvenanceLink, ProvenanceChain) + explanation.rs
(provenance_chain). Change: every derived link records its intake-record ids, so
beliefs unwind to exact observations. Pays: link-level provenance completeness;
explanations cite intake ids, not just source names. Effort S. Risk: unbounded
id lists — cap with a stated newest-N + first-seen policy, never silent cut.

G5. Hybrid BM25+RRF retrieval → rag/propose path. Graphiti: search/search.py +
search_config.py (bm25 + cosine_similarity per layer; RRF default, MMR optional;
reranker_min_score) + search_config_recipes.py (EDGE/NODE/COMBINED_HYBRID_SEARCH_RRF)
+ search_utils.py (hybrid_node_search). Physis: rag.rs (TokenFixedRetriever,
TokenCounter, token-budget packing) + propose.rs (cosine-only; top-3 0.712
propose-don't-verify). Change: lexical leg beside cosine, RRF fusion, keep budget
packing + propose-does-not-verify. Pays: cheapest portable retrieval win (0.519
says cosine alone cannot judge). Effort M. Risk: double tuning surface — keep
cosine-only as a named baseline for honest comparison.

G6. Point-in-time query semantics → reconstruct_status_at. Graphiti:
search/search_filters.py (valid_at/invalid_at/expired_at range pushdown) +
search_helpers.py (Present = still valid). Physis: epistemic.rs
(reconstruct_status_at) + temporal.rs (is_valid_at). Change: "believed at T?" /
"valid during [a,b]?" become first-class query parameters answered from the
validity triple + audit trail. Pays: temporal accuracy; promotes
reconstruct_status_at from string-matching helper to query semantics. Effort M.
Risk: stringly-typed posterior_state parsing is brittle — normalize status
encoding inside this item, not after.

G7. Invalidate-don't-delete discipline. Graphiti: edge_operations.py (superseded
edges timestamped, never DETACH DELETEd; only explicit Edge.delete removes) +
invalidation_edges_dict beside search hits. Physis: hypothesis.rs (Superseded) +
contradiction.rs + temporal.rs (valid_until). Change: revision marks superseded
items with invalidation timestamps, keeps them history-readable; true deletion is
explicit + separately audited. Pays: contradiction retention + temporal accuracy
+ auditability in one rule. Effort S. Risk: store growth — state a
retention/compaction story before data gets large.

## 3. Explicitly NOT taking (with reason per item)

N1. Graph DB substrate (Neo4j 5.26, FalkorDB, Cypher, APOC). Breaks the offline
Rust + sled zero-service contract; TypedEdge adjacency expresses the walks.
N2. LLM extraction/judging/reranking (extract_edges.py prompts + date-guessing,
dedupe_edges.py judgment, LLMReassessor default, cross-encoder rerankers).
Nondeterministic, key-gated, the 0.519 failure mode; Atlas itself keeps LLM
opt-in behind HeuristicReassessor.
N3. Telemetry default-on (posthog dep). No phone-home by architecture, not config.
N4. Suspect inference code (ATMS.py hyperresolution mutating nogood labels;
atms_algo.py:181 TODO-flagged nogood order). Author-flagged non-standard;
resources.md §7 already excludes both.
N5. Full AGM K* tail as proof (K*4 = 1 scenario, K*6 = 3). Weak-form regression
tests only (A3), never "AGM-complete".
N6. Paid rerankers first ($50–125/mo extraction; BGE/OpenAI/Gemini rerank).
BM25+RRF (G5) is the portable win; rerankers wait for a benchmark number.
N7. Dead surface + weak evidence (Kumiho-gRPC 10/51 wired; BMB 1.000 on 149
self-authored questions with competitor columns SKIPPED, README:451). The honest
cells are the structural contradiction/cross-stream/forget gaps, not the 1.000.

## 4. Merge order — phased roadmap P0/P1/P2

P0 — invariants + history first (constrain everything later). Items: A7, G7, G1
(fields only). Done: zero-shift wave leaves fitness bit-identical, zero
HypothesisTransitions (test: zero_shift_wave_leaves_fitness_untouched);
superseded items history-readable with timestamps, excluded from current queries
(test: superseded_items_stay_queryable_for_history); triple round-trips sled
JSON (test: temporal_triple_serialises). No scoring change — that is the point.
P1 — revision correctness + ingest protocol. Items: A2, A6 (+breakdown), G2, A5
(rationale record only, no queue surface). Done: mid-chain revision revises
exactly dependents in BFS order, cycles recorded (test:
midchain_revision_revises_exact_dependents); recomputes name their terms (test:
fitness_recompute_reports_term_breakdown); ingest returns the triple with logged
judgments (test: ingest_returns_resolved_invalidated_new); large-Δ stays Open
with rationale (test: large_delta_routes_to_open_with_rationale).
P2 — time, retrieval, gate. Items: G3, G4, G6, G5 (cosine-only kept as baseline),
A3, A4. Done: out-of-order ingest replays to in-order state (test:
late_evidence_replays_to_same_state); belief-at-T matches the trail on seeded
chains (test: point_in_time_matches_audit_trail); fusion beats cosine-only or
ships disabled with negative result recorded (test:
hybrid_fusion_vs_cosine_baseline); AGM shapes pass at published N/N, K*4/K*6
marked weak (module: agm_shape_gate); one-link tamper breaks verify at that
sequence (test: ledger_tamper_breaks_chain_at_sequence).

## 5. Acceptance gates (monster benchmark mapping + honesty constraints)

Mapping (contestants RAG/Graphiti/Atlas/TMS-ATMS/Physis): A1 → downstream revision
accuracy; A2 → downstream accuracy + cost; A3 → the revision-correctness score
itself; A4/G4 → provenance completeness; A5/G7 → contradiction retention +
recovery after false evidence; A6 → cost + reproducibility; A7 → guards all
(no phantom movement); G1/G6 → temporal accuracy; G2 → stale-belief rate +
recovery; G3 → forgetting-resistance + hypothesis diversity; G5 → outcome
prediction via better shortlists (propose only — see below).
Honesty constraints (audit FAILED: cosine 0.519 ≈ chance; structural 0.536;
ancestry 0.534; propose top-3 0.712 proposes, never verifies — nothing below
upgrades those numbers alone). Falsifiers: A1 fails if mid-chain accuracy does
not beat the cosine wave on seeded chains; A5/G7 fail if pairs are unrecoverable
after evidence retraction; G1/G6 fail if belief-at-T is no better than current
is_valid_at filtering; G5 fails if fusion does not beat cosine-only (ships
disabled, negative recorded); A3 fails if tests pass only as tautologies (each
must fail on ≥1 naive implementation — mutation-tested or it does not count);
A4 fails if verify passes after a payload edit. No number cited without its null
(permuted-labels or cosine-only) beside it.

## 6. Open questions (max 5, each with suggested probe)

Q1. Ripple: help or churn on physis fitness? Probe: P1 seeded chains, A2 walk vs
cosine wave, accuracy vs stale-belief rate — ship the winner, keep the loser as
a named flag.
Q2. Does expired_at pull weight without transaction-time queries? Probe: G1 fields
behind a flag; if no task queries the third leg, G1 stays fields-only, G6 cut.
Q3. Can ingest judgment (G2) stay deterministic? Probe: overlap+recency rules on
50 hand-built pairs first; below clearly-above-chance, G2 ships flag-only (never
auto-invalidates).
Q4. Is BM25+RRF worth it on 0.519 geometry? Probe: frozen cosine baseline +
probe-set comparison as the ship gate; negative = recorded finding, not failure.
Q5. Cheapest durable adjudication queue (A5)? Probe: in-trail rationale record
only; file/CLI surface iff manual resolutions actually occur — no infra on
speculation.
