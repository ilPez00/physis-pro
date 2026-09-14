# WorldState — architecture audit, hypothesis, formulation, and the first experiment

Written 2026-09-14. Phase 1 of the brief ("inspect before coding") was run
against `physis-core` and `physis_pro` as they stand at commit `31eeded`. This
file is deliverables 1–4 of the brief (architecture audit · hypothesis ·
minimal formulation · minimal benchmark) plus the specification of experiment
**E55**, which is the smallest run capable of falsifying the thesis.

Nothing here proposes rewriting a working component. Two-thirds of the
machinery the brief asks for already exists and is measured; the audit says
which third does not.

---

## 1. Architecture audit

### 1.1 The layers that already exist

| brief's layer | what implements it today | state | verdict |
|---|---|---|---|
| observations | `physis-core/src/observe.rs` (1007 lines) — `Observation`, append-only log, `append`/`read`/`read_tail`/`by_source`/`concurrent`, watchers `watch_fs` `watch_git` `watch_shell` `watch_agent` | shipped; ActivityWatch bucket/event/watcher shape chosen deliberately as the only domain-neutral primitive in the 21-system survey | **reuse unchanged** |
| representations | `physis-core`: `embed.rs` (`RandomProjectionEmbedder`, `MemoEmbedder`), `embed_onnx.rs` (`OnnxEmbedder`), `embed_ngram.rs` (`SyntheticNGramEmbedder`), `model_provider.rs` (`ModelProvider` trait, `DiskTableModel`, `NgramDecoderModel`); `physis_pro`: `src/embed/{registry,onnx,clip,jina}.rs` | several genuinely different spaces already selectable by config | **reuse; no new embedder** |
| objects with coordinates | `CoherenceNode` — `physis-core/src/models.rs:48`, mirrored `src/models.rs:205`. Carries `embedding`, derived `coherence_score` (kNN density), `asserted` verdict, `cell_pin`, **`embedder` provenance**, `previous_coherence_state`, `preference` | shipped | **reuse as the entity carrier** |
| relations | `relation.rs` — `TypedEdge` + `RelationType` (`Supports` `Contradicts` `Causes` `Influences` `Precedes` …) with provenance, confidence and `TemporalValidity` on every edge | shipped | **reuse** |
| world state | **`WorldState` already exists**: `physis-core/src/transform.rs:252` — `{ facts: Vec<Proposition>, constraints: Vec<Constraint> }`, with `triples()`, `without_relation()`, `substitution_invariants(a,b)`, `incoherent_after()` | shipped, symbolic, deterministic, no embeddings | **extend — do not author a second `WorldState`** |
| structural mapping | `transform.rs` symbolic homomorphism engine (`find_homomorphisms`, `Predicate`, `TriplePattern`, backtracking join); `structsim.rs` node-level structure comparison in one embedding space (`flatten_json`, `compare_structures`) | `transform`'s ANALOGY arm measured E53: top-3 0.500 (n=86) / 0.446 (n=294) vs popularity 0.337/0.303 and permuted null 0.209/0.256 | **reuse both; they are the two halves of "structure first"** |
| temporal | `temporal.rs` (259 lines, `TemporalValidity{valid_from, valid_until, trigger}`, `narrow_until`); `epistemic.rs` replayable audit trail ("what did Physis believe at T"); `becoming.rs` (Wald–Wolfowitz runs test: meaning-change vs name-collision, z-scored); tests `temporal_narrowing.rs`, `temporal_dream.rs` | **the thinnest real layer.** Validity intervals exist; *state sequences* do not | **extend — this is where new code goes** |
| retrieval | `rag.rs` — `TokenFixedRetriever`, `Bm25Index`, `fuse_rrf`, `Selection::CascadeRerank`; absolute relevance floor 0.66; `act.rs::bearing_on` keeps frozen cosine as the library baseline; `pack.rs`, `map.rs` | measured: `hit@5` 0.57 semantic vs 0.14 lexical; cascade polarity 0.250 → 0.875 | **reuse as the baseline arm** |
| experiment harness | 70+ `physis-core/examples/experimentNN_*.rs`, `benchmarks/retrieval/run.py` ("the one that can fail"), `physis-core/benchmarks/{ground-truth,direction,results/*.json}`, `justfile`, `scripts/declared-never-called.sh`, `just recall` | mature; every new experiment follows the `examples/` + `benchmarks/results/*.json` convention | **reuse the convention exactly** |
| visualization | `physis-core/src/studio.rs` (+ `studio_lab.rs`, `studio_communities.rs`), `src/web/`, `physis_wow/*.html` | exists, mostly hand-seeded dashboards | **reuse the plumbing, not the diagrams** |
| tests | physis-core lib 270 pass; `tests/` in both crates; clippy clean on `src/`, red on physis-core `examples/` | — | keep green; do not add red |

