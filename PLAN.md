> **PHYSIS-FIRST:** While developing Physis, use Physis itself for search, retrieval, context packing, navigation, and routing whenever applicable; log every Physis-assisted operation and its conventional alternative/baseline where measurable (tokens/context, latency, hits/misses, errors, fallbacks, task outcome) so normal development continuously produces dogfooding and benchmark data.
read physis_resources.txt

# Physis Pro — Master Plan

## Active development cycle — 2026-09-16 to 2026-09-29

See [Physis as the impossible computer — agent-native semantic
world](docs/plans/2026-09-19-impossible-computer.md) for the live programme:
agents as first-class objects in the HUD, `@agent` calls, task graphs,
crystallization, semantic zoom and offline operation, with per-step files,
estimates and exit gates. It supersedes the sequencing of [Core and Pro: the
next ten development days](docs/plans/2026-09-18-agentic-workspace.md), which
retains the two-project boundary, market/packaging experiments, Settings
memory/primitives editor and P0–P5 gates. (The 2026-09-15 doc an earlier line
named was never committed — corrected 2026-09-19.) Pro owns application
features; Core remains the reusable engine with a small CLI/JSON interface.
This cycle supersedes older three-project split proposals for implementation
planning.

## First priority — hygiene + execution optimizations (2026-09-17)

