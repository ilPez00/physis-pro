# Related work sweep — arXiv, 2026-09-14

> Purpose: find literature that either (a) supplies resources for developing
> physis's surviving primitives, or (b) reframes experiments the research track
> refuted, so the refutation becomes a mechanism rather than a dead end. Every
> entry below was retrieved from arXiv on this date. House rule unchanged: any
> adopted idea still has to beat a construction-matched null on physis's own
> harnesses before it ships.

## 1. Reframing the embedding-geometry refutation (E1–E18, ARI ≈ 0.10)

The settled answer "embedding geometry cannot give stable ontology structure"
had no mechanism — it measured two embedders disagreeing and stopped. The
anisotropy literature supplies the missing mechanism and a retest protocol.

| paper | claim relevant to physis |
|---|---|
| **arXiv:2606.29571** — *Anisotropy Decides Cosine vs. Rank Metrics for Text Embeddings* (Parupudi) | The geometric condition deciding when cosine is the wrong comparison metric; rank-based metrics win under anisotropy across 19 parameter-free metrics. |
| **arXiv:2011.04393** — *Positional Artefacts Propagate Through Masked Language Model Embeddings* (Luo et al., ACL 2021) | Persistent outlier neurons tied to positional embeddings are a major **cause** of anisotropy; clipping them improves word-sense discrimination and mean-pooled sentence embeddings. |
| **arXiv:2203.03235** — CLIP vs GPT-2 geometry (ACL 2022) | High anisotropy is not inevitable: contrastive pretraining keeps intra-layer self-similarity ≤ 0.25 while GPT-2 exceeds 0.95. |

**Reframe:** E1–E18 asked "does cosine geometry certify?" and got no. The new
question is "does the *anisotropy of each embedder* predict how far its
geometry disagrees with the next?" — a per-embedder anisotropy measurement
(mean pairwise cosine / intrinsic-dim estimate) becomes a covariate in the
cross-embedder ARI experiment. If ARI ≈ 0.10 is an anisotropy artifact, rank
metrics (2606.29571) or outlier clipping (2011.04393) should move it; if not,
the refutation survives with a mechanism attached. Cheap experiment, uses the
existing mapper harness.

## 2. The antonym rule (E83, 14% recall) and the contradiction rule (lost to NLI)

| paper | claim relevant to physis |
|---|---|
| **arXiv:2601.13251** — *Beyond Cosine Similarity: Taming Semantic Drift and Antonym Intrusion in a 15M-Node Turkish Synonym Graph* (Tosun et al.) | Names the exact failure: embeddings cannot separate synonyms from antonyms. Fix: a **three-way synonym/antonym/co-hyponym discriminator** (90% macro-F1) plus topology-aware clustering that blocks transitive drift chains (hot → spicy → pain → depression). |
| **arXiv:2109.08927** — *Weakly Supervised Explainable Phrasal Reasoning with Neural Fuzzy Logic* (Wu et al., ICLR 2023) | Phrase-aligned NLI: detect phrases, align, predict per-phrase NLI labels, induce the sentence label by fuzzy formulas — NLI **with a ledger-compatible explanation**. |

**Reframe:** E83's static rule has 14% recall because it fires on the *pair*;
2601.13251 shows the separable unit is the *relation type*. Replacement: train
or prompt a three-way relation discriminator over the WordNet reader already in
the tree (commit c9b8075) plus the Italian/English ontology vocabulary — no new
corpus required for a first measurement. The shipped contradiction rule stays
retired (our NLI result already matches the field), but the EPR-style
phrase-aligned variant is worth one run because it emits per-phrase evidence,
which is what the ledger needs to record a contradiction as *why*, not *what*.

## 3. `becoming` — order-aware meaning change

| paper | claim relevant to physis |
|---|---|
| **arXiv:2404.00176** — *The LSCD Benchmark: a Testbed for Diachronic Word Meaning Tasks* (Schlechtweg et al., *SEM) | Standardizes the modular pipeline: Word-in-Context → Word Sense Induction → graded change scores over time. |
| **arXiv:2403.00226** — *A Semantic Distance Metric Learning approach for LSCD* (Aida & Bollegala) | Sense-aware encoders + a learned distance metric comparing a word's usages across two corpora; supervised from WiC data. |
| **arXiv:2404.00345** — replacement-schema interpretable SCD | Models semantic change via lexical-replacement simulations; interpretable, LLM-evaluable. |

