# Architectural map (mission Section 1) — status of every mechanism

Compiled 2026-09-11 from direct source reading + the prior research record
(`research/perspective-discovery/FINAL_REPORT.md`, `RESEARCH_LOG.md`,
`research/mapper/E1–E18`, `research/RESEARCH_STATUS.md`). Tags:

```text
OBSERVED      = read directly in source during this mission
RESEARCHED    = established by this repo's own prior experiments (cited)
INFERRED      = concluded from headers/docs, not re-verified here
HAND-CODED    = human-supplied structure in the shipped system
LEARNED       = parameters fitted by a mechanism in-repo
DISCOVERED    = structure an in-repo mechanism produced from data
MOCKED        = scaffold standing in for a real mechanism
```

## 1. Representation layer

| mechanism | location | status |
|---|---|---|
| `VectorEmbed` trait (L2-normalized contract) | `physis-core/src/embed.rs:16` | OBSERVED |
| `RandomProjectionEmbedder` — SHA-256 char n-gram hashing, seed 42, dim 384 | `embed.rs:69` | OBSERVED; RESEARCHED: **non-semantic** — runs on it are not evidence (Reconnaissance fact 1, RESEARCH_LOG) |
| `OnnxEmbedder` MiniLM 384-d mean-pool | `embed_onnx.rs:57`, `models/model.onnx` | OBSERVED; determinism probed (`examples/probe_embedding_determinism.rs`) |
| BGE-base 768-d weights | `models/bge-base-en-v1.5/` | OBSERVED present; RESEARCHED: +0.132 AUC over MiniLM, 22/22 configs (E18 / Stage 13) |
| `SyntheticNGramEmbedder` | `embed_ngram.rs:9` | OBSERVED, MOCKED (its own header: test scaffold) |
| CLIP / Jina-clip-v2 embedders | `physis-pro/src/embed/{clip,jina}.rs` | INFERRED from FINAL_REPORT source notes; not re-read here |

## 2. Classification / ontology layer

| mechanism | location | status |
|---|---|---|
| 5×14 semiotic grid (70 anchors) | `models.rs:549/596`, `ontology.rs` | OBSERVED; HAND-CODED; corpus regenerated after audit (RESEARCHED Stage 11: misfiling proxy 31.8% → 16.2%) |
| `CellClassifier` — cosine nearest-entry, `Fixed(1)` top-k | `classify.rs:91,363` | OBSERVED; RESEARCHED: plain cosine to cell centroid beat every structural alternative tried (Stage 12; E14–E16) |
| `OntologyLoader` deterministic domain ordering | `ontology.rs:357` | OBSERVED (fix for the split-corrupting HashMap nondeterminism) |
| coverage-gap discovery vs the fixed grid | `discovery.rs` (`DiscoveryConfig`, `cluster_at`) | OBSERVED; RESEARCHED: not free-form discovery — proposes new entries for a FIXED grid |

## 3. Engine layer (`physis-core`)

| mechanism | location | status |
|---|---|---|
| `PhysisCore` — nodes, hypotheses, contradictions, dreams, wiki trie, persistence + embedder provenance | `core.rs:73–98`, `store.rs` | OBSERVED |
| `CoherenceNode` (embedding + label + coherence scores + cell pin) | `models.rs:48` | OBSERVED |
| hypothesis lifecycle (statuses, evidence, revisions, fitness) | `hypothesis.rs` | OBSERVED; RESEARCHED: tested in `tests/epistemic_*` |
| epistemic audit trail, contradictions w/ resolution | `epistemic.rs`, `contradiction.rs` | OBSERVED |
| asserted / unasserted knowledge | `models.rs` (`PropKind::Observed` vs asserted in `transform.rs`) | OBSERVED |
| dream engine (goal-driven proposal) | `dream.rs`, `physis-pro/src/dream.rs` | OBSERVED (headers + call sites); INFERRED details |
| `becoming` diachronic sense analysis | `becoming.rs:183` | OBSERVED header; RESEARCHED: works, needs a partition it cannot produce (FINAL_REPORT refined plan) |
| temporal validity, transition memory, chronos | `temporal.rs`, `physis-pro/src/machines/chronos.rs` | OBSERVED/INFERRED |
| PDCA process loop | `process.rs` | OBSERVED (process_cycles = 0 in shipped state — RESEARCH_LOG) |
| symbolic transform algebra (Triples, Generalize/Substitute/Analogize, coherence-gated) | `transform.rs` | OBSERVED — **not embeddings-aware**; the recursion it supports is symbolic, not representational |
| `TypedEdge` relations, delta engine, provenance | `relation.rs`, `delta_engine.rs`, `provenance.rs` | OBSERVED; RESEARCHED: not wired to embedding geometry in prior tracks |
| coherence dimensions (6: semantic, logical, temporal, causal, procedural, empirical) | `coherence_dimensions.rs` | OBSERVED; used as 0/1 gates by transforms |
| recursion / certified recursion | prior experiments 7/9/11/16 | RESEARCHED: PARTIALLY SUPPORTED — refuses bad recursions, never beats flat baseline; blind recursion harmful (−0.30) |
| multi-membership soft scoring | prior experiments 8/12/13 | RESEARCHED: SUPPORTED (dual margins positive 9/9 across two datasets), with measured FPR cost |
| cross-embedder agreement as certification | experiments 19/24 | RESEARCHED: FALSIFIED (ARI=1.0 on known-bad, −0.069 on known-good) |
| volitional / action loop | `experiment31_outward_loop.rs`, pro `src/` | OBSERVED (file), INFERRED mechanics |

## 4. Prior research verdicts this mission builds on (all RESEARCHED)

1. Plain cosine is the strongest baseline anyone has beaten here — and only
   where the data is structurally multi-membership.
2. No geometric signal certifies a discovered split (6–7 mechanism families
   falsified). This is why Section 8 (cross-perspective convergence) is the
   most interesting untested idea: certification-by-convergence of
   independent routes was never attempted.
3. Cross-embedder partition identity is ≈ ARI 0.10 — a ready-made null for
   route convergence.
4. Determinism requires: pinned embedder, persisted identity, deterministic
   ordering, construction-matched controls.
5. The embedder, not the mechanism, holds most of the available distance
   (BGE +0.132).

## 5. Gap analysis → what this mission adds (nothing above tests it)

| mission ask | prior status | this mission |
|---|---|---|
| perspective as a formal object with lineage | never built | `Perspective` struct, serialized per node |
| residual relational structure as candidate SOURCE | residuals only diagnosed (cross-cutting items), never used to GENERATE organizations | residual graph → generators |
| multiple independent generators compared | one mechanism at a time | `P_edge` / `P_soft` / `P_ref` + controls |
| convergence across routes as evidence | never attempted (only cross-embedder, which failed as certification) | attempted as convergence, with the 0.10 null |
| held-out prediction from a frozen invented perspective | holdout rediscovery existed for ontology entries (exp30), not for invented re-representations | frozen-transform protocol |
| transfer across surface domains | not tested | dataset 2 |
| recursive invention with stopping | recursion over clusters tested; recursion over REPRESENTATIONS never | level-2 children with lineage |
| decisive synthetic surface≠latent dataset | datasets B/A/D were hand-written CAR/WordNet corpora | seeded generator + difficulty calibration |
| generation (Section 18) | not attempted here | conditional on tiers; recorded honestly if not reached |

No duplicate mechanism is built: the only prior machinery reused verbatim is
`cosine_sim`, k-means/greedy clustering shapes, the ONNX embedders, and the
track's control discipline.