> Supersedes §32.6 P0 until done. Evidence in `system remember` obs:16–18.
> **Status 2026-09-17:** items 1–2, 4–6 done (see per-item notes); 3 left to
> the W1/W2 authors (not this branch's work to commit); 7–8 closed as no-op.

Hygiene (evidence-ranked, fewest first):
1. ✅ CI clippy gate verified present: `.github/workflows/ci.yml` runs
   `cargo clippy --all-targets -- -D warnings` (default features include the
   industrial stack; the alias is a no-op per the ci.yml comment). Local gate
   now green: `cargo clippy --lib -- -D warnings` clean.
2. ✅ Warnings cleared (they were 35, not 8 — `-D warnings` would have failed):
   unused imports/params underscored or dropped, 19 auto-lints via
   `clippy --fix`, `intent_ir` unwrap-after-is_none rewritten, `clamp` idiom,
   `to_vec`, `DependencyGraph: Default`. Dead `ported/tui.rs`,
   `ported/world.rs`, `orchestration` scaffolding got module-level
   `allow(dead_code)` with ponytail comments naming the upgrade path.
3. ⏸ Dirty tree (Phase 30 W1/W2 files): left to the owning slices.
4. ✅ Scan coverage: `physis-core/src/main.rs::run_scan` clipped node labels at
   5000 chars instead of skipping long files — `physis-core scan src` now
   registers 223/221 files (was 42). Submodule edit; core pointer moves with
   the core repo's own commit.
5. ✅ CLI embedder: plan was wrong that it is env-only — physis-core's default
   features exclude `embed-onnx`, so the shipped core binary could never load
   weights. Fixed by downloading BGE-base ONNX **quantized** weights (110 MB,
   replaced the 436 MB fp32 export; same 768-d output, self-test passes, scores
   within 0.005 of fp32 on the probe set) to `models/bge-base-en-v1.5/`
   (gitignored) and building the core binary with `--features embed-onnx`.
   Measured: `classify` 60s+ → 29.9s (debug) → 19.5s (release build);
   `scan src` 6.7s release for 223 files, semantic. Probe example
   `embed_probe` kept for diagnostics. Next lever if classify latency matters:
   persistent exemplar-embedding cache (Pro's `EmbedCache` pattern) — not
   built, YAGNI until CLI classify volume justifies it.

Execution hot path (checked 2026-09-17, evidence in `src/web/search.rs`,
`src/rag.rs`, `src/web/mod.rs`):
6. ✅ `retrieve_nodes()` (`src/web/search.rs`) now scores candidates inside the
   tenant lock (dot product, same contract as `concepts_search_handler`) and
   keeps only the top `4×top_k` (floor 32) before the MMR token packer — the
   full-store id/label/384-dim-embedding clone per request is gone. Serves
   `/api/v1/search/nodes` + `/api/v1/ask`. Verified: 1033/1033 lib tests,
   `http_surface` search (5) + ask (3) suites green.
7. Already-fast, leave alone: per-query `embed_text` (spawn_blocking, no cache —
   one model call, fine); `EmbedCache` load/save per batch (scan/pack path only,
   not per-request); token trie per-invocation (CLI only).
8. Skipped: SIMD `cosine_sim`, per-tenant content-index, document-lock
   granularity — all YAGNI until a profile says otherwise.

## Active development cycle — 2026-09-16 to 2026-09-29

Research objective: [the semantic operational computer](docs/semantic-computer/RESEARCH_DIRECTIVE.md).
The [v1 implementation skeleton](docs/semantic-computer/CONTRACT.md) defines
event/capability contracts and the controlled benchmark that the Sonnet swarm
will implement. Literature anchors remain unverified until checked in originals.

## Overview

Physis Pro is an **industrial backoffice intelligence engine** that combines
ontological reasoning, semantic classification, PDCA cycles, and dream-driven
countermeasure generation. It ingests machine data, documents, and personal
history; classifies them on a 70-cell semiotic grid; and closes the loop from
anomaly detection to corrective action.

**Active vertical:** Industrial backoffice intelligence (Lean / Industry 4.0).
**Monetization:** service-led pilots → Industriale/Enterprise licensing (Pro),
plus MIT Core as the agent-directory distribution funnel (see
`docs/plans/2026-08-30-productization.md`).

---

## Current State (2026-08-18)

| Layer | Status |
|-------|--------|
| Engine | Holarchic runtime with real-time graph processing, semantic classification, temporal scheduling; 5 compilation blockers fixed (quality.rs, web/mod.rs, coherence.rs, main.rs, industrial_benchmark/secom.rs); `cargo check --features web` compiles cleanly; 759/759 lib tests pass |
| Industrial | OEE tracking, maintenance baselines, anomaly detection, Italian shift reports |
| Document | Ingest, classify, duplicate detection, admin review queue |
| Personal history | Vault import (Obsidian/git log), browser history, OPML, chat JSONL, praxis backfill |
| Study | Study log with rhythms, Elenchus argument trainer |
| UI | Pulse (vitals), Search (by content), Atlas (organic graph + object explorer), Studio (ontology editor), Files (graph file manager), Console (multi-tab dashboard), Agent, Demo, Study — one shared shell, no external assets; new customer-facing pages: index.html (landing), customer_dashboard.html (usage/jobs/tier), api_tester.html (endpoint experimentation), pricing.html (4-tier: Free Trial/Starter/Professional/Enterprise), about.html (capabilities/contact) |
| Licensing | Offline license enforcement with vendor-signed envelope; tier derived from signed feature list (no forged upgrades); banner shows tier. **2026-08-30:** per-job billing (Free Trial/Starter/Professional/Enterprise, pay-per-job) is superseded by the service-led offer in `docs/COMMERCIAL_MODEL.md` — pilots → Industriale/Enterprise licensing |
| Multi-tenancy | Per-user tenant state in sled, `X-Physis-User` header scoping; two deployment nodes: omo and pansa (shared PostgreSQL + model volume, unique API tokens per node; `RAILWAY_PG_CONNECTION_URL` or direct PostgreSQL; ONNX weights from S3/Supabase via `MODEL_BASE_URL`) |

---
| Model & N-Gram Infrastructure Layer | **Completed & Verified (v0.1.26, 190/190 unit tests pass)**: Interchangeable `ModelProvider` & `NGramTable` traits, registry-managed checksum-verified models & tables, variable-order backoff n-gram tables (orders 1–5, add-$k$, stupid-backoff), lexical & structural table kinds, versioned `PHYSISNG1` compact binary format, tokenizer compatibility gates, and unified config-driven pipeline (`RunConfig`) allowing zero-source-change model/table hot-swapping. |



## Embedder expansion — sentence-transformers · jina/clip · jepa (added 2026-09-14)

> Context: the `VectorEmbed` registry already ships BGE-base (768-d), BGE-M3
> (1024-d), ONNX MiniLM (384-d), Jina-clip-v2 (1024-d text+image), CLIP and the
> random-projection last resort, all behind the self-test gate
> (`src/embed/registry.rs`, `src/embed/mod.rs` cascade, `src/web/mod.rs`). What
> follows is the plan to widen the model surface without touching callers.

### E1 — Sentence-Transformers as a first-class kind ✅ Done (2026-09-14)

Any [sentence-transformers](https://www.sbert.net) model exported to ONNX
(`optimum-cli export onnx -m sentence-transformers/<model> models/<dir>/`) becomes
selectable via config, reusing the proven `OnnxEmbedder` engine:

- `EmbedderKindConfig::SentenceTransformers` (`"sentence-transformers"` in
  `config.json`) added in `src/config.rs`.
- Dimension and pooling resolved from the model directory name
  (`st_layout()` in `src/config.rs`): `all-MiniLM*`/`paraphrase-multilingual-MiniLM*`
  → 384 mean, `all-mpnet*`/`bge-*` → 768 mean, `distiluse*` → 512 mean,
  `paraphrase-multilingual-mpnet*` → 768 mean, unknown → 384 mean. Known ST
  exports use mean pooling; the layout table is the single place to extend.
- Wired into all three selection sites (`src/embed/mod.rs` CLI cascade,
  `src/web/mod.rs` server cascade, `src/embed/registry.rs` registry): as
  `embedders.primary` it participates in the same self-test-gated chain and
  falls back exactly like the other kinds when the export is missing.
- Config example:
  ```json
  "embedders": { "primary": { "kind": "sentence-transformers",
                               "model_dir": "models/paraphrase-multilingual-MiniLM-L12-v2",
                               "enabled": true } }
  ```
- Rationale: the multilingual MiniLM exports close the measured Italian gap
  (tier 0.733 with the English embedder) at 384-d cost, without BGE-M3's 2.2 GB.

### E2 — Jina / CLIP hardening (next)

Status: both embedders exist (`src/embed/jina.rs`, `src/embed/clip.rs`) but the
bundled jina-clip-v2 fp16 export fails the self-test (~0.06 top score) and is
deliberately excluded from the auto-fallback cascade. Remaining work:

1. Re-export jina-clip-v2 in fp32 (or q8) and verify against the semantic
   probe; a passing export is opted back into the fallback chain after BGE-M3.
2. CLIP vision weights are optional; wire `embedders.image` through
   `EmbedderRegistry.image` so video (`src/video.rs`) uses CLIP when Jina
   vision is absent — currently `image` is parsed but only the Clip branch
   populates it.
3. Measure jina-clip-v2 text vs bge-base on the cell-classification fixture
   (`tests/classification.rs` accuracy floor) before any promotion.

### E3 — JEPA (later, research-gated)

Two distinct tracks, mirroring the Fourier precedent (keep them named apart):

- **JEPA reranker** (Phase 11D, feature `jepa-rerank`): lightweight predictor
  trained on *this corpus's* classify decisions, re-ranking top-k cell
  candidates from `CellClassifier::classify`; falls back to pure cosine when
  the feature is off. Admission test: must beat the construction-matched null
  (permuted labels) on the E21 blind re-filing protocol before it ships.
- **V-JEPA video encoder** (research only, behind `embed-jepa`): frame-patch
  target representations as an alternative to the CLIP mean-pool clip vector.
  Do not promote without the same null-gated protocol.

> House rule inherited from the research track: no embedder/reranker ships
> without a measurement against a construction-matched control. `propose`
> (+0.531 top-3) and `coverage` (*t* = +12.11) set the bar for what "works"
> means here.


### E4 — Literature sweep: resources, links, and reframed failures (2026-09-14)

Full memo with reasoning per entry: **`research/RELATED_WORK_2026-09-14.md`**.
Everything below was pulled from arXiv on 2026-09-14 and mapped to a physis
primitive or a failed experiment.

| # | Paper (link) | Feeds into | What it changes |
|---|---|---|---|
| R1 | [Anisotropy Decides Cosine vs. Rank Metrics](https://arxiv.org/abs/2606.29571) | E1–E18 geometry refutation | Anisotropy is the covariate that decides when cosine fails → retest ARI with anisotropy measured; rank metrics as fallback |
| R2 | [Positional Artefacts Propagate Through MLM Embeddings](https://arxiv.org/abs/2011.04393) | same | Outlier-neuron clipping as a candidate fix; mechanism for why embedders disagree |
| R3 | [Beyond Cosine: Antonym Intrusion in Synonym Graphs](https://arxiv.org/abs/2601.13251) | E83 antonym rule (14% recall, dead) | Three-way synonym/antonym/co-hyponym discriminator (90% F1) replaces the pair-firing rule; buildable on the in-tree WordNet reader |
| R4 | [Explainable Phrasal Reasoning with Neural Fuzzy Logic](https://arxiv.org/abs/2109.08927) | contradiction rule (retired, lost to NLI) | Phrase-aligned NLI emits per-phrase evidence — ledger records *why*, not just *what* |
| R5 | [The LSCD Benchmark](https://arxiv.org/abs/2404.00176) · [Semantic Distance Metric Learning for LSCD](https://arxiv.org/abs/2403.00226) | `becoming` | External benchmark (WiC → WSI → graded change) for an internal-only primitive; shuffle-timestamps null is trivial to build |
| R6 | [Hyperbolic Residual Quantization](https://arxiv.org/abs/2505.12404) | `propose` / grid | Grid is a latent hierarchy Euclidean cosine flattens; HRQ gives a legitimate second chance for geometry under the same E21 protocol |
| R7 | [REVA: Reusable Evidence View Aggregation](https://arxiv.org/abs/2609.11209) | context compiler | Compressor gains are unstable vs truncation → add truncation-at-budget null to the 35% claim; attention-trace score store matches the embed cache design |
| R8 | [Diagnosing JEPA World Models (ACPC)](https://arxiv.org/abs/2608.12939) | JEPA reranker (E3) | Pre-register Invariance Radius / Separation Rate as the falsifiable admission test before any training |
| R9 | [Evidence Tracing & Execution Provenance survey](https://arxiv.org/abs/2606.04990) | epistemic ledger | Field-standard vocabulary (typed provenance graph, claim support, memory lineage) for the differentiator's public docs |
| R10 | [MemLineage](https://arxiv.org/abs/2605.14421) | epistemic ledger | Tamper-evident entries (RFC-6962 Merkle log + Ed25519) and refusal on untrusted ancestry — the integrity half the ledger lacks |
| R11 | [ProPlay](https://arxiv.org/abs/2606.12780) | predict-then-record loop | Per-transition reliability records = lighter-weight prior art for the shipped predictor |

**Failed experiments this reopens (nothing settled is overturned):**

- E1–E18 (geometry cannot certify) → reframe as *anisotropy-conditioned*
  geometry (R1, R2); and R6 shows only Euclidean-cosine geometry was ever
  excluded.
- E83 (antonym rule, 14% recall) → reframe as *relation-type* classification
  (R3).
- Shipped contradiction rule (beaten by NLI) → stays retired, but R4's
  phrase-aligned variant adds the evidence granularity the ledger needs.
- 35% compression (chain test only) → truncation-at-budget null missing (R7).
- `transform` / `transplant` (never null-tested, RESCOPE §8.5) → unchanged;
  still the open gap, unaffected by this sweep.

**What this gives us (the honest ledger):**

1. **Mechanisms where we had verdicts.** "Geometry doesn't certify" and "the
   antonym rule is dead" become *conditional* claims with named causes —
   which is the difference between a refutation and a physics.
2. **External benchmarks for internal primitives.** `becoming` (R5) and
   `propose`-under-HRQ (R6) can be scored against public fixtures — a win is
   evidence a stranger can check; a loss is information we don't have to pay
   for twice.
3. **Falsifiable gates before spend.** R8 lets us write down what failure of
   the JEPA reranker looks like *before* training it; R7 adds the null that
   protects the compression claim from the same critique REVA levels at the
   field.
4. **A recognized design space for the differentiator.** The provenance
   survey (R9) means the ledger can be positioned in the literature instead
   of alone, and R10 shows its missing enforcement half is deterministic and
   buildable — strengthening the bet RESCOPE §8.5 already endorsed.
5. **Cheap queue.** Items R1 (anisotropy covariate), R3 (WordNet
   discriminator), R5 (LSCD export), R7 (truncation null) all reuse existing
   harnesses, corpora, or models already in the tree — no new infrastructure
   to buy before the next measurement.

### E5 — Literature for future experiments (added 2026-09-14, second pass)

Memo §9–10 (`research/RELATED_WORK_2026-09-14.md`). Design inputs for the
experiments that have *not run yet*, keyed to the open agenda:

| future experiment | paper(s) | design change it dictates |
|---|---|---|
| `transform`/`transplant` null tests (RESCOPE §8.5 gap) | [Cross-Domain Mapping in Humans and LLMs](https://arxiv.org/abs/2603.19087) | Null must be structure-preserving/function-scrambled (LLMs transfer structure, not surface); record semantic distance as covariate |
| E19/E20 un-splittable clause | [SSR-GRPO](https://arxiv.org/abs/2512.12678), [GENRE](https://arxiv.org/abs/2010.00904) | Hard-negative fixture ladder with cosine-band ceilings to localize the failure before choosing a fix |
| E80 pooling destroys file-level content | [ColBERT](https://arxiv.org/abs/2004.12832), [MoganColBERT-TR](https://arxiv.org/abs/2608.26344) | MaxSim-over-cached-chunks A/B — no new model, no re-embedding |
| E77 temporal semantics / state restoration | [THBKG](https://arxiv.org/abs/2608.05982), [AnoT](https://arxiv.org/abs/2408.00872), [NS-ST-GraphRAG](https://arxiv.org/abs/2609.05139) | Valid-time *intervals* on evidence edges (field converged on intervals, not timestamps); detector/updater/monitor split |
| midchain revision propagation (A2/A5) | [ChainEdit](https://arxiv.org/abs/2507.08427), [Bilinear representations & reversal curse](https://arxiv.org/abs/2509.21993) | Revision accuracy is a function of store geometry — one experiment ties A2's metric to the R1 anisotropy covariate |
| retrospective dream (`dream_over_history`, 23.6) | — (no hit; nearest: ProPlay R11, MemLineage R10) | **Recorded gap = proprietary territory**; nothing commodity to benchmark against |

Priority merge (memo §10): MaxSim A/B and anisotropy covariate are free wins;
transform/transplant null design comes next (highest value, needs the most
careful control); valid-time intervals are a schema change cheap now, expensive
to retrofit. E15 realistic-error retest and second-rater agreement remain
un-anchored (F7).

---

---


## Research track — epistemic mechanisms (status 2026-09-09)

Full record: **`research/RESEARCH_STATUS.md`**. Per-experiment reports sit with
their code in `research/perspective-discovery/` (E1), `research/mapper/`
(E2–E18, E21–E23) and `research/convergence/` (E19–E20).

### Settled

| question | answer | key evidence |
|---|---|---|
| Can embedding geometry give stable ontology structure? | **NO** | cross-embedder ARI ≈ 0.10 |
| Was the shipped corpus sound? | **NO — fixed** | 43.3% misfiled; regenerated in 0.1.21 |
| Can the machine *filter* discovery candidates? | **YES, shipped** | `coverage`, Welch *t* = +12.11 |
| Can it *rank* them? | **YES, shipped 0.1.20** | precision@25 0.480 vs 0.132 base |
| **Can any tested signal certify a filing?** | **NO — settled E14–E18** | on **real** misfilings every scorer is at chance: cosine **0.519**, CONSTRAINT 0.536, ANCESTRY 0.534. ANCHORVOC's 0.664 was an entry-length artifact that fails replication |
| Does cross-model **divergence** carry signal? | **NO (E1)** | 0.602 vs 0.733; equals its null once conditioned on the stronger support |
| Can Qwen n-grams be a groundable **map**? | **NO (E2)** | no disproportionate sparse gain; propagation worse than ignoring the map |
| Is it a **data** problem? | **NO (E3/E4)** | 24× the text made every level worse |
| Is the **70-cell grid** the constraint? | **NO (E5)** | 57→14 classes gains +0.014 kappa |
| Is the mode axis weak from missing definition? | **PARTLY (E6/E7)** | population explains it, r = +0.718 with log class size |
| Do ontologies propagate **hypertrophically**? | **YES (E8)** | hypertrophy index **−0.644**; WORK sits +0.003 above random |
| Should the grid be **expanded**? | **NOT YET (E8)** | pre-flight STOPs on sparsity at every level |
| Is a discrete **signifier address** worth formalising in Lean? | **NOT YET (E9)** | best address 0.244 held-out vs geometry's 0.447 |
| Does **downstream viability** beat local correctness? | **NO (E10)** | +0.01 identity for −5.8 depth |
| Can the **anchors** be improved? | **NO (E12/E13)** | three attempts refuted; headroom over a probe built from the cell's own entries is **+0.004** |
| Does presenting several *structurally congruent* domains improve abstraction? | **NO (E19)** | C5 +0.467 vs one domain +0.400 and a token-matched +0.450; replicated at 4B, 7.6B and 9.2B |
| Is that a small-model artifact? | **NO (E19b)** | the ceiling does not climb with scale — ORACLE +0.667 at 4B, +0.500 at 7.6B, +0.750 at 9.2B, and C5 never separates from one domain |
| Can geometry *propose* a filing, given human decisions? | **YES (E21)** | top-3 **0.667** against a permuted null of 0.136 and a lookup table of 0.368 |
| Can the **mode axis** be made coherent? | **NOT FROM THIS CORPUS (E27–E29)** | merging, head-only representation, factorised decoding, a stronger embedder and unsupervised discovery all fail — mode is *absent from the representation*, not underdetermined by its labels |

| **Does the underlying arithmetic object induce representation-invariant probability laws / statistical relations?** | **NO (E51)** | moving from geometry (E50 negative) to probability, return times, gap statistics, and relational invariance across independent representations ($\psi, \theta, \pi, M/\sqrt{x}$, blind held-out, three null controls). $0/5$ candidate laws clear transfer AND arithmetic-specificity AND held-out survival: what transfers is null-equivalent (generic cumulative-counting behaviour), what is prime-specific does not transfer. See `physis-core/research/impossible_machine/EXPERIMENT51_RESULTS.md` |
| Is there **any** object-level property of the primes that is both arithmetic-specific AND out-of-sample stable? | **YES (E52) — first one found** | the Hardy–Littlewood small-gap (pair-)correction: deviation $D$ of small even-gap frequencies from the Cramér independence model is $\approx 12\times$ the worst independence null under a blind held-out split, and survives it ($D=1.222$ discovery vs $1.258$ held-out; twin ratio $2C_2\approx 1.45>1$ in both halves). Caveat: the π/θ and ψ views count the same primes, so this is specificity + stability, not independence across representations. See `physis-core/research/impossible_machine/EXPERIMENT52_RESULTS.md` |

### Three things that reversed under scrutiny, kept because the process is the point

**Stage 12's certification refutations were reached by an invalid route (E14).**
Its injection moves an entry to a *uniformly random* cell. Cosine leads by 0.207
under that policy and **trails CONSTRAINT by 0.106** under a plausible near-miss
— the scorer detects *distance moved*, not wrongness. The conclusion survives;
the reasoning did not. `FINAL_REPORT.md` and `RESEARCH_LOG.md` now carry the
correction inline at the sentences it invalidates.

**ANCHORVOC looked like the first real certification signal and was not
(E16→E18).** 0.664 on real misfilings, significant — then entry length alone
scored 0.619, no vocabulary effect survived controlling for it (+0.048, ns), and
it failed to replicate on the audit of a filing the rater had not produced.
Four experiments, each correcting the last, ending where the track stood before
E14 for much better reasons.

**E19's "decisive test" was not a control at all (E19b).** S5 was described as
C5's tokens with congruence destroyed. Its five bodies are C5's, **byte-identical,
cyclically shifted against five identical headings** — tf-cosine 1.000 — and the
generation prompt tells the model to discard exactly the part that moved. `C5 ≈
S5` is near-tautological. Two rebuilds followed and both are recorded: **B5b's
falsifier fired at 4B, 7B and 9B** (all five schemas converged on ¬C1), and
**B5c's fires at 4B and not at 9B**. The verdict is unchanged and now rests on
A1 and A5, which are valid. **The transferable finding: a control arm's validity
is a property of the stimulus *and the model* together, and must be checked per
model rather than assumed from the design.**

### The one constraint everything converges on

**The corpus is too small, and that is measured rather than argued.**

- E5: 22 of 57 cells hold fewer than 5 entries; macro-F1 0.318 against accuracy 0.447.
- E7: a well-populated mode cut to 3 training entries scores **F1 0.06** — the
  F1 = 0.00 modes are empty, not ill-defined. **n ≈ 20 → F1 0.26–0.49.**
- E8: the pre-flight STOPs the *current* grid on sparsity — 35 of 57 classes
  below 6 training entries.
- E10: the mechanism under test fired in under 3% of generations.

**Populate first, expand second.** `research/mapper/EXPANSION_SPEC.md` has the
deficit table, forecast and acceptance test;
`research/mapper/E11_CANDIDATE_MINER.md` has a validated worklist (WordNet,
recall@100 = 0.497 against a 0.086 shuffled control).

### Standing rules, each earned

1. **Treat any positive as artifact until a construction-matched control says
   otherwise.** Seven have died to one.
2. **Report kappa, never accuracy against a majority floor**, on unbalanced
   multi-class problems.
3. **Check what the ground truth is made of.** Synthetic perturbations encode a
   difficulty assumption; E14 showed Stage 12's was the entire signal.
4. **Run the pre-flight** (`research/mapper/preflight.py`) and honour it. It was
   overridden once, by its own author, in E10.
5. **Coverage filters and ranks discovery candidates; it does not verify that a
   candidate is correct.** Sell the filter, not the judgement.
6. **Ask the machine to propose, not to judge.** Earned by E21: the same
   geometry is a failed judge at 0.45 top-1 and a usable proposer at 0.667
   top-3. **Change the question before changing the representation.**
7. **Causes are user-assessed unless an exact error log is provided.** E23: on
   activity text, lexical convention (0.628) beats semantics (0.575). A label
   that was written down can be read; a cause that was not written down must be
   asked for, not inferred.
8. **A diagnostic criterion is not an objective function.** Earned by E27→E28:
   head-only made the mode axis measurably *more coherent* (−0.061 → −0.024) and
   **−0.091 worse at predicting**. Compression answers "is this an object";
   held-out top-k answers "does this predict". Do not optimise the first and ship
   the second.

### The turn (2026-09-09): stop asking the machine to judge

Every refutation above refuted **unsupervised, zero-shot judgement**. COSINE,
NAME-ONLY, CONSTRAINT, ANCESTRY and ANCHORVOC are fixed formulas with nothing
fitted to a human decision. The supervised, human-in-the-loop version had never
been measured, and the question a person filing 582 entries actually has is not
*"is the machine right"* but *"can it narrow 57 cells to 3 so I choose instead
of search."*

**E21 — geometry as a proposer. The first positive here that survives its
construction-matched null.** `research/mapper/E21_PROPOSER.md`. Ground truth is
the rater's revised cell in `refiling_gate0.tsv`, 582 blind entries, 57 targets.

| at 436 decisions seen | top-1 | top-3 | top-5 |
|---|---:|---:|---:|
| **geometry** | **0.438** | **0.667** | **0.755** |
| prior `old`→`new` lookup | 0.184 | 0.368 | 0.474 |
| majority | 0.089 | 0.255 | 0.359 |
| **permuted (the null)** | 0.036 | 0.136 | 0.233 |
| length only (E18's confound) | 0.010 | 0.036 | 0.071 |

**Top-1 0.438 essentially reproduces E5's 0.447. The geometry did not get
better.** A number filed as a failure is the same number that makes a usable
proposer once the output is three candidates instead of one. The learning curve
is still climbing at 436 (top-3: 0.385 → 0.508 → 0.595 → 0.667).

**Division of labour, measured.** `domain` 5 classes, top-2 **0.837**; `mode`
14 classes, top-2 0.678. **The machine proposes the domain and defers the
mode.** A stronger embedder moves mode (+0.065 top-1) and not domain top-2
(+0.003, saturated), so what limits domain is label noise, not encoding.

### Architecture: Physis does not adjudicate meaning

**It proposes, carries and defers.** Interpretations arrive as tokens from a
trusted source — human or another machine — and the job is to make them cheap to
produce and to propagate, not to verify them. Physis does not need qualia. It
needs a trust relation and an exchange currency, and the currency is tokens.
Trust runs both ways: Physis accepts a human's or a peer machine's reading, and
offers its own to be accepted on the same terms.

This is the stance the whole negative record argues for. Every attempt to make
the machine adjudicate lost; the one thing that worked was making it propose and
letting a rater decide.

**The vector may collapse a structure; the interface need not.** E25: an additive
embedding space cannot distinguish `dog + tooth` from `dog-modulus|tooth|`, so a
compositional ontology has to be **carried explicitly** rather than hoped for in
geometry. E26 says which structures are worth carrying, and the answer is
measured rather than chosen:

| | domain | mode |
|---|---|---|
| own-subspace compression | **4-d, lossless** (−0.003) | not a subspace (−0.034) |
| proposer top-2 | **0.837**, saturated | 0.678, still moving |
| confusion structure | one symmetric merge pair | a four-way blur over 68% of the corpus |

Three methods, none aware of the others: **domain is a small well-formed object;
`mode` as currently defined is not a coherent one.** So the interface carries
domain as a four-number coordinate and **defers mode to the person** — and no
reweighting or projection will change that, because mode fails to compress for
lack of linear structure rather than for lack of a better embedder. Domain and
mode are also **not perpendicular** (principal angles 35.8–54.5°, mean 46.1°) and
orthogonalising them *hurts both*, so the overlap is shared signal, not
interference.

**Corollary, and it is a hard rule: causes are user-assessed unless an exact
error log is provided.** Where the signal is explicit — a compiler error, an
exit code, a stack trace — take it literally. Where it is not, hand it to the
person and do not infer. **E23 is the evidence** (`research/mapper/run_e23.py`):
on 528 conventional-commit subjects, predicting the activity label from the
activity text gives TF-IDF **0.628** and an embedding **0.575** against a 0.451
majority floor and a 0.222 permuted null. **Lexical convention beats semantics.**
So machine-generated supervision is real where a convention or an exact log
carries the label, and is not a route to inferring causes that were never
written down.

### Open, and what each needs

| question | needs |
|---|---|
| **Does E21's proposer learn a policy or the corpus?** | **a second rater on a sample. This decides whether the positive is worth building on** |
| Which grid revision? | a human call on E22's brief, below. The machine has nothing further to offer (E29) |
| Can the mode axis be rescued at all? | entries **observed in use** — an activity trace. Every route from static description is closed (E27–E29) |
| Will populating the starved modes raise mode kappa? | the entries themselves |
| Does anything detect a real misfiling? | a mechanism nobody has proposed yet — and E21 says stop looking for a judge |
| Why does **C4 / differentiation** defeat everything? | open from two directions: the worst clause for every arm in E19 even when stated outright, and the one clause a dense embedding cannot split in E20 |

### The axis re-brainstorm, briefed from the corpus rather than a blank page

`research/mapper/E22_AXIS_STRUCTURE.md`. **Proposals only; no script touches
`GRID_AXES.md`.** Same human/machine split as E21, one level up.

**The finding that reframes it.** `corpus.json` ships a name-only and a
name+hints vector. Paired over 40 identical splits: hints help `domain`
(**+0.022**, *t* = +3.13) and **hurt `mode`** (**−0.034**, *t* = −5.39, better in
6/40). Hints are subject-matter vocabulary, so they pull entries toward what
they are *about* and away from what is being *done*. E6/E7's population account
stands but is not the whole story, and **adding entries would never have found
this**. A cheap follow-up killed the obvious fix: entries whose *names* name an
activity are no better predicted on mode (−0.009, null).

**Mode is three problems, not one:**

1. **The core blur — 68% of the corpus.** WORK/CREATE/MAINTAIN/SENSE hold 397 of
   582 entries and are mutually confusable; WORK 0.44 and MAINTAIN 0.39 **with
   ample population**. Not sparsity — four definitions that carve nothing apart.
   **Start here; everything else is smaller.**
2. **A definition failure, not a population failure.** WALK has n = 27 — like
   LEARN (36, 0.66) and PLAN (35, 0.64) — and scores **0.18**. The corpus gave it
   a fair trial and it failed. Redefine or retire.
3. **Genuinely unmeasured — do not judge these.** GUIDE 13, PLAY 12, LIFT 11,
   DESTROY 4, BRAINSTORM **1**. E7: a mode cut to 3 entries scores F1 0.06.
   Populate to n ≈ 20 or drop. One real signal survives the sparsity: **GUIDE →
   LEARN is 0.48 one-way with 0.01 back** — as written, GUIDE is a kind of LEARN.

**Domain:** **FABRICATE ↔ CONSTRUCT confuse at 0.22/0.21, symmetric** — one
distinction wearing two names, the cleanest merge candidate in the grid. CONSTRUCT
is also the sink BOND leaks into one-way (0.14/0.03), which reads as a definition
too broad. **BOND at 0.76 is the best-defined class on either axis** and is the
model for the rest.

### What was tried on the mode axis, and what ships (E24–E29)

| route | result |
|---|---|
| merge the confused modes (E27) | **worse.** At matched granularity, mode-merged-to-5 scores −0.187 against domain's −0.007 |
| build mode from the head word (E28) | **worse by −0.091 top-3.** Retracted — see the correction below |
| factorise the cell into domain × mode (E28) | **worse by −0.028.** The axes share signal and are not independent (E26) |
| a stronger embedder (E21) | mode +0.065, domain top-2 +0.003 (saturated). Helps, does not close |
| discover a better carve (E29) | **none exists here.** Clustering the blur recovers *subject matter*; the current four beat every discovered partition |
| **halve the hints, one joint proposer (E24/E28)** | **SHIPS. Top-3 0.705 vs 0.651, *t* = +12.13, better in 24/24 paired splits** |

**A correction kept visible, because it cost an experiment.** E27 recommended
building the mode axis from the head, since head-only halves mode's
*incoherence*. E28 measured the cost in *prediction*: −0.091. E25's own limits
had already said *"not a prescription to drop the modifier."*

> **Compression asks whether an axis is a coherent object. Held-out top-k asks
> whether a representation predicts. A diagnostic criterion is not an objective
> function.**

**Where mode stands.** An entry is a name plus subject-matter hints — what
something is *about*. Mode asks what is being *done*, and nothing in the corpus
ever observed anything being done. So the four definitions cannot be repaired
*from this corpus*, and the route left standing is the one that motivated the
supervision question: **entries observed in use, not described.**

## Roadmap

### Phase 1 — Technical Perfection (In Progress)

| # | Deliverable | Status | Owner | Notes |
|---|-------------|--------|-------|-------|
| 1.1 | Scanner overhaul: parallel WalkDir + streaming SHA256 | ✅ Done | — | rayon + 64KB buffer |
| 1.2 | Pipeline dedup: `run_ingest_pipeline()` | ✅ Done | — | 160 lines extracted |
| 1.3 | Disk indexer (`physis-pro index`) | ✅ Done | `src/indexer.rs`: stat fast path (re-hash only changed), sled catalog, `--watch`/`--interval`; e2e verified |
| 1.4 | Critical unwrap elimination | ✅ Done | — | `unwrap_or_default()` in VFS |
| 1.5 | CI/CD: cargo check + clippy + test | ✅ Done | — | 3 feature sets |
| 1.6 | Inventory command (sled DB) | ✅ Done | — | + SMA-30 forecast |
| 1.7 | Process command (TCP checks) | ✅ Done | — | Behind `industrial` feature |
| 1.8 | Real-time MQTT/Modbus adapters | ✅ Done | — | `src/industrial.rs`: `MachineAdapter` trait gains `publish()`; MQTT subscribe (rumqttc) + `publish` to `physis/<id>/cmd`; new `modbus::ModbusTcpAdapter` (tokio-modbus 0.17) with `ModbusMap` register decoding + `publish` writes coils/registers; `AdapterRunOptions` + `run_adapter`/`run_loop` driver emits normalized `OperationalEvent` JSON per poll, dedupes by state fingerprint, flags anomalies via `detect_anomalies`, optionally registers into an engine (`post_event`); CLI `physis-pro connect adapter --protocol mqtt\|modbus --endpoint … [--post-to … --temp-max … --once]`; 3 new tests (ModbusCommand parse, ModbusMap defaults, mock-loop `once`) |
| 1.9 | BGE-base embedder + self-test gate | ✅ Done | — | 768-d, MiniLM→RP fallback |
| 1.10 | Automated ontology discovery | ✅ Done | — | `discover` + `--ai` legacy path |
| 1.11 | Config dedup (`BUILTIN_ONTOLOGIES` table) | ✅ Done | — | 10 fns → 1 lookup |
| 1.12 | GPU support investigation | ✅ Done | — | ROCm EP dead *for ONNX Runtime* (2026-08-13). Not a fleet-wide fact: **omo does GPU inference today** via ollama/Vulkan (`OLLAMA_VULKAN=1`, `HSA_OVERRIDE_GFX_VERSION=10.3.0`, gfx1032 / RX 6650 XT), ~38 tok/s on a 9B against feather's 4.4 CPU-only. Research compute should go there — see `research/convergence/HANDOFF.md` §5 |
| 1.13 | Goals carry text + persist | ✅ Done | — | `label` field + sled per tenant |
| 1.14 | HTTP router test suite | ✅ Done | — | 11 in-process tests via oneshot |
| 1.15 | Live product demo (`/demo`) | ✅ Done | — | Seed+watch+classify, 11 HTTP tests |
| 1.16 | Study Log + Elenchus trainer (`/study`) | ✅ Done | — | 12 HTTP tests, per-tenant isolation |
| 1.17 | TUI wiki browser (WIKI tab) | ✅ Done | — | Meaning-ranked, graph walk, heatmap |
| 1.18 | Semiotic node editing (exchange parts) | ✅ Done | — | `POST /coherence/edit` + `/delete` |
| 1.19 | Fixed-token node search | ✅ Done | — | `POST /search/nodes`, budget-bounded |
| 1.20 | Watch-mode auto-ingest | ✅ Done | — | Background thread + `POST /watch` |
| 1.21 | Vault importers (Obsidian, git log) | ✅ Done | — | `src/vault.rs` |
| 1.22 | Personal-history importers | ✅ Done | — | `src/history.rs` |
| 1.23 | Praxis backfill | ✅ Done | — | `src/praxis.rs` |
| 1.24 | Spanner Graph sink | ✅ Done | — | `--features spanner`, off hot path |

### Phase 2 — Interface Mimicking Nature (Done 2026-08-24)

| # | Deliverable | Status | Owner | Notes |
|---|-------------|--------|-------|-------|
| 2.1 | `NodeView` + `observable()` exhaustive destructure | ✅ Done | — | `models.rs:244`; no `..` in destructure — new fields force a decision |
| 2.2 | Pins persist across edits; clearing is explicit | ✅ Done | — | Web `"cell": null` → `PinEdit::Clear` (tested); CLI `node-edit --clear-pin` |
| 2.3 | Pin disagreements feed `QualityTracker` automatically | ✅ Done | — | `web/coherence.rs`: dissent penalizes the auto cell, pins reinforce |
| 2.4 | Single projection for all surfaces | ✅ Done | — | Web module dropped its own DTO; TUI parses NodeView subsets |

### Phase 3 — User Experience (Backoffice Polish)

| # | Deliverable | Status | Owner | Notes |
|---|-------------|--------|-------|-------|
| 3.1 | Web-based Dashboard (React/Next.js) | ✅ Done (superseded) | — | React/Next framing dropped for the no-build Bioluminal Forge shell; the substance ships: `/api/v1/fs/graph` FileThemeGraph over HTTP + force-directed (cose) theme↔file visualization on the console File-map tab and `/ui`, with file preview, moves, similarity edges |
| 3.2 | Natural Language Interface (chat) | ✅ Done | — | `POST /api/v1/ask`: plain-English question → embedder-consistent retrieval over coherence+goal labels (token-packed) → grounded synthesis via the provider cascade with [n] citations, 300-token cap. Degrades gracefully: no provider / no hits / synthesis failure each return the raw ranked matches + an explicit note, never an error. Ask bar on every page renders the answer above the matches it is grounded in; pure `ask_prompt()` unit-tested (citation contract) + http_surface degradation test |
| 3.3 | Quality Dashboard UI | ✅ Done | — | `GET /api/v1/quality` enriched: per-failure records (embeddings stripped, `DOMAIN/MODE` display cells, newest first) + open/resolved/mean-severity aggregates; pure `quality_response()` builder, 4 unit tests + 2 http_surface tests. Console Quality tab (`#tab-quality`, unified Bioluminal Forge shell): KPI tiles, zero-filled 30-day trend bars, recent reports with severity/resolution/correction pills, penalty & boost bar lists |

### Phase 4 — Monetization

| # | Deliverable | Status | Owner | Notes |
|---|-------------|--------|-------|-------|
| 4.1 | Tiered licensing (Core / Industrial / Enterprise) | ✅ Done | Tier enum derived from the signed feature list (no separate field → no forged upgrades); banner shows tier; vendor tool `--tier core\|industrial\|enterprise` expands bundles; 9 tests incl. cross-tier denial (56341a8); offline enforcement was already done |
| 4.2 | Industry-specific ontology packs | ⬜ Pending | — | Aerospace, Food Processing, … |
| 4.3 | ROI-based pricing | ⬜ Pending | — | Quality failures prevented / inventory optimized |

### Phase 5 — Industrial Wedge

| # | Deliverable | Status | Owner | Notes |
|---|-------------|--------|-------|-------|
| 5.1 | CSV/JSON shift importer | ✅ Done | — | `src/shifts.rs`, partial acceptance w/ line-level errors |
| 5.2 | Customer-ready report metadata + evidence export | ✅ Done | — | `ReportMetadata` in md/html/json; `/api/v1/shifts/export` evidence bundle |
| 5.3 | Durable tenant/plant-scoped shift history | ✅ Done | — | sled per tenant, `/api/v1/shifts/*` + aggregated report |
| 5.4 | First paid pilot + protocol decision | ⬜ Pending | — | Before adding more adapters |

### Phase 5B — Interest Engine (Personal Vertical)

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| 5B.1 | Interest core: observations, decayed profile, sled store | ✅ Done | `src/interest.rs`, 7 tests |
| 5B.2 | Document analyzer: pptx/docx text+metadata, plaintext | ✅ Done | OOXML behind `daemon` feature |
| 5B.3 | Daemon binary: clipboard + watched dirs → loopback API | ✅ Done | `physis-interest-daemon`, port 19878 |
| 5B.4 | Semantic embedder in daemon (BGE via ONNX) | ✅ Done | cascade + self-test + seed cache; `--features daemon,embed-onnx` |
| 5B.5 | Console tab for the interest timeline | ✅ Done | console.html `#tab-interest`: daemon connect, decayed profile, recent events, offline help |

### Phase 8 — QA-Found Defects (2026-08-24 full-feature audit)

> Source: `docs/QA_VERDICT_2026-08-24.md`. Fix order = severity order.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| 8.1 | VFS re-ingest crash: dangling-symlink EEXIST (`orchestrator.rs:44` `.exists()` follows links) | ✅ Done | `symlink_metadata()` + stale-link repair/prune; 3 vfs tests incl. rename regression |
| 8.2 | `query` CLI finds nothing: trie process-local (`cli.rs run_query`) | ✅ Done | `run_query_nodes` recalls persisted coherence labels from sled; trie hits still shown first |
| 8.3 | RP-embedder score compression (0.88–1.00 flat, wrong top cells) | ✅ Done | BGE weights validated: CORE top-1 domain 0.891, HARD 1.000; floors recalibrated (2026-08-24 provenance); RP limits documented in README + CLI warning |
| 8.4 | `order` command stub — persists nothing (`main.rs:904`) | ✅ Done | `place_manual_order` w/ cooldown + sled order log; `inventory` shows history |
| 8.5 | `\u0000` literal in `quality` output cell names | ✅ Done | physis-core summary renders `DOMAIN/MODE`; regression test |
| 8.6 | rachmaninov WARN spam every 10s at sim=0.0000 on empty holarchy | ✅ Done | Zero-signal skip + ≥3-node threshold + existing rate-limit; 2 tests |
| 8.7 | timeline/machine input schemas undiscoverable | ✅ Done | `machine <name> --schema` (executable example, pre-licence); timeline long_about |
| 8.8 | Cell-level classify precision regressed: 0.636 vs 0.923 (2026-06-12 calibration) | ✅ Done | Root cause: smoothing diluted anchors post-expansion. Sweep (bge-base) → shipped max scoring: CORE cell 0.636→0.782, Italian 0.667→0.867 dom; `TopKStrategy` seam kept |


### Phase 9 — Repowise Pattern Adaptation (2026-08-24)

> Source: `docs/reference/repowise-adaptation.md`. Clean-room pattern
> re-implementations in Rust — repowise is AGPL, physis-pro is proprietary;
> **no code transfer, patterns only**.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| 9.1 | Session-transcript adapter registry (`src/sessions/`) | ✅ Done | `discover` + per-line `normalize` → None contract; claude_code/codex adapters; substring prefilter; 11 tests incl. real-transcript ground truth (`c7db9f6`) |
| 9.2 | Anti-hallucination grounding gate (`src/grounding.rs`) | ✅ Done | apply_gate(): ungrounded fields dropped, zero-evidence candidates rejected, Grounded/Partial verdict; 7 tests (5cf0893) |
| 9.3 | Decision mining → embedding dedup | ✅ Done | 7dcf91a core + 53d89b0 embedding dedup (dedup_by_embedding, intra-batch collapse, broken-vector guard) + decisions-mine CLI; 10 tests |
| 9.4 | Holarchy community detection (`label-propagation`, no new deps) | ✅ Done | core: deterministic label-prop + cohesion + read-only invariant, 6 tests (afc2a29); 9.4b: GET /api/v1/communities + Communities studio tab, live-verified on :19877 (5b5d4a7) |
| 9.5 | Wiki generation over the holarchy (`wiki-gen` feature) | ✅ Done | outline core: group_nodes + name_groups (9e86c27); 9.5b prose + tour (b047601): `render_page` deterministic skeleton + LLM summary injection, `execution_tour` Kahn order w/ cycle-safe fallback; 12 tests total |
| 9.6 | HTTP surface for wiki-gen | ✅ Done | POST `/api/v1/wiki/generate` (outline+pages+tour, cascade naming skipped when degraded/deterministic) + GET `/api/v1/wiki/tour`; empty-tenant explicit error; http_surface coverage test (d9b72a6) |
| 9.7 | Overview dashboard (`/pulse`) + main-server communities endpoint | ✅ Done | `GET /api/v1/communities` (`web/coherence.rs`: label-space rollup — resolved_edges ids mapped to labels before detection, edge-less labeled nodes kept as one singleton community, largest-first; pure `communities_payload` + 3 tests) + `/pulse` page (`src/bin/ui/pulse.html`, Bioluminal Forge + ui-shell): KPI ribbon, coherence distribution, dream-replay attention feed, dream proposals, communities grid, wiki execution tour, semiotic heatmap — repowise dashboard patterns per adaptation doc, thin read-only UI over existing endpoints; live-verified on :19899 |

### Phase 10 — Datawork: Space · Time · Cause (2026-08-26)

> Source: `docs/plans/2026-08-26-space-time-cause-datawork.md`. Filing axes the
> meaning grid cannot carry — where/when/why-made/what-for — as one
> `FileAddress` per file plus a causal ledger and two new ontologies.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| D1 | `FileAddress` on FileNode + scan/watch capture | ✅ Done | `src/address.rs`: SpaceRef/TimeRef/CauseRef + fs-metadata derivation (degrades to `now`, never fails ingestion); `FileNode.address` serde-defaulted so old graph JSON still loads; captured in `build_theme_graph`, demo CLI graph builder, `/api/v1/fs/graph` DTO (`place/period/modified/mechanism` fields); 4 unit tests + save/load round-trip incl. legacy-node coexistence |
| D2 | Space + time ontologies (`place`, `period`) | ✅ Done | `config/space_ontology.json` (22 places), `config/time_ontology.json` (16 periods); dedicated `space_domains`/`time_domains` buckets — deliberately NOT classification themes (never join classifier centroids); facet CLI parity via `--kind place\|period`; grid-cell validation tests |
| D3 | Causal ledger (sled, per-tenant) → OperationalEvent | ✅ Done | `src/ledger.rs`: CauseEvent (monotonic seq) + lineage/descendants/purposes walks; projects onto OperationalEvent pipeline under connector `cause` with replay-stable event_keys; sled rows keyed `ledger:{uid}:{seq}` (append-only, idempotent save); RuntimeState field, lazy tenant load + default-tenant startup load; watch thread records Generated/Corrected per file diff; 5 unit tests |
| D4 | Vault/history/praxis importers write addresses | ✅ Done | causal events per import run (not per doc): vault→`received(vault-import)` + `derived(git-log ×N)`, history→`received(bookmarks\|browser-history\|feeds\|chat-log)` by export type, praxis→`derived(praxis-backfill)`; /scan records `generated(scan)`; all persist through the same idempotent sled save |
| D5 | `where` CLI + `/api/v1/files/query` + /ui panels | ✅ Done | tenant-scoped POST endpoint composing space (`place` prefix) · time (`within_days`/`during YYYY-MM`) · cause (`mechanism`\|`actor`) · free-text facets, newest-first matches + by_place/by_period/by_mechanism rollups; `physis-pro where <query> [--place] [--cause] [--within N] [--during]` reads the persisted `_default` ledger serverless with ASCII rollup bars; console Timeline tab gains the Cause-ledger card (rollup pills + recent events, fetch-once + refresh); e2e test covers recording, all four facets, and tenant isolation |
| D6 | Purpose links (`serves` events) file↔goal | ✅ Done | `POST /api/v1/files/serve` validates the goal per tenant and records `serves(planner)` as final cause — ledger history, no parallel truth; `GET /api/v1/files/lineage?path=` returns upstream chain + descendants + purposes (Serves excluded from chains by construction); `GET /api/v1/goals/list` feeds the picker; console Timeline card gains the serve form + click-to-trace lineage; e2e: attach → facet query → lineage → 404 on unknown goal → tenant isolation |
| D7 | Mailbox import (email × datawork) | ✅ Done | `src/mails.rs` (feature `email`, now in default features so `.eml` scans parse instead of registering empty shells): walks export dirs — `.eml`/`.msg` per message, RFC-4155 `.mbox` split on sender lines with indexed sources (`file.mbox#3`), garbage chunks skipped; labels `mail:<subject> — <from>` embed subject+body with native Date parsed. `POST /api/v1/mail/import` (scan-root confined) registers nodes + records `received(mailbox)` per source file; e2e covers registration, cause trail via facet query, confinement |

**Phase 10 complete (D1–D7).**

### Phase 10b — Datawork surfaces (2026-08-26)

> Follow-ups found while landing D1–D7. All buildable now; no external deps.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| T1 | Timeline pipeline consumes the ledger | ✅ Done | `GET /api/v1/study/session` merges `to_op_events()` into the assembled timeline (chronologically interleaved with study events; connector-tagged via `properties.mechanism`), exposes a `causes` count, tallies/rhythms stay study-scoped; empty-ledger tenants unchanged (back-compat asserted); /study page renders merged view for free + mechanism chip in entry meta; e2e covers interleaving, counts, scoping |
| T2 | /ui place+time filters | ✅ Done | fs/graph already ships `place`/`period` per file (D1) — two facet selects on the graph FM header populated from the scan, combined with text search into one dimming pass, status line shows visible count; selections survive re-scans when still valid |
| T3 | Lineage as edges | ✅ Done | selecting a file node draws its why-made chain into the cose graph: rose round-diamond cause nodes (`mechanism·actor`) chained input→artifact with dotted arrow edges + lime `serves·goal` nodes for purposes; cleared on reselect/re-render, best-effort (never breaks selection) |
| T4 | Ontology pack mechanism (4.2-lite) | ✅ Done | `load_all` scans `{data_dir}/packs/*.json` (sorted, deterministic): entries join `custom_domains` keyed `pack-stem:name`, interpretants attributed `pack:<stem>`, broken packs skipped with a warning — packs are data, no rebuild. Example pack `docs/examples/logistics_pack.json` (9 entries); unit test covers load + skip + classification enrichment |

### Phase 10c — Adoption track (2026-08-26)

> Source: `docs/business/ADOPTION.md`. Make the agent-facing surface match
> what the product can actually do.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| A1 | MCP surface parity | ✅ Done | `src/mcp.rs`: 18 → 23 tools. `remember` (register labeled node + verdict into the shared sled graph), `recall` (token-packed semantic retrieval over nodes+goals, same corpus contract as /search/nodes), `why`/`serve`/`causes` (causal ledger: lineage+purposes, purpose links validated against agent goals, facet browse). Startup opens `{data_dir}/state.sled`, rehydrates default tenant, degrades to session-only memory if sled unavailable |
| A2 | llms.txt served per instance | ✅ Done | repo root → public route `/llms.txt`, capability coverage tested |
| A3 | Registry submission pack | ✅ Done | `docs/business/mcp-server-submission.md`; owner runs topics command (CI token lacks scope) |
| A4 | Crate published | ✅ Done | `physis_pro v0.1.10` live on crates.io |
| A5 | Classification reach fix | ✅ Done | root cause: `classification_domains()` chained only `human+custom`, so machine/agent/engineering/office/semiotic/category/natural/social/abstract entries (≈576) were authored and displayed but never scoreable — industrial text fell back to human cells. All nine domain buckets now chained (seeds 352 → 928); `place`/`period` stay excluded by design; regression test pins OEE/Error-Recovery reachability + filing-axis exclusion; accuracy_floor passes under the real embedder. physis-core's studio copy needs the same treatment in its own release |
| A6 | Output-quality validation | ✅ Done | `tests/output_quality.rs` (ignored probe, real bge embedder, production ONNX cascade) prints live output for classify/recall/ask/ledger/lineage/serve. Surfaced and fixed a second defect: cell exemplar entries returned in insertion order — core 0.1.10 now ranks them by their own similarity. Verified: OEE query fronts [OEE Metrics], agent-failure fronts [Agent Loop/Tool Invocation/Error Recovery], recall top-1 correct with wide margins, ask synthesizes correct `[1]`-cited answers via groq; accuracy_floor re-passes. Shipped as physis-core 0.1.10 + physis_pro 0.1.13 |

### Phase 11 — Corpus comprehension: summaries · structure (2026-08-27)

> Two limits top-k retrieval cannot fix by ranking harder. A question about the
> whole corpus ("what are the recurring themes", "what do we keep failing at")
> has no single node holding the answer, so no similarity ranking can surface
> it — rank *community summaries* instead. And flat chunking throws away the
> heading chain that makes a chunk mean anything — parse the document's own
> structure and make each section carry its ancestors.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| C1 | `corpus` module — community summaries + global packing | ✅ Done | `src/corpus.rs`: `MemberFacts` → `CommunitySummary` (TF-IDF keywords distinctive *across* communities, majority-vote facets with deterministic tie-break, representatives by keyword coverage, win/failure tally, template digest). Default path is offline and model-free, preserving the air-gapped guarantee; `pack_global` mirrors `rag::TokenFixedRetriever` (skip-not-truncate, so half a digest is never cited); `global_prompt` states the unit is a cluster, not a document. 15 unit tests |
| C2 | `doctree` module — hierarchical document index | ✅ Done | `src/doctree.rs`: parses ATX + setext headings (fenced code blocks skipped, so `# comment` in a shell snippet cannot shred the tree), YAML frontmatter skipped, every line lands in exactly one section, headingless documents degrade to one whole-document section. `Section::retrieval_text()` = breadcrumb + body, so the embedder sees the context the author implied but never repeated; `outline`/`max_depth`/`children` for navigation. 19 unit tests |
| C3 | Modules registered + surfaced over HTTP | ✅ Done | both modules were written but never declared in `lib.rs` (dead files, zero coverage). Now registered; `src/web/corpus.rs` adds `GET /api/v1/corpus/summaries` (offline, no provider needed) and `POST /api/v1/corpus/ask` (rank digests through the content-addressed embed cache → `pack_global` → grounded synthesis, degrading to ranked communities when no provider is configured). Facets come from where the graph records them: cell from `cell_pin`, verdict from `asserted`, mechanism from the causal ledger; `place`/`period` live on `FileAddress` and stay absent rather than guessed. Isolated nodes join one singleton community, same convention as the communities rollup, so nothing is invisible to a corpus-level question. 3 unit tests + e2e (coverage invariant, budget bound, citation resolution, tenant isolation) |
| C4 | doctree wired into retrieval (additive) | ✅ Done | `POST /api/v1/rag/retrieve` gains `unit: "chunk"\|"section"`. `"section"` parses each ingested document into its heading tree and embeds `breadcrumb + body` per section; the response gains `unit` plus a `sources` array parallel to `chunks` (file path + section path + heading level), so a hit cites "Spindle Assembly > Reassembly > Bearing preload" instead of an offset. Additive by decision: the default flat path keeps the same corpus, ids and vectors, so no cached embedding is invalidated. e2e pins the breadcrumb inside the embedded text, hit↔source alignment, and flat-path parity |
| C5 | CLI + MCP surface for corpus questions | ✅ Done | `physis-pro themes [--min-size N] [--json]` clusters the `_default` graph into communities and prints each summary (offline, lib `corpus::gather_facts`+`build_summaries` reused by the web handler); MCP `physis_themes` tool returns the same summaries (min_size filter) for agents |
| C6 | Console tab for summaries | ✅ Done | `console.html` `#tab-themes` under **Mean**: load-on-activate `GET /api/v1/corpus/summaries`, KPI tiles + one card per community (title, keywords, dominant facets, outcome pills, digest, representatives); refresh button; `.chips`/`.pill` styles added to `physis.css` |

### Phase 12 — Demo-grade GUI: content search · organic graph · ontology studio (2026-08-29)

> Three gaps the dashboards could not close by polish. Every search surface
> ranked **labels** — the names the graph already knows — so the paragraph
> nobody labeled was unreachable. Every graph loaded cytoscape from a CDN, so
> the air-gapped install this engine is sold into drew nothing. And the
> dictionary could be *read* but not *written*, which meant "add your own
> vocabulary" was a filesystem chore, not a product.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| 12.1 | `content` module + `POST /api/v1/search/content` | ✅ Done | `src/content.rs`: hybrid retrieval over **doctree sections** — BM25 (k1=1.2, b=0.75, idf never negative) fused with cosine, each channel max-normalized over the candidate set then weighted 0.55 semantic. Two channels because an industrial corpus is half codes and half prose: vectors treat `E4412`/`E4413` as near-identical, terms miss "cuscinetto grippato" for "seized bearing". Quoted phrases are required verbatim; snippet picks the densest window and returns **character** offsets (byte offsets land mid-codepoint on the first accented word). Facets (place/period/kind/file) count the whole match set, not the page. Index is built once and held on `AppCtx`; staleness is a metadata-only walk (size+mtime), so freshness is checked per request while read+embed happens once; embeddings served from the content-addressed `EmbedCache`. Without a semantic model the index is built lexical-only and the response says `semantic_index: false` rather than ranking random projections confidently. Place facet falls back to the folder relative to the scan root — the address's own `place` defaults to the root itself, which would be one value covering everything. 12 unit tests + 2 e2e |
| 12.2 | `/search` — investigation surface | ✅ Done | `src/bin/ui/search.html`: facet rail with proportion bars, ranked sections citing the heading chain, `<mark>`ed snippets, preview pane reading the section back off disk (with outline, and an explicit note when the file changed since indexing), mode segmented control, `j`/`k` navigation, stale-response guard. Status line states what actually ran — requested vs effective mode, index rebuild cost, missing semantic model. Ask bar now offers content search on every page instead of implying an empty corpus |
| 12.3 | Organic renderer + `/atlas` | ✅ Done | `src/bin/ui/organic.js` (served at `/organic.js`, **no CDN**): force simulation on a plain canvas — breathing amplitude from coherence, edge sway from weight, community blobs as radial fields rather than hulls (a hull asserts a boundary the clustering does not have), travelling pulses on recorded edges only. Layout seeded from a hash of the node id, never `Math.random`, so an unchanged graph lands in the same place and "did it change?" stays answerable. `GET /api/v1/atlas/graph` (`src/web/atlas.rs`) joins nodes + edges + communities server-side: recorded holarchy edges plus derived kNN similarity edges, **tagged and never merged**; label-propagation over both layers so register-only tenants are not a starfield; unconnected nodes share one community, matching the communities rollup. 5 unit tests + 1 e2e |
| 12.4 | Ontology studio write path | ✅ Done | `src/web/ontology_studio.rs`: list / read / save / delete packs under `{data_dir}/packs`, names validated (rejected, never sanitized — a `..` join escapes a sandbox and a silent rename loses a customer's pack), cells validated against the 70-cell grid, **all** errors returned at once. Writes `OntologyEntry`, not `DomainDef`: the loaded shape serializes `"axis_kind": null`, which the loader rejects — the studio would have written packs the engine skipped at startup, and the round-trip test caught it. Atomic write. `/studio` page: pack list with live/pending state, 70-cell density picker, hint editor, and a **before/after** panel that classifies real samples through `/api/v1/planner/simulate`. Deliberately no hot-swap of the live classifier (ontology + centroids are immutable in `SharedCtx`, read lock-free on the hot path); the surface says a pack goes live on restart instead of pretending. 5 unit tests + 1 e2e |
| 12.5 | Object explorer (Palantir parity) | ✅ Done | Atlas right pane: entity card (label, cell with auto-vs-pinned provenance, verdict, coherence, links, community, first seen), neighbour list with edge kind + weight, closest-by-meaning recall, causal lineage + purposes for file-backed nodes, and the actions that write back — assert succeeded/failed (idempotent re-register), pin/unpin a cell, attach a goal, hand the label to Ask or to content search |
| 12.8 | CLI honored `PHYSIS_EMBEDDER` | ✅ Done | Found by using 12.7: `physis-pro find` with `PHYSIS_EMBEDDER=random-projection` set still loaded the whole ONNX cascade and spent ~25 minutes embedding the ontology the operator had just asked it to skip — the variable was read by the web server and the interest daemon but never by `select_embedder`. An ignored instruction is worse than an unsupported one: the only symptom was the wait. One `wants_random_projection()` predicate now serves all three call sites (CLI selection, server selection, the server's DEGRADED decision), so the spelling of the request cannot drift between them; unit-tested on both documented spellings |
| 12.7 | Agent + CLI parity for content search | ✅ Done | `physis-pro find <query> [--dir --mode --place --period --kind --top-k]` (serverless: ingests, embeds through the persistent `EmbedCache`, prints citation + facets + snippet) and MCP `physis_find` (same corpus contract, `PHYSIS_SCAN_ROOT`-confined like `physis_pack`). Two honesty fixes fell out of using it: a vectorless index now reports `lexical` whatever was requested — saying "hybrid" would let a caller read paraphrase matching into a pure keyword result — and `citation()` drops the leading crumb that only repeats the filename (`policy.md › policy › Escalation`). The preview endpoint therefore resolves a section by exact path **or unique trailing suffix**, ambiguous suffixes matching nothing rather than guessing between two sections that both end in "Notes". 14 unit tests |
| 12.6 | `--features full` build repair | ✅ Done | Pre-existing break at HEAD: `mcp.rs` called `web::coherence::query_coherence_nodes` through a private module and read a `verdict` field `NodeView` does not have. Module is now `pub(crate)` (both surfaces must project through one function or they answer differently) and the tool returns `rating` + `asserted` |

### Phase 8b — Engine-side read-back wired to the boundary (2026-08-30)

> Closes the open thread flagged in the plan index: `reduce()` existed in Rust
> and no surface could reach it, so the machines' whole point — perceive there,
> mean here — stopped at the boundary.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| 8b.1 | `bridge_node` was being handed the label, not the id | ✅ Done | Found while wiring the endpoint. `chronos`, `euclid` and `empedocles` `reduce()` all called `core.bridge_node(&text)`; `bridge_node` looks the argument up in `core.nodes` and returns `None` for anything that is not an id, so **every node these reducers registered stayed unbridged** and the read-back loop silently never closed — the `Option` return made it a no-op with no error. All three now bridge the returned id, with a regression test asserting every registered node has a holon |
| 8b.2 | `machines::run::reduce` — one dispatcher for both surfaces | ✅ Done | `run()` is pure (JSON → JSON, touches nothing); `reduce()` is the other half (registers, bridges, reports). Both live in `run.rs` so HTTP and CLI cannot drift on what a machine *means*. Returns `ReduceOutcome { score, registered, nodes_before, nodes_after }` — the id set is diffed rather than guessed, so a repeat run reporting zero new nodes is legible as dedup rather than as failure. `REDUCERS` names the three machines with an engine-side half; asking any other to write is an **error, not a silent no-op**, because a caller who got `{"score": 0}` back could not tell nothing had been written. 2 unit tests |
| 8b.3 | `POST /api/v1/machines/:name/reduce` | ✅ Done | Tenant-scoped (a machine writes into the caller's graph, never a shared one), persists each new node through sled and **reports** the persisted count and any persist error rather than swallowing it — a reduce that looked like it worked and vanished on restart is the worst of both. Unknown machine → 404, stateless machine → 400 naming the ones that do write. `GET /api/v1/machines` now carries `writes` per machine so a client need not discover it from a 400. e2e covers registration, holarchy bridging via recall, dedup on repeat, tenant isolation, both refusals and the registry flag |
| 8b.4 | `physis-pro machine <name> --reduce` | ✅ Done | Same dispatcher, serverless: loads the persisted `_default` graph, reduces, writes back. The payload is parsed before app init because stdin can only be read once. Verified live: first run registers and persists, second reports "nothing new" with the dedup reason, stateless machines refuse with the same message as HTTP |

### Phase 10v — Space·Time·Cause verified end to end (2026-08-30)

> The second open thread from the plan index. D1–D6 each had their own unit or
> handler test; nothing walked the chain a customer actually walks.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| 10v.1 | End-to-end walk over a real scan | ✅ Done | One e2e drives scan → `/api/v1/fs/graph` (every file carries `place`, a 7-char `period` bucket, an fs-derived `modified` and `mechanism: generated`) → the ledger (`cause: generated` with rollups) → time facets (this month hits, `1999-01` misses) → goal → `/files/serve` → `/files/lineage` (chain non-empty, purpose present, and `serves` **absent from the chain** — a final cause is not a production step) → tenant isolation. Passed unmodified: the HTTP half of D1–D6 works as documented |
| 10v.2 | CLI `scan` recorded no cause | ✅ Done | The walk's server-less half did **not** work. `POST /api/v1/scan` recorded `generated(scan)`; `physis-pro scan` recorded nothing, so on an air-gapped install `physis-pro where` answered *"the causal ledger is empty — run a scan, watch a directory, or import a vault first"* to a user who had just run a scan — the one instruction the message gives could never work. Now both paths write the same event, keyed on the **canonical** path so `scan .` and `scan /abs/path` are one history rather than two. Extracted as a free `record_scan_cause(db, dir)` so it is testable without building a whole `PhysisApp`; unit test covers the event shape, the canonical object and that a second scan is a second producing act |

### Phase 13 — `/notebook`: user-composed dashboards (2026-08-30)

> Design: `docs/plans/2026-08-29-notebook-dashboard-editor.md`. Shipped
> complete: N1–N5, plus two defects the build itself surfaced (N6, N7).

Every surface today is a composition *we* authored, so giving a maintenance
lead their screen means editing HTML and shipping a binary; that is the wrong
cost for a layout question, and it is why Console keeps growing tabs. The
proposal is an ordered grid of cells the user adds, removes, reorders and
resizes, persisted per tenant in sled and shareable as JSON.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| N1 | Widget registry + server-side notebook validation | ✅ Done | `src/notebook.rs`: 12 widgets, each a thin reader over an endpoint that already exists. A cell carries a widget **id and params** — never HTML, JS or a URL — and the endpoint lives in the registry, which is code the server ships. That asymmetry is the guarantee: the registry is code, the notebook is data, so a shared notebook cannot make a recipient's browser fetch something of the author's choosing. `validate()` reports **every** problem at once (one round-trip per error is not an editor) and refuses unknown widget ids outright rather than storing them to skip at render — a stored reference nobody checks is how a payload eventually rides in. Undeclared params are refused too, so a cell cannot smuggle a field to the endpoint behind it. Geometry is a closed set (spans 3/4/6/12, height presets, 24-cell cap) and interval refresh has a 15s floor. 12 unit tests |
| N2 | Per-tenant persistence + CRUD | ✅ Done | `src/web/notebook.rs`: `notebook:{uid}:{id}` in sled behind GET list / GET one / POST save / DELETE. `localStorage` was rejected in the design: a notebook that lives in one browser cannot be opened on the machine wired to the projector, handed to a colleague, or survive a cache clear — the three things a composed dashboard is for. The path is the identity (a body claiming another id would write to a key the caller never addressed); persistence-off returns 503 rather than a success with nothing written; a tenant with no notebook gets the **starter composition**, not a blank grid or a 404. e2e covers save/list/read/delete, tenant isolation, both refusal paths, and that markup in a widget id is rejected |
| N3 | `/notebook` page — render an ordered grid | ✅ Done | Seven shared renderers (`kpis`, `bars`, `list`, `table`, `hits`, `feed`, `heat`); a bespoke renderer per widget is how a catalogue stops being cheap to extend. Numeric widgets declare **which fields** they plot — found by looking at the first render, where "verdict distribution" was plotting `cluster_count` and `dream_cycle_count` too, which is a dump, not a distribution. A retired widget renders an explicit card naming it; a failed fetch renders an error distinct from an empty result, because those two looking identical is most of the perceived brokenness in a dashboard. Each cell footer shows the endpoint and its latency |
| N4 | Editor: add / remove / reorder / resize / export / import | ✅ Done | Edit mode adds per-cell move/width/height/remove controls (buttons, not drag-only — drag fails accessibility and fails the shop-floor tablets), a picker that builds the param form from the registry, and rename/new/delete. Export copies the layout as JSON; import keeps the id being edited so someone else's layout cannot silently overwrite a different notebook of yours. Save failures render the server's full error list. One scheduler per interval cell, suppressed while the tab is hidden. Verified live: added a pinned `"no fault found"` search, saved, reloaded — composition persisted and the query answers live |
| N5 | "Add to notebook" from Search and Atlas | ✅ Done | `addToNotebook(widget, params, describe)` lives in `ui-shell.js`, not on each page: the direction that matters is the one nobody plans for — a query someone ran once, kept, and now sees every morning — so any page that can name a widget and its params gets the affordance, including pages not written yet. It read-modify-writes the whole notebook through the same save endpoint the editor uses, so a pinned cell is validated exactly like a hand-built one; there is no second, laxer path into a notebook. Search pins the query **with its active filters** (so the panel answers the same question tomorrow, not a broader one); the Atlas entity panel pins a standing search for the selected node. Verified live from both surfaces; page-serving test pins the wiring |
| N7 | XSS on the import path (found by security review) | ✅ Done | `data-span` interpolated a cell field into an attribute unescaped. Server validation did not cover it: **Import renders a pasted layout before it ever reaches the server**, and a shared layout is precisely the input an attacker controls — a span of `6" onload="…` broke out of the attribute. Two fixes, because either alone is a half-measure: the sink now coerces (`Number(c.span)`) and escapes (`esc(c.height)`), and `sanitizeImported()` **rebuilds** each cell field-by-field from the registry rather than filtering a copy, so anything the format does not describe is dropped by construction rather than by a blacklist. What it drops is reported, never silently repaired. Verified against a hostile document (attribute-breaking span and height, markup as a widget id, undeclared params, markup in the title): nothing executed, no handler attribute reached the DOM. A CI test pins the sink so it cannot regress without a browser in the loop |
| N6 | Quoted phrases mark as one span | ✅ Done | Fallout from N4's first live pinned search: `"no fault found"` highlighted as three separate words. A phrase and each of its words match at the same offset and the renderer keeps the first of an overlapping pair, so ascending order handed the mark to the single word — the phrase the user asked for verbatim, shown as three coincidences. `snippet()` now orders longest-span-first at a shared start; fixes `/search` too |

### Phase 14 — Monitor mode: the phone as a perception device (2026-08-30)

> Full design: `docs/plans/2026-08-30-monitor-mode.md`. A day leaves traces
> nobody writes down — the accelerometer knows there was a workout, the GPS
> knows it was the gym, the clock knows the fifth Tuesday did not happen. None
> of it reaches the graph, so the engine reasons about documents while the life
> those documents are *for* stays invisible.
>
> This is small because the split it needs is the one Phase 8b closed: **device
> perceives, engine means**. Every reducer already exists — `kinesis`→`soma` for
> motion and effort, `topos` for recurring places, `chronos` for rhythms and the
> ones that are slipping, `empedocles` for readings against a baseline. The
> engine work is a mapping layer and widgets, not new concepts.
>
> **Pro adds synthesis, never capture.** Normal mode is the deterministic
> reducers (they are the product); Pro is the provider cascade phrasing them. A
> Pro tier that can *see* more than Normal is two products and a user can trust
> neither.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| M1 | `monitor` module — sensor batch → machine primitives | ✅ Done | Pure and tested: the sensor→machine mapping is a product decision and belongs in the engine, not re-implemented in every client |
| M2 | `POST /api/v1/monitor/ingest` | ✅ Done | One call per sync, not one per machine — a phone on mobile data should not make four round trips to record one morning. Routes to the reducers, records causes |
| M2b | Observations are remembered, not only rhythms | ✅ Done | Found by the first e2e: the graph held the workout, the place and the readings but **no lesson**. `chronos` keeps *rhythms* — three Tuesdays become "lesson every 7d" and one Tuesday becomes nothing — which is right for chronos and wrong for a monitor, where "there was a 50-minute conversation at 14:00" is the fact the day was worth noticing. Each observation is now also remembered on its own, with the timestamp **in the label** because registration dedupes by label: without it Tuesday's lesson and Wednesday's collapse into one node and the second day looks like it never happened. The bare label stays bare so chronos can still match it across days |
| M3 | Notebook widgets over what M2 produced | ✅ Done | One `coherence.facet` **graph-query** widget (label / verdict / limit) rather than four bespoke monitor panels. Each of `workout`, `place:`, `rhythm:`, `conversation` would have been a thin filter over the same endpoint, and four registry entries to keep in step with label formats that will change; one widget plus a saved param is the same panel with none of that. Widgets, not a page: "show everything" is a wall, and a wall is not *derived from state by construction* — the notebook already lets each person compose the panels their life actually has |
| M4 | Android foreground service: sensors → batched events → M2 | ✅ Done | `clients/physis-monitor/` — Kotlin, **builds** (3.1 MB debug APK, verified: zero Play Services or Firebase classes linked). A foreground service and not a background job because the honest version of this app is the one the system shows is running. `TYPE_STEP_COUNTER` (hardware, already counting) rather than polling the accelerometer; `LocationManager` at 5 min / 50 m rather than the fused provider; first step read establishes a baseline and reports nothing, so a fresh install cannot claim the day's steps as one window's workout. Samples queue in memory and are dropped once the engine confirms them — the phone is a sensor, not an archive, and an archive on the phone is a second copy of a life to lose. A failed sync **keeps** its batch, which is safe precisely because the engine's translation is pure and registration dedupes by label. Every source is off until switched on and asks its own permission at that moment; the reboot receiver restores only what the user had chosen. Dependencies: `androidx.core` + `androidx.appcompat`, nothing else — `HttpURLConnection`, `org.json` and `LocationManager` are AOSP |
| M5 | Audio features with **no retention** | ✅ Done | The one piece with real legal exposure. Recording yourself is your business; a lesson or a call captures *other people*, which leaves the GDPR household exemption and engages art. 615-bis / 617 c.p. The liability is in storing audio and the value is in the inference, which does not need it: on-device RMS / spectral flux / VAD / speaker-count on a rolling buffer discarded in the same pass, never PCM on disk. The event is "conversation, 2+ voices, 50 minutes, at the university" — the audio was never the point. Shipped as `AudioProbe.kt`: one `ShortArray` allocated once and overwritten per ~62 ms frame, loudness + zero-crossing rate + an adaptive noise floor, recorder released *before* the result is returned. The guarantee is structural — the file contains no `File`, no `OutputStream`, no `Uri` and no network call, verified by grep over the source. Voice count comes from turn-taking and returns **null when the window gives no honest basis for a guess**, because "you met someone" recorded on no evidence is worse than a gap |
| M6 | Distribution: Play + direct APK from git | ✅ Done | **Owner decision:** F-Droid dropped because it requires every dependency to be free software. Recorded correction: F-Droid's constraint is *licensing*, Play's is *review*, and Play reviews background location and microphone — this app's two most sensitive permissions — far more harshly. The useful consequence is that **none of this needs Play Services** (`LocationManager`, `SensorManager`, `AudioRecord` are AOSP), so building without them costs nothing, ships on Play, ships from git, and leaves F-Droid open if review turns out to be the harder road. Pro mode points at the user's own instance, which is the honest answer to any store's "where does the data go". Shipped: `PRIVACY.md` (what leaves the phone per source, and why the microphone design is what it is), `RELEASE.md` (build, signing, direct-APK checksums, the **complete Play data-safety answers**, the background-location justification, the microphone disclosure, and the review friction to expect), `README.md`. Release builds are deliberately **not minified** so an APK published from git can be diffed against the source that claims to produce it |

### Phase 15 — Monetization v2 (2026-08-30)

> Source: `docs/plans/2026-08-30-productization.md` + `docs/COMMERCIAL_MODEL.md`.
> The repo was trying to be three products at once. Split into two — MIT **Core**
> (distribution funnel via MCP/agents) and EULA **Pro** (service-led pilots →
> Industriale/Enterprise licensing) — and drop the stale 4-tier SaaS / per-job
> metering (`BUSINESS.md`, the old `pricing.html`) that describes a product this
> repo is not.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| 15.1 | SKU → feature-slug entitlement table + order-form mapping | ✅ Done | `Tier::sku()/from_sku()/ALL` + `ENTITLEMENTS` table + `entitlement_for()` in `src/license.rs`; vendor tool `physis-license list-skus` prints the order-form table; `/settings` and `physis-pro license` expose the SKU. The licence still stores only features — the SKU is presentation, so a forged upgrade stays impossible |
| 15.2 | License `inspect` / `info` + trial-key generator | ✅ Done | `physis-pro license [--json]` (instant — dispatched before `PhysisApp::new`); vendor tool `sign --trial` mints a 14-day eval (TRIAL- prefixed id, maintenance included, traceable) |
| 15.3 | Key rotation + revocation | ✅ Done | `LicenseError::Revoked` + `check_revoked()` enforced in `license::load()` against a drop-in `revocations.json` (`PHYSIS_REVOCATIONS_FILE` override); vendor tool `revoke --id X [--list]` writes the sorted list to ship to air-gapped hosts. Malformed lists fail open (a vendor typo must not brick installs); key **rotation** stays out of scope — the module documents that rotating the vendor key reissues every licence |
| 15.4 | Customer/plant/host identity in report metadata | ✅ Done | `ReportMetadata` gains `customer` (from licence org, else `unlicensed`), `plant` (`PHYSIS_PLANT`), `host` (`PHYSIS_HOST`/`/etc/hostname`), `version` (build). `identity()` fills every report via `new`; rendered as an evidence line in shift-report markdown + HTML. 1 unit + 1 e2e test |
| 15.5 | Pricing page rewritten to the offer ladder (Pilot €1.5–2.5k → Industriale €1,500/mo → Enterprise €5,000/mo) | ✅ Done | `/` landing + `/pricing` + `/about` rewritten to the service-led model and routed publicly; supersedes the per-job SaaS tiers |

### Phase 16 — Integration spine (2026-08-30)

> Normalize every source into one typed, timestamped `OperationalEvent`, promote
> the substrate to a typed property graph, add the temporal/audit layer, and bind
> dream output to outbound actions. Each step is additive — the embed/coherence/
> dream engine only reads `.coherence`/`.embedding`, so nothing breaks.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| I1 | Canonical `OperationalEvent` (connector, mechanism, actor, ts, payload) | ✅ Done | `src/operational.rs`: kind/status/provenance + stable `event_key` (Sha-256) for dedupe; `From<&MachineReading>` + `From<&IngestedDoc>`; ledger-mechanism mapping |
| I2 | Adapters as `SourceAdapter` emitting `OperationalEvent` | ✅ Done | industrial adapter `post_event` now emits the full structured event (was a flattened register); machine/doc adapters normalize via `From` impls |
| I3 | One `post_event` ingestion path | ✅ Done | `POST /api/v1/events` (`src/web/operational.rs`): dedupe by `event_key` (idempotent), bounded per-tenant ring (`op_events:{uid}`, ≤10k, oldest dropped), causal-ledger `record_at`, optional coherence-node registration. Adapter change confirmed the "100 lines" claim |
| I4 | Provenance survives every projection (timeline, ledger, report, lineage) | ✅ Done | events persisted per tenant and rehydrated on startup/`with_user`; `GET /api/v1/events` (newest first) + `GET /api/v1/events/timeline` (the shared `OperationalTimeline::from_events` pipeline); ledger CauseEvent carries actor/object/observed_at into lineage/purposes |
| I13 | **Scan v2 — hierarchy tree + subtree-skipping diff + one node per doc** | ✅ Done | `src/tree.rs`: persistent per-root `DirTree` (dir layout + per-document mtime/size/content-hash/labels) with Merkle subtree fingerprints; `meta_walk` is stat-only (no content reads) and `diff` skips whole unchanged subtrees. `DirTree::scan_run` (shared by `/scan` and **watch-mode**) loads the previous tree → stat-walks → diffs → hashes+extracts ONLY changed/new docs (`process_file`, downgrading mtime-bumped-but-same-content to unchanged) → embeds one node **per document** (path-independent structural text → identical content anywhere is computed once by the content-addressed cache) → deletes removed. A document is scanned at most once. Watch-mode now uses the tree too (per-doc Generated/Corrected cause events preserved; old per-dir hash cache removed). 25 tree unit tests + 2 e2e (idempotent re-scan, one-node-per-doc) |
| I5 | `CoherenceNode` gains `type` + `properties` (default-empty, back-compat) | ✅ Done | `src/models.rs::CoherenceNode` + `NodeView` gain `kind` + `properties` (`serde(default)` → legacy rows load as untyped). Exhaustive `observable()` destructure forces the projection to carry them. `CoherenceNode::with_kind/with_properties/property` builders. `/coherence/register` accepts `kind` + `properties` (dedupe-by-label updates them); the event ingestion stamps kind=source-family + properties from the event. 2 unit + 1 e2e test |
| I6 | Named link types + inverse traversal (replace bare `semantic_type: u32`) | ✅ Done (coherence-graph layer) | `PhysisCore::add_typed_edge(source, RelationType, target, conf, w)` (named vocabulary: Supports/Contradicts/Causes/Influences/Precedes/Follows) + `edges_from` (forward) + `edges_to` (inverse traversal). HTTP: `POST /api/v1/coherence/add-edge`, `GET /api/v1/coherence/edges?node_id=` (both directions, tenant-scoped, unknown relation/endpoint → 400). The low-level repr(C) holarchy `Edge.semantic_type: u32` is left intact — it is serialization-critical and its callers pass a generic `1`; the named vocabulary lives on the `TypedEdge` layer where I5's typed axis sits. 1 e2e test |
| I7 | 70-cell grid → object types (Machine, WorkOrder, SensorReading, Invoice) | ⬜ Pending | |
| I7 | 70-cell grid → object types (Machine, WorkOrder, SensorReading, Invoice) | ✅ Done | `models::object_type_for(domain, mode)` — a **total** resolver over the real 5×14 grid (HEAL/CONSTRUCT/FABRICATE/BOND/STUDY × 14 modes): mode refinements beat the per-domain default (e.g. FABRICATE·SENSE → `sensor_reading`, FABRICATE·PLAN → `work_order`, FABRICATE·WORK → `machine`). `OBJECT_TYPES` vocabulary + `is_valid_object_kind`. Surfaced: `/classify` response gains `object_type`; `/coherence/register` auto-stamps `kind` from the winning cell when untyped (no second embedding). 3 unit + 1 e2e test |
| I8 | Dream output → typed actions (open WorkOrder, schedule maintenance) | ✅ Done (decide half) | `models::ActionKind` + `TypedAction` + `typed_action_for(node_kind, id, outcome, prevented)` — a replayed failure node proposes one concrete action: machine/asset → `schedule_maintenance`, work_order/sensor → `open_work_order`, document → `review_document`, person/goal → `escalate`, resolved → `acknowledge`. `/dream/replay` entries now carry `typed_action {kind, object_id, object_kind, summary}` — agent-dispatchable, not free text. Act half (dispatching the action) is I11/I12. 3 unit + 1 e2e test |
| I9 | Append-only mutation log (node/link/verdict) | ✅ Done | `src/audit.rs` `MutationLog` — sealed per-tenant rows `{seq, at, op, subject_id, subject_kind, actor, detail}`, capped ≤10k, sled `audit:{uid}` (whole-list, resumes seq across restarts). Hooked: register/assert, edit, delete, edge:add. `GET /api/v1/audit` newest-first + `?node_id=` change history + `?as_of=RFC3339` time-travel slice. 5 unit + 1 e2e test |
| I10 | Time-travel + audit view | ✅ Done | audit slice by `as_of` (`GET /api/v1/audit?as_of=`) answers "what changed when OEE dropped"; point-in-time node *existence* already served by `/coherence/nodes/all?as_of=` (`created_at <= cut`). Deeper full-state reconstruction from the log is future work — the log is the replayable source for it |
| I11 | Dream countermeasures → webhook/CSV export | ✅ Done | `POST /api/v1/actions/dispatch` — the **act** half of I8: delivers typed actions to `PHYSIS_ACTIONS_WEBHOOK` (air-gapped site leaves it unset → honest `configured:false` + note, never a silent failure); per-action delivery status + audit `action:dispatch`. `GET /api/v1/dream/replay.csv` exports the same replay as RFC-4180 CSV for spreadsheets/workflows. 3 e2e tests |
| I12 | ERP/MES/ticketing bindings | ⬜ Pending | only when a named customer names their system |

### Phase 17 — UI/UX honesty (2026-08-30)

> The 10 product pages (Pulse…Demo) share the Bioluminal Forge shell and are
> strong. The customer-facing pages are orphans — unrouted, using CSS classes
> that no longer exist, describing a "pay-per-job API platform", with broken JS
> and dead `/api/v1/my/*` endpoints. This phase makes the customer surface stop
> lying about the product.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| U1 | Landing (`/`), pricing, about rewritten to the real model + design system | 🔨 In progress | this change |
| U2 | `customer_dashboard.html` → license/deployment status page over real endpoints | ⬜ Pending | no billing backend exists; stop faking one |
| U3 | Remove or fold `api_tester.html` into Agent/Studio | ⬜ Pending | duplicates Notebook + Search affordances |
| U4 | Route `/`, `/pricing`, `/about` publicly; keep `/pulse` as product home | 🔨 In progress | |
| U5 | 10-minute demo: `prospect.csv → shift report + ontology studio` | ✅ Done | `physis-pro demo <dir> [--demo]`: reads every shift `*.csv` → `report.html`+`report.md` (Italian OEE report, per-machine losses, evidence-grade identity line) and runs the ontology studio (`studio.html`) over the same folder. `--demo` seeds a sample `shifts.csv` + demo corpus idempotently (real data never overwritten). Verified end-to-end against a vendor-stamped build + trial license |
| U6 | `--screenshot` mode / per-release capture pipeline | ✅ Done | `scripts/screenshots.sh`: boots the web server (real ONNX model required — marketing shots need semantics; fail-loud otherwise), captures `/`, `/pricing`, `/about`, `/deploy`, `/settings`, `/pulse`, `/console`, `/search`, `/studio`, `/demo` with headless chromium at 1440×900 into `shots/`, and writes a reviewable `shots/index.html` contact sheet. Verified end-to-end (10 valid PNGs). Local loopback capture opts into `PHYSIS_DEV=1` |
| U7 | Re-audit pages against the offline/WCAG floors in `physis.css` | ✅ Done | Audit found the system already compliant: palette contrast ≥ ~6.5:1 (AA+), `:focus-visible` on every interactive element, `prefers-reduced-motion` kills all animation, nav/ask-bar carry `aria-label`, nav sets `aria-current`. Added the one real gap: **skip-to-content** (WCAG 2.4.1) — `ui-shell.js` injects a `.skip-link` on every page (product + marketing) targeting the first `<main>` |

### Phase 18 — Feature testing (2026-08-30)

> The engine is unit-tested (759 lib tests) but tests are per-phase, not
> funnel-shaped. Add cross-surface tests for the journey a customer actually
> walks, and prove the air-gapped moat in CI.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| T1 | E2E funnel: scan → shift report → quality → dream → `where` → lineage | ✅ Done | `funnel_scan_report_quality_dream_where_lineage_is_tenant_isolated` — the whole customer journey in one test under the offline embedder: scan (file → labeled node + ledger `generated` event) → shift report (`/api/v1/shifts/report?format=md` renders OEE + provenance) → quality (hot reading vs thresholds → anomaly + audit) → dream (failing typed node → typed action) → where (`/api/v1/files/query?cause=scan` → `by_mechanism.generated`) → lineage (`/api/v1/files/lineage?path=` shows the scan cause) — plus tenant-b sees an empty ledger/events/audit. Also corrected the 15.4 test to hit the shift-history report (`/shifts/report`) instead of the live-board one |
| T2 | Adapter-contract test: stub `OperationalEvent` inherits dedup+anomaly+ledger | ✅ Done | `POST /api/v1/events` now takes `thresholds` and anomaly-checks accepted **machine** events (temperature/vibration/rpm from event properties via `reading_from_event` → `detect_anomalies`); each hit is audited (`anomaly` op) and returned in the response. e2e proves the I4 "new adapter ≈ 100 lines inherits dedup + ledger + anomaly + timeline" claim end-to-end (hot reading flagged + audited; cool reading not) |
| T3 | Typed-object round-trip (type+props survive persist/edit/delete) | ⬜ Pending | verifies I5 back-compat |
| T4 | Temporal-log replay + audit view | ⬜ Pending | verifies I9–I10 |
| T5 | Marketing-page smoke: `/`, `/pricing`, `/about` 200 + no external assets | ✅ Done | `marketing_pages_serve_expected_copy_without_external_assets`: `/`, `/pricing`, `/about`, `/deploy` serve 200 with the real product copy (not the stale "API platform" framing), load the shared `/physis.css`, and fetch no external CSS/JS; `/settings` serves its own copy. The air-gap rule the product obeys is asserted on the marketing pages too |
| T6 | Entitlement matrix (each SKU = exactly its slugs, cross-tier denial) | ✅ Done | `entitlement_matrix_enables_exactly_its_bundle` iterates all 3 tiers against the full feature universe: every bundle feature granted, every non-bundle feature denied (no over-grant), SKU↔tier consistent with the signed bundles |
| T7 | Air-gapped test: no weights, no network → lexical path, no 500, no CDN | ✅ Done | `air_gapped_path_degrades_honestly_without_weights_or_network`: classify 200 + names `random-projection` model; embed 200 with dim; extract 200 with slots OR an explicit degradation note — never 500, never fabricated. (The whole http_surface suite runs under the random-projection embedder as the air-gap worst case; `/ask` degradation already covered separately) |
| T8 | Un-ignore `output_quality` on a scheduled (non-push) job | ✅ Done | `.github/workflows/output-quality.yml`: daily `cron` + `workflow_dispatch` job that caches/fetches BGE-base (same contract as the accuracy-floor job) and runs `cargo test --features full --test output_quality -- --ignored --nocapture`. The heavy review-oriented probe stays off the push path; the accuracy floor remains per-push |

### Phase 19 — Agent surface (2026-08-30)

> **Thesis:** Physis is not "LangChain + Semantic Router" — it is the layer both
> are missing: routing that learns, budget-bounded grounded retrieval, and
> memory that carries verdicts. The harness owns the loop; Physis owns the
> semantics, served over MCP + HTTP. This phase closes the four gaps agents
> expect (route table, structured output, streaming/tool schemas, tracing) and
> ships a thin SDK. **Non-goals:** no LCEL/chains, no integration catalog, no
> vector-store zoo — composition stays data, and the harness stays out of repo.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| P1 | Declarative route table (`src/routes.rs` + `src/web/routes.rs` + MCP `physis_route`) | ✅ Done | `Route { name, cell, action, examples }` as JSON data in `{data_dir}/routes.json` (composition-as-data, same rule as the notebook registry). `GET/POST/DELETE /api/v1/routes`, `POST /api/v1/routes/match` (classify → cell → routes, deterministic), MCP tool. Validation reports every problem at once; routes persist before `ok`. 6 unit tests |
| P2 | Structured output / slot extraction (`src/extract.rs` + `src/web/extract.rs`) | ✅ Done | `POST /api/v1/extract {text, schema}` — schema-constrained extraction through the cascade; tools-first, plain JSON prompt fallback, explicit `note` on failure (never a fabricated object). 4 unit tests (fenced/prose JSON, garbage, schema in message) |
| P3 | Streaming + tool schemas | ✅ Verified | `run_agent_stream` + `POST /api/v1/agent/run` → SSE of `AgentEvent`s already shipped (web/agent.rs); `ProviderCascade.complete(.., tools, ..)` already takes `ToolDefinition[]`. Nothing to build — recorded so the claim is checked, not assumed |
| P4 | Thin SDK (`clients/physis_client.ts`) | ✅ Done | Zero-dep fetch client mirroring the MCP surface: health/settings, classify/ask, routes CRUD + `route`, `extract`, `recall`. Field names match the wire (`token_budget`/`top_k`) |
| P5 | Quality-failure trace (`GET /api/v1/trace?failure_id=`) | ✅ Done | Wired the previously-dead scaffold: resolves the failure from `QualityTracker`, joins the tenant's causal-ledger events within ±1h (tenant-scoped), returns failure + ledger trail. Removed the dead `display_cell`/`TraceFailureRequest` |

### Phase 6 — Administration / Datawork

| # | Deliverable | Status | Owner | Notes |
|---|-------------|--------|-------|-------|
| 6.1 | Fixed demo corpus + Admin Review Desk flow | ✅ Done (core) | demo corpus existed (studio --demo); review_desk.rs: run_desk() exceptions queue (missing fields, low confidence, fingerprint duplicates) + markdown report, 4 tests (ac62f29); HTTP surfacing = 6.1b done (f78c372 POST /api/v1/admin/desk) |
| 6.2 | CSV/XLSX batch import with validation | ✅ Done (CSV core) | batch_csv.rs: per-row validation (missing fields, ISO dates, EU decimal amounts), status/error codes, reviewed export round-trip; 4 tests (a05267a); HTTP surfacing done (f78c372 POST /api/v1/admin/batch); XLSX via existing calamine = remainder of 6.2b |
| 6.3 | Invoice/PO/delivery-note reconciliation | ✅ Done (core) | reconcile.rs: 3-way match keyed on vendor+PO, qty/amount tolerance ±2%/€0.05, duplicate-invoice flagging, multi-note delivery accumulation, markdown report; 10 tests (1965427); no payment automation per plan |
| 6.4 | Weekly management-pack templates | ✅ Done (core) | management_pack.rs: render_weekly_pack() — headline metrics w/ provenance, exceptions table, reconciliation status + actionable detail, notes; deterministic output; 3 tests (348b7b5); HTTP surfacing done (f78c372 POST /api/v1/admin/pack); HTML variant = 6.4b if needed |

### Phase 7 — Decision Planner

| # | Deliverable | Status | Owner | Notes |
|---|-------------|--------|-------|-------|
| 7.1 | Version ontology/overlay scenarios | ✅ Done | — | `/planner/simulate` compares candidate ontologies without mutating live state |
| 7.2 | Coverage gaps + affected reports/workflows | ✅ Done | `ImpactAnalysis::derive`: newly covered/uncovered, per-cell deltas (affected aggregates), mean confidence delta; stored on every version |
| 7.3 | Dated signifier shifts as reviewable hypotheses | ✅ Done | — | `web/scenarios.rs`: changed items saved as dated `hypothesis:true` shifts |
| 7.4 | Scenario review + promotion + version history | ✅ Done | — | save/list/promote per tenant; reversible; console planner tab renders versions + coverage ± with promote buttons |

---

## Key Decisions

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-08-18 | `data_dir` defaults to `./.physis` (CWD) | Portable across dev/prod |
| 2026-08-18 | Loopback auth requires `PHYSIS_DEV=1` | Prevent accidental exposure |
| 2026-08-13 | ROCm feature deleted | ONNX Runtime removed ROCm EP upstream; dead config |
| 2026-08-13 | BGE-base stays production | BGE-M3 lost on CORE accuracy, 2.5× slower |
| 2026-06-12 | Active vertical = industrial backoffice | Only vertical with a named prospect |
| 2026-06-12 | Edge adapters wait for customer | No adapter guessing; customer names machines |

---

## Key Metrics

| Area | Metric | Target |
|------|--------|--------|
| Classification (text) | Top-1 accuracy on curated test set | >0.85 |
| Classification (image) | Top-1 zero-shot on held-out photos | >0.80 |
| Graph throughput | Node mutations/sec | >10,000 |
| Dream quality | Human eval acceptance rate | >0.6 |
| Sync latency | P95 MCP broadcast to N instances | <500ms |
| Embedding latency | P99 ONNX embed (text) | <50ms |
| Embedding latency | P99 CLIP image embed | <200ms |
| Embedding latency | P99 Jina embed (text) | <100ms |
| Ontology density | Entries per cell (mean) | >25 |
| Content recall (TUI) | Forgotten-item found in top-5 | >0.85 |
| Recall latency | Keystroke → re-ranked results | <150ms |

---

## Work Packets — the multi-year skeleton (board 2026-09-11)

The executable plan for this repo lives in **`packets/`** (canonical, versioned
in git): `packets/PROTOCOL.md` governs how agents work (claim → gate → report →
commit; one packet per agent; the gate is the spec), `packets/README.md` lists
the repo red lines. `/home/gio/plans-staging/physis-pro/packets` is the
2026-09-01 deployment snapshot from feather and is **superseded** by this
directory — it lacks PH-014..PH-016 and carries a pre-claim PH-000.

Status was re-measured 2026-09-21 (newest dated note wins; the 2026-09-20 and
2026-09-11 tables below are earlier measurements) — three new packets, all
`review`, all with a reproducible gate in their report:

| id | phase | status | one-line |
|---|---|---|---|
| PH-024 | 0 | review | `physis-hud` is the one front door: views (world/graph/notebook/tasks/agents/settings/setup/help), editable settings that write only changed keys, `physis-hud --setup`, one knob table for all three TUI panels |
| PH-025 | 0 | review | guided execution Phase A: `src/guided/` compiles the world (capabilities, permissions, object state) into the model's allowed language; `physis-pro guided "<intent>"`; exact facts bypass the model with zero calls |
| PH-026 | 0 | review | the world surface: `WorldObject` ported as a read-only view over `nav::ObjectId` (edges carry `Relation.origin`), `src/hud/` library surface, projection switches that cannot move the focus, capability-derived palette, `--query` for headless slice frames |
| PH-027 | 0 | review | the world surface becomes operational: bare text resolves through the intent chain with its route on the status line, five producer-less routes deleted, `SemanticLens` + structured projection rows shared by two lenses, honest score-leg names (`vector`), `--no-semantic`, `affordances_for_identity`, descend/ascend over repository objects, `:actions`; deferred work is PH-029 (reducer ownership for navigation, structural symbols) and PH-028 (prepopulating the world) |
| PH-029 | 0 | review | reducer owns navigation, symbols become real objects: `MoveCursor`/`Select`/`ClearSelection`/`FocusObject`/`Descend`/`Ascend` routed through `dispatch()→reduce()→apply_hud_effects()`; key loop no longer writes `sel` directly; Rust qualified-symbol extraction (`pub fn`/`pub(crate)`/`async`/`unsafe`/`impl<T>`/`impl Trait for Type`) with negative fixtures; `back()` ordering bug found and fixed (capture focused before `Ascend` emits `SetCursor{0}`); remaining duplicate state documented as `selected`/`FocusState.selection` bridge |
| PH-028 | 0 | review | prepopulate the world: first-indexed the repo on pansa (16 750 objects, 469 s, 111 MB); `NavIndex.embedder_stamp` (`kind:dim`, serde-defaulted); Option A re-tag (`open_aliased`/`export_store`); Option B portable bundle (`export_bundle`/`import_bundle`) with embedder-stamp refusal; Option C remote embed offload (`PHYSIS_REMOTE_EMBED`, `RemoteEmbedder`, `+remote:<host>` label always shown); CLI `physis-pro nav export/import/retag-store/stamp` |

Reconciliation note (2026-09-21): `HEAD..main` is **0** commits on
`agent/PH-025`, and `src/world_object.rs` does not exist on `main` — the
WorldObject prototype lives on the unmerged `local-main-import` branch. PH-026
ported the view type instead of merging `main`.

Gate evidence: `packets/reports/PH-024.md`, `packets/reports/PH-025.md`,
`packets/reports/PH-026.md`, `packets/reports/PH-027.md`,
`packets/reports/PH-028.md`, `packets/reports/PH-029.md`
(`cargo clippy --all-targets -- -D warnings`; `cargo test --features tui --bin
physis-hud` 45 passed; `cargo test --lib hud::` 43 passed; `cargo test --lib
nav::` 25 passed; `cargo test --lib world_object::` 13 passed; `cargo test --lib
scanner` 6 passed; headless matrix 9/9 rc=0 LEXICAL ONLY).
Sequencing authority for the HUD programme:
`docs/plans/2026-09-20-physis-hud.md`.

Status was re-measured 2026-09-11 on `research/perspective-invention`:

| id | phase | status | one-line |
|---|---|---|---|
| PH-000 | 0 | review | baseline hygiene: clippy green, tree clean |
| PH-001 | 0 | claimed | three one-line drift fixes: version, env typo, feature set |
| PH-002 | 0 | todo | `install.sh` is broken as written |
| PH-003 | 0 | todo | licence claims contradict each other (human decides) |
| PH-004 | 0 | todo | semantic self-test does not discriminate |
| PH-005 | 1 | todo | feedback loop inflates confidence without accuracy |
| PH-006 | 1 | todo | pro-tier gating (decision packet) |
| PH-007 | 1 | todo | `pack` incremental index (9m59s cold) |
| PH-008 | 1 | todo | embedder registry: image slot vs active embedder |
| PH-009 | 1 | todo | test-suite speed (790 s full gate) |
| PH-010 | 1 | todo | release engineering: tag → signed package pipeline |
| PH-011 | 1 | todo | docs staleness sweep |
| PH-012 | 2 | todo | core API stability contract |
| PH-013 | 2 | todo | security review pass |
| PH-014 | 0 | done | release and build integrity |
| PH-015 | 1 | todo | content-search flake under full-suite concurrency |
| PH-016 | 0 | review | clippy gap under `--features industrial` (resolved by de-featuring) |
| PH-100 | D | todo | discovery: write the next packets from evidence |

Next up after PH-001: **PH-002 → PH-003 → PH-004** (phase 0), then PH-015
(eats the known test flake that otherwise poisons every full gate).
*(Superseded — all four have moved since; see the dated notes below.)*

> Re-measured 2026-09-19 (PH-011 sweep; the 2026-09-11 rows above are history,
> not rewritten): PH-002 done, PH-003 done, PH-005 done, PH-006 blocked
> (PH-001 features decision missing), PH-007 done, PH-008 done, PH-009 done
> (suite 790 s → 105 s), PH-010 done (tag pending go-ahead), PH-011 in
> progress, PH-014 done, PH-016 done, PH-017–PH-020 done, PH-021 review,
> PH-022 review, PH-023 todo (gate unmet, no local NLI server). Next in
> queue: PH-011 → PH-012 → PH-013 → PH-015.
>
> Re-measured 2026-09-20 (the 2026-09-11 table and the 2026-09-19 note above are
> history, not rewritten): **PH-001 review** (report written — it had sat
> `claimed` nine days with the work committed and unreported), **PH-015
> review** (root-caused: not a flake — a misclassified `invoices.md` ingested
> with an empty body, fixed in `f383789`, guarded and tested 2026-09-20),
> **PH-021 done**, **PH-022 done** (both gates hold in full; the two
> `notebook.rs` clippy errors they cited are gone), **PH-023 released**
> (depends PH-021), **MS-001 done** after independent review with amendment
> A1, **MS-002/MS-003 released**. Still blocked: PH-006 (waits the human
> features decision), MS-004–MS-008. Still claimed: PH-004. Next eligible:
> MS-002 / MS-003 / PH-023, then PH-013, then the recurring PH-011 sweep.
>
> Re-measured 2026-09-20 (later the same day; the notes above are history, not
> rewritten): **PH-004 done** — the claim is discharged and independently
> reviewed (a second session re-ran the full gate on a fresh build; the
> implementation files are byte-identical `5e074ce`..`e4b5d12`). The
> discriminating probe, the `selftest` subcommand and the `/health` pins were
> already committed; what was missing was a test carrying `semantic_self_test`
> in its name (`cargo test semantic_self_test` matched **zero** tests and
> exited 0), the real-weights arm's silent skip, and the CLI exit-code
> contract. All three added; full gate green. Report:
> `packets/reports/PH-004.md`. An abandoned, non-compiling `src/web/mod.rs`
> WIP from an earlier session was preserved to `/tmp` and reverted — it
> duplicated logic HEAD already had.
>
> Also 2026-09-20: **MS-002 done** — independently reviewed (citations re-checked
> at both `ed8dc29` and `54b8a8d`, A1 confirmed binding, zero-production-caller
> and zero-instrumentation claims confirmed). The report's deferral of the
> runnable mutation matrix is accepted under §Checks' report-only clause, and
> the criterion is carried by new packet **`MS-002-harness`** — built the same
> day and now `review`: `physis-core/tests/replay_contract.rs`, 7 arms green,
> gate run twice, the negative arm proven to fail.
> **MS-003 returned to `todo`** — its report's acceptance-gate claim (wrong
> dimension names, only two hard-required) and its PH-051 citation (4/5
> transferred, not 0/5) are wrong; four citation fixes required, no
> re-execution.

## Plan Index & Reconciled Status

Single entry point for *what* each planning document covers and *where it
stands. Detail docs are not duplicated here — they are linked. Status is
reconciled against the live code (verified 2026-08-28).

| Plan document | Area | Status | Notes |
|---------------|------|--------|-------|
| `PLAN.md` (this file) | Master roadmap | Living | Phase tables 1–7 + 10/11; authoritative status for shipped work |
| `docs/plans/2026-08-29-notebook-dashboard-editor.md` | Phase 13 — `/notebook` composable dashboards | ✅ Shipped | N1–N5 built 2026-08-30; registry-as-references, per-tenant persistence, pinned queries |
| `docs/plans/2026-08-30-monitor-mode.md` | Phase 14 — monitor mode (mobile perception) | 🔨 In progress | Engine slice M1–M3 here; Android client M4–M6 is a separate repo over HTTP |
| `docs/plans/2026-08-30-productization.md` | Phase 15–18 — monetization v2 · integration · UI/UX · feature testing | 🔨 In progress | Pro/Core split, offer ladder, saleability gates, `OperationalEvent` spine, temporal log, customer-page rewrite, funnel tests |
| `PLAN.md` Phase 19 | Agent surface (route table, extract, SSE, SDK, trace) | ✅ Done (P1–P5) | Harness out of repo; MCP + HTTP are the contract |
| `docs/plans/2026-08-26-space-time-cause-datawork.md` | Phase 10 — Space·Time·Cause | ✅ Shipped + verified | D1–D6 (+ D7 mail) built; see `AGENTS.md` Space·Time·Cause section. End-to-end verified 2026-08-30 (Phase 10v) — HTTP chain passed as documented, CLI `scan` was found recording no cause and fixed |
| `docs/plans/2026-07-15-phase8-engine-readback.md` | Phase 8 — engine-side reduce | ✅ Done | Tasks 1–6 done (2026-07-16); the residual boundary wiring shipped 2026-08-30 as Phase 8b — `POST /api/v1/machines/:name/reduce` + `machine --reduce`, and the bridge-the-id fix that made the loop actually close. `cargo test --lib machines::` = 56 pass |
| `docs/plans/2026-07-16-ecosystem-next.md` | Ecosystem rescue + Praxis/Agora | ✅ Done (2026-07-16) | A–E complete; Python `Machine` base verified pure-perception; Praxis→physis recall wired; Agora ToS pivot documented |
| `docs/archive/DEVELOPMENT_PLAN.md` | Semiotic editing / NodeView / pins | 📚 Archive | historical |
| `docs/archive/DEVPLAN.md` | Industrial MVP / admin / decision planner | 📚 Archive | historical |
| `docs/archive/LEAN_I40_PLAN.md` | OEE / SPC / A3 / JIT | 📚 Archive | historical |
| `docs/archive/INDUSTRIAL_VERTICAL_PLAN.md` | BGE-M3 eval / MQTT / shift report | 📚 Archive | historical |
| `docs/archive/ONTOLOGY_EXPANSION_PLAN.md` | 70-cell grid / holonic graph | 📚 Archive | historical |

### Open threads (reconciled)
- ~~**Phase 8 boundary wiring**~~ — **closed 2026-08-30** (Phase 8b above).
  Both surfaces now reach the engine-side `reduce()`. Wiring it surfaced a
  defect that had made the loop a no-op even in Rust: all three reducers
  bridged the node's *label* instead of its id, so nothing they registered ever
  entered the holarchy.
- ~~**Space·Time·Cause verification**~~ — **closed 2026-08-30** (Phase 10v
  above). The HTTP chain passed unchanged; the check earned its keep on the
  CLI side, where `scan` had never recorded a causal event, leaving `where`
  permanently empty on a server-less install.

---

## Hardening Backlog — External Evaluation (2026-08-31)

An end-to-end evaluation was run against the built binaries rather than the
documentation: licence gate, embedder cascade, classification, the SECOM
benchmark, `pack` retrieval, the HTTP surface, and CI configuration. What
follows is ordered by commercial risk, not by effort. Each item names the
evidence that produced it so it can be re-checked.

### H1 — Fail closed when the licence public key was not stamped in

`src/license.rs` falls back to a hardcoded development public key when
`PHYSIS_LICENSE_PUBKEY` is absent at build time. That key
(`3b6a27bc…8b59da29`) is the Ed25519 public key of the all-zeros seed, so its
private half is guessable by anyone without repository access. A licence signed
with a 32-byte zero key was minted and accepted by a `cargo build --release`
binary as "Enterprise tier, 1 seat, expires 2027-12-31".

`scripts/package.sh` does require the variable, so official packages are not
affected — but any binary built the ordinary way carries no enforceable gate,
and one such binary reaching a customer or a demo host removes the premise of
the Pro layer.

Fix: when `option_env!("PHYSIS_LICENSE_PUBKEY")` is `None`, compile a build
that refuses to start unless `PHYSIS_ALLOW_DEV_LICENCE=1` is also set, and
print an unmistakable "DEVELOPMENT BUILD — licence gate disabled" banner on
every run. Keep the dev key usable for tests; make it impossible to ship by
accident.

**Status — CLOSED (2026-09-07).** `license.rs` compiles `DEV_KEY_BUILD`
(`option_env!`-based) and `enforce()` exits 2 on a dev-key build unless
`PHYSIS_ALLOW_DEV_LICENCE=1` (exact string; `true`/`yes` don't count), printing
the dev banner on every allowed run. Licences are still signature-verified
against the embedded key in dev mode — a minted-with-foreign-key licence is
refused either way. Verified end-to-end with a local binary: zeros-key-signed
"Enterprise" licence without the env → exit 2; with the env → runs with the
dev banner; foreign-key licence → signature refused in both modes.

### H2 — Make the semantic self-test actually discriminate

`embed::semantic_self_test` compares "a photograph of a dog" against "a
photograph of a puppy" and "quarterly corporate tax accounting spreadsheet".
The related pair shares four of its five tokens, so any bag-of-words hash
passes. The random-projection embedder passes it, and `/health` and
`/api/v1/embedder` consequently report `"semantic": true, "degraded": false`
while the CLI simultaneously warns that the same embedder is non-semantic.

Measured through `/api/v1/embed` on the random-projection embedder:

| pair | cosine |
|------|--------|
| `car` / `automobile` | 0.401 |
| `car` / `banana` | 0.525 |
| "the spindle motor overheated and tripped" / "thermal fault shut down the drive" | 0.843 |
| "the spindle motor overheated and tripped" / "invoice payment terms net thirty days" | 0.823 |

Fix: replace the probe with lexically disjoint synonym pairs and require a
margin over a distractor set — for example `car`/`automobile` must beat
`car`/`banana` by a fixed margin, over several pairs. Any embedder that fails
must set `degraded: true` on every health surface. This is the guard PREMORTEM
P4 was written for and it currently does not guard.

**Status — CLOSED (2026-09-07).** The probe is now three lexically disjoint
pairs (`car`/`automobile`, `dog`/`puppy`, spindle-overheat/thermal-fault) each
required to beat its distractor by 0.05 (`SEMANTIC_PROBES`, embed/mod.rs).
Unit tests pin both directions: the random-projection embedder must fail the
probe, and a guard test keeps the pairs disjoint so the overlap bug can't
return. The `SelfTestCache` key now carries a probe version
(`SEMANTIC_PROBE_VERSION`), so verdicts recorded under the lax probe are
invalidated. Verified against the real BGE model: the accuracy floor still
passes (CORE 0.873 / cell 0.727 / ITALIAN 0.867, model=bge-base), and the web
surfaces' existing `semantic`/`degraded` logic keeps its contract
(offline-by-request → semantic=false, degraded=false).

### H3 — Never degrade the embedder silently

`OnnxConfig::model_dir` defaults to the relative path `./models`, so the same
command classifies correctly from the repository root and falls back to
random-projection from any other working directory, with no error — only the
one-line warning. Same input, from two directories:

- from repo root (bge-base, dim 768): `FABRICATE×MAINTAIN — 0.840`, nearest
  entry "Machine Maintenance" — correct.
- from elsewhere (random-projection, dim 384): `BOND×GUIDE — 1.000`, nearest
  entries "Mentor and Lead", with the next four cells at 0.997–0.995.

Fix: resolve the model directory against the binary location and the data
directory, not the working directory; add `PHYSIS_MODEL_DIR`; and make a
downgrade to random-projection an error unless `PHYSIS_EMBEDDER` explicitly
requested it. A silent fallback that produces confident nonsense is worse than
a refusal.

**Status — MOSTLY CLOSED.** `embed::model_dir` already searches
`PHYSIS_MODELS` → `MODEL_DIR` → `./models/<name>` → `<data_dir>/models/<name>`
(embed/mod.rs), and the web server aborts startup on an implicit RP downgrade
unless `MODEL_BASE_URL` is set or the operator asked for offline mode
(web/mod.rs). Remaining gap: the CLI still degrades to RP with only a warning
instead of refusing; `PHYSIS_MODEL_DIR` is spelled `PHYSIS_MODELS` here.

### H4 — Stop the feedback loop inflating confidence without accuracy

`QualityTracker::adjust_score` computes `raw × (1 − penalty + boost)` clamped
to `[0,1]`. Boosts are per-cell and input-independent, so a boosted cell is
promoted for every subsequent input; the clamp then destroys ordering once
several cells saturate at 1.0. The SECOM run with the ONNX embedder shows the
effect exactly: mean adjusted score rises 0.523 → 0.939 while accuracy stays
at 0.700 and `top1_changed` is 0.

In an industrial QA setting this is the worst possible failure shape — the
system becomes more confident and no more correct. Fix: cap `cell_standing` so
boosts cannot saturate (for example bound the multiplier to `[0.5, 1.2]`),
and report raw and adjusted scores separately everywhere so a boost is visible
as a boost rather than as evidence.

### H5 — Replace the SECOM benchmark or stop citing it

The embedded sample is 21 observations with 10 features (real UCI SECOM is
1567 × 590). The default run splits to a 10-row test set: 60% accuracy with the
random embedder, 70% with ONNX, against a 61.9% majority-class baseline. At
that size the number carries no information, and the report's own
"Interpretation" section says feedback had no measurable effect.

Fix: either wire `--dataset <path>` to the full UCI CSV and report accuracy,
precision/recall on the rare FAIL class, and a majority-class baseline
alongside; or remove the benchmark from any customer-facing claim. A weak
benchmark that is quoted is a liability; one that is not quoted is dead code.

### H6 — Gate Italian accuracy on the model that actually ships

`tests/classification.rs` gates Italian top-1 domain accuracy at 0.80 only for
multilingual models (`jina-v2`, `bge-m3`). Jina is deliberately excluded from
the auto-cascade and bge-m3 is 2.2 GB, so the model that actually loads is
`bge-base-en-v1.5`, which is English-only and ungated. The test's own comments
record its Italian history: 0.733 → 0.867 after Italian anchors → **0.667**
after the 2026-08-14 ontology expansion diluted nearest-entry scoring.

The product is sold in Italian, to Italian plants, with an Italian shift
report as the pilot deliverable. Two of every three Italian queries landing in
the wrong domain is a first-meeting-ending defect. Fix: pick one multilingual
model as the shipped default and gate on it, or add Italian anchor entries
back until bge-base clears a stated floor — and gate at that floor regardless
of model family. Also make `accuracy_floor` **fail** rather than `return` when
no model is on disk, so a skipped download cannot produce a green CI run.

### H7 — Give `pack` an incremental index

A cold `physis-pro pack src "…"` over 189 files / 2238 chunks took 9m59s wall
and 5164s CPU. Retrieval quality was excellent — the top three chunks were
`bin/license_tool.rs:81-120`, `license.rs:641-680`, `bin/license_tool.rs:41-80`
for a query about Ed25519 licence verification — but no agent will wait ten
minutes, and the pitch for this tool is that it runs locally with no model API.

The warm path depends on `EmbedCache`, which is a single JSON file loaded in
full and rewritten in full on every invocation, with no eviction. It reached
33 MB after two runs on `src/`. At 768 f32 per chunk in JSON text (~15 KB), a
100k-chunk customer corpus is a 1.5 GB parse per invocation.

Fix: move chunk vectors into the existing sled store keyed by content hash,
mmap or stream them, evict by age, and reuse the `index` catalog so unchanged
files are never re-chunked. Then publish a cold and warm number.

### H8 — Repair the front-door install path

**RESOLVED by PH-002.** The script previously pointed at a non-existent web
binary and wrapped it in the Python gateway; it could not succeed as written.
`install.sh` now installs the real `physis-pro*` binaries to `~/.local/bin` and
config to `~/.physis`, is idempotent, needs no sudo, and supports `--dry-run`.
`scripts/package.sh` remains the documented signed customer-install route.

### H9 — Reconcile the licence claims on customer-facing pages

`pricing.html`, `about.html`, `index.html` and `customer_dashboard.html` state
that the core is MIT and that `physis_pro` is "dual-licensed — MIT for the
crate API". `physis-core/Cargo.toml` declares `license = "Apache-2.0"`, and
the root `Cargo.toml` carries a comment explaining that `physis_pro` has no
SPDX id because `LICENSE.txt` is a proprietary EULA. Any enterprise legal
review will find this before it finds anything else. Pick the true statement
and make all four pages say it.

### H10 — Small correctness debt

Re-measured 2026-09-11 (packet PH-001); all three items below have moved:

- ✅ Version: the handler already reads `env!("CARGO_PKG_VERSION")`
  (`src/web/mod.rs`); the drift fix landed in an earlier session. A test now
  pins it (tests/http_surface.rs asserts the `/health` body version against
  the same macro).
- ✅ Log variable: the deeper truth was that *no* code reads
  `PHYSIS_LOG_LEVEL` — the web server reads `PHYSIS_LOG` (EnvFilter syntax).
  `.env` (had the `PHICS_LOG_LEVEL` typo) and `DEPLOYMENT_OMO_PANSA.md` now
  both name `PHYSIS_LOG`; the filter contract is pinned by unit tests
  (`log_filter_from_env`). The old `.env` was later purged (it had been
  committed with a live key) and is gitignored, which left the variable
  undocumented for a fresh deployer; **closed 2026-09-20** — `.env.example`
  now carries `PHYSIS_LOG=info` with the trap named, and the comment there
  deliberately does not reproduce the typo token, because PH-001's own gate
  is `! grep -rn 'PHICS_LOG_LEVEL' .env*`.
- ⬜ Features — **measured 2026-09-20, decision still open (rung 5)**: the
  premise is stale — `embed-onnx` **is** in `default`; only `mcp` is out.
  Adding `mcp` costs +37,625,272 B on the debug binary (486,490,640 →
  524,115,912; +35.9 MiB, +7.7%) and +31 s on an incremental build, and what
  it gates is the legacy `serve` listener plus five web sites — the MCP
  surface this fleet actually uses, `system serve`, ships in the default
  build and answers an `initialize` handshake. Recommendation: keep `mcp` out
  of `default`. Analysis and commands in `packets/reports/PH-001.md`; this is
  PH-006's blocked input and only a human marks it decided.

### H11 — Scope discipline

The surface is 181 distinct HTTP routes, 46 CLI subcommands, 29 MCP tools and
18 named machine reducers, built by one person in 101 days. The reducers are
real algorithms — `chronos` is a median inter-arrival rhythm detector with
next-due prediction and a regularity×evidence confidence, which is exactly
maintenance-interval mining — but nothing in the naming reaches a plant
manager. Roughly 15.3k lines sit in industrially sellable modules (OEE, shifts,
maintenance, monitor, inventory, reconcile, report, machines, ingest) against
75.3k in `src/` overall.

Fix: pick the pilot path — ingest → OEE → anomaly → Italian shift report — and
freeze everything outside it. Rename the reducers on every customer-facing
surface to what they compute (`chronos` → "recurring-event detector"). Keep the
Greek names internally if they help; they cost sales externally.

### What the evaluation confirmed works

Recorded so the backlog above is not mistaken for a verdict on the whole system.

- OEE decomposition in `src/oee.rs` is textbook-correct (availability =
  run/planned, performance = ideal_cycle × total / run capped at 1, quality =
  good/total, world-class at ≥0.85) and depends on no embedder at all. The
  pilot deliverable is the part that does not carry model risk.
- With bge-base loaded, classification is genuinely good on real industrial
  text, and `pack` retrieval put the right three chunks first.
- Security defaults fail closed: the web server refuses to bind loopback with
  no token unless `PHYSIS_DEV=1`, RBAC is three-tier with method-derived
  defaults, and filesystem handlers canonicalize under a scan root.
- CI runs `clippy --all-targets -D warnings`, downloads the real model under
  `set -euo pipefail` for the accuracy job, and exercises the Spanner sink
  against the official emulator.
- The licensing infrastructure is real work: Ed25519 envelopes, tier derived
  from the signed feature list, revocation lists, artifact signing,
  clock-rollback detection.
- Comments throughout record the failure that motivated the code rather than
  restating it. That is the single strongest maintainability signal in the
  repository and it should survive any refactor.

### Sequencing

H1, H2 and H3 are one focused block on trust in the engine's own status
reporting and should land together — each is a case of the system claiming
health it does not have. H6 and H4 are the accuracy block and gate the first
pilot. H7 gates any agent-facing sale. H5, H8, H9 and H10 are pre-sales
hygiene. H11 is a standing constraint, not a task.

---


### Phase 20 — Ontology node visualization: fragments, instances, origin links (2026-09-02)

> **Goal:** every ontology node in the Atlas/Studio visualizations must cite the
> exact text fragment it was derived from, list ALL instances of that concept
> (conceptual and concrete), and provide hyperlinked-clickable references back to
> each origin so the user can "search for them in their origin."

**Current state:** `NodeView` (the projection every surface uses) carries `id`,
`label`, `score`, `asserted`, `cell`, `pinned`, `created_at`, `kind`, `properties`
— but NO source fragment, NO instance list, NO origin URI. The `properties` bag
is a `serde_json::Map<String, Value>` (Phase 16 I5) and is the natural place to
store this, but it is not populated with provenance references during ingestion.
`OntologyNode` (the typed graph) has `props: HashMap<String, String>` but also
lacks fragment/instance/origin fields. `ContentHit` (content search) already
carries `citation` + `path` + `snippet` — the fragment-citation machinery exists
in the search layer but is NOT surfaced at the ontology-node level.

**Gaps:**
1. `CoherenceNode.observable()` -> `NodeView` does not include `source_fragment`,
   `instances`, or `origin_url`. Adding fields here forces every surface to
   decide (the exhaustive-destructure rule), which is the point.
2. `OntologyNode` (ontology_graph.rs) has no `instances` list — concept nodes
   from `merge_discovery` carry `count`/`samples` strings, not structured
   instance references.
3. No origin URIs (file paths, line ranges, section paths) are attached to
   nodes at registration time.
4. The Atlas UI (`atlas.html`) object explorer shows label, cell, score, links,
   and "In documents" — but that search uses `node.label`, not the actual
   fragment the node maps to. It does not list instances of a concept.

**Plan:**

| #  | Deliverable | Est. Effort | Notes |
|----|-------------|-------------|-------|
| 20.1 | `NodeView` gains `source_fragment: Option<String>` (most relevant text snippet, <=256 chars), `instances: Vec<InstanceRef>`, `origin_url: Option<String>` (clickable URI) | Med | Exhaustive destructure in `observable()` — every surface must opt-in |
| 20.2 | `InstanceRef` struct: `{ id, label, kind: "conceptual"|"concrete", origin: String (file:line or URI), fragment: String (<=128 chars) }` | Small | Conceptual instances are other ontology nodes; concrete are coherence nodes pointing here |
| 20.3 | `OntologyNode` gains same fields — `instances` populated during `merge_discovery` from `samples`/`hints`, each sample becomes an `InstanceRef` | Small | |
| 20.4 | Ingestion path writes `origin_url` + `source_fragment` into `CoherenceNode.properties` at registration | Med | `properties["origin_url"]`, `properties["source_fragment"]` |
| 20.5 | Atlas UI explorer: node detail panel gains "Source fragment" + "All instances" sections with hyperlinks | Med | JS: `openEntity()` already exists; extend the panel HTML |
| 20.6 | `CoherenceNode` backfill: one-time migration populates `source_fragment` from `label` for nodes where missing | Small | `properties` already persisted; backfill is additive |
| 20.7 | MCP tool `physis_node_instances` — given node label, returns all InstanceRefs | Small | Reuse MCP surface pattern from Phase 19 |

---

### Phase 21 — Multimodal ingestion pipeline: OCR, MP3, WAV, image, video (2026-09-02)

> **Goal:** extend the ingestion pipeline (`src/ingest.rs`, `src/content.rs`) to
> accept OCR'd PDFs/documents, MP3/WAV audio, images (CLIP embedding), and video
> (frame-sampled CLIP), reducing all to the shared text+vector space that feeds
> classification, the holarchy, and causal ledger.

**Current state:**
- Images: `src/embed/clip.rs` has `ClipEmbedder` with `embed_image()` (ONNX CLIP
  vision encoder + tokenizer), gated behind `--features embed-onnx`. `embed/mod.rs`
  wires it as the `image` embedder in `EmbedderRegistry`. The `/classify` and
  `/embed` endpoints accept base64 `image` in the request body (per AGENTS.md
  "Multimodal & Sensory"). **BUT** `embed-onnx` is documented as BROKEN
  (ort 0.13 API drift) — the CLIP embedder compiles but the semantic path is
  not buildable in this tree (physis-pro-dev SKILL, "embed-onnx feature is BROKEN").
- Audio: `src/bin/voice.rs` (`physis-pro-voice`) does arecord -> whisper ->
  llama3.2 extraction -> `OntologicalMap`. `ai/provider.rs` has
  `ProviderCascade::transcribe()` (Whisper via Groq/OpenAI). `monitor.rs`
  has `SignalType::AudioPCM` and records speech *duration* as an event, but
  explicitly does NOT store the audio (privacy by design). The interest daemon
  (`physis-interest-daemon`) has clipboard + watched-dir observation but no
  audio ingestion.
- Video: AGENTS.md says "planned: frame-sample -> embed_image -> temporal
  pool. Not built." `embed_onnx.rs` has no video path.
- OCR: `ingest.rs` has `extract_pdf()` using `pdf-extract` (text-layer only).
  For scanned PDFs (image-only), `pdf_extract::extract_text` returns empty and
  the code falls back to raw bytes as lossy UTF-8 — **no OCR pass**. No
  `tesseract`, `pdfium`, `mupdf`, or OCR crate dependency exists in Cargo.toml.
- Docx/pptx: `interest.rs` handles via `ooxml_analysis` (zip-of-XML). This is
  feature-gated behind `daemon` feature, not in the default ingest path.

**Gaps:**
1. No OCR for scanned PDFs — only text-layer extraction. Scanned docs silently
   produce empty/garbage bodies.
2. No audio ingestion pipeline — `voice.rs` is a streaming daemon, not the
   batch ingestion path. `ingest.rs` has no `DocKind::Audio`.
3. CLIP image embedder exists but is unbuildable (ort 0.13 API drift).
4. No video ingestion — frame sampling + temporal pooling not implemented.
5. No unified multimodal type in `ingest.rs` — `DocKind` enum is text-centric
  (Invoice, Email, Spreadsheet, VoiceTranscript, Report, Unknown).

**Plan:**

| #  | Deliverable | Est. Effort | Notes |
|----|-------------|-------------|-------|
| 21.1 | Fix `embed-onnx` feature so CLIP vision encoder builds — either pin `ort` to a compatible version or update API calls | Med-High | Blocked on ort 0.13 migration; see PREMORTEM.md P0 |
| 21.2 | `DocKind` adds `Image`, `Audio`, `Video`, `ScannedPDF` variants | Small | Update `classify_file()` extension matching |
| 21.3 | OCR backend: integrate `tesseract` or `pdfium-render` for scanned PDFs | Med | `pdfium-render` has a pure-Rust path; `tesseract` is a system dep. Pick one. |
| 21.4 | Audio ingestion: `extract_audio_transcript()` using `ProviderCascade::transcribe()` — MP3/WAV -> Whisper -> text | Med | Reuse existing Whisper path in `ai/provider.rs`; `DocKind::Audio` |
| 21.5 | Image ingestion: frame-extract from video -> CLIP embed -> average pool | Med | Depends on 21.1; sample 1 frame/5s, temporal average |
| 21.6 | Video ingestion: `extract_video_frames()` -> CLIP embed each -> temporal pooling | Med | Depends on 21.1; use `ffmpeg` CLI or `image`/`ndarray` |
| 21.7 | All multimodal bodies flow through the same `doctree` heading parser + `ContentIndex` so content search covers them | Small | Additive — `section_units()` already calls `doctree::parse(&d.title, &d.body)` |
| 21.8 | `Interest engine` (Phase 5B) gains multimodal observation types — `ObservationKind::Image/Audio/Video` | Low | `interest.rs` already has `ObservationKind`; extend enum |

---

## Phase 22 — Epistemic revision: Atlas + Graphiti integration (2026-09-09)

> Sequenced after §21; depends on the classify/holarchy endpoints. This section is an index — normative detail lives in the two docs named in 22.0, not here.

### 22.0 Normative pointer

- Detail: `docs/ATLAS_GRAPHITI_INTEGRATION_PLAN.md` (§0 principle, §1 A1–A7,
  §2 G1–G7, §3 NOT-taking, §4 P0/P1/P2, §5 gates + falsifiers, §6 questions).
- Background: `docs/resources.md` §§9–11 (Atlas v0.1.0-alpha.1 Ripple/AGM;
  Graphiti v0.30.2 tri-temporal/hybrid; comparison table + monster benchmark).
- Extension: `docs/BRAINSTORM_ATLAS_NTMS_TMS_GRAPHITI.md` (T1–T18: the TMS
  family — ATMS environment labels, NATMS negated justifications, JTMS
  retraction cascade — plus Atlas trust quarantine/promotion, decision
  lineage, alias resolution, stream floors, authority registry, federated
  adjudication, SSE trail stream, bounded ingest cycles, working-memory
  pressure; Graphiti namespaces, driver trait layer, two-clock watermarks;
  and the opencode-orchestrator inspection O1–O4; normative for T items,
  cross-checks A/G boundaries).
- Rule (§0): patterns, protocols, test shapes only — never dependencies. No
  graph DB, no LLM service, no Python runtime. Offline Rust over sled +
  in-process embeddings. Any item not statable as observable behavior of a named
  physis symbol plus a cargo test name is out of scope.

### 22.1 Atlas takes A1–A7

| Take | Physis target symbol | Observable behavior | Cargo test | Benchmark metric moved |
|---|---|---|---|---|
| **A1** Ripple downstream reassessment pass | `delta_engine.rs` (`evaluate_mutation`, `OntologyDeltaReport`) + `hypothesis.rs` (`transition_to`, `recompute_fitness`) + `epistemic.rs` (`FitnessShifted`, `StatusTransition`) | after any `transition_to`, walk dependents and recompute fitness per dependent with one epistemic event per step; proposals first, writes only via the shadow-frame commit | `midchain_revision_revises_exact_dependents` | downstream revision accuracy |
| **A2** DEPENDS_ON-only walk + edge taxonomy discipline | `relation.rs` (`TypedEdge`, `RelationType`) + `delta_engine.rs` (`find_neighbors`, `build_adjacency`, `dfs_collect`, `MAX_PROPAGATION_DEPTH=5`) | walk declared DependsOn edges only (not cell-pin/label-prefix adjacency); count justification hops; keep caps + cycle recording; proposals-are-not-writes | `midchain_revision_revises_exact_dependents` (BFS order, cycles recorded) | downstream revision accuracy + cost |
| **A3** AGM 49-scenario gate over status transitions | `hypothesis.rs` (`HypothesisStatus` ×9, `transition_to`) + `epistemic.rs` (`StatusTransition`) | port shape taxonomy (simple 10 / multi 8 / chain 8 / temporal 8 / adversarial 15; K*2×12 / K*3×8 / K*4×1 / K*5×9 / K*6×3 + Relevance 7 + Core-Retainment 9) as tests over transitions, no Cypher; K*4/K*6 weak-form only | module `agm_shape_gate` (N/N per shape) | the revision-correctness score itself |
| **A4** DERIVED_FROM + SHA-256 ledger → provenance hardening | `provenance.rs` (`ProvenanceLink`, `ProvenanceChain::summary`) + `explanation.rs` (`provenance_chain`, `human_readable_summary`) | each link carries content hash + previous-link hash; verify walk reports first break with sequence coordinates; explanation surfaces the chain, not the last link | `ledger_tamper_breaks_chain_at_sequence` | provenance completeness |
| **A5** Adjudication proposal/approve flow | `contradiction.rs` (`Contradiction::new`, `ResolutionStatus::Open`) + `epistemic.rs` (`ContradictionDetected`) | small-Δ auto-recomputes; large-Δ/contradiction/high-authority stays Open with recorded rationale, resolves only via explicit approve/reject/adjust/synthesize; rationale record before any file/CLI surface | `large_delta_routes_to_open_with_rationale` | contradiction retention + recovery after false evidence |
| **A6** ReassessWeights-style tunable propagation params | `delta_engine.rs` (GAMMA, DEGRADATION_THRESHOLD, MIN_IMPACT, MAX_PROPAGATION_DEPTH) + `hypothesis.rs` (`recompute_fitness` weights 0.20/0.15/0.15/0.25/0.25 + penalties) | one named documented parameter set + per-term contribution record on each recompute; defaults frozen, tuning as separate scored configs | `fitness_recompute_reports_term_breakdown` | cost + reproducibility |
| **A7** No-perturbation invariant as regression test | `delta_engine.rs` (`evaluate_mutation`, `classify_delta`) + `hypothesis.rs` (`recompute_fitness`) | zero-shift wave leaves fitness bit-identical and emits no HypothesisTransition | `zero_shift_wave_leaves_fitness_untouched` | guards all (no phantom movement) |

Notes: A1 needs the A6 governor or one revision degrades fitness everywhere.
A2 kills the blind GAMMA=0.85 cosine wave; DependsOn-sparse graphs log a stated
fallback breadth rule. A4 is corruption-detection, never a signature.

### 22.2 Graphiti takes G1–G7

| Take | Physis target symbol | Observable behavior | Cargo test | Benchmark metric moved |
|---|---|---|---|---|
| **G1** expired_at leg → TemporalValidity extension | `temporal.rs` (`TemporalValidity`, `permanent`/`from`/`during`, `is_valid_at`, `overlaps`) | add system-recorded invalidation leg + backfill-vs-arrival distinction for late/out-of-order evidence; P0 fields only, queries filter in P2 with G6 | `temporal_triple_serialises` (triple round-trips sled JSON) | temporal accuracy ("true then" vs "recorded then") |
| **G2** (resolved, invalidated, new) ingest protocol | `delta_engine.rs` (`evaluate_mutation`, `OntologyMutation`, `OntologyDeltaReport`) + `hypothesis.rs` (`add_supporting_evidence`, `add_contradicting_evidence`) | ingest returns the triple with a deterministic judgment step (overlap+recency, every call logged) before fitness update; superseded keeps timestamps, stays queryable | `ingest_returns_resolved_invalidated_new` | stale-belief rate + recovery after false evidence |
| **G3** Episode + watermark model → epistemic trail | `epistemic.rs` (`EpistemicAuditTrail`, `EpistemicEvent`, `history_for`) + `history.rs` | raw intake kept as ordered replayable records with arrival-vs-assertion times + per-stream high-water mark; watermarks split in types, not comments | `late_evidence_replays_to_same_state` | forgetting-resistance + hypothesis diversity + ingest-order replay |
| **G4** Per-edge episodes[] → provenance chains | `provenance.rs` (`ProvenanceLink`, `ProvenanceChain`) + `explanation.rs` (`provenance_chain`) | every derived link records intake-record ids; explanations cite intake ids; stated newest-N + first-seen cap, never silent cut | intake-id citation in `human_readable_summary` + `ledger_tamper_breaks_chain_at_sequence` (chain leg) | link-level provenance completeness |
| **G5** Hybrid BM25+RRF retrieval → rag/propose path | `rag.rs` (`TokenFixedRetriever`, `TokenCounter`, token-budget packing) + `propose.rs` (cosine-only today) | lexical leg beside cosine, RRF fusion, budget packing + propose-does-not-verify kept; cosine-only retained as named baseline | `hybrid_fusion_vs_cosine_baseline` (or ships disabled, negative recorded) | outcome prediction via better shortlists (propose only) |
| **G6** Point-in-time queries → reconstruct_status_at | `epistemic.rs` (`reconstruct_status_at`) + `temporal.rs` (`is_valid_at`) | "believed at T?" / "valid during [a,b]?" answered from validity triple + audit trail; status encoding normalized inside this item | `point_in_time_matches_audit_trail` on seeded chains | temporal accuracy |
| **G7** Invalidate-don't-delete discipline | `hypothesis.rs` (`Superseded`) + `contradiction.rs` + `temporal.rs` (`valid_until`) | revision stamps invalidation timestamps, keeps items history-readable and out of current queries; true deletion explicit + separately audited | `superseded_items_stay_queryable_for_history` | contradiction retention + temporal accuracy + auditability |

Notes: G1 is dead weight until queries filter on it — paired with G6. G2 starts
from overlap+recency rules with every judgment logged (LLM judging NOT taken).
G7 states a retention/compaction story before data gets large.

### 22.3 Explicitly NOT taking

- **N1 — Graph DB substrate** (Neo4j 5.26, FalkorDB, Cypher, APOC): breaks the offline Rust + sled zero-service contract; TypedEdge adjacency expresses the walks.
- **N2 — LLM extraction/judging/reranking** (extraction prompts + date-guessing, dedupe judgment, LLM reassessor, cross-encoders): nondeterministic, key-gated, the 0.519 failure mode; Atlas itself keeps LLM opt-in.
- **N3 — Telemetry default-on** (posthog dep): no phone-home by architecture, not by config.
- **N4 — Suspect inference code** (ATMS hyperresolution mutating nogood labels; TODO-flagged nogood order): author-flagged non-standard; already excluded.
- **N5 — Full AGM K* tail as proof** (K*4 = 1, K*6 = 3): weak-form regression tests only (A3), never "AGM-complete".
- **N6 — Paid rerankers first** ($50–125/mo extraction; BGE/OpenAI/Gemini rerank): BM25+RRF (G5) is the portable win; rerankers wait for a benchmark number.
- **N7 — Dead surface + weak evidence** (Kumiho-gRPC 10/51 wired; BMB 1.000 on 149 self-authored questions, competitor columns SKIPPED): the honest cells are the structural contradiction/cross-stream/forget gaps, not the 1.000.

### 22.4 Merge order P0/P1/P2 (deps vs Phases 1–7)

- **P0 — invariants + history first** (constrain everything later): A7, G7, G1
  (fields only). Needs classify (Phase 1) + hypothesis lifecycle (Phase 7) to
  exist; touches no scoring. Nothing from Phases 4–6 required.
- **P1 — revision correctness + ingest protocol**: A2, A6 (+breakdown), G2, A5
  (rationale record only, no queue surface). Needs holarchy/typed-edge
  endpoints (Phases 2, 7) for the DependsOn walk; A2 kills the Phase-1 cosine
  wave, so it lands after the classifier it constrains.
- **P2 — time, retrieval, gate**: G3, G4, G6, G5 (cosine-only kept as
  baseline), A3, A4. Needs the P0 triple + P1 ingest protocol; G5 is measured
  against the Phase-3 ask/propose path without changing its contract.

### 22.5 Acceptance gates + honesty constraints

- Mapping (contestants RAG/Graphiti/Atlas/TMS-ATMS/Physis): A1 → downstream
  revision accuracy; A2 → downstream accuracy + cost; A3 → the
  revision-correctness score itself; A4/G4 → provenance completeness; A5/G7 →
  contradiction retention + recovery after false evidence; A6 → cost +
  reproducibility; A7 → guards all (no phantom movement); G1/G6 → temporal
  accuracy; G2 → stale-belief rate + recovery; G3 → forgetting-resistance +
  hypothesis diversity; G5 → outcome prediction via better shortlists (propose
  only).
- Honesty constraints (audit FAILED: cosine 0.519 ≈ chance; structural 0.536;
  ancestry 0.534; propose top-3 0.712 proposes, never verifies — nothing below
  upgrades those numbers alone). Falsifiers: A1 fails if mid-chain accuracy does
  not beat the cosine wave on seeded chains; A5/G7 fail if pairs are
  unrecoverable after evidence retraction; G1/G6 fail if belief-at-T is no
  better than current `is_valid_at` filtering; G5 fails if fusion does not beat
  cosine-only (ships disabled, negative recorded); A3 fails if tests pass only
  as tautologies (each must fail on ≥1 naive implementation — mutation-tested or
  it does not count); A4 fails if verify passes after a payload edit. No number
  cited without its null (permuted-labels or cosine-only) beside it.

---

## Phase 23 — Formal semantics track: Triune Continuum × Kripke × Physis (2026-09-09)

> Source: `physis_resources.txt` §"Formal semantics track" (the section that
> opens the file, repeated before the closing prior-art list) and the
> `PHYSIS_STEP` protocol. This is the research framing Phase 22's machinery
> was built for; it adds no endpoints and touches no Phase 1–7 contract.

### 23.0 Research hypothesis + the standing non-claim

Tarski supplies the grounding relation between domain/model and representation
(`G: U → M` — in Physis terms, the embedder + ontology classify step);
Kripke supplies the model for progressively extending a *partial*
interpretation; Physis adds empirical outcome, temporal state, contradiction
preservation, and revision as constraints on successive semantic states.

**Non-claim (never drop this):** Physis is *not* a Kripke truth theory.
Kripke's construction concerns semantic truth/fixed points; Physis concerns an
open-world epistemic state. "Fixed point" always means **locally stable under
currently available evidence and context**, never metaphysical truth — this is
the existing scope note, now with a formal vocabulary.

### 23.1 The operator model, mapped to symbols that exist

The track's crucial separation — **semantic construction ≠ epistemic
acceptance ≠ truth** — is two operators:

```
I_{t+1} = Γ(I_t, O_t)                 semantic construction (additive)
S_{t+1} = R(S_t, I_{t+1}, V_t, C_t)   epistemic revision
```

| Operator | Physis symbol today | Status |
|---|---|---|
| Γ — extend partial interpretation | `embed.rs` + `classify.rs` (`CellClassifier`) + `propose.rs` (propose-does-not-verify) | exists; partial interpretation = proposals, never forced TRUE/FALSE (Kripke leg) |
| R — revise epistemic state | `epistemic.rs` (`EpistemicAuditTrail`, `StatusTransition`) + `hypothesis.rs` (`transition_to`, `recompute_fitness`) + `contradiction.rs` | exists; A2/A5/P1 separate the judgment from the write |
| Wave/walk between them | `delta_engine.rs` (`evaluate_mutation`, `depends_on_walk`) | the one place Γ and R still share a function — see 23.3 |

### 23.2 The semantic object `h = ⟨e, τ, π, σ, ρ, κ⟩` — audit

| Component | Meaning | Physis symbol | Gap |
|---|---|---|---|
| e | representation | embedding (`Hypothesis.embedding`, node `embedding`) | none |
| τ | type/ontology position | `ontology_refs`, `cell_pin`, the 5×14 grid | none |
| π | provenance | `provenance.rs` + `Revision` | G4 intake ids (P2) |
| σ | epistemic state | `HypothesisStatus` ×9 + `fitness` | none |
| ρ | revision history | `revision_history` | G3 episode/watermark trail (P2) |
| κ | context/validity conditions | `TemporalValidity` (`valid_from`/`valid_until`/`expired_at`) | G6 query semantics (P2) |

Every component of the tuple already has a home; the P2 items close the two
gaps. No new struct is planned — the tuple is an audit lens, not a type.

### 23.3 Deliverables (each with its gate)

| # | Deliverable | Effort | Gate / falsifier |
|---|---|---|---|
| 23.1 | **Operator-separation invariant:** Γ never writes status — `evaluate_mutation`'s semantic proposals ride the report; status changes are derivable only through the R-side trail (`StatusTransition`). Split or assert the distinction inside `delta_engine.rs`. | M | `gamma_report_carries_no_status_writes` — passes only if a naive "evaluate_mutation mutates status directly" implementation fails it |
| 23.2 | **Additive-construction invariant:** `I_t ⊆ I_{t+1}` — semantic construction never erases; deprecation is state/provenance (the Γ-side face of G7's invalidate-don't-delete). Interpretations retire via κ/σ, not deletion. | S | `interpretations_never_shrink_on_ingest` over a seeded ingest sequence |
| 23.3 | **Provisional fixed-point test:** status `locally stable` vs `revised` reported when `S_t` and `S_{t+1}` are equivalent under current evidence (the PHYSIS_STEP stage 10). Distinct from E20's geometric fixed points (density maxima) — same word, different object; cross-reference, do not merge. | S–M | `stable_status_matches_zero_delta_and_evidence_window`; falsifier: status claims stability while any κ-window evidence is unresolved |
| 23.4 | **Replay/dream hook (stage 11):** failed/inert/contradictory structures feed `SELECT_REPLAY_CANDIDATES` → counterfactual proposals — the Physis-vs-Atlas divergence the resources file calls "exactly where to concentrate research" (continuous empirical representation + belief maintenance, with failure memory and replay). | M | proposals cite the ledger events that motivated them; no deletion of replayed items |
| 23.5 | **The critical experiment** (extends 22.5): same belief-update benchmark under **(1)** monotonic/vector retrieval (current cosine propose path), **(2)** AGM revision (Atlas), **(3)** TMS/ATMS (`ATMS-in-Python` as the small reference), **(4)** Atlas/Ripple, **(5)** Physis. Metrics: contradiction retention, downstream revision, temporal validity, provenance preservation, recovery after false evidence, hypothesis generation. | L | every number cited with its null; the claim is about the *additional behavior produced by coupling semantic representation to empirical epistemic revision* — not about inventing belief revision |
| 23.6 | **Temporal dream — retrospective proposals over the time machine:** replay the audit trail (G3 assertion order) over retired branches — superseded/failed/inert hypotheses and historically severed DependsOn connections — and **propose edits to old branches**: reactivate a retained hypothesis whose pattern re-presents itself, restore a severed connection whose target re-confirmed, retire a connection whose target keeps contradicting. Mechanism: `physis-core/src/dream.rs` (`dream_over_history` → `RetrospectiveProposal`). Proposals cite the historical event/mutation ids that motivated them; nothing applies without an explicit commit (G7 kept the branches, so they can be re-proposed). The dream takes the trail by shared reference — it cannot write. | S–M | `dream_proposes_reactivation_from_retained_branches`, `dream_proposes_restoring_severed_connections`, `dream_proposes_retiring_repeatedly_contradicted_connections`, `dream_never_writes` — every citation resolves to a real event/mutation id; the trail is byte-identical after a dream |

Standing baselines for 23.5 (per the resources file): Atlas, Graphiti, and an
ATMS implementation are the first three external baselines — the ATMS arm is
concrete now: the label algorithm from `ATMS-in-Python`/`NATMS`, re-implemented
in the benchmark harness, with the Nixon Diamond as the named cross-check
scenario (`docs/BRAINSTORM_ATLAS_NTMS_TMS_GRAPHITI.md` §2 T4). Study-only (no
dep): `Nous` (persistent epistemic substrate — what the modern memory
ecosystem is converging toward), `Agent_Memory_Techniques` (map),
ontology-embedding implementations (OWL2Vec/DL2Vec/RDF2Vec/PyKEEN) for the
symbolic-scaffold ↔ semantic-space constraint question.

### 23.5 Interaction evidence + the timelessness constraint (Mindalogue, 2026-09-09)

Source: `mindalogue.md` (Zhang et al. 2025, HCI). A node+canvas non-linear
surface over an LLM measurably reduced task steps and comprehension load
against a linear chat baseline (n=16, paired study, SUS/NASA-TLX/CSI). Two
consequences for Physis:

- **Interaction:** external validation for the Atlas / Studio / Console
  surfaces (Phases 12/17/19) — the canvas is the interaction model, the
  trail is not the UI. Their "AI-generated relationships" future work stalls
  on annotation accuracy, the same wall as cosine 0.519: Physis may
  *propose* relations between nodes; gates and humans decide.
- **The timelessness constraint (standing rule):** temporal coherence does
  NOT require discarding concurrent hypotheses or old data points, and
  **not all of Physis is time-aware**. Clocks live only in the trail/replay
  layer (G3's two clocks, per-stream watermarks); coherence, classification,
  and hypothesis state stay timeless. The time machine exists so the past
  can be *re-proposed* (23.6) — never rewritten, never deleted.

### 23.6 Order

23.1–23.3 land with/after P2 (they need G3/G6 vocabulary to be honest);
23.4 needs the P1 ingest protocol; 23.6 (temporal dream) rides on G3's
assertion-ordered replay and G7's retained branches — both are already in,
so it can land immediately; 23.5 is last — it is the paper, and it
consumes everything before it. Nothing in this phase may weaken the honesty
constraints of 22.5: cosine 0.519 / propose top-3 0.712 stand.


## Phase 24 — The full resource implementation map (2026-09-09)

> Answers "how could all resources be implemented?" — every surveyed
> codebase and prior-art thread (Atlas, Graphiti, ATMS-in-Python, NATMS,
> Truth-Maintenance-System, opencode-orchestrator, llm_wiki (nashsu
> v0.6.11, 2026-09-10), Mindalogue, and the
> TMS/AGM/temporal-KG/provenance/paraconsistency/ontology-embedding
> literature in `physis_resources.txt`) mapped to a concrete landing: a
> named symbol, a wave, a gate, and a status. Rule unchanged: an item
> without a named symbol plus a cargo test name does not enter the
> schedule; codebases are read, never depended on.

### 24.1 The map

Status: **DONE** (landed, gated) · **SCHEDULED** (ordered waves below) ·
**GATED** (blocked on a measurement that must exist first) ·
**CONVENTION** (docs/process, not code) · **STUDY-ONLY** (informs design and
the paper; never ported) · **NEVER** (§22.3 / brainstorm §4 exclusions).

| Item | Source | Lands in | Gate | Status |
|---|---|---|---|---|
| A7 no-perturbation invariant | Atlas | `delta_engine.rs` | `zero_shift_wave_leaves_fitness_untouched` | **DONE** |
| G7 invalidate-don't-delete | Graphiti | `hypothesis.rs` views | `superseded_items_stay_queryable_for_history` | **DONE** |
| G1 `expired_at` leg | Graphiti | `temporal.rs` | `temporal_triple_serialises` | **DONE** |
| A2 DependsOn-only walk | Atlas | `delta_engine.rs` | `midchain_revision_revises_exact_dependents` | **DONE** |
| A6 named weights + breakdown | Atlas | `hypothesis.rs` | `fitness_recompute_reports_term_breakdown` | **DONE** |
| A5 adjudication routing | Atlas | `delta_engine.rs` | `large_delta_routes_to_open_with_rationale` | **DONE** |
| G3 two clocks + watermarks | Graphiti (+T18) | `epistemic.rs` | `late_evidence_replays_to_same_state` | **DONE** |
| 23.6 temporal dream | Mindalogue note + G3/G7 | `dream.rs` | `dream_never_writes` (+4) | **DONE** |
| **T4** Nixon Diamond scenario | ATMS-in-Python | benchmark seed test | `nixon_diamond_retains_both_sides` | **DONE** (W1) |
| **T3** retraction cascade | JTMS | `hypothesis.rs::retract_evidence` + A2 walk (`retract_evidence_with_cascade`) | `evidence_retraction_replays_to_state_without_it` | **DONE** (W1; G3 ✓) |
| **G4** per-link intake ids | Graphiti | `hypothesis.rs`/`provenance.rs`/`epistemic.rs` + `explanation.rs::cited_intake_ids` | `citations_resolve_to_intake_ids` | **DONE** (W1) |
| **G6** point-in-time queries | Graphiti | `epistemic.rs::point_in_time_status_at` + `temporal.rs::is_valid_at` now reads `expired_at` | `point_in_time_matches_audit_trail` | **DONE** (W1; G3 ✓) |
| **G5** BM25+RRF beside cosine | Graphiti | `rag.rs::Bm25Index` + `fuse_rrf` + `measure_hybrid_vs_cosine` | `hybrid_fusion_vs_cosine_baseline` (shipping verdict derived from the measurement; negative ships disabled) | **DONE** (W1; RRF k=60 matches llm_wiki `search_project`) |
| **A3** AGM shape gate | Atlas | `hypothesis.rs` tests | `agm_shape_gate`, mutation-tested | **SCHEDULED W2** |
| **A4** hash-chained provenance | Atlas | `provenance.rs` | `ledger_tamper_breaks_chain_at_sequence` | **SCHEDULED W2** |
| **23.1** Γ/R separation invariant | formal track | `delta_engine.rs` | `gamma_report_carries_no_status_writes` | **SCHEDULED W2** |
| **23.2** additive-construction invariant | formal track | ingest path | `interpretations_never_shrink_on_ingest` | **SCHEDULED W2** |
| **23.3** provisional fixed point | formal track | hypothesis status surface | `stable_status_matches_zero_delta_and_evidence_window` | **SCHEDULED W2** |
| **T1** environment labels | ATMS | `hypothesis.rs` | `hypothesis_reports_minimal_environments` | **SCHEDULED W2** |
| **T2** negated justification parties | NATMS | `contradiction.rs` | `negated_justifications_coexist_without_collapse` | **SCHEDULED W2** |
| **T10** stream confidence floors | Atlas | intake consts | `stream_floor_lowers_initial_confidence` | **SCHEDULED W2** |
| **G2** ingest triple | Graphiti | `delta_engine.rs` + `hypothesis.rs` | `ingest_returns_resolved_invalidated_new` — **GATED on 50 hand-built pairs above chance**, else flag-only | **GATED W2** |
| **A1** ripple reassessment | Atlas | `delta_engine.rs` behind a flag | **GATED on Q1 probe** (seeded chains: walk vs cosine wave, accuracy vs stale-belief) | **GATED W3** |
| **T5** quarantine → promotion | Atlas `trust/` | ingest path | `quarantined_ingest_absent_until_promoted` | **SCHEDULED W3** (after G2) |
| **T9** alias registry | Atlas `resolution/` | new `resolution.rs` | `alias_resolution_precedes_fuzzy_never_merges` | **SCHEDULED W3** |
| **T11** authority registry | Atlas `people/` | `contradiction.rs` | `authority_tier_blocks_auto_resolve` | **SCHEDULED W3** |
| **T8** lineage walker + foundation alerts | Atlas `lineage/` | `explanation.rs` + inverted A2 walk | `weakened_foundation_flags_dependent_decision` | **SCHEDULED W3** |
| **T6** graph-distance RRF leg | Graphiti | `rag.rs`/`propose.rs` | `graph_distance_leg_vs_g5_baseline` (after G5) | **SCHEDULED W3** |
| **T7** community rollups derived | Graphiti | `studio_communities.rs` | `community_summary_derives_from_members` | **SCHEDULED W3** |
| **T12** federated adjudication | Atlas `multi_tenant/` | tenant-scoped contradiction entries | `cross_tenant_contradiction_routes_to_both_queues` | **SCHEDULED W3** |
| **T16** group-id namespaces | Graphiti | sled key prefixes in `store.rs` | `operations_are_group_scoped_by_default` | **SCHEDULED W3** |
| **T17** driver trait layer | Graphiti `driver/` | `store.rs` trait + in-memory double | `store_ops_trait_serves_in_memory_double` | **SCHEDULED W3** |
<!-- PHASE24_CONT2 -->
| **L1** ingest queue (crash recovery, retry, per-source discard) | llm_wiki `ingest-queue.ts` (nashsu v0.6.11, surveyed 2026-09-10) | `ingest.rs` queue controller over the G2 triple | `ingest_queue_crash_recovery_retries_then_persists` | **SCHEDULED W4** |
| **L2** review system: flag → human resolve → re-sweep rebuilds index | llm_wiki `sweep-reviews.ts` + `review-utils.ts` | `review_desk.rs` queue surface on top of A5 adjudication (P1 shipped the rationale record only) | `review_resolution_refreshes_derived_index` | **SCHEDULED W4** (after A5 queue surface) |
| **L3** 4-signal graph relevance leg (direct/overlap/neighbor/type affinity) | llm_wiki `graph-relevance.ts` (weights 3.0/4.0/1.5/1.0) | third RRF leg in `rag.rs::rank_hybrid` | `graph_affinity_leg_vs_g5_baseline` | **SCHEDULED W4** (after G5 measurement lands) |
| **L4** two-step ingest: analyze first, write pages second (source-tagged) | llm_wiki `ingest.ts` | `doctree.rs`/`wiki_gen.rs` split analysis from page write (proposals are not writes) | `ingest_analysis_precedes_page_write` | **CONVENTION** (already propose-don't-write) |
| **L5** deep research: multi-query web search → auto-ingest | llm_wiki `deep-research.ts` + `web-search.ts` | `dream.rs::dream_over_history` counterfactual *propose* half exists; the scheduling half is new | `deep_research_proposals_are_reviewed_not_applied` | **SCHEDULED W4** (dream 23.6 first) |
| **L6** multimodal: embedded images → vision-LLM caption → image-aware retrieval | llm_wiki `vision-caption.ts` | `sensory.rs` `VisionFeature` arm of the intake bus | `vision_caption_intake_labels_sensory_frame` | **SCHEDULED W4** |
| G5 RRF constant k=60 convergence | llm_wiki `search_project` (Rust backend, `score: 1/61`) | `rag.rs::rank_hybrid` | `hybrid_fusion_vs_cosine_baseline` | **CONVERGED / DONE** (W1) |
## Architecture Constraints

> **Interface mimicking nature:** what is shown must be derived from state by
> construction, never assembled alongside it. An organism has no reporting
> layer — wilting *is* turgor, not a readout of it.

> **Embedder consistency:** nodes must be embedded with the **same** embedder
> that embeds queries, or cosine recall is meaningless.

> **Tenant isolation:** tenant-scoped handlers use `AppCtx::with_user(uid, |rt| …)`.
> Shared immutable infra stays global in `SharedCtx`.

---

## Milestone Dependencies

```
Phase 1 ──┬──> Phase 2 ──> Phase 3
            │        │
            │        └──> Phase 7 (Decision Planner)
            └──> Phase 4 ──> Phase 5
                               │
                               ▼
                           Phase 6 (Admin/Datawork)

Phase 22 (epistemic revision, after §21) ──> depends on classify/holarchy endpoints (Phases 1–2, 7)
```

Phase 6 is mostly independent of Phases 1–5. Phase 7 depends on classify/reconstruct/holarchy endpoints. Phase 22 (epistemic revision, after §21) depends on the classify/holarchy endpoints (Phases 1–2, 7) and constrains them; it needs nothing from Phases 4–6. Phase 23 (formal semantics) is the research framing over Phase 22's machinery: 23.1–23.3 land with/after P2, 23.4 after the P1 ingest protocol, 23.5 last.


## Scope note: what Physis does not claim
Physis does not determine metaphysical truth (it maintains operational validity). It does not replace domain experts (humans dispose). It does not establish that embeddings encode ontology. It does not prove general self-organised criticality (one attraction--repulsion phase-transition experiment; statistics pending). It does not establish superiority over LLMs on general reasoning. It does not guarantee correct contradiction resolution. It does not infer the correctness of human verdicts. Current evidence establishes architectural behaviour and selected empirical properties, not general industrial efficacy.
- Phase 22 still does not claim that revision cascades, hybrid retrieval, or provenance hardening certify a filing — cosine 0.519 / propose top-3 0.712 stand; the takes propose, carry and defer, they do not verify.
- Phase 23 fixes the formal vocabulary and its non-claims: Physis is not a Kripke truth theory; "fixed point" means locally stable under currently available evidence and context, never metaphysical truth; the 23.5 benchmark claims only the additional behavior produced by coupling semantic representation to empirical epistemic revision — not the invention of belief revision.
todo: investigate tmg/ntmg/graphiti/atlas as codebases in /home/gio/dev — done 2026-09-09, see docs/BRAINSTORM_ATLAS_NTMS_TMS_GRAPHITI.md (T1–T7);

---

## Consolidated Agent Todos (2026-09-11)

Merged from every agent's todo file across `/home/gio/dev`. Each entry carries its
source agent and file. Status reflects the last write, not necessarily the live
state of the code.

### physis-pro — `.opencode/todo.md`

| Mission | Status | Notes |
|---|---|---|
| M1–M9 | ✅ Completed | Project discovery, core engine, web layer, ingest, benchmark, CLI, MCP, agent harness, verification |
| M10 — Impossible Machine / RH attack | ✅ Shipped 2026-09-11 | Lean formalization + E48–E50 experiments; formal target recorded as OPEN |
| M11 — Perspective Invention | ✅ Shipped 2026-09-11 | Tier 0 FAILED outcome, documented honestly |
| M12 — Dashboard | ✅ Shipped 2026-09-11 | `dashboard.html` + `/dashboard` route + scoped gates |
| M13 — Corrective Feedback Engine | ⛔ INTERRUPTED | ~50 tasks unchecked; full list in `physis_loop/.opencode/todo.md` below |

### physis_loop — `physis_loop/.opencode/todo.md` (M13 full task list)

All unchecked. Research prototype; not production-critical.

| # | Module | Tasks |
|---|---|---|
| M1 | Core Type Definitions | S1.1.1–S1.4.2: `Artifact`, `Intent`, `Observation`, `Constraints` dataclasses + methods |
| M2 | Environment Abstraction | S2.1.1–S2.2.6: `Environment` ABC + `PythonEnvironment` with subprocess, timeout, exception handling |
| M3 | Diagnosis | S3.1.1–S3.1.3: `Diagnosis` dataclass + `is_high_confidence()` + `__str__` |
| M4 | Mutation | S4.1.1–S4.2.4: `Mutation` dataclass + `apply()`/`_apply_recursive()` + `minimal_mutation()` with preservation rules |
| M5 | Generator Interface | S5.1.1–S5.2.2: `Generator` ABC + `PlaceholderGenerator` |
| M6 | Feedback History | S6.1.1–S6.1.9: `FeedbackHistory` with record/current_attempt/last diagnosis/observation/stagnation detection + (de)serialization |
| M7 | CoherenceEngine | S7.1.1–S7.3.1: `__init__`, `run()` main loop, `_check_constraints`, `_diagnose`, stagnation break, `run_corrective_loop()` |
| M8 | Integration Tests | S8.1.1–S8.6.1: valid artifact, broken correction, history recording, repeated failure, replaceable environment, preservation |
| M9 | README and Demo | S9.1.1–S9.2.4: README + `main.py` runnable demo |
| M10 | Final Verification | S10.1.1–S10.2.2: full suite, syntax check, import chain, mark all TODOs |

### Root — `/home/gio/dev/.opencode/todo.md`

| Mission | Status |
|---|---|
| M1 — Replace cavecrew interpolations with literals in `delegation.ts` | ✅ Completed (Worker + Reviewer) |

### praxis_webapp — `praxis_webapp/.opencode/plans/opencode_plan.md`

11-phase master plan to integrate all services from `/home/gio/.env` into Axiom.
No checkboxes — tracked in `claude_steps.txt`. Phases: env alignment → multi-LLM
provider layer → enhanced search → STT → TTS → image gen → translation/emotion/OCR →
vector stores & memory → 3D + voice pipelines + Telegram → admin panel → DB migrations
& docs.

### praxis_webapp — `praxis_webapp/docs/notes/plan.md`

| # | Item | Status |
|---|---|---|
| 1 | Run pending Supabase migrations | ✅ Done 2026-06-01 |
| 2 | Set `CREDENTIAL_ED25519_*` on Railway | ✅ Done |
| 3 | **Deploy backend** — verify `auditEconomy()` OK | ⬜ Pending |
| 4 | _(optional)_ Register GitHub OAuth App | ⬜ Pending |
| 5 | Remaining genuinely-hard splits (deferred) | ⬜ Pending |
| 6 | Deliberately left (atomic RPCs can't replicate 1:1) | ⬜ Parked |

### Other agent directories

No `.codex`, `.kilo`, or `.cline` todo files found. `.claude` directories exist in
physis-pro, praxis_webapp, t3code, deepseek-harness-src, vellum-assistant — none
contain todo files. `aion/plan.md`, `cyclops/PLAN.md`, `cyclops/plan.md` exist but
contain no unchecked tasks.

## Fourier Structural Basis Experiment (COMPLETE)

**Status**: All 7 TODO items verified `[x]` (2026-09-12)

**Experiment**: Fourier Structural Basis Experiment — diagnose experimental preconditions, execute hypothesis gate, verify and report

**Code changes**:
- Fixed `compare_node_lists()` embedder reference in `src/main.rs:1859` (`embedder.as_ref()`)
- Added `#[derive(Debug)]` to `BenchmarkDataset` and `ConnectSource` enums
- Added Fourier contexts note to `AGENTS.md`

**Verification results**:
- `cargo check --all-targets`: 0 errors, 0 warnings
- `cargo test -p physis-core --lib`: 174/174 tests pass
- Git commit: `ca0b15d` on `agent/PH-001`
- Git push: pushed to `origin agent/PH-001`

**Fourier contexts documented in AGENTS.md**:
1. Fourier Structural Similarity — node-list comparison via cosine similarity
2. Fourier Synthesis for fBm — fractional Brownian motion generation

**Next falsifiable experiment**: None — experiment complete and closed.

## Phase 25 — WorldState: structure before mapping (2026-09-14)

Full statement in `docs/plans/2026-09-14-worldstate-program.md`; this section is
the plan-of-record extract, with the numbers and the kill conditions inline so
nobody has to open the other file to know whether an item is allowed to ship.

### 25.0 The research question, stated so it can fail

> Can a computer be organised around navigable world-state representations
> rather than files, folders, keywords and application-specific interfaces?

Operationalised: **does explicit structural/world-state representation give a
measurable advantage over ordinary embedding similarity** for navigation,
retrieval, temporal reasoning, contradiction detection and cross-representation
agreement? This is a hypothesis under test, not a premise. The immediate goal is
**not** an operating system; it is the smallest prototype that demonstrates the
primitive plus a paper made from what survives.

The pipeline under test, replacing `files → folders → applications → commands →
search`:

```text
observations → representations → structures → world state → temporal
trajectories → retrieval/navigation → actions
```

### 25.1 Audit result — what is reused unchanged

Phase 1 of the brief was run against commit `31eeded`. **Two thirds of the
machinery exists.**

| layer | implementation | verdict |
|---|---|---|
| observations | `physis-core/src/observe.rs` (1007 lines; append-only `Observation`; `watch_fs` `watch_git` `watch_shell` `watch_agent`) | reuse unchanged |
| representations | `embed.rs` (`RandomProjectionEmbedder`, `MemoEmbedder`), `embed_onnx.rs`, `embed_ngram.rs`, `model_provider.rs`; Pro `src/embed/{registry,onnx,clip,jina}.rs` | reuse; **add no embedder** |
| entity carrier | `CoherenceNode` (`physis-core/src/models.rs:48`) — embedding, derived `coherence_score`, `asserted`, `cell_pin`, **`embedder` provenance**, `previous_coherence_state` | reuse |
| relations | `relation.rs` `TypedEdge` + `RelationType`, each edge carrying provenance, confidence, `TemporalValidity` | reuse |
| **world state** | **already exists** — `physis-core/src/transform.rs:252`: `{facts, constraints}`, `triples()`, `without_relation()`, `substitution_invariants()`, `incoherent_after()` | **extend; a second `WorldState` is forbidden** |
| structural mapping | `transform.rs` symbolic homomorphism (`find_homomorphisms`, backtracking join, no embeddings) + `structsim.rs` (node-level, one space) | reuse — the two halves of "structure first" |
| temporal | `temporal.rs` `TemporalValidity`/`narrow_until`, `epistemic.rs` replay, `becoming.rs` runs-test | **thinnest real layer; new code goes here** |
| retrieval | `rag.rs` `TokenFixedRetriever`, `Bm25Index`, `fuse_rrf`, `Selection::CascadeRerank`, floor 0.66, `act::bearing_on` | reuse as the baseline arm |
| harness | `examples/experimentNN_*.rs`, `benchmarks/retrieval/run.py`, `benchmarks/results/*.json`, `scripts/declared-never-called.sh`, `just recall` | reuse the convention exactly |

**Missing, and therefore the whole of the new work:**

1. a temporal *sequence* of world states (`TemporalValidity` marks when a fact
   holds; nothing holds `state_t0 → state_t1 → …`);
2. structure derived independently per representation and then compared — all
   prior cross-representation work compared **clusterings**, not structures;
3. navigation (traversal from a current state) as opposed to ranked selection;
4. a shell over the world (`locate/neighbors/history/changed/compare/trace/why`);
5. the observation → representation join (`ngram_table::build_structural` has
   three call sites — `physis-core/src/main.rs:1902`, its definition, one test —
   and none is the log).

**Dead or unproven, do not build on:** `transplant` (zero call sites anywhere) ·
`OnnxEmbedder` (was exported-and-uncalled once; every benchmark number of that
era came from a lexical hash — grep call sites first) · `structsim`, `becoming`
(never put in front of a null).

### 25.2 The prior negatives that constrain the design

Not background. These already refute several obvious designs, and Phase 25 is
built so as not to re-run them.

| finding | number | consequence |
|---|---|---|
| embedding geometry gives no stable ontology structure | **cross-embedder ARI ≈ 0.10**; +25% corpus replaced half the anchors (ARI 0.198) | "cluster A, cluster B, compare partitions" is measured and negative. **ARI 0.10 is the null E55 must beat, using relations rather than partitions.** |
| no cheap signal certifies structure | every scorer at chance on real misfilings (cosine 0.519, CONSTRAINT 0.536, ANCESTRY 0.534) | do not score correspondences and call that validation; test by held-out prediction |
| cross-model disagreement carries no epistemic signal | paired *t* = −10.53, 0/22 | two embedders disagreeing is not evidence of anything |
| the machine cannot judge belonging | AUC 0.410 | correspondence is proposal, adjudication is separate and measured |
| `propose` works where `judge` failed | top-3 **0.712** vs null 0.136 | φ emits a **ranked candidate list**, never a verdict |
| embeddings cannot separate agreement from contradiction | C2: embedding@0.90 precision 0.667/recall 1.000 vs structural 1.000/0.375; MemStrata **AUROC 0.59** (arXiv 2606.26511) | contradiction detection in E58 must not be cosine-based |
| `transform` ANALOGY beats a degree-preserving control | top-3 0.500 (n=86) / 0.446 (n=294) vs popularity 0.337/0.303, permuted null 0.209/0.256 | the one arm with prior positive evidence; E55 is its generalisation |

### 25.3 Hypotheses, each with its null

- **H1 (main).** Structure derived independently in two representation spaces,
  followed by *tested* correspondence, beats raw geometry, random
  correspondence and cosine correspondence — on held-out items and under a
  third representation. *Null:* no better than cosine; agreement stays at
  ARI ≈ 0.10.
- **H2 (temporal).** An ordered sequence of world states improves retrieval and
  state reconstruction over order-blind similarity. *Null:* shuffling state
  order leaves every metric within noise. **Run this null first** — it is cheap,
  and order-blindness is the exact defect `becoming.rs` was written against.
- **H3 (invariants).** Which relations survive a change of representation is
  predictable. *Null:* survival is indistinguishable from a degree-preserving
  random subset.
- **H4 (navigation).** Traversal retrieves what ranked similarity misses on
  adversarial cases. *Null:* navigation is a slower `TokenFixedRetriever`.
- **H5 (downstream).** An LLM given a world-state region beats the same LLM
  given top-k text on relational and temporal questions. *Null:* equal. External
  context: STALE reports the best evaluated model at **55.2%** overall.

### 25.4 Formulation (minimal, every symbol maps to existing code)

`O` observations (`observe::Observation`) · `R_i : O → ℝ^{d_i}` one per embedder,
identified by the `embedder` tag already on `CoherenceNode` · `Γ_i = Γ(R_i(O))`
a **graph** (mutual-kNN ∪ typed relations ∪ symbolic triples), computed **per
space with no reference to the other space**.

```text
W_t = ( E entities · F facts · C constraints          [existing transform::WorldState]
        X_i coordinates per representation             [NEW field]
        N structural neighborhood (Γ_i | E)            [NEW field]
        p ordinal position     [POSITION IN TEXT — sentence index, not a clock]
        P provenance           [observation seq ids]
        κ coherence/confidence [coherence_score, asserted]
        T transitions          [NEW mechanism — the only one] )
```

`τ : W_p → W_{p+1}` decomposes into five set-operations over `E`/`F`:
**persistence, change, emergence, disappearance, recurrence**. No causal claim —
the word is *temporal transition*; `RelationType::Causes` is used only when a
causal test supports it.

`φ : Γ_A → Γ_B` = ranked candidates. `A(Γ_A, Γ_B, φ)` reported as **four**
numbers, never one: mutual-kNN preservation@k, typed-edge Jaccard, ARI (for
continuity with the 0.10 prior), relation-type confusion. The quantity under
test is `Δ = A(φ_structure) − A(φ_baseline)` with bootstrap CI, on held-out
items, then again under representation C.

**Traceability is a hard requirement:** every derived edge stores
`observation seq → representation id → operator → derived edge`. An edge without
that chain is a bug.

### 25.5 Benchmark, frozen before evaluation

`benchmarks/worldstate/corpus.jsonl` — 60–100 sentences, one small evolving
world, 8–12 entities; per sentence: `pos` (its ordinal index in the text), entities,
gold typed relation, gold transition label. **Mandatory:** ≥15 adversarial pairs where
lexical overlap is high and structural relevance low, ≥15 the reverse, and
agreeing near-duplicates as negatives (without them a contradiction metric
measures similarity — the C2 lesson, learned by discarding a run). Held-out
split 70/30 **by entity**. Corpus hash, seeds, embedder checksums, crate commit
recorded in every artifact.

Arms: `baseline` (no alignment) · `random` (degree-preserving) · `cosine` ·
`supervised` (**explicitly labelled a supervised condition**, an upper reference
not a competitor) · `structural` (the hypothesis).

### 25.6 E55 — the first experiment

`physis-core/examples/experiment55_structure_before_mapping.rs`, artifact
`benchmarks/results/worldstate-e55.json`.

A = `bge-base-en-v1.5` (D4's embedder), B = character n-gram /
`SyntheticNGramEmbedder`, C = jina/CLIP held back for transfer. **Verify both A
and B are actually called before trusting a number.**

**Pass condition, fixed before the run:** `structural` beats `cosine` and
`random` on ≥3 of 4 metrics, non-overlapping bootstrap CI on the primary metric
(mutual-kNN preservation@10), and held-out plus third-embedder runs keep the
sign.

**Kill conditions:** `structural ≈ cosine` ⇒ stop, record next to E53 in
`transform.rs`'s header, and re-examine whether E53's positive was
corpus-specific · wins on the training split only ⇒ report as overfitting ·
all arms near ARI 0.10 ⇒ the prior negative generalises, publish the negative
and pivot to H2, which does not depend on cross-space agreement.

Cost: 3–5 days, mostly corpus authoring and metric plumbing.

### 25.6a E55 RESULT (2026-09-14) — NEGATIVE, kill condition fired

Run the day it was specified. Corpus `benchmarks/worldstate/corpus.jsonl`
sha256 `264c40a07303…`, 72 sentences, frozen before any arm. A =
`bge-base-en-v1.5` (ONNX, `PHYSIS_MODEL_DIR=../models/bge-base-en-v1.5`),
B = `RandomProjectionEmbedder(384)`. Seed 20260914. Artifacts
`benchmarks/results/worldstate-e55-{all,train,heldout}.json` (+ `-sweep`).
Record: `physis-core/research/E55_STRUCTURE_BEFORE_MAPPING.md`.

Top-1 correspondence accuracy, identity as gold, never shown to a matcher:

| split | random (null) | anchor (8 known pairs) | supervised (gold labels) | **structural** |
|---|---|---|---|---|
| all (72) | 0.028 | 0.236 | 0.083 | **0.014** |
| train (49) | 0.020 | 0.306 | 0.082 | **0.020** |
| heldout (23) | 0.130 | 0.609 | 0.174 | **0.043** |

ARI with no mapping: 0.287 / 0.278 / 0.000.

**The hypothesis arm is at its null in every split.** A six-point sweep
(k ∈ {3,5,10} × profile ∈ {6,24}) holds it in 0.014–0.056 against a null of
0.028, so it is not a hyperparameter. It is also not a broken descriptor: the
same code recovers identity at top-1 > 0.99 when both spaces are the same space
(`identical_spaces_recover_identity`).

Three things the run does establish: **(1)** the cross-space information exists
— eight known anchor pairs lift top-1 from 0.028 to 0.236 (0.609 held out), so
the failure is specific to recovering correspondence *without* seeds; **(2)**
sorted-neighbourhood descriptors are what fails; **(3)** the gold transition
label is too coarse to identify a sentence (supervised 0.083), which is a fact
about the label, not about supervision. The 0.287 ARI is **not** an improvement
on the settled ARI ≈ 0.10 — it is a one-topic 72-sentence corpus, and quoting it
as progress would be the error this plan exists to prevent.

**Consequences, applied:** H1 is not pursued with this descriptor; the next
legitimate attempt is a seeded propagation matcher (IsoRank-style), pre-registered
as a new experiment, never as a retune of this one. **The program pivots to H2**
(states ordered by position in text), which does not depend on cross-space
agreement and whose null is exact. The negative is carried in
`src/worldstate.rs`'s module header.

**Corpus shortfall, recorded not hidden:** the spec asks for ≥15 adversarial
pairs of each kind; the frozen corpus has 4 `adv_lex` + 4 `adv_struct` (2 pairs
each) and 5 `agree_dup`. Sufficient for E55, which uses none of them.
**Insufficient for E57**, which is built on exactly those cases — E57 does not
run until the corpus is extended and re-frozen with a new sha256.

### 25.6b E56 RESULT (2026-09-14) — H2 survives: order carries structure, geometry alone does not

Same frozen corpus (`264c40a07303…`), bge-base-en-v1.5, seed 20260914.
Artifacts `benchmarks/results/worldstate-e56{,-gap3,-gap5}.json`; record
`physis-core/research/E56_POSITION_FIRST.md`. **No timestamp is read anywhere:
the coordinate is ordinal position (25.7b).**

**L1 — transition derivation vs the shuffled-order null.** Set operations over
the state sequence (first mention · repeated fact · changed predicate over the
same subject/object pair · re-mention after `RECUR_GAP` = 6) derive a label per
position without ever reading the gold label. The null re-runs the *entire
derivation* on a permuted reading order:

```text
accuracy 0.486 · majority floor 0.333 · shuffled null 0.291 ± 0.040 · z = +4.88
emerge 0.700 (n=20) · persist 0.444 (9) · change 0.375 (24) · recur 0.214 (14) · disappear 1.000 (5)
```

`disappear` 1.000 is **a six-word cue list, not a finding** — it is declared in
`worldstate.rs` and it is what that row measures. `recur` 0.214 rests on an
unswept constant and is NOT MEASURED until `RECUR_GAP` is swept.

**L2 — position locality.** Mean |Δpos| over mutual-kNN edges 16.82 vs a
random-pair null of 24.19, ratio 0.695. Descriptive, no arm.

**L3 — "what preceded this?"** Gold is mechanical (nearest earlier sentence
sharing an entity). All 61 queries: cosine-blind 0.098 · cosine+position 0.311 ·
**position-only 0.475** · shuffled null 0.115. That ranking is a **corpus
artifact**: the gold is *the nearest* earlier mention, so answering `p−1, p−2,
p−3` is handed a large share for free. Dropping every query whose antecedent is
nearer than a gap:

| gap | n | cosine blind | cosine + position | position only | shuffled null |
|---|---|---|---|---|---|
| ≥3 | 28 | 0.071 | **0.286** | **0.000** | 0.107 |
| ≥5 | 23 | 0.087 | **0.304** | **0.000** | 0.130 |

Position-only collapses to exactly zero; order-blind cosine stays at its null;
**only the combined arm answers the question**. Same shape as item A's result
that retrieval and ranking are different problems. Intervals overlap
([0.143, 0.464] vs [0.000, 0.250]) at n = 28 — directional, not settled.

**Verdict on H2:** state reconstruction supported (z = +4.88); retrieval
supported in direction only. Read with E55: **the ordering of a representation
carries recoverable structure, while its geometry alone does not transfer across
representations.**

**Consequences, applied:** (1) build the five transition predicates into
`WorldState` as `T` — the derivation beats its null; (2) rebuild the "what
preceded" gold without recency bias before E57 — the gap filter is a patch, not
the fix; (3) sweep `RECUR_GAP`, which currently decides a whole label; (4) the
1000-sentence corpus moves onto the critical path, because at n = 28 the L3
interval cannot close.

### 25.6c RECUR_GAP swept, and E57 — the world-model arm (2026-09-14)

**`RECUR_GAP` swept (E56 follow-up).** The constant that decides the whole
`recur` label, run at 2/3/4/6/8/10/12/16, 500 permutations each:

| gap | accuracy | shuffled null | z | recur recall |
|---|---|---|---|---|
| 2 | 0.500 | 0.286 ± 0.040 | +5.34 | 0.429 |
| 3 | 0.486 | 0.288 ± 0.041 | +4.84 | 0.357 |
| 6 (pre-registered) | 0.486 | 0.291 ± 0.040 | +4.88 | 0.214 |
| 10 | 0.472 | 0.293 ± 0.038 | +4.73 | 0.071 |
| 16 | 0.472 | 0.293 ± 0.034 | +5.33 | 0.071 |

**L1's headline is insensitive to the constant** — accuracy 0.472–0.500 and
z ≈ +4.7 to +5.3 everywhere — so the effect is not an artifact of the gap. The
`recur` label itself is entirely gap-determined and tops out at 0.429; it is now
measured, and it is the weakest real label at every setting. The headline stays
at the pre-registered gap 6; gap 2 is not adopted, because adopting the
maximising setting after seeing the sweep is the tuning this plan forbids.

**E57 — "what does this supersede?"** New generated corpus
`benchmarks/worldstate/corpus-large.jsonl` (seed 20260914, sha256
`2eaff7bb33d7…`, 1320 sentences, 150 queries), gold = the planted fact the query
supersedes **in different words**, distractors between them (two per cycle are
near-paraphrases of the gold about another attribute). Record
`physis-core/research/E57_SUPERSESSION_RETRIEVAL.md`, artifact
`benchmarks/results/worldstate-e57.json`.

| arm | top1 | top3 | 95% CI |
|---|---|---|---|
| cosine (order-blind) | 0.000 | 0.000 | [0.000, 0.000] |
| cosine + position | 0.013 | 0.060 | [0.000, 0.033] |
| recency only | 0.000 | 0.207 | [0.000, 0.000] |
| **entity + position + cosine** | **0.700** | **0.993** | **[0.627, 0.773]** |
| shuffled-position null | 0.000 | 0.007 | [0.000, 0.007] |

Flat across distance: 0.710 / 0.727 / 0.600 / 0.688 / 0.792 at gaps 3–11.
Order-blind cosine is at **exactly zero** because 150 near-identical planted
facts exist and global similarity retrieves the wrong machine's copy every time
— entity disambiguation in its purest form. **Only structure + order + geometry
together answer the question**; every single leg and the null are at ~0. Same
shape as E56 L3 and item A's cascade: the legs are complementary and must not be
collapsed into one score.

**Limits, stated with the number:** the corpus is generated, so this closes an
interval on a retrieval arm and is not evidence about real text; and the arm is
**handed gold entity links**, so 0.700 is an upper bound on a pipeline whose
extraction step has not been measured. Extraction is the next experiment.

**Two discarded runs, kept because they are the method.** The first two versions
scored **1.000 at every distance** and were thrown away: run 1's distractors had
no lexical overlap with the gold, and run 2 still let the query reuse the gold's
exact wording, so both were near-duplicate matching. Only after the query was
paraphrased did the arm fall to 0.700 and the benchmark become able to fail.
**A 1.000 is a bug report about the benchmark, not a result.**

### 25.6d E58 RESULT (2026-09-14) — extraction costs nothing, or everything, depending on one rule

`src/worldstate.rs` (extraction), `examples/experiment58_extraction.rs`,
artifact `benchmarks/results/worldstate-e58.json`, record
`physis-core/research/E58_EXTRACTION.md`. E57's arm read **gold entity links**;
E58 replaces them with links extracted by three declared model-free rules —
`id` (identifier-shaped token), `cap` (capitalised mid-sentence), `df` (content
token repeated in ≥ 3 sentences).

**End-to-end, 150 queries:**

| link source | top1 | top3 | vs gold |
|---|---|---|---|
| gold links | 0.700 | 0.993 | — |
| extracted, all three rules | 0.013 | 0.060 | **−0.687** |
| **extracted, `id` only** | **0.700** | **0.993** | **+0.000** |
| extracted, `id`+`cap` | 0.700 | 0.993 | +0.000 |

**The finding: adding a high-recall rule destroyed the task it fed.** `df` takes
entity recall to 0.966 and precision to 0.163 — it admits `pressure`,
`tolerance`, `line`, which every cycle shares — so the entity filter links
everything to everything and the arm collapses to plain cosine+position, landing
on exactly the 0.013 that arm scored in E57. The downstream step does not want a
good extractor in the F1 sense; it wants a **specific** one. F1 0.277 vs 0.947
reads as a moderate gap and is the difference between 0.013 and 0.700. Third
independent arrival at the same lesson as item B's false-alarm accounting and
C2's F1 caveat: **the aggregate metric is not the operational one.**

**Licensed:** on the generated corpus the pipeline is end-to-end complete with
no gold annotation and no model in extraction — E57's "0.700 is an upper bound"
caveat is lifted *for this corpus*.

**Not licensed:** it does not transfer to the hand corpus, where `id` has
precision 0.969 and recall **0.304**, because real text says `the coolant pump`,
`Filter-A`, `Rossi`. The generated corpus's entities are identifier-shaped by
construction, so `id` nearly reads the generator's mind — which is why 0.947 is
only ever quoted beside the hand corpus's 0.304. `min_df` was not swept.

**Next on this line:** noun-phrase extraction on the hand corpus — the case
where identifiers do not exist — scored the same way with the end-to-end arm
beside it.

### 25.6e E59 RESULT (2026-09-14) — noun-phrase links carry the arm, and E58's lesson needed narrowing

`src/worldstate.rs` (`extract_noun_phrases`), `examples/experiment59_noun_phrases.rs`,
artifact `benchmarks/results/worldstate-e59.json`, record
`physis-core/research/E59_NOUN_PHRASES.md`. Hand corpus (72 sentences), no POS
tagger: determiner + up to three tokens that are not stopwords, not `-ing`/`-ed`,
and not a trailing `-s` **after** the phrase has a head. Each phrase emits the
full key and its head (`coolant-pump` and `pump`), which is what lets a later
`the pump` link with no coreference model.

| extractor | P | R | F1 | downstream top1 (28 queries, gap ≥ 3) |
|---|---|---|---|---|
| gold links | — | — | — | **0.393** [0.214, 0.571] |
| **noun phrase** | 0.488 | 0.699 | **0.575** | **0.357** [0.179, 0.536] |
| df only | 0.420 | 0.804 | 0.551 | 0.321 |
| id+cap only | 0.972 | 0.343 | 0.507 | 0.214 |
| cosine (blind) | — | — | — | 0.071 |

Noun-phrase links recover most of the gold-link arm and every link arm beats
order-blind cosine. **n = 28, intervals overlap heavily — directional only.**
Top-3 is where gold still separates (0.714 vs 0.393): the chunker's links find
the right candidate first often enough, and keep it in a short list rarely
enough. `np + id+cap` is identical to `np` — the chunker already emits both —
so the merge arm is redundant and is recorded as such.

**Correction to 25.6d.** E58 said a high-recall low-precision link rule destroys
the downstream task (`df`: 0.700 → 0.013). Here the same rule scores 0.321, third
of six. The corrected statement: **high-recall generic links collapse the entity
filter when the corpus contains many near-identical cycles** — exactly when
disambiguation is what the filter is for. The generated corpus has 150 sentences
of one template, so a link on `pressure` joins all of them; the hand corpus tells
one story once, so a generic link is loose rather than fatal. Both runs stand;
the mechanism is corpus redundancy, not recall as such.

**Licensed:** a model-free chunker carries the link layer on hand-written prose
— the world-model arm does not need an annotated corpus.
**Not licensed:** n = 28; precision 0.488 means half the keys are not entities;
and the chunker was written beside this corpus and has never been run on text it
was not authored against. That last one is the next task on this line.

### 25.6f E60 RESULT (2026-09-14) — the chunker does not transfer, and a wrong link layer is worse than none

Corpus `benchmarks/worldstate/corpus-docs.jsonl` (sha256 `f19c1cb4d264…`, 168
sentences from this repo's own docs), built by `make_corpus_docs.py`. **Gold is
the author's backticked spans** — an entity annotation written by someone
documenting code, with no knowledge of this extractor; the chunker never sees
the backticks. Record `physis-core/research/E60_UNSEEN_PROSE.md`, artifacts
`worldstate-e59-docs.json` and `worldstate-e59.json`.

| extractor | docs F1 | hand F1 |
|---|---|---|
| noun phrase (E59) | **0.159** | **0.575** |
| id+cap | 0.144 | 0.507 |
| df | 0.168 | 0.551 |
| **code ids** (added in E60) | **0.520** (P 0.832) | 0.000 |

**The chunker does not transfer**: F1 0.575 where it was authored, 0.159 where
it was not. The `code ids` rule was added after seeing that failure, so it is
scored on both corpora on purpose — best on docs, **emits nothing** on the hand
corpus, so `np + code` is identical to `np` there. Additive-safe.

Downstream, antecedent at gap ≥ 3:

| links | docs top1 (43 q) | hand top1 (28 q) |
|---|---|---|
| gold links | **0.698** | 0.393 |
| **cosine (blind, no links)** | **0.349** | 0.071 |
| df only | 0.302 | 0.321 |
| code ids | 0.209 | 0.000 |
| noun phrase | 0.093 | 0.357 |

**On unseen prose every extracted-link arm loses to using no links at all**, and
on the hand corpus the ordering reverses. Gold links reach **0.698** — the
highest world-model number in the series, on prose written for humans — so the
ceiling is fine and the extractor is the bottleneck.

**Third revision of the link-quality statement** (25.6d said precision, 25.6e
narrowed it to redundancy; this adds the other half):

> An entity filter helps only when it is specific enough not to link everything
> **and** complete enough not to hide the answer. Redundancy punishes low
> precision (E58: 0.700 → 0.013); sparsity punishes low recall (E60: `code ids`
> at precision 0.832 still scores 0.209, because recall 0.379 means the filter
> never admits the answer). When neither holds, **no filter beats a bad filter**
> and plain similarity is the safer default.

**Not licensed:** 43 queries with overlapping intervals; the backtick gold is an
annotation of *code* entities, so sentences about "the grid" or "the null" have
no gold and are excluded; `min_df`, `max_len` and the code rule are unswept.

### 25.6g `physis-world` SHIPPED (2026-09-14) — the navigational layer, running on this machine

`physis-core/src/bin/world.rs`, binary `physis-world`, quickstart
`docs/WORLD_SHELL.md`. Built on E55–E60, no new mechanism: the shell is the
measured arm plus E56's derivation, wrapped in verbs, over `observe.rs`'s log.
**The filesystem is untouched** — `observe` appends and writes nothing else, and
the whole world model can be deleted without losing a file.

First real run over `~/dev`: **3232 observations** (2580 fs, 400 git, 252
terminal), 93% carrying at least one extracted link, 3805 mutual-kNN edges, 919
components, derived transitions emerge=2767 / none=212 / recur=128 /
persist=119 / disappear=6. `retrieve "which experiment measured the null for the
grid"` returns the E21, E19 and E19b commits — the three places in the history
where that null was actually built.

Verbs: `observe · inspect · locate · neighbors · history · changed · compare ·
trace · why · retrieve`.

**Three rules the interface obeys.** (1) Every answer prints its chain —
observation seq → representation → operator → position. (2) **"No link" is never
printed as "not here"**: on a miss the shell falls back to string match and
labels the fallback, because the link layer's recall is the measured bottleneck
(E60) and hiding it would reintroduce Gap 3. (3) `why` prints the linked arm and
the blind arm side by side and does not choose — which wins is corpus-dependent
and measured (0.700 vs 0.013 on generated text; 0.209 vs 0.349 on this repo's
prose).

**Two defects found by using it, both fixed.** `watch_git` takes no `known`
list, so a second `observe` re-appended every commit — visible as `why 1650`
answering with position 570 at cosine 1.000. Fixed at the writer (drop
already-recorded `(source, subject)`) and at the reader (collapse duplicates for
ranking); **the log is not rewritten**, because it is append-only and tidying
history for a display is the move this architecture refuses. And
`observe::append` returns `(first_seq, last_seq)`, not a count — the first
version printed "1733 new, 1732 in log".

**Not claimed:** not an operating system, no understanding, no new measurement.
The shell's numbers are the experiments' numbers, printed where they can be
checked.

### 25.6h E61 RESULT (2026-09-14) — the interpreter flips E60's sign, and "call the model only on misses" is refuted

`research/e61_interpreter_extraction.py` + `PHYSIS_E59_EXTRA` scoring. Docs
corpus (168 sentences, gold = author's backticks). Interpreter
**`qwen2.5:7b-instruct-q4_K_M`, local on omo, temperature 0**. Artifacts
`worldstate-e59-docs-llm.json`, `…-llmfb.json`. Record
`physis-core/research/E61_INTERPRETER_EXTRACTION.md`. First experiment in the
series where a model takes part, and it takes part at one step: naming what a
sentence is about.

| links | top1 | top3 | 95% CI |
|---|---|---|---|
| gold links | 0.698 | 0.977 | [0.558, 0.837] |
| **rules + interpreter (all sentences)** | **0.488** | **0.698** | [0.349, 0.628] |
| interpreter alone (all sentences) | 0.465 | 0.558 | [0.326, 0.605] |
| cosine (blind, no links) | 0.349 | 0.581 | [0.209, 0.488] |
| rules + interpreter (**fallback only**) | 0.326 | 0.488 | [0.186, 0.465] |
| np + code (best rule arm, E60) | 0.209 | 0.279 | [0.093, 0.326] |
| interpreter, fallback only | 0.093 | 0.093 | [0.023, 0.186] |

Extraction: interpreter F1 0.524, rules+interpreter **0.563** (recall 0.823) vs
the best rule's 0.520.

**1. The sign flips.** For the first time on prose the extractor was not written
against, a link arm beats using no links at all — 0.488 vs 0.349 — closing 57%
of the rules-to-gold gap, with top-3 (0.698) equal to gold's top-1. Intervals
overlap at n = 43: a direction with a mechanism, not a magnitude.

**2. Model-only-on-misses is refuted, and this is the keeper.** The fallback
condition scores **0.326, below blind cosine** and 0.162 below calling the model
everywhere. The rules' defect is not only the 29 sentences where they produce
nothing — it is the wrong links they produce everywhere else, which patching
leaves in place. A filter is only as good as its worst links. **If the model is
called at all, it must be called on everything** — the opposite of what a
cost-conscious design assumes.

**Determinism:** temperature 0, fixed prompt, model+endpoint stored beside the
artifact; re-running the first 15 sentences gave **15/15 identical**. The scorer
reads the saved file, so every number is regenerable with the endpoint down.

**Not licensed:** n = 43 with overlapping CIs; one corpus, one model, one prompt;
extraction precision *fell* (0.480 → 0.428) while F1 rose, and E58 measured a
corpus where that trade is fatal; and **cost is unmeasured** — 168 local calls
took minutes against microseconds for rules, while `physis-world` runs over 3232
observations.

**Next:** wire an explicit, off-by-default `--interp` into `physis-world` that
states its cost, and measure the same two scores on the machine log — 94%
filenames and commit messages, a third text regime, neither of the two measured
so far.

### 25.6i E62 RESULT (2026-09-14) — the machine log as a third regime; the interpreter result replicates, one claim narrows

Corpus `benchmarks/worldstate/corpus-log.jsonl` (sha256 `90539ad9baa8…`) built
from `~/.physis-core/observations.jsonl`: **388 observations (234 git, 154
terminal), 159 scored queries — the largest n in the series**. Gold is what each
source already carries: the conventional-commit scope (`fix(ci):` → `ci`) plus
dotted filenames, and for terminal the program invoked plus path arguments. `fs`
observations are **excluded on purpose** — their subject is a path, so a
path-derived gold would be the string `code ids` already extracts. Record
`physis-core/research/E62_MACHINE_LOG.md`.

| links | top1 | top3 | 95% CI |
|---|---|---|---|
| gold links | 0.528 | 0.730 | [0.447, 0.604] |
| **rules + interpreter** | **0.465** | 0.629 | [0.390, 0.541] |
| interpreter alone | 0.459 | 0.623 | [0.384, 0.535] |
| df only | 0.403 | 0.579 | [0.327, 0.478] |
| rules + interpreter (fallback only) | 0.377 | 0.453 | [0.308, 0.453] |
| cosine (blind) | 0.340 | 0.472 | [0.270, 0.415] |
| np + code (best rule arm) | 0.296 | 0.321 | [0.233, 0.371] |

**Replicates:** interpreter links beat no links (0.465 vs 0.340) and land within
one interval of gold (0.528) — second regime, second gold, tighter interval.
**Replicates:** partial coverage loses to full (0.377 vs 0.465).
**Does not replicate:** on docs, partial was worse than *no links at all* (0.326
vs 0.349); on the log it beats blind. E61's strong form was regime-specific. The
surviving claim: **partial interpreter coverage always loses to full; whether it
also loses to no links depends on the corpus.** 25.6h is narrowed accordingly.

**New:** `df` — repeated content tokens, the crudest rule — is the best
non-interpreter arm on the log (0.403 > blind 0.340), while E58 measured the same
rule as fatal on a redundant generated corpus. Three regimes, three different
best rules: **there is no corpus-independent extractor in this series**, now
measured rather than suspected.

**Shipped with it:** `physis-world` reads an interpreter cache via
`PHYSIS_WORLD_INTERP` (off by default), keyed by observation `seq` because
positions shift as a log grows and seqs do not; converter
`benchmarks/worldstate/links_to_cache.py`. Under half coverage it prints the
measured warning instead of silently running a configuration both experiments
scored as worse. On the 3232-observation log the cache covers 377, the warning
fires correctly, and `locate ontology` gains the `refactor(ontology):` commit the
rules miss entirely.

**Not licensed:** overlapping intervals between rules+interpreter and gold; one
machine's log, one week, mostly one repository; and **cost is still unmeasured**
— 388 calls ≈ eight minutes on omo, so covering 3232 observations is roughly an
hour and the log grows.

### 25.6j E63 RESULT (2026-09-14) — the entity abstraction is unnecessary on real text and decisive under near-duplicate load

Direct test of the challenge *"maybe it is a problem of scale, and linking with
a search function, word per word, is good enough."* Three free arms added:
`bm25 + position` (`rag::Bm25Index` over earlier positions only), `shared word +
cos`, `rare word + cos`. Record `physis-core/research/E63_LEXICAL_SEARCH.md`;
artifacts `worldstate-e59-{docs,log}-lex.json`, `worldstate-e57.json`.

**Documentation, 43 queries:** gold 0.698 · **bm25+position 0.535 (top3 0.744)**
· rare word 0.488 · rules+interpreter 0.488 · shared word 0.465 · interpreter
0.465 · blind 0.349 · best rule arm 0.209. **BM25 beats every extraction arm,
including the interpreter, at zero marginal cost.**

**Machine log, 159 queries:** gold 0.528 · rules+interpreter 0.465 · interpreter
0.459 · **bm25+position 0.447** · shared word 0.434 · df 0.403 · blind 0.340.
BM25 is **statistically indistinguishable** from the interpreter
([0.371, 0.522] vs [0.390, 0.541]) and costs nothing.

**Generated supersession corpus, 150 queries — the hypothesis breaks:**
entity+position+cosine **0.700** · cosine+position 0.013 · **bm25+position
0.013** · shared word 0.013 · blind 0.000. Fifty-fold.

**Mechanism, and it is in the corpora rather than the method.** Docs and the log
are lexically diverse, so what distinguishes the right antecedent *is* which
words occur — a search function reads that signal better than a 7B naming
entities, because the model's names are lossy and the words are not. The
generated corpus is 150 near-identical cycles where every sentence shares nearly
all its vocabulary and the near-paraphrase distractors share **more** words with
the query than the true gold does; BM25 therefore ranks a distractor first,
exactly as cosine does. The entity link wins there because it removes 149 cycles
*before* ranking, leaving the embedder to read pressure-vs-temperature inside the
one that remains. **The entity abstraction is doing partitioning under
redundancy, not understanding.**

**Consequences, applied:** (1) the interpreter is **not justified** by these
measurements — E61/E62 stand as measurements, their practical conclusion is
superseded; (2) the shipped default is lexical — `physis-world why` now prints
the lexical arm alongside linked and blind, with the measured reason; (3)
redundancy is the deciding property and is cheaply measurable in advance (mean
pairwise term overlap), which is the next experiment.

**Not licensed:** the redundant corpus is synthetic and extreme by construction
(real analogues — log lines, templated alerts, ticket text — are unmeasured); 43
and 159 queries with overlapping intervals; and **nothing here tests corpus
size** — "scale" in the question turned out to mean redundancy, a different axis,
which is itself the finding.

### 25.6k E64 RESULT (2026-09-14) — a counted word table replaces the embedder on every arm the shell uses

`src/worldstate.rs` (`word_table_count`, `sentence_from_table`), selected by
`PHYSIS_E64_REP`. Record `physis-core/research/E64_WORD_TABLE.md`. Three
representations: `model` (embedder per sentence), `table-model` (embedder once
per vocabulary word, sentence = mean), **`table-count`** (random indexing with
PPMI over a co-occurrence window — **no neural inference at any point**).

| arm | model | table-model | table-count |
|---|---|---|---|
| log, gold links (159 q) | 0.528 | 0.516 | **0.497** |
| log, gold links top3 | 0.730 | 0.755 | **0.742** |
| log, blind cosine | 0.340 | 0.321 | 0.333 |
| docs, gold links (43 q) | 0.698 | 0.674 | **0.674** |
| docs, gold links top3 | 0.977 | 0.953 | **0.953** |
| docs, blind cosine | 0.349 | 0.279 | 0.279 |
| bm25 + position (control) | 0.447 / 0.535 | identical | identical |
| **build time** | 12.15 s / 7.79 s | 47.06 s / 36.44 s | **6.0 ms / 3.3 ms** |

**Counting matches the transformer on filtered arms** — gold-link top-1 within
0.031 and 0.024, top-3 within 0.012 and 0.024, intervals overlapping — at
roughly **2400× lower build cost**, and the gap grows with corpus size. **The
model is genuinely better at unfiltered similarity** (blind 0.349 vs 0.279):
encoding earns its cost when ranking everything against everything, not when
ranking inside a candidate set. **`table-model` is the worst of both** — costlier
to build than the model itself (47 s vs 12 s) and no better than counting;
averaging a transformer's word vectors discards what the transformer is for. The
BM25 row is identical across all three by construction, which is the control
proving the harness changed only what it claimed.

**Found by shipping it.** Defaulting `physis-world` to `table-count` broke
free-text queries — "which experiment measured the null for the grid" returned
`strings.xml` and two i18n YAMLs, the exact unfiltered case the table is weakest
on. The fix was not restoring the model but dropping the vector space from the
query path: `retrieve` now ranks by **BM25** (E63's best arm on real text) and
returns the E20, research-agenda and E2 commits. `PHYSIS_WORLD_QUERY=model`
remains as the one defensible model call in the shell.

**Architecture consequence:** the navigational layer now runs with **no neural
inference anywhere** — counts for vectors, BM25 for filtering and queries,
position for order, entity links only where redundancy forces partitioning
(E63). A 3232-observation world loads and answers in ~2 s with no model on disk
required. Not "a model is unnecessary in general", but **navigating one's own
machine does not need one**.

**Not licensed:** two corpora with overlapping intervals; the table is built on
the corpus being navigated and cross-corpus transfer is untested; `window = 4`,
`dim = 512`, PPMI floor unswept; the near-duplicate corpus was not re-run under
the table representations.

### 25.6l E65 RESULT (2026-09-14) — the filter picks itself, once the right thing is measured

`src/worldstate.rs` (`redundancy`, `lexical_margin`, `filter_from_margin`),
`examples/experiment65_redundancy.rs`, artifact `worldstate-e65.json`, record
`physis-core/research/E65_FILTER_CHOICE.md`.

**The obvious statistic fails.** Raw term overlap calls the machine log 0.786 —
as redundant as the generated templates at 0.802 — and the lexical filter wins
on the log anyway. Log records are file paths sharing long common prefixes, so
overlap is huge while **BM25's IDF discards exactly those shared tokens**.
Overlap measures what the filter already ignores.

**Ambiguity after weighting works.** `lexical_margin` = mean
`(score1 − score2) / score1` over the best two other documents:

| corpus | margin | bm25+pos | entity link | predicted | correct? |
|---|---|---|---|---|---|
| generated templates | **0.044** | 0.013 | **0.700** | entity link | ✓ |
| documentation | 0.235 | **0.535** | 0.209 | bm25 | ✓ |
| machine log | 0.520 | **0.447** | 0.296 | bm25 | ✓ |
| hand-written (n=28) | 0.265 | 0.286 | 0.357 | bm25 | ✗ (arms tie) |

Right on all three corpora where the arms differ by 0.151–0.687; wrong only
where they are 0.071 apart with overlapping intervals. Boundary set at **0.15**,
in the empty gap between 0.044 and 0.235 — **four corpora is four points**, and a
finer threshold would be false precision.

**Shipped:** `physis-world inspect` prints the statistic and the recommendation;
`why` marks the favoured arm. This machine's 3232-observation world measures
**0.157**, barely above the boundary — shown rather than hidden, which is why the
shell prints all three arms.

### 25.6m E66 + the open domain axis + copy/paste (2026-09-14)

**Copy/paste — the first verb in this shell that puts something out.**
`physis-world copy <term|position>` moves the text **with its provenance**: a
JSON envelope carrying `seq`, source, timestamp and extracted links, to the
system clipboard (`wl-copy` / `xclip` / `xsel` / `pbcopy`) or, when none exists —
headless, ssh, this machine — to `~/.physis-core/clipboard.json`, **reporting
which**, because a copy that silently goes nowhere is worse than a refusal.
`physis-world paste` prints it with its provenance and writes nothing;
`paste --into FILE` appends the text plus a `<!-- physis: seq … -->` line, never
truncates, and is the only file write in the shell. Both verbs append an
observation, so the log records what was handed over, not only what was asked.

**E66 — why 70 and not n.** `examples/experiment66_axis_fitness.rs`, artifact
`worldstate-e66-axes.json`, record `physis-core/research/E66_AXIS_FITNESS.md`.
Same 731 entries, same embeddings, D4's size-preserving permutation null,
relabelled three ways:

| axis | classes | fitness | null | normalised (f−n)/(1−n) | failing |
|---|---|---|---|---|---|
| DOMAIN/MODE | 70 | 0.438 | 0.038 | 0.416 | 8 |
| DOMAIN only | 5 | 0.672 | 0.209 | **0.585** | 0 |
| MODE only | 14 | 0.536 | 0.121 | 0.472 | 0 |

Read the normalised column — a null scales with 1/k, so ratios flatter small
axes. **Both axes carve: every class on both clears its own null**, and the
domain axis is the *more* recoverable of the two, which does **not** support the
intuition that domains are convention and modes are structure. What nothing here
supports is **five** or **fourteen**: the external anchors say 35 of 70 cells are
`disputed` against WordNet noun supersenses (D1) and 6 of 14 modes fail against
verb supersenses with `verb.social` (12.2% of verb mass) owned by no mode (D2a).
Internal coherence says the axes are real; external anchors say the counts are
convention.

**Applied.** `src/semiotic_grid.rs`: `DEFAULT_DOMAINS` is a pack, `domains()` is
the axis in force (`PHYSIS_DOMAINS`, comma-separated), `cell_count()` replaces
the hard 70, `is_canonical` and `fallback_anchors` follow the active axis,
`DOMAINS` kept as an alias. Modes stay fixed — the operative axis — until Track
D's EXPAND/MERGE/RETIRE proposals are adjudicated by a person.
`docs/WHAT_PHYSIS_IS.md` §1, §2 and rule 5 rewritten: "70 tables" becomes "70
under the default pack".

### 25.7 Then, in order

- **E56 positional** — build `T` and the five predicates over **position in
  text**; **shuffled-order null first** (see 25.7b).
- **E57 navigation** — traversal vs `TokenFixedRetriever` + cascade on the
  adversarial subset; the baseline is the shipped cascade, not a strawman.
- **E58 downstream** — LLM + top-k vs LLM + world-state region on the local
  fleet; contradiction detection not cosine-based.
- **E59 falsification suite** — one harness for the cases designed to kill the
  thesis, including where ordinary embeddings win.
- **Shell** — `physis world locate|neighbors|history|changed|compare|trace|why|
  retrieve|inspect`, thin, over machinery already measured. **The filesystem is
  overlaid, never replaced** (Gap 5: 34 years of semantic filesystems died in
  the write path). No shell before E55 and E56 have numbers.
- **Visualisation** — every figure generated from a `benchmarks/results/*.json`
  artifact by a script in the repository, artifact hash printed on the figure.
  No hand-drawn ontology diagrams in the paper.
- **Paper** — *Structure Before Mapping: Testing Cross-Representation World
  Models for Semantic Navigation*. Claims tagged demonstrated / measured /
  hypothesised / speculative; `NOT MEASURED` legal in every table; failures kept
  as a section.

### 25.7b Position in text, not wall-clock time (decision, 2026-09-14)

**The temporal dimension of Phase 25 is ordinal position in the text, not a
timestamp.** `W_p` is the state after reading sentence `p`; the sequence is the
reading order of the corpus.

Why this is the stronger choice, and not merely the cheaper one:

1. **It is always available.** Every document, transcript, log and source file
   has an order. Wall-clock metadata is missing, wrong or coarse for most of
   them, and a corpus with real timestamps would have to be authored, which
   makes the clock a property of the author rather than of the data.
2. **It is deterministic and re-derivable.** Position needs no parsing, no
   timezone, no extraction step and no interpreter — so H2's null (shuffle the
   order) is exact, and the whole temporal arm stays reproducible from the
   corpus file alone.
3. **It isolates the variable actually under test.** The claim is that *order*
   carries structure that order-blind similarity discards. Position tests
   exactly that. A clock adds duration, gaps and rate, none of which H2 claims.
4. **`becoming.rs` already works this way.** Its Wald–Wolfowitz runs test
   separates "one thing changed" from "two things share a name" from the
   *sequence* of occurrences, not from their timestamps. Phase 25 inherits a
   measured mechanism instead of inventing one.
5. **It composes with the clock later.** `TemporalValidity` stays exactly where
   it is, for facts that genuinely carry valid-time; position and clock are
   different coordinates on the same object and E56 can add the second one
   after the first has a number.

Consequences, applied throughout Phase 25: the corpus field is `pos`, states are
`W_p`, the five transition predicates are computed between adjacent **positions**,
H2's null shuffles sentence order, and the shell verb is `trace <obj> <pos-range>`.
Nothing in this phase claims a duration, a rate or a cause.

### 25.8 Standing constraints

1. No second `WorldState` — extend `transform.rs:252`.
2. No new embedder; verify call sites before trusting an existing one.
3. No `transplant` (zero call sites); never cite `benchmarks/ground-truth` as
   retrieval evidence (identical score under a lexical hash).
4. Correspondences are ranked proposals, never verdicts.
5. Contradiction is not cosine.
6. Freeze corpus and metrics before evaluation; record seeds, hashes, versions.
7. Every relation traces to an observation, or it is a bug.
8. A negative goes into the module header of the code it constrains, and stays.

---

## Phase 26 — Adaptive documents and the minimum sufficient world model

**Added 2026-09-15. Status: implementation handoff, not implemented or benchmarked.**
User direction: make Physis simpler, lighter, and more useful; consider the
existing uncommitted scaffold; begin with an adaptive text document. Investigate
one small frozen embedder and one tiny predictive/diffusion/flow model, but
require every learned component to beat cheaper alternatives. This section is
the worker packet; do not create another architecture-document hierarchy.

This phase adds a bounded Pro application to the current two-project development
cycle. It does not replace Physis's coordinate system, seven operations, historical
experiments, or the Core/Pro boundary. The Phase 25 position-only experiments
remain position-only; operational event sequence and observed timestamps below
are additional fields for this new experiment. The new encoder comparisons are
explicit research arms, not a reason to duplicate the production embedder.

### 26.1 Product outcome and order of work

The first demo is a document whose dependencies are visible:

> Change a source quantity from 18 units to 3. Preview linked values and exact
> calculations changing. See related prose that needs review. Click a proposed
> change to see its source. Accept or reject it. Undo the accepted repair
> without erasing the original human edit. Reload and obtain the same history.

The model is not the demo. Useful behavior must exist with no model loaded.

1. Exact blocks, bindings, revisions, dependencies, patches, and durable history.
2. One existing web surface with edit, preview, accept, reject, undo, and why.
3. BM25 and one frozen contextual embedder, evaluated rather than assumed useful.
4. Identity/lookup/ridge, then a tiny MLP predicting unresolved consequences.
5. Cheap probability models, then delta/residual diffusion or flow if justified.
6. Broader computer interaction only after the document workflow is reliable.

Initial exclusions: a new frontend framework, vector database, message bus,
standalone memory engine, global ontology redesign, generic multi-agent repair
framework, screenshot generation, encoder pretraining, and a continuously running
LLM. Do not delete existing product features to meet this scope. Keep the new
workflow small and isolate unrelated work.

### 26.2 Read the code before reusing the scaffold

The following are observations from the 2026-09-15 source review, not fresh build
results. Workers must recheck them at their actual starting revision:

| Location | Observed state | Worker instruction |
|---|---|---|
| `src/core.rs::edit_node` | Four non-test textual call sites were found, including CLI, web, agent and Studio paths | Reuse content-swap semantics where applicable; wrap with document revision/persistence rules. A graph label swap is not a complete file transaction. |
| `src/web/coherence.rs::coherence_edit_handler` | Calls `edit_node`; reads/re-embeds content; persists/audits | Inspect concurrency and failure handling before extending it. Do not claim an asynchronous embed followed by an unchecked swap is revision-safe. |
| `src/semantic_delta/mod.rs` | Local hash embedder; `no_semantic_change` assigns distance 1.0; model escalation sets a constant confidence | Do not use these values as semantic evidence. Reuse only corrected, tested behavior needed by this slice. |
| `src/dependency/mod.rs` | Edge types and propagation scaffold; incomplete scope logic and suspicious result construction | Retain explicit relation concepts. Prefer simple reachability for confirmed dependencies over uncalibrated attenuation. |
| `src/staleness/mod.rs` | Classification overlaps with `dependency` | One decision path for the MVP; do not maintain duplicate classifiers. |
| `src/repair_regions/mod.rs` | Selector had zero external callers; constructor call omitted `delta`; anchor logic contains invalid/incomplete code | Not a ready integration. Keep exact replacement ranges and confirmed locks; avoid a large region optimizer initially. |
| `src/orchestration/mod.rs` | Workers return fixed strings marked verified | Never treat this as a model or successful repair. No fake-worker fallback in a production path. |
| `src/metrics/mod.rs` | Precision/waste contain constants; some quantities mix tokens and nodes | Replace only metrics required by actual tests; use null/unknown when truth is absent. |
| `src/ported/system.rs`, `physis-core/src/observe.rs` | Existing observation read/append seams | Adapt the existing store; typed document payloads live in Pro. Read current code because relocation is ongoing. |
| `src/atomic.rs` | Existing durable replacement helper | Inspect its write/fsync guarantees and reuse rather than inventing another file writer. |

The uncommitted work identified by the user as Kilo's is shared work, not a clean
branch owned by a new worker. Capture the starting diff. Do not claim authorship
from file names, restore the tree to HEAD, or delete the six modules wholesale.
The earlier free-model reviews were advisory: claims that flow is automatically
one-step, that mixtures are too heavy, or that a blank-line parser solves all
Markdown were rejected. Review actual semantics rather than copying those plans.

### 26.3 Worker protocol and ownership

Current authorization is to append this plan. This packet does not itself start
workers or authorize deployment. When the coordinator starts implementation,
give each worker one bounded packet, explicit file ownership, and an exit gate.

**Before editing:**

1. Read applicable `AGENTS.md`, the Physis skill, this phase, and the current
   [semantic-computer contract](docs/semantic-computer/CONTRACT.md).
2. Follow the pansa/randomesh workflow for code changes, builds and tests. Verify
   host, checkout and branch; an existing executable may predate current source.
3. Record commit IDs, dirty tracked/untracked paths, assigned files and baseline
   failures. Use an isolated checkout when appropriate, preserving relevant
   uncommitted changes intentionally rather than silently dropping them.
4. Recall the existing workspace record through available Physis history/pack/read
   tools. If unavailable, report the missing connection and use exact source reads.
5. Reconfirm file paths after the Core/Pro migration. Treat old docs as navigation
   aids, not proof that a symbol exists, runs, or belongs in Core.
6. Never print, attach, commit, or train on `.env` values. Use existing authenticated
   free routes where available. Select any needed credential without sourcing the
   whole file into a shell or sharing all credentials with a worker.

**Ownership and dependency graph:**

| Packet | Owner role and files | Depends on | Exit artifact |
|---|---|---|---|
| P0 | Coordinator: baseline audit, contract names, shared `src/lib.rs`/Cargo/routes, integration decisions | Nothing | Frozen interfaces, fixture, baseline transcript, assigned files |
| P1 | Document worker: one small `src/adaptive_document/` module and focused integration tests | P0 | Stable identities, bindings, patch logic, persistence/recovery/undo tests |
| P2 | Integration worker/coordinator: existing web handler/service adapter and shared registrations | P1 contract; integrate after P1 | Actual request→service→persistent effect→readback |
| P3 | UI worker: selected existing page/assets only | P0 frozen wire fixture; live integration after P2 | Visible edit/preview/why/accept/reject/undo behavior |
| P4 | Evaluation worker: `benchmarks/semantic-computer/adaptive-document/` and fixtures | P0 event schema | Ground-truth suite, export checker, real per-case results |
| P5 | Research worker: `research/adaptive-document/` experiments and pinned experiment configs | P4 dataset; product need not wait | Baselines, embedding/delta tests, gated tiny-model comparison |
| P6 | Reviewer: read-only source/runtime audit and concise handoff | P1–P4 | Verified findings, regressions, unresolved risks; no self-certified score |

Parallelize P3's fixture-based interface work and P4's test corpus after P0.
P1 owns document semantics; P2 owns shared wiring. Do not let several workers
edit shared handlers, Cargo, or module exports concurrently. Coordinate expensive
builds on the laptop; multiple harnesses are not a reason to run duplicate builds.
P5 starts only when real input/output contracts are stable. Workers do not spawn
additional workers unless their coordinator explicitly assigns that authority.

Prefer small existing files or one cohesive document module. The packet names
are ownership boundaries, not a demand to create seven modules, services, or
permanent design documents. Preserve Core as a reusable mechanism library.

### 26.4 Frozen minimal contracts for P0/P1

Use existing ID/reference/event wrappers where they fit. The pseudocode below
defines required semantics, not an instruction to duplicate repository types.

```text
Block:
    id: stable opaque identity
    document_id, revision, content_hash, raw_ref
    kind: paragraph | heading | code | other_supported
    span: [utf8_byte_start, utf8_byte_end) at document_revision
    authored_by, locked: explicit metadata, not inferred from proximity

Link:
    id, source_block_id, dependent_block_id
    behavior: copy | calculate | review
    binding: named source/target fields or explicit bound spans
    transform_id?: registered pure transform (never arbitrary code)
    provenance: human_confirmed | parser_proven
    source_revision_at_confirmation, revision, active

Change:
    event_id, document_id, block_id, before_revision, after_revision
    before_raw_ref, after_raw_ref, before_hash, after_hash
    actor, operation, sequence, parent_event_ids
    occurred_at?: timestamp, recorded_at: timestamp

Patch:
    id, document_id, triggered_by_change_id
    expected_document_revision, expected_graph_revision
    expected_block_revisions, expected_source_hashes
    replacements: [{block_id, range, expected_old_bytes, new_bytes}]
    review_only_block_ids, reasons: [{link_id, source_change_id}]
    validators: named exact checks
    status: proposed | applied | rejected | conflicted | failed | recovery_needed
    application_event_id?, superseded_by?, reverted_by?
```

Patch IDs and idempotency keys are stable across retries. A network retry is not
a second application. Empty/no-op edits create no semantic-change claim. A
rejected proposal remains in history and has no document mutation.

Direction is **source → dependent** in this adapter. Map existing `DependsOn`
conventions explicitly; do not reverse a relation accidentally. Quote, citation,
similarity, ownership and causal dependency are distinct. Only confirmed copy or
calculation bindings produce automatic replacement candidates. A `review` link
produces a reason to inspect, not invented replacement prose.

Use integers or exact decimals with explicit units for bound quantities. Do not
round-trip engineering quantities through an embedding or unqualified `f32`.
Derived text must distinguish a live bound value from a historical quotation.

Retain exact byte offsets at a named revision. Convert browser UTF-16 indices at
the API boundary; never apply them directly as Rust UTF-8 byte indices. The
server verifies boundaries and old bytes. Reject overlapping replacement ranges.
Apply multiple non-overlapping replacements from right to left.

The first fixture may declare stable IDs and bindings in an application sidecar.
Support a documented subset of Markdown before general parsing. Blank lines
inside fenced code are not paragraph boundaries. A moved block retains identity
only with explicit editor lineage or an unambiguous match. Ambiguous split/merge
or external replacement invalidates affected bindings for review.

### 26.5 P1 guide: dependency and proposal logic

Use explicit ordered traversal, deduplicate by ID, and cap work per request.
Reachability is exact for the declared graph; the graph itself may be incomplete.
There is no claim that all semantic dependencies have been found.

```python
def propose_after_source_edit(snapshot, observed_change):
    assert observed_change.after_revision == snapshot.revision
    affected = reachable_dependents(
        snapshot.confirmed_links, observed_change.block_id,
        visited=set(), stable_order=True, max_nodes=MAX_AFFECTED)
    proposal = Patch(bound_to=snapshot, triggered_by=observed_change.event_id)

    # Calculation edges need dependency order, not arbitrary BFS evaluation.
    order, cyclic_groups = topological_order(affected.calculation_graph)
    proposal.mark_review(cyclic_groups, reason="cyclic calculation")
    preview = snapshot.read_only_copy()
    for block in order:
        links = affected.confirmed_inputs(block)
        if block.locked:
            proposal.mark_review(block, reason="locked dependent")
        elif all_inputs_bound_and_supported(links):
            new_bytes = registered_exact_transform(links, preview)
            proposal.add_exact_replacement(block, new_bytes, reasons=links)
            preview.replace_in_memory(block, new_bytes)
        else:
            proposal.mark_review(block, reason="unresolved dependency")

    proposal.mark_review(affected.review_only, reasons=affected.source_paths)
    reject_overlaps_or_conflicting_writers(proposal)
    validate_preview_and_unchanged_regions(snapshot, proposal)
    persist_proposal_metadata(proposal)  # does not change document bytes
    return proposal
```

For a copy chain, use values from the in-memory preview so downstream copies see
the proposed upstream value. When two sources disagree, report a conflict rather
than selecting the last traversal result. Cycles among review-only links may be
traversed once; cycles among calculations require review, not guessed iteration.
Cap traversal with an explicit incomplete-result flag; never silently drop nodes.

Do not merge disconnected affected spans into one broad rewrite region. A patch
can contain several disjoint replacements while preserving every intervening byte.
Keep human-authored/locked content metadata separate from staleness and route.

### 26.6 P1 guide: persistence, concurrency, and undo

The raw document remains authoritative. Reuse the existing observation store and
durable file writer; add a narrow Pro payload adapter and immutable content refs.
An index/graph/embedding cache is a rebuildable view, not another source of truth.
An event body's summary alone is insufficient to restore prior text.

Minimum application transaction:

```python
def apply_patch(request, store, workspace):
    with workspace.document_transaction(request.document_id):
        previous = store.lookup_operation(request.idempotency_key)
        if previous is terminal:
            return previous.result
        if previous is pending:
            return reconcile_pending(previous)  # never blindly reapply

        patch = store.read_patch(request.patch_id)
        require(patch.status == "proposed")
        current = workspace.read_exact_document_and_metadata()
        require_current_revisions_hashes_and_locks(current, patch)
        candidate = apply_nonoverlapping_ranges(current.bytes, patch.replacements)
        require(exact_outside_range_equality(current.bytes, candidate, patch))
        require(all_registered_validators_pass(candidate, patch))

        before_ref = store.put_immutable(current.bytes)
        after_ref = store.put_immutable(candidate)
        store.append_durable("apply_intent", key=request.idempotency_key,
                             patch_id=patch.id, before_ref=before_ref,
                             after_ref=after_ref, expected_revisions=current.revisions)
        # Recheck external file state immediately before the replacement.
        require(workspace.current_hash() == current.hash)
        workspace.replace_atomically_and_sync(candidate)
        actual = workspace.read_exact_bytes()
        require(hash(actual) == hash(candidate))
        event = store.append_durable("apply_observed", key=request.idempotency_key,
                                     before_ref=before_ref, after_ref=after_ref)
        update_materialized_views_or_mark_dirty(event)
        return applied_result(event, validators="verified")
```

Worker must specify real locking guarantees. A lock within Physis serializes
Physis clients but does not make arbitrary external editors cooperate. Initial
support is one managed document edited through the service; externally changed
files are detected and conflicted. Do not claim fully atomic compare-and-swap
against uncooperative writers. General filesystem concurrency is a later task.

If writing or validation fails, record the observed failure where possible and
return failure. If document replacement succeeds but terminal logging fails,
return/recover `recovery_needed`, not "nothing happened". On restart inspect each
pending intent: current hash equals intended after → record recovered observation;
equals before → record not-applied; equals neither → conflict requiring inspection.
Never reconstruct an application merely from an intent record.

Do not publish in-memory success while durable state failed. A failed cache update
does not reverse a successfully committed raw write; mark the cache stale and
rebuild it from observed events. Keep event order by store sequence; timestamps
need not be unique or monotonic across processes.

```python
def propose_undo(application_event, current):
    original = read_verified_application(application_event)
    # Undo is a new compensating patch, not deletion of history or revision rewind.
    inverse = swap_old_and_new_ranges(original.replacements)
    if not ranges_and_revision_still_match(current, original.after):
        return conflict("Document changed since this repair; inspect inverse patch")
    return new_patch(inverse, bound_to=current,
                     reason={"undo_of": application_event})
```

Initially reject undo after any intervening document revision. Later permit
non-overlapping edits only with tested range/identity rebasing. Rejecting a stale
undo is safer and simpler than overwriting a later user edit. Source edit and
dependent repair are separate events; undo of the latter preserves the former.

### 26.7 P2/P3 guide: one service, one visible workflow

Expose service operations once: `snapshot`, `edit_source`, `propose`,
`decide_patch(accept|reject)`, `propose_undo`, `history`, `why`.
Route names are frozen by P0 after inspecting current handlers. Reuse existing
transport conventions, but do not overload `coherence/edit` with untyped patch
verbs merely to avoid adding a narrow document endpoint. Existing callers must
retain their behavior. New request fields require validation and versioning.

Mutating requests carry document/patch identity, expected revisions, and an
idempotency key. Use existing authorization and user/workspace resolution;
document and proposal state must be scoped the same way as the authenticated
resource. A user-selection header alone must not become new authorization logic.
Do not silently bypass permissions, locks, or revision checks through an older
CLI/agent route. Any alternate writer invalidates/reconciles document views.

```text
UI edit submission:
    send edited block + expected revision + operation ID
    show source save result
    request/render dependent preview bound to the returned revision
    display exact replacements separately from review-only notices

UI acceptance:
    send patch ID + expected revision + new operation ID
    applied -> render returned observed revision
    conflict -> keep user text, refresh state, request a fresh preview
    failed/recovery_needed -> show observed status, do not pretend rollback

UI rejection:
    record rejection; keep document bytes; retain source edit

UI why:
    show source change, named binding/transform, affected passage, and event refs
```

Do not compute binding logic twice in JS and Rust. Client fixtures are temporary
test data and must not appear as live outcomes. Do not render predicted vectors,
confidence scores, worker names, or ontology internals in the default document
flow. Show practical reasons and actual outcomes. Keyboard access, focus after
updates, escaped document text, and a readable diff are part of P3 acceptance.

Visual scope: a restrained highlight on affected text, clickable source links,
an inline before/after preview, and one undo action. Full graph visualization,
animated agents, extra dashboard tabs, and a separate chat panel are unnecessary.
A revision scrubber may follow once replay works; never display guessed history.

### 26.8 P4 guide: fixture, acceptance tests, and data export

Build a resettable fixture with independent ground truth:

```text
B1: source quantity = 18 units
B2: bound copy of quantity = 18 units
B3: bound calculation, 2 × quantity = 36 units
B4: prose recommendation linked as review-only
B5: unrelated historical quotation containing the number 18
B6: locked text with a review dependency

human edit B1: 18 -> 3
expected proposed replacements: B2 18 -> 3, B3 36 -> 6
expected notices: B4 review; B6 locked/review
expected unchanged bytes: B4/B5/B6 and all outside replacement ranges
undo of accepted dependent repair: B2/B3 revert; B1 stays 3
```

Create adversarial variants: negation, quantity units, repeated identical blocks,
emoji/accented text, fenced code, block moves, split/merge, disconnected dependents,
cycles, multiple conflicting inputs, stale graph revision, external edit, missing
blob, rejected patch, duplicate request, failed write, and interrupted commit.

| ID | Required observable behavior |
|---|---|
| AD01 | Supported block parsing preserves exact bytes and stable IDs; unsupported forms are explicit. |
| AD02 | Source edit produces one observed transition with correct before/after refs. |
| AD03 | Only confirmed dependents are traversed; unrelated repeated numbers are untouched. |
| AD04 | Copy/calculation preview produces exactly 3 and 6 for the canonical fixture; review links invent no prose. |
| AD05 | Disjoint patches preserve all outside bytes; overlaps and conflicting writers are rejected. |
| AD06 | Locked content cannot be changed through any document mutation route. |
| AD07 | Stale document, block, source or graph revisions cause conflict before application. |
| AD08 | Repeating an idempotency key applies once and returns the recorded result. |
| AD09 | Rejection changes proposal history and zero document bytes. |
| AD10 | Undo restores only the dependent repair; intervening edits cause conflict. |
| AD11 | Restart rebuilds the same observed document metadata/proposals; lost raw blobs are reported. |
| AD12 | Interruptions before/after replacement recover according to observed hashes; no false success. |
| AD13 | Cycles terminate; calculations in cycles require review; traversal caps report incompleteness. |
| AD14 | Two user/workspace contexts cannot read/apply each other's patches. |
| AD15 | Actual UI→API→service→store round trip produces the fixture outcome and readable provenance. |
| AD16 | Events with equal/out-of-order timestamps replay by store sequence; export preserves attribution. |

Ground truth lives in fixture expectations, not in the implementation's own
affected-set output. A test that a patch is nonempty does not establish that it
is correct. A parser's line-count test does not establish identity preservation.
Record all attempted cases, including timeouts and abstentions.

Transition export requirements:

```text
schema_version, event_id, sequence, parent_event_ids
workspace_id, session_id, document_lineage_id, object_id
before_revision, after_revision, before_raw_ref, after_raw_ref, content_hashes
actor_kind, operation_kind, typed_args, action_payload_ref
action_attribution: exact | inferred | unknown
decision_cutoff_sequence, precondition_ref, dependency_snapshot_ref
outcome: observed | failed | partial | cancelled | unknown
prediction_ref?, patch_ref?, validator_results, human_feedback?
encoder_id?, encoder_revision?, preprocessing_hash?, vector_refs?
execution_mode: fixture | real_local | real_remote
```

Export only the selected consented workspace. Credentials, unrelated private
paths, hidden personal data and provider configuration are excluded. Snapshot
watchers may miss intermediate edits; infer neither exact actions nor causal
parents from time proximity. Missing action labels remain missing.

### 26.9 P5 guide: encoder selection and operation-delta experiment

Read primary sources before implementing. Reuse the existing production
`VectorEmbed`/ONNX seam after verifying the actual loaded encoder. Model absence
may fall back to a visibly lexical/BM25 path, never a hash presented as semantic.
Record checkpoint revision, tensor count, tokenizer, prefix, pooling, precision,
truncation and cache keys. Frozen model memory counts toward system size.

Initial candidates and arithmetic FP32 weight floors:

| Candidate | Approximate parameters / dimensions | Role |
|---|---|---|
| [Potion-base-8M](https://huggingface.co/minishlab/potion-base-8M) | 8M / 256; ~32MB | Static pooling control; operation/order limitations must be tested. |
| [MiniLM-L6-v2](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2) | 22.7M / 384; ~91MB | First contextual candidate; short blocks, default 256 word-piece limit. |
| [BGE-small](https://huggingface.co/BAAI/bge-small-en-v1.5), [E5-small](https://huggingface.co/intfloat/e5-small-v2), [GTE-small](https://huggingface.co/thenlper/gte-small) | ~33.4M / 384; ~134MB | Small retrieval alternatives; obey each model's preprocessing. |
| [Nomic v1.5](https://huggingface.co/nomic-ai/nomic-embed-text-v1.5) | ~137M / up to 768 | Supported dimension-reduction/longer-context upper control. |
| [EmbeddingGemma](https://huggingface.co/google/embeddinggemma-300m) | ~300M nominal / up to 768 | Multilingual upper control, not default. |
| [BGE-large](https://huggingface.co/BAAI/bge-large-en-v1.5) | ~335M / 1024 | Native 1024-d upper control only. |

These are candidate specifications, not measured RAM or throughput. Confirm
actual tensor counts before publication. Test dimensions 128/256/384/512/768/1024
with train-fitted PCA or explicitly supported Matryoshka reductions. Separate
within-model dimensionality from across-model capacity. Do not pad vectors and
call that an information-preserving dimension experiment.

One encoder maps multiple regions: `Z_t = {E(x_i,t)}`. IDs, exact facts, order,
timestamps and graph structure remain external. DOM/text serialization is a
textual-computer experiment, not proof of universal image/audio representation.

```python
groups = split_by_workspace_lineage_session_then_forward_time(raw_events)
train, validation, test = freeze_split_manifest(groups)
E = load_frozen_encoder(pinned_revision, documented_preprocessing)
cache_key = (raw_content_hash, encoder_revision, preprocessing_hash)
vectors = encode_unique_revisions(E, cache_key, inference_mode=True)

for transition in train:
    z0, z1 = vectors[transition.before], vectors[transition.after]
    delta = z1 - z0  # FP32 subtraction; do not normalize away magnitude
    add_operation_example(transition.operation, transition.operands, delta)

prototypes = training_only_operation_means()
evaluate_on_unseen_objects_templates_and_operands(prototypes, validation)
# Test set is opened only after methods and thresholds are frozen.
```

Required delta controls: identity; character/token edit distance; inserted-token
embedding; per-operation mean; same entity/different operation; same operation/
different entity; magnitude/length-matched shuffled labels; token-order shuffle;
hash/random-projection encoder. Report zero/near-zero delta cases separately.
Test addition, removal, quantity magnitude, negation, scope, rename/move,
composition and inverse operations. A shared inserted noun is not evidence of
reusable operational geometry.

Score same-operation vs different-operation AUROC, direction and magnitude,
held-out future retrieval, and downstream affected-region quality. Do not require
embeddings to store object identity or exact arithmetic; assess the complete
learned-plus-explicit system as well as representation-only ablations.

### 26.10 P5 guide: tiny world-model math and pseudocode

The research state is `S_t=(x_t,Z_t,G_t,H_t)`. Predict unresolved consequences:
`p(z_next | z_current, action, available_context)`.

Three distinct tasks must not be conflated:

1. Pre-action forecasting: no post-action state is available in the condition.
2. Post-source-edit dependency repair: source after-text is valid input; the
   dependent's eventual repaired text is still unavailable.
3. Exact known patch: apply to raw text and embed directly. This is a mandatory
   baseline, not a world-model achievement.

Start with `c = [z_current:384, E(action_payload):384, explicit_features:64]`.
The explicit features encode operation/role masks, counts and small historical
summaries, not arbitrary numeric encodings of object IDs. Neighbor-token/history
arms may follow, with identical available evidence in all comparison arms.

```text
future target:   y_A = z_next
delta target:    y_B = z_next - z_current
residual target: y_C = z_next - f_cheap(c)

ridge: [c;1] -> delta                         ~319,872 coefficients
MLP:   832 -> 512 -> 512 -> 384              ~886,144 parameters
flow:  [y_noisy:384;c:832;time:32]
       -> 480 -> 480 -> 384                 ~1,015,104 parameters
```

Counts above include linear biases and are arithmetic design estimates; count
actual tensors in implemented models. Use fixed sinusoidal time features and a
simple nonlinear MLP first. No text tokenizer/vocabulary head in W. A roughly
1M W plus 22.7M E is a roughly 23.7M total learned system, not a 1M system.

Given conditioning, state/delta/residual targets are translations. They do not
intrinsically remove conditional information. Any capacity gain must come from
measured conditioning/optimization/inductive bias, not that arithmetic identity.
Generate residual training targets from chronological/out-of-fold predictions of
the cheap arm, not its overly optimistic in-sample fit. Freeze it for evaluation.

```python
# Conditional flow matching; pseudocode, not production implementation.
target = train_only_scale(raw_target)  # inverse transform saved with artifact
s = uniform(0, 1, batch)
noise = normal_like(target)
y_s = (1 - s) * noise + s * target
velocity_target = target - noise
loss = mean_square(W(y_s, time=s, condition=c) - velocity_target)
optimizer_step(loss)

def sample_flow(c, K, nfe):
    y = gaussian_noise(K, latent_dim)
    for s in fixed_grid(nfe):
        y = numerical_solver_step(W, y, s, c)
    target_samples = inverse_scale(y)
    return reconstruct_future(target_samples, target_kind, z_current, cheap(c))
```

For diffusion compare `y_tau = alpha_tau*y + sigma_tau*epsilon` and
`MSE(epsilon_theta(y_tau,tau,c),epsilon)` using an existing scheduler/sampler.
Future, delta and residual arms use the same backbone, optimizer/tuning budget
and raw-space scoring. Flow inference usually requires integration; a one-step
Euler arm is an experiment, not a guaranteed quality shortcut.

Compare identity, observed-delta lookup, ridge, MLP, GRU/linear state-space,
small causal transformer, residual bootstrap, diagonal Gaussian, 2/4/8-component
mixture-density network and finite-candidate energy scoring before diffusion.
An energy model normalized over retrieved candidates is a candidate distribution,
not an implicitly normalized density over all possible states.

For probabilistic evaluation use K>=2 sample energy score, sliced CRPS and known
branch probabilities in resettable stochastic fixtures. Means can lie between
incompatible futures; best-of-K is diagnostic only. A selection rule may not see
the realized future. Record sample count and actual network evaluations (NFE).
Report retrieval after normalization separately from original-space vector error.

### 26.11 Research implementation references and reuse decisions

References were inspected during planning; workers must pin current revisions,
check licenses and verify checkpoint/runtime compatibility before use. Paper
results are author reports, not reproduced Physis results.

| Primary source / repository | Direct reuse and limit |
|---|---|
| [LeWorldModel](https://arxiv.org/abs/2603.19312), [official code](https://github.com/lucas-maes/le-wm) | Small action-conditioned prediction/planning; ~15M visual-model precedent, not a text checkpoint. |
| [DINO-WM](https://github.com/gaoyuezhou/dino_wm) | Frozen encoder plus learned dynamics; reuse architectural recipe. |
| [Flow-JEPA](https://arxiv.org/abs/2608.29029), [code](https://github.com/HuoYanchen/Flow-JEPA) | Latent trajectory flow comparison; RTX 6000 Ada experiments do not establish 8GB laptop feasibility. |
| [DIAMOND](https://github.com/eloialonso/diamond) | Conditional diffusion and rollout evaluation; Atari pixels differ from document latent states. |
| [Diffusion Forcing](https://arxiv.org/abs/2407.01392) | Per-position noise and multi-step robustness if history rollouts justify it. |
| [Meta flow_matching](https://github.com/facebookresearch/flow_matching) | Preferred existing probability paths and solvers. |
| [DiT](https://github.com/facebookresearch/DiT) | Conditioning/denoiser blocks only if multiple tokens justify a transformer. |
| [TD-MPC2](https://github.com/nicklashansen/tdmpc2) | Compact dynamics/planning comparisons; no mandatory RL stack or large multitask dataset. |
| [LeJEPA](https://github.com/galilai-group/lejepa) | Anti-collapse representation learning only if frozen E fails. |
| [Mamba](https://github.com/state-spaces/mamba) | History compression upper comparison after GRU/explicit history; include kernel/runtime costs. |
| [Consistency](https://github.com/openai/consistency_models), [Shortcut](https://github.com/kvfrans/shortcut-models) | Accelerate a model that already wins; count teacher/training cost. |
| [CUWM](https://arxiv.org/abs/2602.17365), [WebDreamer](https://github.com/OSU-NLP-Group/WebDreamer) | Computer-action evaluation and planning tasks; large-model simulators stay external comparison arms. |
| [V-JEPA 2](https://arxiv.org/abs/2506.09985), [GAIA-2](https://wayve.ai/wp-content/uploads/2025/03/GAIA_2_Technical_Report.pdf) | Mechanism references; do not inherit billion-parameter production designs. |

Use NumPy/scikit-learn for linear/PCA baselines, PyTorch for W experiments,
Sentence Transformers for reference E, and the existing ONNX seam for deployment
comparison. Prefer ordinary configs and a result JSON file to a new experiment
orchestration platform. Verify exact dependency versions before adding them.

Text repair is a **separate** track: exact transforms → retrieval/substitution →
[FLAN-T5-small](https://huggingface.co/google/flan-t5-small) (~77M) or
[DistilRoBERTa](https://huggingface.co/distilbert/distilroberta-base) (~82M) →
[SmolLM2-135M](https://huggingface.co/HuggingFaceTB/SmolLM2-135M-Instruct) →
[MDLM](https://github.com/kuleshov-group/mdlm),
[SEDD](https://github.com/louaaron/Score-Entropy-Discrete-Diffusion),
[Block Diffusion](https://github.com/kuleshov-group/bd3lms).
Check exact checkpoint sizes. Compare masked completion, variable-length span
infilling, small AR and text diffusion on the same bounded repair tasks. Clamp
context, validate exact required facts and outside bytes, and count editor memory
separately. A latent forecast cannot magically decode itself into correct text.

### 26.12 Ablations, routing, and resource gates

Run a staged matrix rather than its full Cartesian product:

| Stage | Arms | Primary question |
|---|---|---|
| A | Exact/BM25; E only; E+order; E+graph; E+order+graph; BM25+order+graph | Does learned geometry add useful information? |
| B | Identity/table/retrieved delta/ridge/MLP/GRU/transformer | Is dynamics prediction necessary and nonlinear? |
| C | Bootstrap/Gaussian/mixture/energy | Do distributions improve decisions cheaply? |
| D | Future/delta/residual diffusion and matching flow arms | Does diffusion/target choice earn iterative inference? |
| E | One E versus specialists; 128–1024 dimensions | Does a common encoder justify its compromises? |
| F | Exact/substitution/MLM/infiller/AR/text diffusion | Which local repair method is smallest and adequate? |
| G | Fixed routing, explicit-dependency heuristic, calibrated surprise, random matched-cost routing | Does compute allocation improve quality per cost? |

Capacity ladder for W: **1M, 5M, 10M, 25M, 50M, 100M, 250M**. Screen 1M/5M;
scale only with evidence of underfitting and adequate data. Keep frozen E, learned
projections, ensembles, editors and teacher models in separate and total counts.
Within-family comparisons match parameters; deployment comparisons match total
latency/memory/cost. Equal parameter count alone is not equal computation.

Targets, not benchmark results: exact edit/highlight p95 <100ms; incremental
short-block E+tiny prediction p95 <250ms; optional background planning <1s.
Measure actual hardware, threads, runtimes, precision, token length, batch size,
cold load and warm latency. Synchronize GPU timing and include transfers. Report
both current Physis process memory and incremental cost of the new workflow.

| W parameters | FP32 weights | Approximate FP32 Adam floor including gradients/moments |
|---:|---:|---:|
| 1M | 4MB | 16MB |
| 5M | 20MB | 80MB |
| 10M | 40MB | 160MB |
| 25M | 100MB | 400MB |
| 50M | 200MB | 800MB |
| 100M | 400MB | 1.6GB |
| 250M | 1GB | 4GB |

These are arithmetic floors, not measured RAM/VRAM requirements. Add E,
activations, optimizer temporaries, buffers, cache and framework. Initial 1–5M
cached-vector experiments have a provisional 1–3GB RAM and optional 1–3GB VRAM
allowance, to be replaced by measurements. The 250M arm may not fit training in
8GB and is never an MVP dependency. At 384-d FP32, 100k vectors occupy 153.6MB;
100k before/after/action triples occupy 460.8MB before metadata. Index shared
revisions and stream batches. Check FP16 delta cancellation before compressing.

Freeze E; train W on cached vectors, batch 64–256, roughly 10–30 epochs with
validation early stopping. Begin with 2k–10k valid transitions if available.
Measure a 200-step warm pilot before predicting training duration. A planning
budget of one GPU-hour or a few CPU-hours per first 1M trial is a limit to test,
not a claim about throughput. Data collection may take longer than model fitting.
No dataset size is claimed available by this plan.

Compute cost includes encoding + retrieval/graph + candidates × horizon × K ×
NFE × predictor work + validators + editor/API work. Batching eight samples at
eight NFE does not remove their 64 evaluations per candidate step. Report energy
with sensor scope and idle subtraction; if no meter/telemetry exists, record
`NOT MEASURED`. TDP times latency is not an energy measurement.

Routing starts from explicit policy and cheap empirical outcome tables. Later
attention may combine counted constraints, dependency degree, relevance and
calibrated uncertainty. Ablate terms. Actual surprise `d(prediction, observation)`
is known only after observation; it updates subsequent routing, not the earlier
decision. High surprise may mean missing state, drift or unstable embeddings,
not falsehood. Low cosine change cannot certify unchanged numbers or negation.
The human is a distinct privileged actor with raw-state access and override,
not an expensive model score. External LLMs are tools for validated local gaps.

Measure tokens/API calls on paired workflows at matched verified quality,
including retries and routing overhead. Separate observed savings from offline
"would have called" estimates. Count rejected proposals, abstentions and failures.

### 26.13 Decision gates and publication requirements

Freeze primary outcomes and margins before final test evaluation. Suggested gates:

- **No learned model:** ship exact/BM25 if it meets the product acceptance and
  retrieval/affected-region quality targets. Models are not required for completion.
- **No learned W:** retain E+table/ridge if more complex dynamics do not improve
  validated decisions at an acceptable cost.
- **No diffusion:** keep a cheaper distribution if diffusion/flow has no meaningful
  proper-score and downstream benefit, requires excessive NFE, or wins only under
  best-of-K selection or unchanged-dominated average error.
- **Diffusion promotion:** predeclare a practical decision benefit such as at
  least 3 percentage points higher validated success at equal cost, or at least
  20% lower cost with quality within 2 points; require grouped confidence intervals,
  no critical-regression increase and product latency compliance. A 10% relative
  proper-score gain supports but cannot replace useful downstream behavior.
- **Universal E rejection:** specialists retain >2 aggregate quality points or
  >5 critical-domain points advantage, with uncertainty quantified, after exact
  facts/order/chunking improvements. Compare equal observations and total resource
  budgets. Additional screenshots in only one arm are an input ablation.

One failed task regime refutes a deployment choice, not all diffusion or all
embeddings. Publish an inconclusive result honestly. Try a small projection or
adapter before retraining E; never mix different encoder spaces merely because
their dimensions match.

Data splits group by workspace/document lineage/session and forward time. Fit
PCA, scalers, delta prototypes, residual models and thresholds on train/validation
only. Separate natural traces from synthetic fixtures. Resettable stochastic
fixtures need known branch probabilities and repeated interventions; one sample
per condition cannot establish multimodal future coverage or causality.

Benchmarks: retrieval Recall@5/nDCG; semantic-change critical misses; dependency
precision/recall; operational delta transfer/composition; prediction at horizons
1/2/4/8; probabilistic calibration; validated planning; correct local repair and
outside-byte equality; routing risk/coverage; end-to-end latency/RAM/VRAM/energy.
Use three seeds for screening and five for final neural comparisons; resample
independent sessions/workspaces, not adjacent correlated transitions. Keep a
sealed external workspace/task family and publish per-case failures.

Publishable question: **What is the minimum learned predictive geometry still
needed once exact structure and history are supplied?** Release sanitized,
license-compatible fixtures/transitions, versioned contracts, reproducible arms,
resource measurements and failure cases. Human acceptance is preference evidence;
exact validators or independent blinded review establish correctness. Do not use
an LLM judge alone as ground truth or present template success as general computer
competence. Publication is a later milestone, not a one-week promise.

### 26.14 One-day, one-week, and computer-interface milestones

**One-day fixture:** P0 contract + a narrow P1 service + a minimal preview on the
selected existing surface if integration is ready. Exact copy/calculation,
review-only notices, revision-safe application/rejection/undo and persisted
readback. If baseline build/integration issues consume the day, report a tested
service fixture and the unfulfilled UI gate explicitly. No line-count promise.

**One-week local MVP:** finish P1–P4, actual UI/API/store round trip, restart and
crash tests, bounded normal-use collection, optional frozen-E/BM25 comparison.
Tiny-model trials are conditional on data and baseline readiness. Do not postpone
durable history until after the demo; it is part of the learning data contract.

**Computer-interface path:** one document → linked documents → files/Git →
registered commands → DOM/accessibility observations → optional visual encoder.
Each expansion retains stable object/revision/action/observation contracts and
gets its own fixture and actual-effect validators. Do not synthesize a screenshot
when the useful state is already available as text/DOM. No general computer
controller or automatic command execution is implied by the document milestone.

### 26.15 Launch guidance and worker handoff template

For future delegated implementation, prefer the authenticated Kilo/OpenCode free
routes actually available then. On 2026-09-15 the planning delegates used
`kilo/nvidia/nemotron-3-super-120b-a12b:free` through Kilo and
`opencode/mimo-v2.5-free` through OpenCode; both reported zero cost for the planning
runs. This is session evidence, not a guarantee of future price, access or provider
identity. Check catalog/billing metadata before launching; do not silently replace
an unavailable free route with a paid model. Never cycle credentials to evade limits.

Example command shapes only; verify current CLI help and configuration first:

```sh
# Read-only design/source review; the coordinator supplies a bounded task file.
opencode run --pure --agent plan --model PROVIDER/FREE_MODEL \
  --dir /path/to/assigned/checkout --file /tmp/physis-worker-task.md \
  'Read the task file and complete only its read-only review.'

# Implementation only after the coordinator assigns code work and file scope.
kilo run --pure --agent build --model PROVIDER/FREE_MODEL \
  --dir /path/to/assigned/checkout --file /tmp/physis-worker-task.md \
  'Read the task file and implement its assigned packet through its exit gate.'
```

Do not add auto-approval/bypass flags. Preserve existing permission boundaries;
request environment escalation when required, with the real reason. Free-model
availability is not worker completion. Inspect the returned artifact and tool
trace. In the planning run, Kilo wrote a native plan artifact while its final
message only pointed to it; a coordinator must read such artifacts before using
their conclusions. No secrets in task files or retained transcripts.

Task-file template:

```text
Packet: P<number> / short name
Goal: one observable user or experimental outcome
Starting host/checkout/commit: ...
Mode: READ-ONLY REVIEW or IMPLEMENTATION (one, explicit)
Read: applicable AGENTS.md, Physis skill, PLAN.md Phase 26, named source files
Own: exact file paths; shared registrations belong to the coordinator
Inputs: pinned contract/fixture/dataset revisions
Do: numbered bounded changes and mandatory failure cases
Do not: unrelated fixes, new stores/frameworks, fake results, secrets, spawning
Verification: exact relevant checks and actual runtime entrypoint
Exit gate: behavior plus evidence required to accept this packet
Handoff: changed paths, executed commands/status, measured results/denominators,
         remaining gaps, migration notes, dependencies for the next packet
```

### 26.16 Verification guide and definition of done

Coordinator first identifies an appropriate feature/build command for current
source. Use focused tests before broader repository gates; don't mask baseline
failures, use stale binaries, or repeatedly run unrelated expensive suites.
Example shapes for **later implementation on the designated host**, after test
targets exist and current feature names have been checked:

```sh
git status --short
git diff --check
cargo test --offline -p physis_pro --lib adaptive_document
cargo test --offline -p physis_pro --test adaptive_document
python3 benchmarks/semantic-computer/check_contract.py

# Use the installed helper if working; otherwise its actual script directory.
# OUT prevents the module-map check overwriting unrelated tracked documentation.
OUT=/tmp/physis-adaptive-module-map.md physis-check all src tests
physis-check calls ACTUAL_DOCUMENT_ENTRYPOINT src tests
```

These commands are examples, not checks already passed; a nonexistent test name
must not be reported as zero failures. Coordinator defines the minimal applicable
feature matrix and required full gates after the baseline audit. Do not download
models or train merely to validate an append-only documentation change.

For every claim that a capability works, show both a write and a read path and an
actual end-to-end effect. Add a capability manifest entry only once the real
symbols are known. A textual call-site count is supporting evidence, not execution.
Run Physis flow verification on the actual session where supported; never report
the newest unrelated transcript as this worker's verification. Preserve
`NOT MEASURED`/inconclusive outcomes even if a helper prints a generic clean line.

Worker handoff is concise and evidence-backed:

```text
Packet/status: complete | partial | blocked, with exact unmet exit gate
Actual host, checkout, commit and starting dirty state:
Requested provider/model; observed response provenance; reported cost:
Changed files and why:
Commands executed and exit statuses:
Runtime entrypoint -> write -> observed effect -> reload/read evidence:
Acceptance cases passed / total attempted (include failures/timeouts):
Performance: measured values + hardware/settings, or NOT MEASURED:
Known limitations and source evidence:
Next owner / exact prerequisite:
```

Coordinator records a verified outcome through the existing workspace history
mechanism, without asserting experimental hypotheses as proven. Keep large logs
and machine-readable results separate from this plan; append only the outcome
and evidence pointers here when a phase actually changes status.

**Pending acceptance ledger:**

- [x] P0: baseline, ownership, wire/data contract and canonical fixture frozen.
- [x] P1: document/binding/patch/persistence/undo behavior passes AD01–AD13 as applicable.
- [x] P2: actual service wiring, resource scope, conflicts and errors verified.
- [x] P3: visible minimal workflow and source explanations verified in the UI.
- [ ] P4: all AD01–AD16 attempted, results retained, usable transition export.
- [ ] P5: cheap arms measured; any learned component justified by a controlled comparison.
- [ ] P6: independent review of raw effects, replay, byte preservation and reported claims.

**P0/P1 outcome (2026-09-15, local host `/home/gio/dev/physis-pro`, uncommitted
Kilo scaffold preserved, no worktree resets):**

- New module `src/adaptive_document/` (`mod.rs` contracts/parser, `proposal.rs`
  propose/apply/reject/undo, `store.rs` durable persistence over
  `src/atomic.rs::write_json_durable` and an append-only `events.jsonl`).
  Block spans are content-only; separators are gap bytes, so outside-byte
  equality survives every splice. Transforms: `copy` and `scale:<n>` only;
  unknown transform ids return `None` and demote the dependent to review.
- Tests: `cargo test --offline --lib adaptive_document` → 9 passed;
  `cargo test --offline --test adaptive_document` → 13 passed (AD01–AD13);
  `cargo test --offline --lib -- --skip cli::` → 968 passed, 0 failed.
  The three `cli::tests::test_run_*` lib tests hang on a persistent store on
  this host and were excluded; they predate this change.
- Baseline repairs required to build (Kilo's uncommitted, untracked scaffold
  code; all minimal, no module deleted or reset): `staleness/mod.rs` duplicate
  removed; `dependency/mod.rs` `StalenessClassifier::Default` →
  `StalenessClassifier::default()`, partial-move of `trace.target`, three
  `unwrap_or(&vec![])` temporaries now return slices, `map_or` on referenced
  options; `orchestration/mod.rs` float type annotation and a semicolon that
  discarded the router's return value; `metrics/mod.rs` initializer missing
  `cheap_model_tokens`/`strong_model_tokens` (now measured from repair
  results, not constants); `repair_regions` test called
  `select_repair_regions` with a removed third argument. NOT MEASURED: any
  runtime behavior of those six scaffold modules beyond compilation — they
  remain per §26.2 not ready for integration.
- NOT DONE (next packets): P2 web/service wiring, P3 UI surface, P4
  benchmark corpus + AD14–AD16, P5 model experiments, P6 review.

**P2/P3 outcome (2026-09-16, pansa `/home/gio/physis_pro` at `96a98c4`):**

- Service: `src/web/document.rs` — six frozen verbs folded into five endpoints
  on the existing Bearer-gated router: `GET /api/v1/document` (snapshot +
  patches + event history), `POST …/edit`, `…/decide`, `…/undo`, `…/reset`.
  Binding logic stays in `adaptive_document`; the module only resolves the
  tenant to a store root (`{data_dir}/documents/{uid}/{document_id}`), holds one
  process-wide lock across the read-modify-write, and maps `ApplyError` to
  HTTP status. Document ids are validated to `[A-Za-z0-9_-]`, so a tenant path
  cannot be escaped. Tenant scoping reuses `X-Physis-User`, which is what makes
  AD14 structural rather than asserted.
- UI: `src/bin/ui/document.html` (`GET /document`, gated like every product
  page) — block list with an inline editor, the exact before/after replacements
  with their justifying links, review-only notices rendered as notices, accept /
  reject, one undo, and the event history in store order. Conflicts reload
  instead of pretending success. Registered in `PRODUCT_PAGES`.
- Seed: the §26.8 canonical fixture (source, copy, calculation, review prose,
  unrelated quotation, locked block) is written on the first read of an empty
  store, so the page demonstrates real behaviour with no setup. Ground truth
  stays in the acceptance tests, not in the seed.
- Tests: `--lib web::document` → 3 passed; `--test adaptive_document` → 13
  passed (AD01–AD13); `--test http_surface adaptive_document` → 2 passed, a real
  router round trip proving edit → propose → accept → readback → undo and
  cross-tenant refusal (AD14/AD15's request half). AD16 (export) and the
  adversarial corpus remain P4.
- Build repair found while wiring: the deploy feature set
  (`web,cli,embed-onnx`) did not compile at `96a98c4` — `ported::tui` was
  ungated, so `ratatui`/`crossterm` were missing, and the two `mails::tests`
  were not gated behind `email`. Both fixed. The pansa auto-updater had also
  failed every tick since 2026-09-12 because a linked worktree checks out no
  submodule contents; `scripts/pansa-physis-auto-rebuild.sh` now runs
  `submodule update --init` before the build.
- NOT DONE: P4 export/AD16, P5 model arms, P6 independent review.

Success is a small reliable workflow with measured limits. It may contain no
learned dynamics and no diffusion. Extra parameters, services and abstractions
must justify themselves through behavior the smaller system cannot supply.

## Phase 27 — Measured structure of change (2026-09-17)

Status: planned; packets prepared, no agents launched. Research hypothesis only.
This track follows correctness/replay work in Phases 22–23 and the bounded
adaptive-document workflow in Phase 26; it does not displace their unfinished
correctness, isolation, or release gates.

Hypothesis: relations surviving meaningful transformations and coarse-graining
may predict a sparse changing frontier. Test this before building an invariant
spine. Heavy tails do not establish power laws; power laws do not establish
criticality. Useful incremental computation does not require criticality.

Priority within this track (dependencies override numeric ordering):

| Priority | Packet | Deliverable / release condition |
|---|---|---|
| P0 | [MS-001](packets/MS-001-archaeology.md) | Archaeology and measurement contract |
| P1 | [MS-002](packets/MS-002-mutation-replay.md) | Actual mutation replay and incremental correctness |
| P1 | [MS-003](packets/MS-003-multiscale-relations.md) | Linguistic scales and relation survival |
| P2 | [MS-004](packets/MS-004-distribution-analysis.md) | Distribution fitting and concentration |
| P2 | [MS-005](packets/MS-005-sparse-evaluation.md) | Invariant barriers and sparse retrieval |
| P3 | [MS-006](packets/MS-006-criticality.md) | Criticality, finite-size scaling and recurrence |
| P4 | [MS-007](packets/MS-007-routing.md) | Outcome-grounded compute and model routing |
| P5 | [MS-008](packets/MS-008-promotion.md) | Review evidence before minimal production promotion |

Only MS-001 is initially eligible. After review, MS-002 and MS-003 may run
in parallel in isolated workspaces. MS-004 and MS-005 may then run in parallel.
MS-006 is observational, not a prerequisite for the scientific usefulness of
MS-005. MS-007 requires real task outcomes. MS-008 is a conditional review,
not automatic permission to modify production.

> **Updated 2026-09-20.** MS-001 was reviewed independently and is `done`
> (`packets/reports/MS-001.md`, §"Verification note (2026-09-20)"; main
> `e466d4e`, core `ed8dc29`). MS-002 and MS-003 are released to `todo` with
> **amendment A1** attached to both packets: `node_deltas` must be compared as
> a set sorted by `node_id` and floats to epsilon 1e-6, because the DFS wave's
> neighbour order is HashSet-derived and unspecified (`delta_engine.rs:297`).
> MS-004–MS-008 stay blocked behind their dependencies as listed.
>
> **Updated again 2026-09-20 (later).** MS-002 was reviewed independently and is
> `done`: citations re-checked at both `ed8dc29` and `54b8a8d`, A1 confirmed
> binding, zero-production-caller and zero-instrumentation claims confirmed.
> The runnable mutation matrix it left undelivered is carried by new packet
> **`MS-002-harness`** (`review` — built, 7 arms green, gate run twice). MS-003
> was reviewed and **returned to `todo`**:
> its report describes the acceptance gate wrongly (it names
> Counterfactual/Substitution/Analogical; `transform.rs:633-639` sets
> logical/empirical/temporal/causal/procedural and hard-requires only logical
> and empirical) and misquotes PH-051 ("0/5 cleared transfer" against PH-051's
> own 4/5 transfer, 0/5 arithmetic-specific). MS-004 and MS-005 stay blocked:
> MS-003's report is the prerequisite for MS-005's relation-family baseline.

The user's second E65–E70 sequence maps to MS-002/MS-004/MS-006 (critical
structure), MS-003 (linguistic scales and relation families), MS-005 (spine and
incremental avalanches), and MS-007 (routing). Existing E65–E70 research IDs
remain untouched; these MS packet IDs prevent collisions.

Shared agent contract:
- Follow packets/PROTOCOL.md and AGENTS.md. Work on pansa via randomesh/SSH,
  confirm checkout and submodule revisions, preserve pre-existing dirty files.
- Use Physis history/pack/read against the actual checkout. Current MCP may
  point at /home/gio/dev/physis-pro instead of /home/gio/physis_pro; verify root.
- Ownership: own packet report plus research/multiscale/<packet-id>/ only for
  implementation packets. Archaeology and promotion own reports only.
  Production instrumentation needs a reviewed scope amendment before editing.
- Freeze corpus manifest, hashes/revisions, train/validation/test split, seeds,
  controls, estimands and acceptance tolerances before inspecting holdout.
- A negative or inconclusive result can complete a research packet. It cannot
  satisfy a positive promotion gate. Missing required data blocks dependent
  execution. Reviewer must explicitly release dependencies.
- Each report includes Evidence, Commands, Results, Limitations, Decision;
  actual denominators, uncertainty and reproduction commands. Independent
  review is mandatory; artifact existence is not experimental acceptance.
- Never use research/criticality/run.py or results.json as evidence. Earlier
  draft has self-comparison, leakage and incompatible likelihoods. Preserve
  it pending an explicit cleanup decision; do not repair it by default.
- No new invariant engine, broad production rewrite, model purchases, worker
  launch, merge or deployment is authorized by preparing these packets.

Proposed dispatch prompt: "Execute only packets/MS-NNN-<name>.md after its
dependencies have reviewed reports and release decisions. Read PLAN.md Phase
27 and packets/PROTOCOL.md. Confirm pansa workspace and ownership. Stop at the
packet gate; write packets/reports/MS-NNN.md. Do not start another packet."

### Quantitative event-stream methods (research extension)

Detailed proposed Q1–Q8 protocols: [Quantitative event streams](research/QUANTITATIVE_EVENT_STREAMS.md).
Readiness order: context allocation; change-points/EWMA; timestamped excitation;
admissible cointegration; conditional factors/tail models; outcome-grounded routing.
Existing MS packet gates and ownership remain unchanged. No experiments launched.

## Phase 28 — Agentic integration as model-agnostic control plane (evaluation only, 2026-09-17)

Status: evaluation, design-only. No implementation, worker launch, production
edit, merge, deploy, or model purchase is authorized by this section. General
guidelines apply (Phase 27 shared contract + packets/PROTOCOL.md + AGENTS.md):
pansa workspace, confirm checkout and submodule revisions, preserve
pre-existing dirty files, freeze manifests/splits/seeds/controls/tolerances
before holdout, negative results are valid outcomes, independent review is
mandatory, only a reviewer releases dependencies.

### 28.0 Thesis, agentic-integration slice

Physis owns the world representation; models are replaceable cognitive
peripherals. Agent work runs through a shared representation space (coherence
graph + causal ledger + task envelope + MCP transport), never through a
worker's private transcript. Three pathways: symbolic→neural (compile the
smallest relevant context from structure), neural→symbolic (model outputs as
typed proposals with provenance and confidence, validated deterministically
where possible), loop (unresolved region → cheapest sufficient compute →
ingest → delta → propagate → validate → escalate only the remainder).
Cheapest-cognition ladder: parser/rule → deterministic algorithm → statistics
→ embedding → small model → large model/API → human.

### 28.1 What exists (verified 2026-09-17 on pansa)

| # | Piece | Location | Verdict vs thesis |
|---|---|---|---|
| 1 | Agent loop (ReAct over SSE, `MAX_TOOL_ROUNDS` 8, PDCA rails, cascade route + model override) | `src/ai/agent.rs:15,99-307,173-198` | Partial: loop recruits model + tools, but the prompt is system prompt + history + tool outputs — not the structured GOAL / STATE / DEPENDENCIES / CONSTRAINTS / INVARIANTS / UNRESOLVED / OPERATION / SCHEMA envelope. |
| 2 | Curated tool registry (`graph_search`, `node_search`, `classify`, `coherence_register`/`coherence_edit`, `dream`, `opencode`; builtins cleared; unknown whitelist → 400) | `src/web/agent.rs:437-682,284-302` | Partial both directions: bounded retrieval packs exist (symbolic→neural), register/edit write typed nodes with embedder provenance (neural→symbolic) — but graph writes pass no proposal-validation gate. |
| 3 | Answer harness gate (judges the final answer, attaches a verdict, never rewrites; `register_as_hypothesis` off for agent answers) | `src/web/agent.rs:878-905`; threshold default 0.60 in `src/ai/harness.rs:47-69` | Partial: answers judged, graph writes unjudged. |
| 4 | MCP transport, 32 tools (`physis_pack/find/remember/recall/why/serve/causes/coherence_query/discover/themes`, …) | `src/mcp.rs` | Shared-space transport exists; pointer-bundle discipline (paths + line ranges + freshness) exists in the tools but is not enforced on the agent loop's own context. |
| 5 | Delegation contract D0–D6 (versioned envelope, broker states, acceptance ≠ completion, worktree isolation, no auto-merge, free/local worker policy, broker-vs-direct baseline) | `docs/plans/2026-09-14-agent-delegation.md` | Closest to thesis: workers keep no truth (§3), acceptance is independent (§D4). D0/D1 implemented per that file's §12 note; adapters (D2/D3) not re-verified here. |
| 6 | Relation layer: 15 `RelationType` variants with epistemic/causal/procedural families; `transform::Predicate` (Causes/Supports/Contradicts) | `physis-core/src/relation.rs:13-97`; `physis-core/src/transform.rs:19-37` | Partial vs the proposed 16: DEPENDS_ON, PRECEDES, CAUSES, PRODUCES, CONTRADICTS, SUPPORTS exist; CONTAINS, DEFINES, REFERENCES, TRANSFORMS, CALLS, READS, WRITES, CONSUMES, EQUIVALENT_TO, SATISFIES are not first-class. |
| 7 | Human privileged: nav SAME/DIFFERENT/RELATED privileged ops; cell-pin hybrid; tenant-scoped runs + owner-checked abort; ledger `actor` field | `src/nav/mod.rs:1118`; `src/web/agent.rs:77-84,218-242`; `src/ledger.rs:27` | Same event model for human and model actions exists; privileged authority is not uniform across surfaces. |
| 8 | Routing: provider cascade classifies the query | `src/ai/provider.rs:922-944` | Ladder missing: no parser→algo→stats→embed→small→large→human escalation with measured margins. |
| 9 | Computer-state-as-graph | watch/vault/history/mail imports + ledger | Files only. AST, browser, process, device, task objects missing. |
| 10 | Thesis metrics (context/token reduction, call reduction, deterministic %, incremental ratio, persistence across swaps, wrong-output recovery, provenance completeness, intent→verified-change time) | — | Not measured for the agent path. |

### 28.2 Gaps ordered by demo value × research value

1. Structured model-call envelope (GOAL / STATE / DEPENDENCIES / CONSTRAINTS / INVARIANTS / UNRESOLVED / OPERATION / SCHEMA) compiled from graph + ledger instead of a history dump. Biggest demo and research win; the Q6 allocation work measures the retrieval half.
2. Proposal gate on graph writes: typed claim + provenance + deterministic validation before commit, human privileged override. Closes "answers judged, writes unjudged" (§28.1 rows 2–3).
3. Cheapest-cognition router with measured escalation margins (deterministic first, small before large; replaceability test: swap the model, structure persists).
4. Relation-layer extension: only the missing predicates with traceable mappings, plus mapping-destroyed controls per MS-003 discipline.
5. Computer-state objects beyond files (AST / process / task first; browser / device later), each with provenance.
6. Agent-path metrics: the nine thesis metrics with denominators and uncertainty.

### 28.3 Proposed follow-ups (not authorized)

AG-001 envelope compiler; AG-002 write gate; AG-003 cognition router; AG-004
relation extension; AG-005 computer-state objects; AG-006 agent metrics. Each
needs its own packet plus reviewer release, frozen manifest/splits/seeds/
controls/tolerances before holdout, reuse of existing primitives, and an
Evidence / Commands / Results / Limitations / Decision report. Negative
results valid, never promote.

### 28.4 Decision

Phase 28 completes as evaluation. No code, worker, or promotion follows from
this section. Next: reviewer confirms the citations above; the coordinator
mints AG packets in dependency order (AG-001 first) when capacity allows.

## Phase 29 — Model-agnostic neurosymbolic control plane (thesis record, 2026-09-17)

Status: thesis recorded, design-only. No implementation, worker launch,
production edit, merge, deploy, or model purchase is authorized by this
section. General guidelines apply (Phase 27 shared contract +
packets/PROTOCOL.md + AGENTS.md): pansa workspace, confirm checkout and
submodule revisions, preserve pre-existing dirty files, freeze
manifests/splits/seeds/controls/tolerances before holdout, negative results
are valid outcomes, independent review is mandatory, only a reviewer releases
dependencies.

### 29.0 Core thesis

Physis is a model-agnostic neurosymbolic control plane between human intent,
persistent machine/world state, and interchangeable cognitive engines. The
LLM must not own the world representation. Physis does.

```text
human intent
    ↓
PHYSIS
    ├─ persistent symbolic graph
    ├─ ontologies / typed relations
    ├─ invariants / constraints
    ├─ temporal + dependency structure
    ├─ file/substrate priors
    ├─ provenance + uncertainty
    ├─ semantic delta detection
    ├─ residual/surprise detection
    └─ compute routing
         ↓
 deterministic algorithms
 embedders
 small models
 large LLMs
 tools
 human agent
         ↓
actual computer/world state
         ↕
       PHYSIS
```

This phase records the thesis as the long-term architectural direction. It
does not replace Phases 22–24 (correctness/replay), 26 (bounded adaptive
documents), 27 (measured structure of change), or 28 (agentic-integration
evaluation). It constrains them: every follow-up must reuse existing
primitives (coherence graph, causal ledger, typed edges, pack/rag/nav
retrieval, quality standing, orchestration seams) and must not build a
parallel engine where existing state can be adapted.

### 29.1 Three pathways

1. Symbolic → Neural: compile persistent structure into the smallest relevant
   model context. Retrieve graph neighborhoods, dependencies, constraints,
   unresolved questions, prior actions, and high-information regions — never
   dump raw histories/files into prompts.
2. Neural → Symbolic: treat model outputs as proposals, not ephemeral prose.
   Convert useful outputs into typed claims, relations, plans, hypotheses,
   edits, constraints, and uncertainties with provenance and confidence.
   Validate deterministically where possible.
3. Neural ↔ Symbolic loop: unresolved region → cheapest sufficient compute →
   ingest result into graph → measure semantic/structural delta → propagate
   consequences → validate invariants → escalate only remaining uncertainty.

Core rule — cheapest sufficient cognition:

```text
parser/rule
→ deterministic algorithm
→ statistics
→ embedding
→ small model
→ large model/API
→ human
```

Natural language is one interface to the graph, not the store. Computer state
(files, code ASTs, browser state, processes, commands, devices, projects,
tasks, documents, external tools) becomes graph objects with provenance,
not repeated prose translations.

### 29.2 Relation layer, human authority, conceptual API

Universal relation vocabulary (target): CONTAINS, DEFINES, REFERENCES,
DEPENDS_ON, PRECEDES, CAUSES, TRANSFORMS, CALLS, READS, WRITES, PRODUCES,
CONSUMES, EQUIVALENT_TO, CONTRADICTS, SUPPORTS, SATISFIES. Current
`RelationType` coverage vs this list is inventoried in §28.1 row 6; only
missing predicates with traceable mappings are ever added (AG-004), with
mapping-destroyed controls per MS-003 discipline. Source-specific structure
and provenance are preserved beneath normalization.

Human intervention is first-class: pin meanings, override classifications,
establish equivalences, reject hypotheses, modify ontology, manipulate the
machine directly. Human actions enter the same event/state model as model
actions but carry privileged authority (§28.1 row 7; nav privileged ops,
cell-pin hybrid, owner-checked abort, ledger `actor`).

Optimize explicitly for: persistent state over conversational memory,
structure over token repetition, delta over full recomputation, residual over
uniform attention, deterministic computation over model calls, small models
over large models, model replaceability over provider coupling,
inspectability over hidden reasoning.

Conceptual API (direction, not a committed signature):

```rust
observe(world_delta);
update_graph();
detect_invariants();
detect_residuals();
select_unresolved_region();
route_compute(region);
ingest_result();
validate();
propagate();
```

A model call receives a compiled envelope — GOAL / CURRENT RELEVANT STATE /
DEPENDENCIES / CONSTRAINTS / KNOWN INVARIANTS / UNRESOLVED VARIABLES /
REQUESTED OPERATION / EXPECTED OUTPUT SCHEMA — never arbitrary accumulated
chat context (AG-001).

### 29.3 Success metrics, demo, hypothesis

Measure: task quality; context/token reduction; model-call reduction;
percentage handled deterministically; incremental recomputation ratio;
structural persistence across model swaps; recovery from wrong model outputs;
provenance completeness; time from human intent to verified machine-state
change. Each metric ships with denominators and uncertainty (AG-006).

Minimal vertical demo (gated, not authorized): user gives a natural-language
objective over a real code/project workspace; Physis constructs/updates
persistent structure, identifies the relevant region, selects
deterministic/model/human computation, performs or proposes the operation,
validates the resulting state, and preserves the learned structure for the
next task.

Research hypothesis under test: a computer whose semantic/world state lives
outside the neural model can use LLMs as replaceable cognitive peripherals,
achieving better continuity, control, interpretability, and compute
efficiency than systems whose effective world model lives primarily inside
prompts and context windows. Do not build another agent wrapper or chat
frontend. Build the semantic runtime that owns state and recruits agents
when needed.

### 29.4 Quantitative event-stream layer (pointer, not a copy)

The experimental layer treating language, semantic state, agent traces,
repository evolution, and retrieval history as multivariate nonstationary
event streams is specified in
[Quantitative event streams](research/QUANTITATIVE_EVENT_STREAMS.md) under
Phase 27, executed only through MS-001…MS-008 gates. That document is
normative for method detail; this section records scope and discipline only.

Initial experiment set (Q-labels are local to that document): Q1 semantic
volatility (EWMA vs rolling baselines; spikes predicting structural change);
Q2 change-points (CUSUM/Page-Hinkley/BOCD/PELT vs fixed windows, LLM phase
labels, shuffled-order control); Q3 factors (PCA/robust/NMF/incremental,
variance explained, stability, retrieval usefulness); Q4 cointegration as
invariant discovery (Engle-Granger/Johansen/ADF-KPSS-style; priority
hypothesis, admissible trajectories only); Q5 Hawkes excitation
(conditional on timestamps and event counts, vs co-occurrence/transition
counts/shuffle/time-reversed nulls); Q6 portfolio retrieval
(covariance-aware allocation vs BM25/cosine/RRF/MMR/TokenFixedRetriever
under equal token budget); Q7 tail impact (VaR/ES, heavy-tail diagnostics
before EVT/GPD); Q8 walk-forward routing (tokens-to-passing-task on real
worker outcomes, after validated signals).

Standing rules: every method beats cheap baselines (frequency, recency,
BM25, cosine, RRF, MMR, transition counts, rolling averages, existing
Physis structure/order metrics) plus a claim-matched null (shuffle order /
labels, time reversal, degree-preserving rewiring, phase randomization,
bootstrap); walk-forward validation wherever time leakage is possible;
prefer online/incremental forms with cost proportional to the delta, not to
history; instrument CPU/RAM/latency/tokens/calls/recall/success per method;
a method whose compute exceeds its savings is discarded. Negative results
are first-class outputs. Kill criteria: fails a strong null, reproduces
frequency or cosine only, unstable across windows, needs arbitrary
thresholds, overfits one project, no downstream gain, costs more than it
saves.

Target architecture under test:

```text
LANGUAGE / ACTION STREAM → cheap quantitative state estimation
→ regime + invariant + propagation map
→ sparse context/compute allocation → Notebook / agent, cheap ↔ strong
```

Deepest hypothesis: semantic systems may be easier to control as dynamical
systems than to regenerate as prose. Generative models are used only where
the deterministic/statistical layer reaches a measured limit.

### 29.5 Follow-ups (not authorized)

AG-001 envelope compiler; AG-002 proposal gate on graph writes; AG-003
cheapest-cognition router; AG-004 relation extension; AG-005
computer-state objects (AST/process/task first); AG-006 agent-path metrics —
each needs its own packet plus reviewer release in dependency order
(AG-001 first), frozen manifest/splits/seeds/controls/tolerances before
holdout, reuse of existing primitives, Evidence / Commands / Results /
Limitations / Decision report. Q1–Q8 run only inside MS packets. No code,
worker, merge, deploy, or promotion follows from this section.

### 29.6 Decision

Phase 29 completes as thesis record. Next: reviewer confirms §28 citations
and the §29.4 pointer resolve; the coordinator mints AG packets when
capacity allows. Unfinished correctness, isolation, and release gates in
Phases 22–24, 26–28 keep priority over any Phase 29 follow-up.

## Phase 30 — Spatial world surface MVP (2026-09-17)

Status: planned; slice W1 authorized (edge identity + persistence + vocab,
below) + W2 nav-bridge authorized (below). All later slices need their own
plan entry + reviewer release.
General guidelines apply (Phase 27 shared contract + packets/PROTOCOL.md +
AGENTS.md): pansa workspace, confirm checkout and submodule revisions,
preserve pre-existing dirty files (MS-track + nav/HUD work is another
agent's — touch only the files named per slice), freeze manifests before
holdout, negative results valid, independent review mandatory.

### 30.0 Evaluation: spec §1–§34 vs what exists (verified 2026-09-17)

| Spec | Exists | Gap |
|---|---|---|
| §1 EntityRef/Relation identity | `nav::ObjectId` (origin+key, content-hash, rename-stable) + `nav::Relation` (a/b/kind/note/origin/status/created_at); `physis_core::relation::TypedEdge` (uuid id, source/target, confidence/weight, temporal, provenance, evidence) + `PhysisCore.edges` with `add_typed_edge`/`edges_from`/`edges_to` | nav Relation has no stable id, no evidence/scope/validity; TypedEdge has id but **no persistence** (nodes persist via `coh:` keys, edges live in memory only); atlas drops edge identity entirely |
| §2 multiscale center | HUD list+reading+ASCII graph dual (`hud.rs graph_view`), atlas canvas (`organic.js`), doctree sections, `nav resolve/read` | no pan/zoom/semantic-zoom over real objects; no function/block/code children (objects are File/Passage/Directory/Concept/Note/Other) |
| §3 NavigableEntity/plural parents | `nav::semantic_parents` (fs/note/kind, derived on demand, tested plural) | no lens-parameterized children/neighbours trait; no project/semantic/temporal/epistemic parents |
| §4 lenses SEM/REL/STRUCT/TIME/FS/EPI | atlas verdict/domain/community filters; HUD hybrid scorer with weights; `query_with_relations` | no lens toggles, no multi-lens weighting, no TIME/EPI lens |
| §5 REAL ONLY | `RelationOrigin` User/Deterministic/Inferred never collapsed; HUD graph glyphs solid/double/dashed; atlas recorded-vs-resemblance never merged | no REAL ONLY filter; origin not enforced on agent writes |
| §6 epistemic grammar | origin glyphs in HUD; atlas legend recorded vs resemblance | no edge-style-by-origin on canvas; symmetric similarity still drawable as directed |
| §7 bundling | communities + cohesion exemplar; corpus summaries | no bundle objects retaining constituent RelationIds |
| §8–§10 right dock editor | atlas explorer (entity card, neighbours, pin, serve, lineage) + `POST /coherence/edit` | explorer edits nodes only; **edges not selectable/editable**; no relation editor |
| §11 SAME/DIFFERENT/RELATED | `nav rel/rel-infer/rels` CLI + `add_relation` idempotent + `infer_same_source_relations` | no kind-qualified SAME/DIFFERENT prompts; no HUD keybindings beyond pin |
| §12 draw relations | — | missing entirely |
| §13 text-span selection | `SourceRef` path:start-end; `read_object` exact text; evidence span attached on praxis/ledger paths | no temp Selection entity; no phrase→neighbour→relate flow |
| §14–§15 evidence/history | ledger lineage/purposes; audit `edge:add`/register/edit/delete; `GET /audit?node_id=` | edge evidence field exists on TypedEdge but unwritten by API; no per-edge history view |
| §16 representation | `RetrievalScore.components` per-name inspectable; `why:` in HUD | no REPRESENTATION tab joining lexical/semantic/structural/temporal/graph |
| §17–§18 textbox/IntentIR | `ui-shell.js initAskBar` (search inline, agent second); `intent_ir.rs` slots + read-only ops + bind tasks | ask bar searches labels only; IntentIR not wired to selection/scope/lenses or capabilities |
| §19 history | `nav trail` (query/enter push) + crumbs stack | restores nothing (no scope/zoom/lens/selection/camera) |
| §20 graph↔editor continuity | doctree breadcrumb+body; `nav open` exact text | no zoom-threshold center transition; graph and editor separate surfaces |
| §21 FS lens | nav fs substrate + `semantic_parents` fs leg | no FS-lens rendering; hard-link vs semantic-link distinction unwritten |
| §22 notebook handoff | HUD `notebook_selection` (canonical objects, borrow-not-copy) + `addToNotebook` | one direction only; answer evidence not drawn as graph edges |
| §23 clusters as objects | communities singleton convention; corpus summaries | transient clusters have no opt-in identity |
| §24 compare | — | missing; no COMPARE across representations |
| §25 meaning-shift | — | missing |
| §26 why-am-I-here | atlas help text (recorded vs resemblance); HUD explain line | not per-element; similarity edges lack provenance pointers |
| §27 no-LLM navigation | nav/hud/atlas retrieval is LLM-free by construction | must be preserved per slice (gate below) |
| §28 incremental | nav stat fast-path + content-hash embed cache; atlas limit/k/min_sim truncation | atlas still builds full similarity matrix over truncated set |
| §29 layout stability | hash-seeded layout + stable sort; positions not persisted | no pinned-position persistence |
| §30–§31 dock tabs/projections | atlas rail/explorer; PRODUCT_PAGES shell | no tabbed dock; pages switch layouts instead of re-projecting |
| §32 acceptance | nav acceptance walk exists per SEMANTIC_NAVIGATION.md | world-surface acceptance (§32 1–38) not runnable |
| §33 do-nots | origin separation, propose-don't-write conventions | must be gated per slice |

### 30.1 MVP order (spec §34, unchanged)

1. EntityRef/ObjectId/RelationId identity → 2. shared Selection →
3. persistent RelationStore + provenance → 4. renderer over real objects →
5. dock selection binding → 6. node inspector → 7. relation inspector →
8. evidence refs → 9. semantic zoom → 10. bundling → 11. lenses →
12. REAL ONLY → 13. span Focus → 14. notebook handoff → 15. command-box ops →
16. model suggestions (last, async only).

### 30.2 Slice W1 (authorized): edge identity + persistence + full vocab

Rationale: §10/§15/§26 are all blocked on one fact — typed edges have
identity in memory (`TypedEdge.id`) but lose it everywhere else: they are
never persisted (restart wipes them while nodes survive), only 6 of 15
`RelationType` variants are writable via the API, `EdgeProjection` drops
the evidence field, `delete_node` orphans edges, and the atlas graph
carries no edge ids at all. Fix the identity chain first; no renderer,
lens, or zoom work lands on a graph whose edges cannot survive a restart.

Files owned by this slice (all currently clean, no MS/nav/HUD overlap):
`src/core.rs`, `src/web/coherence.rs`, `src/web/atlas.rs`,
`src/web/mod.rs` (rehydrate call sites only), `tests/` (new file
`tests/world_edges.rs`; do NOT append to the dirty `http_surface.rs`).

Deliverables:
- W1.1 `PhysisCore`: `edge_key` + `persist_edge` + `load_edges` (embedder-
  agnostic — edges carry no vectors, so no provenance skip) +
  `delete_edges_for_node`; `delete_node` drops referencing typed edges;
  unit test extends `test_nodes_survive_a_restart` pattern (edges survive).
- W1.2 API: `AddEdgeRequest` accepts all 15 `RelationType` variants +
  optional `evidence`/`provenance` (default provenance `"api"` preserved);
  `EdgeProjection` gains `evidence`; unknown relation still 400 with the
  full variant list in the error.
- W1.3 Atlas: `AtlasEdge` gains `id: Option<String>` + `relation:
  Option<String>`; `build_atlas` takes the typed-edge layer (recorded
  holarchy first, typed second, similarity last; recorded never relabeled);
  handler feeds `core.edges`; existing 5 atlas tests updated, 1 new test
  pins typed-edge identity through `build_atlas`.
- W1.4 Rehydrate: `load_edges` on tenant first access (`with_user`) +
  default-tenant startup, beside the existing `load_nodes` calls.
- W1.5 Persist on write: `coherence_add_edge_handler` writes the edge row
  through to sled (same write-through pattern as register/edit).

Gates: `cargo check --all-targets` + `cargo clippy --all-targets -- -D
warnings` green; new `tests/world_edges.rs` (persist round-trip, full
vocab accept, unknown-relation 400, delete cascades, tenant isolation)
green; no LLM in the path (unit-verifiable — no provider calls);
`git status` shows only slice-owned files changed.

Explicitly NOT in W1: renderer changes, lens toggles, REAL ONLY filter,
span Selection, notebook write-back, command-box ops, model suggestions,
openapi.yaml edits (dirty — another surface owns it), any file under
`src/nav/`, `src/bin/hud.rs`, `src/main.rs`, `packets/`, `research/`.

### 30.3 Later slices (not authorized — one plan entry each)

W3 relation inspector API (edge get/update/dispute/supersede + per-edge
history from audit); W4 atlas edge selection + origin grammar on canvas;
W4 REAL ONLY filter (data-model predicate, both surfaces); W5 span
Selection + evidence attach; W6 lens toggles; W7 semantic zoom over
doctree/code children; W8 bundling with constituent ids; W9 notebook
evidence edges; W10 command-box graph ops via IntentIR; W11 compare/diff;
W12 layout persistence. Model suggestions stay last (spec §34.16).

### 30.4 Decision

Phase 30 opens with W1 only. Later slices wait for W1 review + explicit
plan authorization. MS-track and nav/HUD ownership unchanged.

### 30.5 Slice W2 (authorized 2026-09-17): nav relation identity + HUD relate

Rationale: cline's nav/HUD work is reviewed and sound — content-hashed
`ObjectId`, one scorer, atomic index, honest degradation — but nav
relations had no stable identity (§30.0 row 1 stands) and the HUD could pin
but not SAME/DIFFERENT/RELATE (spec §11 gap stands). W2 closes both inside
cline's files, reusing the W1 identity pattern (derived ids, no migration).

Files owned: `src/nav/mod.rs`, `src/nav/workspace.rs` (pure helpers only —
no core-store bridge; the nav sled and core sled are different stores and
no path exists to open both together), `src/bin/hud.rs`,
`docs/SEMANTIC_NAVIGATION.md` (keys paragraph only).

Deliverables:
- W2.1 `nav::relation_id` (pure, canonical endpoint order) +
  `relation_kind_to_core` (only traceable verbs map; SAME/DIFFERENT/free
  verbs → `None`, skipped openly); 2 unit tests.
- W2.2 HUD: `s`/`d`/`R` keys (`relate_selection`), relations listed in the
  explain pane with origin glyphs + edge ids, graph legend names the id
  scheme. No core writes from the HUD (none possible — see above).
- W2.3 Deferred with reason: nav→core edge bridge needs a designed home
  (server-side handler or CLI core-store path), not a fresh sled open on
  the CLI path. Proposed as W5, not smuggled into W2.

Gates: `cargo test --lib nav::` (17) + `world_edges` (4) green; HUD
compiles warning-free on its own files; no LLM in the path.

Explicitly NOT in W2: core writes from nav/HUD, stored relation ids
(derived, not migrated), renderer/lens/zoom work, `src/core.rs`,
`src/web/*`, `packets/`, `research/`.

### 30.6 Decision (W2)

W2 ships the identity + HUD slice above. Next: reviewer confirms W1+W2;
coordinator proposes W3 (relation inspector API) and W5 (nav→core bridge
design) as separate plan entries when capacity allows.

### 30.7 Untracked inventory + inspection (2026-09-17)

`git ls-files --others --exclude-standard` = 28 paths. Headers/sizes verified
this session; full content review NOT done. No slice may claim these files as
evidence until reviewer verifies content + ownership.

| group | paths | standing |
|---|---|---|
| orchestration harness (opt-in, human-supervised) | `.opencode/commands/orchestrate.md`, `.opencode/orchestration.json`, `.opencode/plugins/orchestration.js`, `.opencode/skills/physis-inspect/SKILL.md`, `clients/opencode-orchestration/` (README, roles/router/router.test/smoke), `logs/auto_update.log` | headers only; Astra advertised-but-rejected, live runs bounded; out of all slice gates |
| MS packets | `packets/MS-002`…`MS-008` | MS-001 **done** (reviewed 2026-09-20, amendment A1); MS-002 **done** (reviewed 2026-09-20, report-only accepted); `MS-002-harness` **`review`** (7 arms green) carries the runnable oracle MS-002 deferred; MS-003 **returned to `todo`** (two wrong claims in its report); MS-004–008 blocked behind their dependencies. No worker launched by this document |
| research | `research/E91_ARCHAEOLOGY.md`, `research/E91_CONCEPT_MAP.md`, `research/QUANTITATIVE_EVENT_STREAMS.md` (Q1–Q8 PROPOSED/NOT MEASURED), `research/criticality/run.py` + `results.json` (invalid draft, never baseline) | E91-A/B citable as audits; QES authorizes nothing; criticality draft is non-evidence |
| nav/HUD slice (W2-owned) | `src/nav/mod.rs`, `src/nav/workspace.rs`, `src/bin/hud.rs`, `tests/world_edges.rs`, `docs/SEMANTIC_NAVIGATION.md`, `scripts/physis-update.sh` | W2.1 (`relation_id`, `relation_kind_to_core` + 2 tests) + W2.2 (`relate_selection`, `s`/`d`/`R`) present per grep; targeted tests not yet run |

Resume 1-2-3-4 (ordered, reviewer-gated): (1) separate dirt — W1 vs 11C vs
nav/HUD as distinct review units; (2) run slice gates — `world_edges`,
`cargo test --lib nav::`, concepts HTTP test; (3) triage clippy — isolate
slice regressions from the 53–61-error pre-existing baseline (stash-compare
vs `main`); (4) reviewer confirms W1+W2 before any W3/W5 proposal.

## Phase 31 — Jev-inspired System-One layer (proposed 2026-09-17)

Status: proposed. No implementation, worker launch, production edit, merge,
deploy, or model purchase authorized by this section. General guidelines
apply (Phase 27 shared contract + packets/PROTOCOL.md + AGENTS.md): pansa
workspace, reuse-first (no parallel engine), freeze manifests/splits/seeds/
controls/tolerances before holdout, negative results valid, independent
review mandatory, only reviewer releases dependencies.

### 31.0 Thesis

Jev as computational model, not fast classifier: structured state → many
parallel typed probabilistic decisions → propagation → minimal action,
escalating to generative reasoning only for novelty/uncertainty. Physis holds
an executable probabilistic ontology, not an index. Preserve distributions
while useful; ontology-invalid outputs unrepresentable via types/schema.
Compilation ladder: LLM/human → SLM/Jev-like head →
classifier/embedder/count-table → graph/FSM/rule/index. Familiarity becomes
software. Whole trajectories compile, not just labels. Large action spaces:
cheap independent scoring → prune → explicit choice among survivors. Cheap
heads verify System-Two traces; interrupt/escalate only when warranted.

```rust
Decision<T> { distribution, confidence: f32, evidence, provenance, cost }
trait SemanticHead { type Output: PhysisType;
  fn decide(&self, world: &WorldSlice, ctx: &Ontology) -> Decision<Self::Output>; }
```

Backends: `rule | FSM | regex | graph | count-table | BM25 | embedding |
classifier | Jev | SLM | LLM | human`.

### 31.1 Reuse map (verify paths at pinned revision before citing)

`src/dependency/` (affectedness flood, LIFO — E91-A) + `src/semantic_delta/`
(no live producer) → WorldSlice/Δ source; `src/rag.rs`, `src/pack.rs`,
`src/nav/` → candidate pools + budgets (Q6/MMR baseline); `src/quality.rs`
standing → salience signal; `physis-core/src/transform.rs`, `relation.rs`,
`delta_engine.rs` → constraints + replay oracle; `src/ledger.rs` +
`physis-core/src/observe.rs`, `hypothesis.rs` → event/provenance substrate;
`src/orchestration/` + `clients/opencode-orchestration/` → routing seam
(placeholders, not workers); `benchmarks/behaviour/` +
`benchmarks/retrieval/` → task records + harness. Prior-art families for the
S1 packet to map w/ links+licenses (not rebuilt): TypeSafe Jev (typed
parallel decisions, calibration, hierarchical selection), MoE / routers /
cascades / FrugalGPT, early-exit / speculative, distillation / active
learning, conformal + calibration, Rete / Datalog / incremental compute,
partial-eval / tracing-JIT, BM25 / n-grams / count-tables / regex / FSM /
prototypes, contextual bandits, DSPy / verifiers.

### 31.2 Evaluation + decisive experiment

Benchmark identical workflows with interchangeable intelligence sources, not
models alone. Metrics: task success, calibration, semantic errors, latency,
RAM, joules, tokens, €, recomputation %, escalation %, compiled-head
coverage, accuracy retained after compilation. Objective: useful correct
world-state transitions / second / joule / euro. Decisive run: repeated
mixed computer/language workflow, strong-LLM only at start; does Physis
discover recurring distinctions/dependencies, compile them into cheaper
typed heads/policies, and cut LLM calls/tokens/latency while preserving task
success? Controls: frequency/recency/BM25/cosine/RRF/MMR arms, equal budgets,
walk-forward + project holdouts, mechanism-destroying nulls. Ablations:
propagation on/off, calibration on/off, bandit vs static routing,
whole-trajectory vs per-label compilation. Failure modes: silent miscompile,
undetected drift, escalation collapse, cost exceeding savings.

### 31.3 Sequence (own packet + reviewer release each)

S1 prior-art/resource map (links+licenses, Jev→module mapping, what NOT to
build); S2 `Decision`/`SemanticHead` pure types + cheapest heads
(count-table/BM25/prototype) on one frozen workflow; S3 calibration/
conformal gates + shadow-test vs strong source; S4 bandit router
(accuracy×latency×RAM×energy×tokens×€); S5 semantic JIT
(record→detect→synthesize→distill→calibrate→shadow→deploy→monitor); S6
trajectory compilation + continuous verification. MS-track and W1/W2 review
keep priority; S1 mints only after W1+W2 confirm.

### 31.4 Decision

Phase 31 opens as proposal only. Next: reviewer confirms §30.7 inventory and
§31.1 reuse paths; coordinator mints S1 (map, no code) when capacity allows.
Prototypes follow positive gated evidence; promotion, if ever, via
MS-008-style review, never direct.

## Phase 32 — Physis MVP demonstration (offline Feather, reuse-first, no parallel demo app)

Status: planned, PLAN.md only. No code, worker, merge, deploy, or model
purchase is authorized by this section. General guidelines apply (Phase 27
shared contract + packets/PROTOCOL.md + AGENTS.md): pansa workspace, confirm
checkout and submodule revisions, preserve pre-existing dirty files, reuse
before building, negative results valid, independent review mandatory.

### 32.0 Absolute constraint

The complete demonstration runs offline on Feather with no generative model:
no LLM, cloud API, remote embedding service, network dependency,
model-generated patches, agent framework, or canned fake graph
transformations. Local deterministic embedders already used by Physis are
allowed where practical, with clean degradation to lexical/structural methods
when unavailable. The demo is about Physis, not about an LLM using Physis.
`OFFLINE` is displayed in the UI. The irreducible loop is
NAVIGATE → SEE STRUCTURE → EDIT REAL TEXT → DETECT Δ → PROPAGATE
CONSEQUENCES → STATE INTENT → MOVE ATTENTION → INSPECT MINIMAL CONTEXT → ↺.

### 32.1 Reuse-first inspection (verified against this checkout)

No parallel demo app. The MVP loop already exists as a thin REPL over real
slices: `src/mvp/repl.rs` (`open`/`scope`/`inspect`/`dependencies`/`recent`/
`contradictions`/`act_flow`/`observe`), `src/mvp/model.rs`
(`artifacts_of`, `Artifact::from(&PhysisObject)`), `src/mvp/deps.rs`
(`affected_dependents`, `changed_symbols`, `classify_intent`, `render_map`),
`src/mvp/slice.rs` (`WorldSlice` + telemetry + `snapshot_hashes`/
`diff_snapshots`), `src/mvp/log.rs` (`TransitionLog`, append-only
`mvp_transitions.jsonl`). Navigation is `src/nav/mod.rs` +
`src/nav/workspace.rs` (`NavWorkspace::open/update/query/read`,
`query_with_relations`, `explain_hit`, `group_hits`, pins/relations/trail,
`sem_off`/`sem_on`, honest `semantic: off` degradation). The existing TUI is
`src/bin/hud.rs` (`physis-hud`: neighborhood list + reading pane + `why:`
pane + query bar, `g` ASCII graph dual view, `s`/`d`/`R` relate,
`relation_id` + origin glyphs solid/double/dashed). Context packing is
`src/pack.rs` (`pack_cached`, content-addressed `EmbedCache`,
`PackResult::candidate_tokens` measured denominator, `summarize`,
`tokens_of`) over `src/rag.rs` (`count_tokens` heuristic,
`TokenFixedRetriever`). Incremental scan is `src/tree.rs`
(`DirTree::scan_run`, stat-only `meta_walk`, Merkle `diff`, one node per
doc) + `src/scanner.rs` (`scan_project`, `.physis` excluded) +
`src/ledger.rs` (`record`/`lineage`/`descendants`/`purposes`). Token math is
`src/estimate.rs` (`calculate_estimate`). Entry is `physis-pro <dir>`
(`src/main.rs` bare-dir before the licence gate) via
`src/bin/physis.rs` + `src/ported/front_door.rs`. Protection is
`tests/mvp_loop.rs` (7 tests), reproducibility is `scripts/mvp-demo.sh` +
`docs/MVP_DEMO.md` + `MVP_STATUS.md` + `POST_MVP.md`. Narrow exact
copy/calculate document flow (`src/adaptive_document/`,
`src/web/document.rs`, `src/bin/ui/document.html`) stays as-is and is not
the repo-loop demo path.

### 32.2 Classification

ALREADY WORKS: persistent index from a real workspace with instant warm
start; nested navigation project → dir → file → symbol via nav objects +
`semantic_parents`; real source open/read with `path:start-end` provenance;
hash-snapshot Δ on save (`changed/added/removed`); precision-first
`affected_dependents` (code-shaped refs, defines-not-implements, measured
261 → 6 on physis-core); intent box as attention op (`scope` → nav rank +
token-fixed pack + telemetry); minimal context object (`WorldSlice` +
per-hit `why:` components + chunk provenance); token counts
(`candidate_tokens` = A, packed tokens = B, `calculate_estimate` =
reduction %); append-only transition records with slice shape; offline
`PHYSIS_EMBEDDER=random-projection` determinism; HUD honest-degradation
header; relation provenance glyphs; demo script + transcript + Feather
install notes (`docs/MVP_DEMO.md` install section: build on target, glibc,
untracked-module payload).

NEEDS ADAPTATION (shortest path, inside existing files only): HUD
`graph_view` from flat ASCII list to nested expand/collapse with zoom/LOD,
pan, select, follow-relations, parent/children reveal, source reveal, and
state channels (selected / human-pinned / intent-relevant / changed /
affected / related / inferred / ordinary) + legend; reading pane into
editable source beside the graph (open/edit/save real files via existing
`src/atomic.rs` writer, graph→source and source→graph where mappings
permit, external-change detection via existing stat fast-path + snapshot
diff); intent focus that visibly reorganizes the graph to HOT/WARM/COLD and
collapses/fades the rest; `[context]` pane showing PRIMARY / DEPENDENCIES /
RELEVANT TESTS / RECENT CHANGES / WHY INCLUDED / bounded source regions;
status bar showing `Δ N changed · M affected · context X%` +
`CONTEXT x% │ Nk potential tokens avoided` + `OFFLINE`; impact explainer
(`WHY AFFECTED` path walks from `affected_dependents` reasons + ledger
`lineage`); edge-class toggles (STRUCTURAL / DEPENDENCY / SEMANTIC / ORDER /
ONTOLOGY / HISTORY — disable openly where the leg has no data); Feather
perf table (cold index / warm startup / index size / idle+peak RAM /
nav+query+delta+propagation+context latency) with cheap-structural-delta →
UI-update → deeper-analysis ordering.

MISSING (build only this): HUD keys for `[context]`, intent focus/clear,
edge-class toggles, legend, parent/children + why-affected pane; zoomed-out
aggregation (group by dir/file, cap nodes, never render thousands); a
frozen demo workspace fixture + scripted A–G replay reusing
`scripts/mvp-demo.sh`; the Feather perf numbers above, measured not
projected.

NOT NEEDED FOR DEMO (cut): any LLM integration, agent orchestration, model
routing, cloud APIs, autonomous editing/coding, voice, browser automation,
desktop control, fleet operation, dream loops, autonomous ontology
invention, multimodality, 3D rendering, GUI replacement, diffusion /
world-models, Jev/System-One, MS/Q/S/AG research packets. Phase 20
fragments/instances and Phase 21 multimodal stay out unless the demo path
itself needs them.

### 32.3 Shortest dependency chain (in order)

1. HUD nested graph over the existing nav index (no second graph model).
2. Editable source pane + save → existing snapshot-diff `observe()` path.
3. Changed/affected/related states + WHY AFFECTED explainer.
4. Intent-focus attention op over the existing `scope()` path.
5. `[context]` pane + A/B token counts labeled potential context tokens
   avoided / context reduction (no billing or quality-equivalence claims).
6. Second-edit incremental update proving O(Δ) remap.
7. Frozen fixture + scripted A–G replay + Feather perf table.
8. Slice gates green: `tests/mvp_loop.rs`, `cargo test --lib nav::`,
   `tests/world_edges.rs`, HUD warning-free build, no LLM in path.

### 32.4 Demo script mapping (replayable, §8 A–G)

A START: networking disabled, `physis <dir>` (or `physis-pro <dir>`),
persistent index, graph visible, `OFFLINE` shown, no login/key/model.
B NAVIGATE: project → subsystem → file → symbol → source through structure;
one dependency edge followed. C EDIT: one meaningful save →
`Δ N objects · M affected · P related · Q% untouched`, graph reacts
unprompted. D CONSEQUENCES: select affected object, show WHY path
(`query.rs::search ← calls update_index() ← update_index() changed` shape).
E INTENT: one sentence in the prompt box → world collapses to HOT/WARM/COLD.
F CONTEXT: `[context]` → exact selection + reasons + WORKSPACE / SELECTED /
REDUCTION / AVOIDED from the real workspace numbers. G EDIT AGAIN: navigate
from focused graph into second source, save, incremental update + attention
recompute. Memorable moments: ZOOM (project → one function through
structure), CHANGE (edit → graph moves unasked), ATTENTION (one sentence →
128k → 8k shape).

### 32.5 Definition of done (the 16 checks)

On Feather, networking disabled, no generative model: (1) launch; (2)
persistent structured graph of a real workspace; (3) nested navigation;
(4) graph → real source; (5) edit + save real source; (6) graph reacts;
(7) affected objects inspectable; (8) why-affected shown; (9) NL intent
typed; (10) attention visibly reorganizes graph; (11) minimal context
inspectable; (12) context → source navigation; (13) second change; (14)
incremental update again; (15) real full-vs-selected token counts; (16)
all of the above with no network or LLM.

### 32.6 Whole-plan priority order (what runs first when capacity is scarce)

P0 — Demo + slice integrity (this phase + W1/W2 review): §32 chain,
`world_edges` + `nav::` + concepts HTTP gates, clippy/test green on touched
files only, `git status` shows slice-owned files. MS/nav/HUD ownership and
`packets/PROTOCOL.md` unchanged.
P1 — Unfinished correctness/isolation/release gates before any new
capability: Phases 22–24 P0–P2 items, Phase 26 P4 export/AD16 + P6 review,
Phase 28/30 reviewer confirmations (W1+W2 before any W3/W5 proposal).
P2 — Pilot-trust hardening: H4 (confidence inflation), H6 (shipped-model
accuracy gate), H7 (pack incremental cost), H1–H3 (licence/dev-key,
self-test discrimination, silent downgrade) — in that order.
P3 — Industrial pilot path only: ingest → OEE → anomaly → Italian shift
report; everything outside it frozen per H11.
P4 — Everything else waits: Phase 20/21 surfaces, Phase 27 MS-002…MS-008,
Q1–Q8 quantitative streams, Phase 28/29 AG-001…AG-006, Phase 31 S1…S6 —
each needs its own plan entry + reviewer release + frozen
manifest/splits/seeds/controls/tolerances before holdout. A negative
completes a research packet; it never promotes. No worker, merge, deploy,
or promotion follows from a thesis/evaluation section.


## Phase 33 — Physis Cortex: hardware-transparent semantic layer (2026-09-20)

Docs-only proposal; no code committed. Plans: `docs/plans/2026-09-20-cortex-pro.md`
and `physis-core/docs/plans/2026-09-20-cortex-core.md`. Packet: `packets/PH-101`
(`review`).

The neural models are NOT the world model. Physis owns objects, graph, relations,
schemas, invariants, files, services, processes, sensors, devices, actions,
provenance, temporal state, history, notebooks, editors, capability validation,
execution, deterministic retrieval, dependency propagation, semantic deltas and
stable configurations. Neural components are sensors: encoder = perception,
embedder = associative recall, ColBERT = fine reranking, small decoder =
ambiguity -> typed IR, larger model = escalation only.

**Hardware transparency.** CPU / AMD ROCm / NVIDIA CUDA must be first-class
interchangeable backends behind one `ComputeBackend` trait, with no
`#[cfg(rocm)]`/`#[cfg(cuda)]` scattered through semantic logic. The existing ROCm
path is preserved, not forked. Note the measured constraint: ONNX Runtime's ROCm
execution provider is a documented dead end (`packets/README.md`), so ROCm for
inference runs through the colibri/llama.cpp HIP stack on omo; `ort` supplies CPU
and CUDA. `PHYSIS_BACKEND=auto|cpu|rocm|cuda` plus per-model TOML overrides.

Priority per the standing order (P0 demo + slice integrity first, then P1-P4):
this is **P4** — it waits on its own plan entry, reviewer release and a frozen
manifest before any holdout. It does not touch the demo slice, the HUD surface,
the agent-native programme or the industrial pilot path.

**Progress 2026-09-22 (branch `agent/PH-101`).** Code now exists for Tasks 1–7
(core: `backend` `devices` `cortex/{encoder,latent,retrieval,router}`,
`bench::run_backend_table`, `physis-core hardware` CLI, `tests/cortex_contract.rs`
9 tests) and Tasks 9–11 (pro: `EmbedderConfig.backend` override + registry
resolution, `src/cortex/` provenance-tagged IR with validator→adapter,
`agents::routing::route_hardware`). Remaining: Tasks 12–15 (compile,
HUD overlay, training signals, `benchmarks/` tables) and the packet's
`--features embed-onnx` contract run (skipped: `ort_sys` link fails on this
box per repo history). Report: `packets/reports/PH-101.md`.