**Reframe:** `becoming` already separates "meaning changed" from "two things
share a name" by looking at *when* each occurrence happened — the same insight
the WiC→WSI→LSCD pipeline formalizes. It has never been scored against the
field's benchmarks. Action: express becoming's graded score in LSCD format and
run it on the standard fixtures from the benchmark repo. A null here is easy
(shuffle timestamps); a win puts a physis primitive on an external leaderboard
instead of only internal ones.

## 4. `propose` / `coverage` / discovery — structure-aware representations

| paper | claim relevant to physis |
|---|---|
| **arXiv:2505.12404** — *Hyperbolic Residual Quantization* (Piękos et al.) | Discrete hierarchical representations via hyperbolic geometry beat Euclidean RQ by up to 20% on WordNet hypernym modeling and hierarchy discovery. |

**Reframe:** the 70-cell grid with subtables is a latent hierarchy that cosine
quantization keeps flattening (that is what the certification refutation
measured, and HRQ says why: Euclidean geometry mismatches hierarchical
branching). Candidate experiment: quantize ontology entries with HRQ-style
hyperbolic codes and rerun the E21 blind re-filing protocol for `propose`.
Same protocol, different geometry — a legitimate second chance for geometry,
which the original refutation (cosine, Euclidean) never actually excluded.

## 5. Context compiler

| paper | claim relevant to physis |
|---|---|
| **arXiv:2609.11209** — *REVA: Reusable Evidence View Aggregation for Context-Efficient RAG Serving* (ICDM 2026) | Two findings physis should adopt: (1) modern compressors have **unstable gains over simple truncation** and add online latency; (2) mining the generator's historical attention traces into a document-keyed, budget-agnostic **score store** renders budget-specific plain-text views, +1.0–5.8 quality at 5–15× less compression overhead. |
| **ConCise** (ICWS 2026) | Multi-step RAG: replace raw-text accumulation with an append-only chain of structured conclusions — cumulative context growth O(N²) → O(N), 64.6% token savings. |

**Reframe:** REVA's "unstable gains over truncation" is a warning for the
35%-compression claim: the RESCOPE chain test (`context <= baseline`) is the
right kind of guard, but truncation at the same budget is the null it has
never faced. Add it. ConCise's append-only conclusions chain is structurally
what `pack_global` digests already are — its O(N²)→O(N) framing gives the
token-fixed retriever a measurable multi-step story.

## 6. JEPA reranker (planned, feature `jepa-rerank`)

| paper | claim relevant to physis |
|---|---|
| **arXiv:2608.12939** — *Diagnosing JEPA World Models with Action-Conditioned Predictive Consistency* | ACPC diagnostic: a representation is good when two views that should behave the same stay close after prediction roll-out. Defines Invariance Radius (IR) / Separation Rate (SR). |
| **arXiv:2609.10464** — *SemiGroup-JEPA* | Gains come from the **encoder** learning features the predictor can carry forward, not from the predictor's dynamics. |

**Reframe:** PLAN.md E3's admission test (beat permuted-labels null on E21) can
be sharpened with ACPC's two numbers: for the reranker, "invariance" = same
cell re-ranked consistently across paraphrases of the query; "separation" =
different cells stay apart. Pre-registering IR/SR gives the JEPA reranker a
falsifiable failure mode before training, in the same spirit as the coverage
t-test.

## 7. The epistemic ledger — the differentiator, now externally grounded

| paper | claim relevant to physis |
|---|---|
| **arXiv:2606.04990** — *From Agent Traces to Trust: A Survey of Evidence Tracing and Execution Provenance in LLM Agents* | Defines execution provenance as a **typed graph** and evidence tracing as its projection onto evidence-support relations; connects grounding, tool-use safety, memory lineage, debugging, audit, recovery in one taxonomy. |
| **arXiv:2605.14421** — *MemLineage: Lineage-Guided Enforcement for LLM Agent Memory* | Cryptographic provenance (RFC-6962 Merkle log over Ed25519 signatures) per memory entry; refuses dispatches whose justification descends from an untrusted ancestor. Drives memory-poisoning ASR to zero at sub-millisecond cost. |
| **arXiv:2606.12780** — *ProPlay* | Procedure graph with per-transition **reliability records** learned from past outcomes — "when to trust prior experience" made concrete. |