### 1.2 What does not exist

1. **A temporal sequence of world states.** `TemporalValidity` marks *when a
   fact holds*. Nothing holds `state_t0 → state_t1 → …` as objects, so
   persistence / change / emergence / disappearance / recurrence are not
   computable today.
2. **Structure derived independently per representation, then compared.**
   `structsim` compares two structures *inside one space*. Cross-representation
   work to date compared **clusterings**, not structures (see §1.4).
3. **Navigation.** Retrieval is ranked selection. There is no traversal of
   relations from a current state.
4. **A shell over the world.** CLI verbs are per-module, not `locate / neighbors
   / history / changed / compare / trace / why`.
5. **The observation → representation join.** `observe.rs` records; the
   embedders consume documents; `ngram_table::build_structural` has three call
   sites and none is the log (`physis-core/src/main.rs:1902`, its own
   definition, one test).

### 1.3 Dead or unproven machinery — do not build on it

- `transplant` — **zero call sites** anywhere (src, CLI, examples, tests).
- `OnnxEmbedder` was exported and uncalled once already; every benchmark number
  of that era came from a lexical hash. **Grep call sites before using it as
  representation B** (`scripts/declared-never-called.sh`).
- `structsim`, `becoming` — never put in front of a null.

### 1.4 The prior results that constrain this program

These are not background. They are the reason several obvious designs are
already refuted, and E55 must be built so as not to re-run them.

| finding | number | consequence for this program |
|---|---|---|
| embedding geometry gives no stable ontology structure | cross-embedder **ARI ≈ 0.10**; +25% corpus replaced half the anchors (ARI 0.198) | **This is the null E55 must beat.** "Cluster A, cluster B, compare partitions" is *already measured and negative*. If structure-before-mapping is worth anything it must beat 0.10 on the same kind of comparison, using relations rather than partitions. |
| no cheap signal certifies discovered structure | every scorer at chance on real misfilings (cosine 0.519, CONSTRAINT 0.536, ANCESTRY 0.534) | Do not add a confidence scorer over correspondences and call it validation. Test correspondences by held-out prediction, not by a score. |
| cross-model disagreement carries no epistemic signal | paired *t* = −10.53, 0/22 configurations | Do not use "two embedders disagree" as evidence of anything by itself. |
| the machine cannot judge whether an entry belongs | AUC 0.410 | Correspondence proposals are proposals; adjudication is a separate, measured step. |
| `propose` works where `judge` failed | top-3 0.712 vs null 0.136 | The shape that works here is **rank a few candidates**, not **decide one**. Design φ as a ranked candidate list. |
| embeddings cannot separate agreement from contradiction | physis C2 (embedding@0.90 precision 0.667 with perfect recall; structural identity precision 1.000 recall 0.375) and MemStrata's independent **AUROC 0.59** (arXiv 2606.26511) | Contradiction detection in E58 must not be cosine-based. |
| `transform` ANALOGY beats a degree-preserving control | +0.291 / +0.189 top-3 | Structure-derived correspondence is *the one arm with prior positive evidence*. E55 is its generalisation. |

**Reading of the audit in one line:** OBSERVE is built, BELIEVE is built and
measured, ACT is partly built, and **time-as-sequence, cross-representation
structure comparison, and navigation are the three genuinely missing pieces** —
which is exactly the brief's WorldState.

---

## 2. Research hypothesis

**H1 (main).** Structure derived independently in two representation spaces,
followed by *tested* correspondence inference, yields higher cross-space
agreement than (a) raw geometry, (b) random correspondence, (c) cosine-nearest
correspondence, and does so on held-out items and under a third representation.

*Null:* structure-derived correspondence performs no better than cosine
correspondence, and cross-space agreement stays at the level already recorded
(ARI ≈ 0.10).

**H2 (positional).** Representing a corpus as a sequence of world states ordered
by position in the text
improves retrieval and state reconstruction over order-blind embedding
similarity.

*Null:* shuffling the state order leaves every metric within noise. **This null
is cheap and must be run first** — `becoming.rs` exists precisely because
order-blindness was the defect there too.

**H3 (invariants).** Some relations survive a change of representation, and
which ones survive is predictable rather than arbitrary.

