read physis_resources.txt

# Physis Pro — Master Plan

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
  (`log_filter_from_env`). Note `.env` is untracked, so only the docs change
  is committed.
- ⬜ Features: premise is stale — `embed-onnx` **is** in `default` now
  (Cargo.toml); only `mcp` remains excluded. Consequence report and the
  decision live in `packets/reports/PH-001.md` (feeds PH-006).

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