**Reframe:** the survey's typed-graph definition is close enough to the
epistemic event store that physis can express its ledger *in the field's
vocabulary* — claim support, memory lineage, failure diagnosis — instead of
inventing terms. MemLineage supplies the integrity layer the ledger lacks
(who/what/when, tamper-evident, refusal on untrusted ancestry), and its
chain-of-custody-not-filtering stance matches the ledger's
believe/record/refute loop. ProPlay's reliability records are a lighter-weight
version of the predict-before-acting loop shipped in commit 7568803. Concrete
next step: a ledger-vs-no-ledger A/B on the already-instrumented
token-savings-on-debugging case (RESCOPE §8.4's open lead), scored the way the
provenance survey scores failure diagnosis.

## 8. Priorities

1. **Now (cheap, uses existing harnesses):** anisotropy covariate on the
   cross-embedder ARI experiment (§1); truncation-at-budget null for the
   compression claim (§5).
2. **Next:** three-way relation discriminator on WordNet + ontology vocab to
   replace the dead antonym rule (§2); becoming in LSCD-benchmark format (§3).
3. **Before any JEPA training:** pre-register IR/SR admission numbers (§6).
4. **Strategic:** adopt the provenance survey's taxonomy for the ledger's
   public documentation — it converts physis's differentiator from a private
   concept into a position in a recognized design space, and MemLineage shows
   the enforcement half is buildable deterministically.

*Written from arXiv retrievals on 2026-09-14; IDs verified as returned by the
portal. Nothing here changes the settled verdicts — it adds mechanisms,
benchmarks, and nulls to the ones already measured.*

## 9. Literature for the FUTURE experiments (open agenda, not the refuted set)

Sourced from the open items in RESEARCH_STATUS.md, RESCOPE §8.5 and the plan's
blocked-on rows. These are experiments that have not run yet; the papers below
shape their design before the first measurement is paid for.

### F1 — `transform` / `transplant` null tests (the RESCOPE §8.5 gap)

| paper | design input |
|---|---|
| **arXiv:2603.19087** — *Assessing the Effect of Cross-Domain Mapping on Creativity in Humans and LLMs* (Liu et al.) | Measured difference: humans transfer **surface features** of an analogy source, LLMs transfer **structural/functional** properties; the benefit of cross-domain mapping grows with semantic distance and appears in the most capable models above a distance threshold. |

**Design consequence:** the construction-matched null for `transplant` must not
be a paraphrase-level null (it would only exclude surface transfer). The null
needs a **structure-preserving, function-scrambled** control: same DependsOn
shape, relabelled semantics. The distance-threshold finding also predicts
transplant should pay off most between semantically distant projects — make
semantic distance a recorded covariate, not an accident of fixture choice.

### F2 — the un-splittable clause (E19/E20, C4 differentiation)

| paper | design input |
|---|---|
| **arXiv:2512.12678** — *SSR-GRPO* (hard-negative mining with hierarchical semantic IDs) | In hard-negative mining, rank-based skipping alone is insufficient — needs a group mask **and a cosine ceiling**. |
| **arXiv:2010.00904** — *GENRE* (autoregressive entity retrieval) | Dot-product against pooled vectors misses fine-grained interactions by construction; generative/autoregressive matching is the alternative when discriminative geometry saturates. |

**Design consequence:** build a hard-negative fixture ladder for the clause
pair that defeated every arm — negative candidates bucketed by cosine band
(cosine ceiling), so the failure is localized: does the clause lose at 0.9+
cosine (pooling artifact) or spread across bands (geometry limitation)? This
decides between R7-style late-interaction fixes and a different representation
entirely.

### F3 — pooling destroys file-level content (E80) — late interaction

| paper | design input |
|---|---|
| **arXiv:2004.12832** — *ColBERT* (Khattab & Zaharia) | Token-level MaxSim late interaction instead of one pooled vector; the canonical demonstration that pooling is a choice, not a requirement. |
| **arXiv:2608.26344** — *MoganColBERT-TR* | Single-vector model adapted to late interaction by one-epoch distillation from a cross-encoder teacher over 1 positive + 7 mined negatives — a recipe reproducible on physis's own corpus. |
| **SLIM / ColBERTer** (2023, sparsified late interaction) | Late interaction can be made index-compact (sparse token vectors, whole-word reduction), i.e. the pooling objection has a storage-bounded answer. |

**Design consequence:** E80's finding (pooling destroys the content signal at
file level) predicts a MaxSim variant recovers it. Cheap test: keep the cached
chunk vectors (already content-addressed per section) and score a file-level
query as max-over-chunks instead of mean-pooled file vector — no new model,
no re-embedding, one A/B against the E80 harness.