*Null:* survival is indistinguishable from a degree-preserving random subset.

**H4 (navigation).** Traversal of structural + temporal relations from a
current state retrieves relevant observations that ranked similarity misses, on
adversarial cases where lexical similarity and structural relevance disagree.

*Null:* navigation is a slower `TokenFixedRetriever`.

**H5 (downstream).** An LLM given a world-state region answers relational and
temporal questions more consistently than the same LLM given top-k similar
text.

*Null:* the LLM performs equally well without the world model. External
context: STALE reports the best evaluated model at **55.2%** overall on
memory-validity, so a small delta here is not a small result.

Every hypothesis above is stated so a negative is publishable. Per the repo
rule, a failed hypothesis gets written into the module header, not deleted.

---

## 3. Minimal formulation

Deliberately small; every symbol maps to something already in the tree.

**Observations.** `O = {o_1 … o_n}`, each an `observe::Observation`
(`seq`, `source`, `t`, `payload`). Append-only, never rewritten.

**Representations.** `R_i : O → ℝ^{d_i}`, one per embedder, each identified by
the `embedder` string already carried on `CoherenceNode`. At least two are
independently constructed; a third is held back for the transfer test.

**Structure operator.** `Γ_i = Γ(R_i(O))` — a graph, not a partition. Edges from
three sources, each already implemented:

- `Γ_knn` — mutual-kNN over cosine (the density notion `coherence_score`
  already uses),
- `Γ_rel` — typed relations from `relation.rs` extracted from the observation
  payload,
- `Γ_sym` — symbolic triples from `transform.rs` (`Predicate`, `Triple`).

Γ is computed **per space, independently, with no reference to the other
space.** That is the whole discipline: structure first.

**WorldState.** Extend the existing symbolic `transform::WorldState`, do not
replace it:

```text
W_t = ( E,        entities            (CoherenceNode ids)
        F,        facts               (transform::Proposition, existing)
        C,        constraints         (transform::Constraint, existing)
        X_i,      coordinates per representation i  (embedding + embedder tag)
        N,        structural neighborhood            (Γ_i restricted to E)
        p,        ordinal POSITION IN TEXT (sentence index; not a clock)
        P,        provenance                         (observation seq ids)
        κ,        coherence/confidence               (coherence_score, asserted)
        T )       transitions to other states        (NEW)
```

Only `X_i`, `N` and `T` are new fields; `T` is the only new *mechanism*.

**Transitions.** `τ : W_p → W_{p+1}` decomposed into the five measurable
predicates the brief names — persistence, change, emergence, disappearance,
recurrence — each defined as a set operation over `E` and `F` between adjacent
states. **No causal claim.** The vocabulary is *temporal transition*; `Causes`
already exists in `RelationType` and is only used when a causal test supports
it.

**Correspondence.** `φ : Γ_A → Γ_B`, emitted as a **ranked candidate list per
node** (the `propose` shape, which works, not the `judge` shape, which does
not).

**Agreement.** `A(Γ_A, Γ_B, φ)` reported as four numbers, never one:
mutual-kNN preservation@k, edge-set Jaccard over typed relations, partition ARI
(for continuity with the ARI ≈ 0.10 prior), and relation-type confusion.

**The quantity under test.** `Δ = A(φ_structure) − A(φ_baseline)`, with
`φ_baseline` instantiated four ways (§4.3), reported with a bootstrap CI and an
effect size, on held-out items, and again under representation C.

**Traceability requirement.** Every derived edge stores
`observation seq → representation id → operator → derived edge`. An edge
without that chain is a bug, and `provenance.rs` + the hash-chain work in
Phase 22 (A4/G4) already define the storage.

---

## 4. Minimal benchmark

### 4.1 Corpus

`benchmarks/worldstate/corpus.jsonl` — **frozen before any evaluation run**,
hash recorded in the results artifact.

- 60–100 sentences describing one small evolving world (workshop/plant is the
  natural fit for the existing industrial vocabulary), 8–12 entities.
- **The temporal coordinate is `pos`, the sentence's ordinal position in the
  text — not a timestamp.** Order is intrinsic to any corpus, needs no
  extraction, and makes H2's shuffle null exact. `becoming.rs` already reasons
  this way (runs test over sequence, not over clock). `TemporalValidity` stays
  for facts that carry genuine valid-time; the two are different coordinates on
  the same object.
- Each sentence carries: `pos`, entities, a gold typed relation, and a gold
  transition label (persist / change / emerge / disappear / recur).