### F4 — E77 temporal semantics: `observed_at`, state restoration

| paper | design input |
|---|---|
| **arXiv:2608.05982** — *THBKG: Temporal Biomedical Knowledge Graph* | The ledger's point-in-time replay, stated as a field requirement: a programme can only be judged on the evidence that supported its linkage **when it entered the clinic**. Valid-time windows on every edge. |
| **arXiv:2408.00872** — *AnoT* (SIGMOD 2025) | Online anomaly detection over temporal KGs with a summarizing **rule graph** that yields interpretable evidence per verdict — detector/updater/monitor split. |
| **arXiv:2609.05139** — *NS-ST-GraphRAG* | Retrieval should select the graph state **valid for the temporal scope of the query** — the formal version of point-in-time replay. |

**Design consequence:** E77's `observed_at` fix should adopt valid-time
windows on evidence edges (not just a timestamp), because F4's literature
converges on intervals, not points. AnoT's detector/updater/monitor split maps
directly onto the industrial anomaly surface and onto the 360° finding that
nothing restores state after perturbation — the monitor component is the
missing third role.

### F5 — midchain revision / dependent-fact propagation (A2, A5)

| paper | design input |
|---|---|
| **arXiv:2507.08427** — *ChainEdit* (ACL 2025) | Ripple-effect propagation over logical rule chains extracted from a KG: +30% logical generalization on edits, with knowledge-aware evaluation that disentangles external dependencies. |
| **arXiv:2509.21993** — *Bilinear representation mitigates reversal curse* (ICLR 2026) | Consistent editing of one fact (with correct propagation to reverse/dependent relations) **depends on the representational geometry** of the store, not just the editing algorithm. |

**Design consequence:** A2's DEPENDS_ON-only walk is the rule-graph half of
ChainEdit; the second paper ties this thread back to §1 — the store's geometry
predicts whether revision propagates consistently. That makes downstream
revision accuracy (the A2 metric) a measurable function of the anisotropy
covariate from R1: one experiment covers both.

### F6 — retrospective dream (`dream_over_history`, plan 23.6)

No direct literature hit for counterfactual replay over retained-but-superseded
branches (searches on failed-trajectory replay returned nothing relevant on
arXiv this date). Nearest neighbours already recorded: ProPlay (§7) for
outcome-conditioned reuse, MemLineage (§7) for refusing justifications that
descend from retracted events. **Recorded as a genuine gap** — which, given
the thesis that the ledger is the half nothing else ships, is good news: the
retrospective proposal mechanism has no commodity twin to be benchmarked
against, only neighbours to be cited.

### F7 — still open with no literature anchor

- The realistic-error retest at ~0.51 (E16/E15 thread) — error-model
  literature was not searched this pass.
- Second-rater agreement for E20's disputed clause — IAA methods are classic
  non-arXiv territory; no new search needed, just a decision.

## 10. Updated priority queue (merging §8 and §9)

1. **Free wins off existing harnesses:** anisotropy covariate (R1), truncation
   null (R7), MaxSim-over-chunks A/B for E80 (F3 — no new model needed).
2. **Design-before-run:** transform/transplant null with structure-preserving
   controls and distance covariate (F1) — the highest-value untested
   primitives deserve the most careful null.
3. **Fixtures:** clause-pair cosine-banding ladder (F2) to localize the E20
   failure before choosing a fix.
4. **Schema now, pay later:** valid-time intervals on evidence edges (F4) —
   cheap to add to the event store, expensive to retrofit.
5. **Strategic:** ledger vocabulary adoption (R9/R10) stands; F6's gap
   confirms the retrospective-dream mechanism as proprietary territory.