- **Adversarial pairs are mandatory** (H4): ≥15 pairs where lexical overlap is
  high and structural relevance is low, and ≥15 where the reverse holds.
- **Agreeing near-duplicates are mandatory** (the C2 lesson): without
  near-duplicate *agreements* as negatives, a contradiction metric measures
  similarity and nothing else.
- Held-out split 70/30 by entity, so alignment cannot memorise.

Second corpus for scale sanity: 1000 sentences generated by the same schema
with a recorded seed. Not for headline numbers.

### 4.2 Metrics (frozen with the corpus)

Structure: mutual-kNN preservation@{5,10} · typed-edge Jaccard · ARI ·
relation-type confusion matrix.
Temporal: transition-label accuracy · state-reconstruction F1 · retrieval
hit@{1,5} with and without temporal position.
Navigation: hit@k on adversarial cases · path length · number of observations
returned.
Downstream: factual consistency, contradiction rate, relational-question
accuracy, temporal-question accuracy.
Everything gets a bootstrap CI. No metric is added after the first evaluation
run.

### 4.3 Arms

| arm | what it is | why |
|---|---|---|
| `baseline` | no alignment; compare Γ_A and Γ_B as they fall | reproduces the ARI ≈ 0.10 prior |
| `random` | random bijection, degree-preserving | the control that killed several earlier claims |
| `cosine` | nearest-neighbour in a naively projected joint space | the obvious method |
| `supervised` | alignment trained on semantic labels — **explicitly labelled a supervised condition** | the brief's requirement; it is an upper reference, not a competitor |
| `structural` | the hypothesis: Γ-derived correspondence, tested | H1 |

### 4.4 Reproducibility

Every run records: corpus hash, seed, embedder ids and checksums (the checksum
registry already exists), crate commit, arm configs, wall time. Artifact
`benchmarks/results/worldstate-e55.json`. Rerunnable with one command, per the
repo rule that every experiment must be rerunnable from the repository.

---

## 5. E55 — the first experiment (the one to build now)

**Name.** `physis-core/examples/experiment55_structure_before_mapping.rs`
(numbering continues E53/E54).

**Question.** Does structure-derived correspondence beat cosine correspondence
on cross-representation agreement, held out, and under a third embedder?

**Procedure.**

1. Embed the frozen corpus with A = `bge-base-en-v1.5` (the embedder D4 used)
   and B = character n-gram / `SyntheticNGramEmbedder`. **Verify both are
   actually called** — `scripts/declared-never-called.sh` first; a lexical hash
   masquerading as a model has already produced a whole generation of numbers
   here.
2. Derive Γ_A and Γ_B independently (mutual-kNN + typed relations). Save both.
3. Emit φ as ranked candidates from structure alone: neighbourhood-profile
   comparison over Γ, no cross-space distance used at any point.
4. Score all five arms of §4.3 on the four agreement metrics.
5. Repeat on the held-out 30%.
6. Repeat the winner under representation C = `jina` (or CLIP text tower).

**Pass condition, stated before the run.** `structural` beats `cosine` and
`random` on ≥3 of 4 metrics, with a non-overlapping bootstrap CI on the primary
metric (mutual-kNN preservation@10), **and** the held-out and third-embedder
runs keep the sign of the effect.

**Kill conditions.**
- `structural ≈ cosine` ⇒ the thesis's central mechanism has no measured
  advantage. Stop, record it in `transform.rs`'s header next to E53, and
  investigate whether E53's positive was corpus-specific.
- `structural` wins on the training split only ⇒ the correspondence is
  memorising; report as an overfitting result, not a win.
- All arms sit near ARI 0.10 ⇒ the prior negative generalises; this is a
  publishable negative and the program pivots to the temporal leg (H2), which
  does not depend on cross-space agreement.

**Cost.** Three to five days, mostly corpus authoring and metric plumbing.

**Then, and only then:**

- **E56 (temporal).** Build `T` and the five transition predicates; run the
  order-shuffled null *first*. Metrics per §4.2.
- **E57 (navigation).** Traversal vs `TokenFixedRetriever` + cascade on the
  adversarial subset. Baseline is the shipped cascade, not a strawman.
- **E58 (downstream).** LLM + top-k vs LLM + world-state region, on the local
  fleet endpoints; contradiction detection must not be cosine-based (§1.4).
- **E59 (falsification suite).** The brief's Phase 9 list, run as one harness,
  including the case where ordinary embeddings win.

---

## 6. The shell (Phase 7) — thin, and late

`physis world <verb>`, one thin CLI over machinery E55–E57 have already
measured. Verbs and their existing backing:

| verb | backed by |
|---|---|
| `locate <obj>` | `CoherenceNode` coordinates + cell pin |
| `neighbors <obj>` | Γ restricted to the object |
| `history <obj>` | `epistemic.rs` trail + `TemporalValidity` |
| `changed <obj>` | transition predicates (E56) |
| `compare <a> <b>` | `structsim` / `transform::find_homomorphisms` |
| `trace <obj> <range>` | provenance chain (observation → representation → operator → edge) |
| `why <transition>` | the same chain, rendered |
| `retrieve <concept>` | navigation (E57) with the cascade as fallback |
| `inspect <relation>` | `TypedEdge` with its evidence |

**The filesystem is not touched.** Storage substrate below, navigational
substrate above — the survey's Gap 5 says 34 years of semantic filesystems died
in the write path.

No shell work before E55 and E56 have numbers. A shell over an unmeasured
mechanism is a demo.

---

## 7. Visualization (Phase 8)

Every figure is generated from a `benchmarks/results/*.json` artifact, by a
script in the repository, with the artifact hash printed on the figure. The
figure list is fixed by the experiments, not by aesthetics: representation-space
maps, Γ_A vs Γ_B with disagreement regions highlighted, before/after alignment,
temporal trajectories, retrieval paths, state transitions. Hand-drawn ontology
diagrams do not appear in the paper.

---

## 8. Paper mapping (Phase 10)

| section | source |
|---|---|
| 4. existing representation model | §1 of this file |
| 5. WorldState formulation | §3 |
| 6. structure-before-mapping | E55 |
| 7. temporal representation | E56 |
| 8. cross-representation alignment | E55 + third-embedder transfer |
| 9. retrieval/navigation | E57 |
| 10. LLM interface | E58 |
| 12. failure cases | E59 + §1.4's prior negatives, which are the strongest part of the record |
| 16. reproducibility | §4.4 |

Title: *Structure Before Mapping: Testing Cross-Representation World Models for
Semantic Navigation*. Claims tagged **demonstrated / measured / hypothesised /
speculative** throughout; `NOT MEASURED` is a legal value in every table.

---

## 9. Standing constraints for this program

1. Do not author a second `WorldState`. Extend `transform.rs:252`.
2. Do not add an embedder. Three usable spaces already exist; verify call sites
   before trusting any of them.
3. Do not use `transplant` (zero call sites) or cite `benchmarks/ground-truth`
   as retrieval evidence (it scores identically under a lexical hash).
4. Correspondences are **ranked proposals**, never verdicts (`propose` works,
   `judge` does not).
5. Contradiction is not cosine (C2; MemStrata AUROC 0.59).
6. Freeze corpus and metrics before evaluation; record seeds, hashes, versions.
7. Every relation traces back to an observation, or it is a bug.
8. A negative result is written into the module header of the code it
   constrains and kept.

---

## 10. Deliverable order, with the artifact each produces

| # | deliverable | artifact | status |
|---|---|---|---|
| 1 | architecture audit | this file §1 | **done** |
| 2 | hypothesis | §2 | **done** |
| 3 | formulation | §3 | **done** |
| 4 | minimal benchmark | §4 + `benchmarks/worldstate/corpus.jsonl` | spec done, corpus to author |
| 5 | baseline | `random` / `anchor` / `supervised` arms | **done** |
| 6 | experiment | `examples/experiment55_structure_before_mapping.rs` + `src/worldstate.rs` | **done** |
| 7 | results | `benchmarks/results/worldstate-e55-{all,train,heldout}.json`; record `research/E55_STRUCTURE_BEFORE_MAPPING.md` | **done — NEGATIVE: structural arm at its null (0.014/0.020/0.043 vs null 0.028/0.020/0.130); anchor arm 0.236/0.306/0.609 shows the information is there** |
| 8 | WorldState implementation | extension of `transform::WorldState` + `T`, now driven by H2 (position in text) rather than H1 | next |
| 9 | navigation shell | `physis world` | gated on E56 |
| 10 | visualization | scripts over the artifacts | — |
| 11 | LLM integration | E58 | — |
| 12 | full benchmark | 1000-sentence corpus, frozen | — |
| 13 | paper | §8 | — |

Work items 4–7 next, in that order, and nothing below the line until E55 has a
number that could have come back negative.
