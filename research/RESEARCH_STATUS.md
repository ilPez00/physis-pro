# Physis research track — full status

The detailed record of the epistemic-mechanism investigation (E1–E10). This was
inlined in `PLAN.md` until it reached 540 lines; the master plan now carries a
consolidated summary and links here.

Per-experiment reports live alongside their code:
`research/perspective-discovery/` (E1) and `research/mapper/` (E2–E10).

---

## Research track — epistemic mechanisms (status 2026-09-08)

A multi-day controlled program on `research/perspective-discovery/` tested
whether embedding geometry can supply stable ontological structure and whether
any cheap signal can certify it. **It cannot, and that is now a settled result
rather than an open question.** Full trail in `FINAL_REPORT.md`; the short form
the roadmap needs:

| question | status | evidence |
|---|---|---|
| Can embedding geometry give stable ontology structure? | **NO** | cross-embedder ARI ≈ 0.10; +25% corpus replaced half the anchors (ARI 0.198) |
| Can a cheap signal certify discovered structure? | **NO — settled by E14–E18, and Stage 12's route to it was invalid** | Stage 12's refutation was decided by a random-injection policy that inverts the ranking; on **real** misfilings every scorer is at chance (cosine 0.519, CONSTRAINT 0.536, ANCESTRY 0.534) |
| Was the shipped corpus itself sound? | **NO — fixed** | 43.3% misfiled; the domain axis had no written definition. Regenerated in physis-core 0.1.21 against `docs/GRID_AXES.md`; misfiling proxy 31.8% → 16.2% |
| Can the machine *filter* discovery candidates? | **YES, shipped** | `coverage`, duplicate control Welch *t* = +12.11; 2000 → 141 |
| Can it *rank* them? | **YES, shipped in 0.1.20** | continuous `CandidateGain::lift`, precision@25 = 0.480 vs 0.132 base, exact *p* = 2.3e-5 |
| Can it judge whether an entry *belongs*? | **NO** | AUC 0.410 on real misfilings, chance inside the CI |
| Should the grid be expanded by splitting crowded cells? | **NO** | apparent substructure was hint format; names-only embedding collapses separation 20–150×, no cell divides |
| Does cross-model *disagreement* carry epistemic signal? | **NO — refuted (E1)** | \|Δ\| 0.602 vs 0.733 baseline, 0/22 configurations, paired *t* = −10.53; held-out `s1+D` 0.650 vs `s1` 0.725; conditioned on the stronger support, Δ falls to 0.423 — indistinguishable from a deranged-pairing null (0.422) |
| Does a sign/magnitude decomposition rescue it? | **NO — but it fixed the arm (E1b/E1c)** | raw sign is +1 on 86% of rows (a scale artifact, AUC 0.513); standardising each support first lifts the family 0.584 → **0.775**, above the 0.739 baseline — but Δz = 8.40·s₁ − 20.41·s₂ is a *linear* reweighting of the two supports, so `s1+s2` still dominates it (0.866) and the real/null gap conditioned on support₂ is **+0.011** |
| Can asymmetric weighting or non-linearity save the weaker model? | **NO (E1d)** | α-sweep is monotone and peaks at α=1 (discard s₁); a 2-D non-parametric estimator of P(inj\|s₁,s₂) — which bounds *every* gating and non-linearity — reaches 0.866 against s₂'s 0.870, and improves monotonically as its kernel widens toward the 1-D solution. A thin real band exists (s₂ deciles 6–7, AUC 0.67) but holds ~8% of positives |
| Then what does a second embedder buy? | **A better baseline** | BGE-base `support_2` alone 0.866, +0.132 over MiniLM, **22/22** configurations, *t* = +18.54; `support_1` adds nothing on top (0.554 within `support_2` strata) |

**Standing rule for this track, earned seven times:** treat any positive as
artifact until a construction-matched control says otherwise. E1 is the seventh:
\|Δ\| scored 0.698 within baseline strata — better than every structural scorer
Stage 12 tested — and the control that killed it was conditioning on the *other*
support instead of the baseline.

**Closed by E1** (`research/perspective-discovery/E1_CROSS_MODEL_DIVERGENCE.md`,
`physis-core/examples/experiment47_cross_model_divergence.rs`): the divergence
hypothesis (H2) is **NOT SUPPORTED**, and the mechanism is known rather than
merely observed — `D = |s1 - s2|` is a folded read-out of `support_2`, and the
folding costs ~0.10 AUC. Per the brief's stopping rule this is the FAIL branch:
E2–E5 were not implemented, and the JEPA/Qwen expansion is **stopped**.

**Open, in priority order, and each is a configuration change rather than a new
mechanism:**

1. **Re-baseline Stage 12 on BGE.** Every structural scorer in this track was
   measured against 0.733. The available baseline is 0.866. Several "close"
   negatives are not close.
2. **Control Stage 13's format confound inside the Stage 12 harness** — score
   names alone, hints dropped, and see whether 0.733/0.866 survive. Currently
   unknown.
3. **Qwen n-gram dictionary (H5): not started, and not startable as specified.**
   See `research/perspective-discovery/QWEN_NGRAM_DICTIONARY_AUDIT.md`. None of
   the three existing "Qwen" artifacts is the intended component — they are
   SHA-256 *character* n-grams in the fallback embedder, `qwen3:4b` generative
   inference, and a vocabulary authored against pre-0.1.21 cells. The AUC 0.575
   figure describes the third and is not evidence for the thesis.
4. **H3 (Stage 13, within-model structural semantics)** remains the most
   promising open direction, and E1 does not conflict with it — both findings
   point at the instrument rather than at the geometry between instruments.
5. **Human-as-reranker: built, not yet run.** The machine ranks a worklist and
   cannot certify it (0.410 AUC on real misfilings, Stage 9), so the last filter
   is a person. `human_rerank_export.rs` emits a 75-item worklist — the
   machine's head-50 plus a 25-item uniform tail — with Stage 12's injected
   misfilings mixed in blind as gold, the machine's score hidden by default, and
   the labels held in a separate file the UI never reads.
   `scripts/score_rerank.py` joins them back: exact recall on the injected,
   human-vs-machine AUC on the same items, the precision lift from human
   filtering, an anchoring check on revealed items, a repair check on named
   destinations, and — the actual deliverable — the native filings a person
   flagged, which this track has never had labelled.

---

## E2 — the mapper hypothesis: **NOT SUPPORTED**

**Hypothesis, independent of E1 and not predicted by it:**

> Can Qwen3-8B n-gram structure be distilled into a relational representation
> dictionary that acts as an initial linguistic map, which Physis can
> progressively ground, correct and reorganize through human/environmental
> feedback?

Restated as the thesis claim to be tested: *language can be treated initially as
a relational representational topology rather than a fixed semantic ontology;
sparse grounding and corrective interaction can progressively constrain that
topology into a more useful map of externally observable phenomena.*

**The decisive criterion, fixed before running:** not whether the map becomes
internally coherent, but whether **progressive grounding makes the map
increasingly predictive of things outside the map**. A map that produces
aesthetically coherent clusters is not validated. The evidence required is
`sparse grounding + successful propagation + successful correction + external
predictive improvement`.

### Ladder, with a stop condition at each rung

| # | step | stop if |
|---|---|---|
| 0 | audit every Qwen/n-gram/tokenizer artifact, label each CURRENT / LEGACY / UNINTERPRETABLE / UNRELATED / REQUIRES REGENERATION | — (done, see below) |
| 1 | define the dictionary precisely: alphabet, unit, context, entry contents, lookup return | the definition is arbitrary rather than derived from Qwen |
| 2 | build the map — L0 n-grams, L1 pairwise relations, L2 relations between relations | a level has no principled definition or pruning criterion |
| 3 | ablate: tokenizer-only vs ordinary n-gram vs Qwen dictionary vs +Physis vs existing embedding | Qwen n-grams do no better than tokenizer-only |
| 4 | higher-order ablation, level by level | correlations-of-correlations add nothing |
| 5 | sparse-grounding curve: 0 / few / many anchors, external held-out prediction | grounding does not improve prediction, or only memorizes labels |
| 6 | emendation: inject wrong interpretations, correct one, measure repair and collateral damage | corrections do not propagate, or destroy unrelated structure |

**Terminology discipline (§4, §19 of the brief).** The map is a *relational
linguistic structure* / *representational topology*. Not meaning, truth, world
structure, causality or ontology until grounding demonstrates otherwise. Do not
write "the system learns meaning", "the map represents the world", or "human
feedback teaches the ontology".

**Three levels kept separate throughout:** LANGUAGE FORM (tokenizer, n-grams,
the Qwen dictionary) / INTERNAL REPRESENTATION (relations, correlations, map
topology) / WORLD GROUNDING (anchors, observations, corrections). Physis sits at
the boundary between representation and grounding.

**E1 does not justify this and does not predict it.** Separate hypothesis,
separate question. JEPA stays out until there is a concrete experimental reason,
and when it enters the comparison is JEPA vs the existing CLIP/Jina, never JEPA
vs nothing.

### Audit result (step 0, complete)

`Qwen/Qwen3-8B/tokenizer.json` fetched — 11.4 MB, tokenizer only, no weights and
no inference. **Byte-level BPE, 151,643 vocabulary entries, 151,387 merge
rules.** The merge list is the find: every token above the 256 byte roots was
*constructed* by merging two earlier tokens, and merge order encodes frequency
in Qwen's training corpus. That derivation DAG is what makes a dictionary
"Qwen-derived" rather than generic — a tokenizer-only baseline has the
vocabulary but not the merge structure, and a corpus-local n-gram model has
neither.

| artifact | label | what it actually is |
|---|---|---|
| `Qwen/Qwen3-8B/tokenizer.json` | **CURRENT / VALID** | the intended source; BPE vocab + merge DAG |
| `physis-core/src/embed.rs:34` `hash_ngrams` | **UNRELATED** | SHA-256 of *character* uni/bigrams into a seeded random basis; a hash is an address, carries no frequency, embedding or transition probability |
| `scripts/gen_cell_vocab.py` generated arm | **UNRELATED** | `qwen3:4b` generative inference producing word lists — a different model and a different mechanism |
| `cell_vocab_AB.json` (AUC ≈ 0.575) | **LEGACY / UNINTERPRETABLE** | authored against pre-0.1.21 cells; 77.6% of entries have since moved. Not evidence for or against the dictionary |
| `mode_anchors_ontology.json` | **CURRENT / VALID** | 70 hand-authored cell definitions, usable as grounding anchors |

**The AUC ≈ 0.575 figure is not relevant to this hypothesis.** It measures a
generative-LLM word list against a stale cell schema. Do not merge it in.

### Result (ladder run, stop conditions fired at rungs 3–5)

`research/mapper/E2_MAPPER_REPORT.md`. Verdict: **MAPPER HYPOTHESIS NOT
SUPPORTED**. Held-out prediction of the human-authored grid cell, 658 entries,
70 cells, 30% held out, majority floor 0.102.

| arm | k-NN | propagation |
|---|---:|---:|
| A tokenizer-only (Qwen vocab, no merges) | 0.315 | 0.193 |
| B ordinary char 3–5 gram (**no Qwen**) | **0.355** | 0.228 |
| C Qwen dictionary, L0 rank-weighted | 0.340 | 0.254 |
| C+ Qwen dictionary + merge-DAG ancestors | 0.228 | 0.147 |
| L1 Qwen + PPMI relations | **0.355** | **0.274** |
| L2 relations between relations | 0.279 | 0.188 |
| **E existing embedding (BGE-base)** | **0.447** | **0.299** |
| CONTROL shuffled / random map | 0.056 / 0.051 | 0.117 / 0.091 |

Four measurements carry the verdict:

1. **No disproportionate sparse gain.** 4.3% of the labels buy 3.2% of the gain
   for the best Qwen arm, 15.2% for BGE. At 20 anchors every Qwen arm is at or
   below the majority-class floor. The hypothesis's decisive prediction is not
   observed.
2. **The map's topology does not carry labels.** Diffusing through it is worse
   than ignoring it for every arm with signal (−0.081 to −0.147) and *better*
   than nothing only for the structureless controls.
3. **Corrections do not propagate** — 0.05–0.31 held-out entries repaired per
   correction, corrupted regions stay corrupted until touched individually. That
   is relabelling, not emendation. No collateral damage either, but only because
   nothing travels.
4. **The Qwen structure does not clear its own baselines.** +0.025 over
   tokenizer-only, −0.015 against ordinary character n-grams, and the merge DAG —
   the most Qwen-specific element — costs −0.112.

**Two caveats kept rather than dropped.** The L2 null is *sample-size bound*: at
658 short entries L1 is already sparse and L2 sparser by roughly a square, so it
does not show that second-order structure cannot help. And ablation D, *Qwen
dictionary + Physis*, was **not run** — the map was never passed into the
hypothesis/coherence/provenance machinery, so what is reported is representation
quality, not Physis. It was skipped because the stop conditions had already
fired, and adding an interpretive layer to a null is the move the brief forbids.

**One pre-registered prediction was wrong, and that is recorded.** Stage 13's
format confound did *not* reproduce: names-only beats hints-included for the
Qwen arm (0.279 vs 0.254) and for BGE (0.320 vs 0.299). The plateau is not on a
house-style axis; it is simply lower than the hypothesis needs.

### E3/E4 — the density diagnosis, tested with 24x the text and REFUTED

E2 closed by blaming the corpus: *"a data problem before it is an architecture
problem."* That was a diagnosis, not a measurement. It has now been measured and
it is **wrong**.

Added: the physis codebase through physis's own `CellClassifier` (5,237 chunks)
and the `~/Documenti` library (10,167 chunks, 342 files, 21.1 M characters of
philosophy, ebooks, sociology, semiotics). Volume 24x; variation far more than
that. Evaluation unchanged -- the 658 human-authored cells, E2's split.
Machine-assigned cells on the new corpora are never scored and never anchored.

| level | PPMI from 658 | PPMI from union | delta |
|---|---:|---:|---:|
| C Qwen L0 | 0.340 | 0.203 | **-0.137** |
| L1 Qwen + PPMI | 0.355 | 0.183 | **-0.173** |
| L2 relations-of-relations | 0.279 | **0.066** | **-0.213** |

L1 edges rose 124x (170,944 -> 21,268,718) and every level got worse; L2 on the
union falls **below the 0.102 majority floor**.

**The two confounds were separated before attributing anything.** Holding the
vocabulary fixed and re-estimating PPMI on the union: 0.218 -> 0.188, so
**density alone costs -0.030**; vocabulary displacement costs a further -0.005.
The collapse is density, on the exact arm the diagnosis named.

**Leg B, node density:** BGE gains +0.051 at full supervision from unlabelled
substrate but **nothing at k=20** -- extra nodes help a dense problem, not a
sparse one, which is the opposite of what the mapper needs. The Qwen arm gains
exactly zero, and edge counts say why: 10 labelled->extra edges against 12,000
labelled->labelled (0.1% cross share, versus BGE's 15.1%). The Qwen n-gram map
puts ontology entries and prose in disjoint regions, so labels cannot diffuse
through substrate nothing connects to. Property of the representation, not a bug.

**Replacement explanation, supported where the old one was not:** the relations
that help are estimated *within the target domain*. PPMI over 21 M characters of
philosophy and Rust encodes the structure of philosophy and Rust, which is
orthogonal to what separates `HEAL/LIFT` from `STUDY/SENSE`. **Domain-matched
sparse statistics beat domain-mismatched dense ones.** The pre-registered
"L2 null is sample-size bound" caveat also dies: given 24x the sample, L2
collapsed further rather than recovering.

**Carried forward:** BGE-base remains the strongest representation this repo has
for grid prediction (0.447 against every constructed alternative). The
"needs more data" escape hatch is closed by measurement.

### Two risks this track has already measured, carried into the design

1. **Stage 13.** A single embedder read two authors' hint-formatting conventions
   as a semantic stratum; separations collapsed 20–150× when hints were dropped.
   A distributional map will encode *who wrote an entry* as if it were
   structure, and sparse grounding would then anchor onto the format axis and
   look like it is working. The names-only format control belongs in E2 from the
   start, not added after a positive.
2. **Stage 9.** Physis scored AUC 0.410 on real misfilings — internal coherence
   was uninformative about correctness. That is the concrete form of the brief's
   own stop condition, *self-consistent without becoming externally predictive*.

**Pre-registered prediction, recorded so it can be wrong:** the grounding curve
rises steeply and then plateaus at whatever axis the distributional map most
strongly encodes — and on this corpus part of that axis is house style rather
than meaning.

**Power constraint, stated up front rather than discovered as a null:** the
corpus is 658 short entries. Level-1 pairwise n-gram statistics off that are
already sparse and level-2 is sparser by roughly a square, so a null on
higher-order relations would be a *sample-size* result, not evidence that
higher-order structure cannot help. Report it as such.

---

---

## E5 — is the 70-cell grid the constraint? **NOT SUPPORTED**

`research/mapper/E5_TAXONOMY_REPORT.md`. FINAL_REPORT.md's conclusion that
"the taxonomy, not the embedding or the calibration heuristic, [is] the dominant
constraint" partly rested on the embedder swap being modest — which E1 revised
(BGE beat MiniLM 22/22, +0.132). So it was tested on its own.

**Proper supervision, kappa rather than the majority floor** (57 classes, 197
held out): k-NN k=5 accuracy 0.447 / **kappa 0.416**; nearest centroid 0.431 /
0.404; softmax 0.437 / 0.399; floor 0.102 / 0.000. Label-noise ceiling ≈ 0.84
(Stage 11's 16.2% misfiling proxy), so the representation reaches about half the
achievable accuracy. Accuracy 0.447 against macro-F1 0.318 says performance is
very uneven across classes.

**Granularity — the decisive test:**

| level | classes | kappa | gain over floor |
|---|---:|---:|---:|
| cell (domain × mode) | 57 | 0.404 | 36.7% |
| mode | 14 | 0.418 | 34.4% |
| domain | 5 | **0.552** | 51.7% |

Collapsing 57 classes to 14 gains **+0.014 kappa**. If granularity were binding,
it would gain far more. Merging confusable cells (average linkage) peaks at
kappa 0.444 and then declines to 0.237. **The grid's fineness is not the
constraint.**

**The asymmetry: domain kappa 0.552 vs mode 0.418.** Stage 11 gave the *domain*
axis a written definition in `docs/GRID_AXES.md` and regenerated the corpus
against it; the mode axis never got that treatment. **E6 then localized it —
see below — and corrected the recommendation this line originally carried.**

**Physis integration (`mapper_physis_feedback.rs`).** The correction loop run
through the shipped `QualityTracker` on the production path
(`penalize_cell` at `PIN_DISSENT_SEVERITY` = 0.25, `report_success`,
`adjust_score` over `classify`'s CellScores). Scored only on entries not
themselves corrected: naive re-seeding costs −0.005 to −0.082; **the
QualityTracker path costs −0.23 to −0.27**. `penalize_cell` applies a global
per-cell penalty, and the cells chosen wrongly most often are the largest ones,
so penalizing them damages the entries that genuinely belong there. Stated
fairly: QualityTracker was built for slow interactive pin dissent, not batched
correction; the finding is that **the existing machinery is not a
correction-propagation mechanism**, not that the shipped feature is broken.

**Where this leaves the constraint**, in order of support: (1) class *sparsity*;
(2) ~16% label noise; (3) a real but narrower definition gap on the mode axis.

---

## E6 — where the mode axis is weak, and why

`research/mapper/E6_MODE_AXIS_REPORT.md`. E5 called the missing mode-axis
definition "the most actionable finding". **E6 measured it and that was
overstated.**

**Population explains most of it.** Pearson r(log class size, F1) = **+0.718**
across the 14 modes. The three modes scoring **F1 = 0.00** are BRAINSTORM (n=3),
DESTROY (n=4) and GUIDE (n=14). A mode with three entries cannot survive a 70/30
split however well its definition is written.

**The definition gap is real but half the size implied.** Stage 11's own test —
does the nearest authored anchor recover the entry's axis value? — with the
domain axis as the control because it *has* a written definition:

| axis | recovered | chance | gain |
|---|---:|---:|---:|
| domain | 0.562 | 0.272 | **39.9%** |
| mode | 0.372 | 0.228 | **18.7%** |

Unchanged when cell names are stripped and only the authored hints are used.

**And the confusion is concentrated, with a sink.** WORK appears in four of the
top five confusion pairs and absorbs 30 errors: CREATE↔WORK 11, SENSE↔WORK 8,
MAINTAIN↔WORK 7, WALK↔WORK 4. Its definition — *"execute, labour, do the routine
doing"* — excludes almost nothing. That is the junk-drawer shape Stage 13 found
at `CONSTRUCT/CREATE`, one axis up. The remainder clusters in
SENSE↔MAINTAIN↔LEARN↔PLAN, which is the signature of a definition problem rather
than a representation one.

**What follows, ranked by evidence:** (1) populate the starved modes — five
modes hold 47 entries between them, a corpus task not a writing task; (2) define
WORK **by exclusion**, one mode rather than the axis; (3) re-test the
SENSE/MAINTAIN/LEARN/PLAN cluster, remembering E5 showed merging cells does not
help — if those four are truly one thing that is a finding about the grid's
design, not a licence to merge.

---

## E7 — do E6's fixes hold up? One confirmed, one shrunk, one refuted

`research/mapper/E7_FIX_TESTS_REPORT.md`.

**1. Population — confirmed and quantified.** Subsampling well-populated modes'
training entries reads the F1(n) curve off modes that have data:

| mode | n | k=3 | 6 | 10 | 20 | 40 | all |
|---|---:|---:|---:|---:|---:|---:|---:|
| WORK | 150 | 0.02 | 0.07 | 0.13 | 0.26 | 0.38 | 0.49 |
| CREATE | 117 | 0.04 | 0.11 | 0.16 | 0.34 | 0.45 | 0.55 |
| SENSE | 93 | 0.04 | 0.18 | 0.29 | 0.49 | 0.56 | 0.59 |

A well-populated mode cut to three training entries scores **F1 0.06 on
average** — WORK, with 150 entries of concept behind it, manages 0.02. So
BRAINSTORM/DESTROY/GUIDE scoring 0.00 is what three entries produces, not
evidence they are ill-defined. **The curve is a forecast:** populating a starved
mode to n≈20 should reach **F1 0.26–0.49**, and most of the gain is bought by
the first twenty.

**2. The WORK sink — real, a third of what E6 implied.** Removing WORK from the
label space lifts the other 13 modes from kappa 0.441 to **0.484 (+0.043)**;
WORK captures 7.8% of non-WORK held-out entries. Worth fixing, not transformative.

**3. The SENSE/MAINTAIN/LEARN/PLAN cluster — refuted.** Restricted kappa 0.605
against a size-matched random-four-mode control of mean 0.629, range 0.454–0.800.
The cluster sits *inside* the control range. E6 was pattern-matching on raw
confusion counts, which are dominated by class size. **Seventh positive in this
track to die to a size-matched control** — and this one needed no new mechanism
to catch, only asking what four arbitrary modes would have scored.

**Revised actions:** (1) populate BRAINSTORM, DESTROY, GUIDE, PLAY, LIFT to ~20
entries each — a corpus task with a quantified payoff; (2) define WORK by
exclusion, worth ~+0.04 kappa; (3) dropped.

**Still not established:** that (1) raises *overall* mode kappa by any particular
amount. The curve predicts each mode's own F1; the axis-level effect depends on
final class balance, which is a decision rather than a measurement.

---

## E8 — a pre-flight checker, and hypertrophic propagation confirmed

`research/mapper/E8_PREFLIGHT_AND_HYPERTROPHY.md`, `preflight.py`.

**A feasibility checker for the deductively-decidable failures** — span,
metric well-posedness, power, leakage, class sparsity. Validated against arms
whose outcome is already known: it **REFUTES** signed Δz (R² = 1.0000 on the
baseline; it lost), **passes** support₂ (R² = 0.117; it won 22/22), and **cannot
kill** \|D\| (R² = 0.866; it carries real variance and merely turned out weak).
That is the honest boundary — it catches algebraic redundancy, not weak signal.
On Lean specifically: the span lemmas are a few lines and a rank computation
answers them with less ceremony, so they are implemented directly.

**Hypertrophy — asked, and confirmed.** Differentiation = intra-cell similarity
minus a size-matched random control; hypertrophy = r(log size, differentiation).

**HYPERTROPHY INDEX = −0.644.** The bigger a mode gets, the *less* it
differentiates. WORK (150) sits **+0.003** above a random sample of the corpus,
CREATE (117) +0.003, MAINTAIN (77) +0.003, WALK (28) +0.002 — as groups of
meanings they are barely groups. The small modes are the differentiated ones
(DESTROY +0.150, BRAINSTORM +0.073). **And the periphery is where it happens:**
every large mode is 4–10× better differentiated once its outer quarter is
removed, which turns "WORK is too broad" into a reviewable list of entries.

**Grid expansion — refuted before running.** The checker STOPs at every level:
the current grid already has 35 of 57 classes below 6 training entries
(forecast F1 0.07); a 2× expansion starves **140 of 140** even assuming perfect
balance, a 4× expansion 280 of 280. The retrospective sting: **the current grid
already fails this check**, so a good deal of this track has been measuring
sparsity rather than the hypothesis under test.

**"Machines categorize better" — half-supported.** k-means partitions must
survive resampling to be taxonomies. ARI across disjoint 80% resamples: k=5
**0.463** (moderately stable), k=14 0.277, k=57 **0.213**, k=140 0.202. For
scale, Iteration 18's cross-embedder ARI of 0.10 was treated as fatal. Machines
can carve at coarse granularity; at the resolution the grid uses they cannot,
not on 658 entries.

---

## E9 — a discrete signifier address, and whether Lean has anything to hold

`research/mapper/E9_SIGNIFIER_ADDRESS.md`. Proposal: replace geometric
membership with a **value** per entry; membership becomes a decidable predicate;
the value is a signifier, not the thing; tokens are the alphabet. Lean would then
earn its keep — consistency of the assignment is machine-checkable. But
formalising noise is worse than not formalising, so the prior question is whether
a discrete address carries the ontology at all. **Standard to beat: geometry's
0.447 held-out cell accuracy.**

**The fractal guess is SUPPORTED.** A dyadic address (recursive bisection on each
node's own principal direction; membership is a prefix test) gains bits per level
of `0.30 0.34 0.30 0.49 0.55 0.65 0.72 0.67` over its eight unsaturated depths —
first half 0.356, second half 0.648, **ratio 1.82**. Information per level is
constant-to-rising, which is what self-similar structure looks like, and it beats
a random address at matched granularity (4.017 vs 3.436 at depth 8).

*(This experiment's first automated verdict said the opposite. It was wrong: my
splitter stops dividing nodes below four members, so past depth 8 the gain is
structurally zero, and averaging those zeros in measured the stopping rule. Bits
per level are now computed only over unsaturated depths.)*

**But no address tested generalises as well as geometry:**

| scheme | held-out cell accuracy |
|---|---:|
| geometry (BGE k-NN) | **0.447** |
| best dyadic address (depth 6, 64 groups) | 0.244 |
| token address, k=1 rarest | 0.091 |
| token address, k>=3 | 0.000 |

**"Use tokens directly" fails on alphabet resolution.** At k=2 the code produces
**653 groups for 658 entries** — it names each entry, so in-sample normalised MI
reads a perfect 1.000 while held-out accuracy is 0.010 and 99% of held-out codes
were never seen in training. Qwen's 151,643-symbol vocabulary over ~22-token
entries makes any exact rare-token conjunction a fingerprint.

**Recommendation: do not build the Lean layer yet, and the direction is not
refuted.** A decidable address currently carries about half of what the geometry
it would replace carries, so formalising one today would guarantee the
consistency of a half-wrong assignment. But the space *is* hierarchically
structured with roughly constant information per level, which is the property an
address scheme needs to be worth having. **The corrective is coarsening**:
merge-rank bins rather than token identities, or the dyadic path truncated where
held-out accuracy peaks. The bar is 0.447.

---

**Synthesis — the corpus is the bottleneck, not the grid.** E5 (collapsing never
helps), E7 (the failing modes are the empty ones), E8 (expansion refuted on
sparsity, machine partitions unstable when fine) converge. **Populate first,
expand second:** a 140-cell grid needs on the order of **2,800 entries** against
the current 658. Available today without touching the grid's shape: the outer
quarter of WORK / CREATE / MAINTAIN / WALK, worth ~+0.043 kappa.

---

---

## E10 — form through selective error: **NOT SUPPORTED in this instantiation**

`research/mapper/E10_SELECTIVE_ERROR.md`. Hypothesis: systems maintain form by
discriminating deviations by their *consequences* — those preventing viable
continuation are abandoned, non-handicapping ones persist and become part of the
form. Tested through the brief's own test case (music), where identity separates
from attributes without argument: **a motif's identity is its interval contour,
not its pitches.**

**§8's register formalism is coherent.** `I(Tr(O)) = I(O)` with `A(Tr(O)) ≠ A(O)`
holds for 24/24 motifs under transposition — a definition holding, not an
empirical finding, and recorded as such.

**40 replicates × 240 generations:**

| arm | depth | identity | diversity | corrections | neutral used |
|---|---:|---:|---:|---:|---:|
| A reject every deviation | **172** | n/a | **171** | 240 | 0 |
| B accept every deviation | 137 | 0.13 | 123 | 132 | 5 |
| C select on **local** correctness | 132 | 0.13 | 125 | 181 | 7 |
| D select on **downstream** viability | 126 | **0.15** | 119 | 187 | **8** |

**Downstream beats local by +0.01 identity and loses 5.8 depth and 6.2
diversity.** The trade is bad. Two of the brief's own falsification targets are
met: downstream viability *is* worse than immediate correctness here, while
neutral variation does *not* cause combinatorial growth (accept-all diversity 123
against downstream's 119) — so it fails for a different reason than predicted.

**What survived:** register. Arm A — reject everything novel, fall back to a
small transposition — reaches the greatest depth and diversity. Partly an
artifact (the fallback is itself a productive move, and it is flagged), but the
reading holds: in this system the deviation that preserves identity while
changing attributes is *register*, exactly as §7 proposes, and the exotic
deviations mostly break the invariant.

**Honest limit:** the mechanism barely fires. Identity-preserving-and-viable
deviations occur 5–8 times in 240 generations, under 3%. The arm comparison had
power; the phenomenon's rate inside the system did not. A real test needs a
deviation set where identity-preserving moves are common and a viability horizon
longer than six steps.

**Process note, recorded because it matters:** the first run had three faults —
`dev_transpose` drew a fresh offset per note (scrambling the contour instead of
transposing, leaving identity 0.00 everywhere), the downstream filter never bound
(arms C and D byte-identical), and **the pre-flight returned STOP on power and
the run proceeded anyway.** The third is the serious one. A checker overridden by
the person who built it is not a control.

---

**Product consequence, stated plainly:** classification against the grid is the
feature this track was trying to certify, and it still has no certified
soundness check. `coverage` filters and ranks discovery candidates; it does not
verify that anything is filed correctly. Sell the filter, not the judgement.

---



---

# Detail: E12–E18 (moved from PLAN.md, 2026-09-08)

## Research track — epistemic mechanisms (status 2026-09-08)

Full record: **`research/RESEARCH_STATUS.md`**. Per-experiment reports sit with
their code in `research/perspective-discovery/` (E1) and `research/mapper/`
(E2–E10).

### What is settled

| question | answer | key evidence |
|---|---|---|
| Can embedding geometry give stable ontology structure? | **NO** | cross-embedder ARI ≈ 0.10 |
| Can a cheap signal certify discovered structure? | **NO — settled (E14–E18)** | On **real** misfilings every scorer is at chance: cosine 0.519, CONSTRAINT 0.536, ANCESTRY 0.534. ANCHORVOC's 0.664 was an entry-length artifact that fails replication |
| Was the shipped corpus sound? | **NO — fixed** | 43.3% misfiled; regenerated in 0.1.21 |
| Can the machine *filter* discovery candidates? | **YES, shipped** | `coverage`, Welch *t* = +12.11 |
| Can it *rank* them? | **YES, shipped 0.1.20** | precision@25 0.480 vs 0.132 base |
| Can it judge whether an entry *belongs*? | **NO** | AUC 0.410 on real misfilings — and **E14 explains why**: 0.880 on randomly-injected errors, **0.506 on plausible ones** |
| Does cross-model **divergence** carry signal? | **NO (E1)** | 0.602 vs 0.733, 0 wins in 22; equals its null once conditioned on the stronger support |
| Can any weighting/non-linearity rescue the weak model? | **NO (E1d)** | 2-D non-parametric estimator bounds every combiner: 0.866 vs s₂'s 0.870 |
| Can Qwen n-grams be a groundable **map**? | **NO (E2)** | no disproportionate sparse gain; propagation worse than ignoring the map |
| Is it a **data** problem? | **NO (E3/E4)** | 24× the text made every level worse; density alone costs −0.030 |
| Is the **70-cell grid** the constraint? | **NO (E5)** | 57→14 classes gains +0.014 kappa |
| Is the mode axis weak from missing definition? | **PARTLY (E6/E7)** | population explains it: r = +0.718 with log class size |
| Do ontologies propagate **hypertrophically**? | **YES (E8)** | hypertrophy index **−0.644**; WORK sits +0.003 above random |
| Should the grid be **expanded**? | **NOT YET (E8)** | pre-flight STOPs on sparsity at every level |
| Is there a discrete **signifier address** worth formalising in Lean? | **NOT YET (E9)** | best address 0.244 held-out vs geometry's 0.447 — though the space *is* self-similar |
| Does **downstream viability** beat local correctness? | **NO (E10)** | +0.01 identity for −5.8 depth, −6.2 diversity |

### E14 — a caveat that propagates backwards

`research/mapper/E14_INJECTION_REALISM.md`. Stage 12 injects misfilings into a
**uniformly random** cell. Re-running the same scorer on the same corpus while
varying only *where* a misfiled entry lands:

| injection policy | AUC |
|---|---:|
| random cell (Stage 12's original) | **0.880** |
| same domain, different mode | 0.827 |
| same mode, different domain | 0.771 |
| **the entry's nearest alternative cell** | **0.506** |
| *Stage 9, the corpus's real misfilings* | *0.410* |

**Making the injected error plausible drops the scorer to chance**, and closes
the 0.410/0.866 gap this track carried unexplained throughout. The scorer detects
**distance moved, not wrongness**: random injection displaces an entry 0.125
below its home similarity, a plausible error 0.029.

**Consequences, stated rather than buried:**

- *"Plain cosine at 0.866 is the best certification scorer here"* — written
  repeatedly in this plan — should read **~0.51 on realistic errors. There is no
  working certification scorer.**
- **Stage 12's refutations need re-testing and are not hereby overturned.**
  CONSTRAINT 0.685, ANCESTRY 0.604 and NON-LATTICE 0.459 were measured against
  cosine's 0.733 under *random* injection. Under realistic injection cosine falls
  to ~0.51; whether the structural scorers fall as far is **unmeasured**. This is
  now the highest-value open re-test in the track.
- **E1's absolute numbers are task-specific; its mechanism is not.** That
  `D = |s1 − s2|` is a folded read-out of `s2` is arithmetic and policy-independent.
- **The Disposer Bench's gold items are random-injected**, so its machine
  precision@50 of 0.640 is inflated and a rater's measured recall overstates
  practice.
- **Unaffected:** E5, E7, E8, E13 measure prediction and retrieval, never used
  injection. The corpus-sparsity conclusion stands.

Stage 12's own header said *"a synthetic perturbation is a lower bound on
difficulty"*. The caveat was written down at the start and never propagated;
E14 quantifies it, and the lower bound turns out to be the entire signal.

### E15 — and the refutations do not survive the correction

`research/mapper/E15_STRUCTURE_SURVIVES.md`. Five scorers × four injection
policies, one pass, no re-embedding.

| scorer | random | **nearest** | drop |
|---|---:|---:|---:|
| **COSINE** | **0.882** | **0.506** | **−0.376** |
| NAME-ONLY | 0.866 | 0.597 | −0.268 |
| **CONSTRAINT** | 0.675 | **0.612** | **−0.063** |
| ANCHORVOC | 0.654 | 0.584 | −0.070 |
| ANCESTRY | 0.627 | 0.566 | −0.061 |

**Under random injection cosine leads by 0.207; under realistic injection it
trails CONSTRAINT by 0.106.** Bootstrap over entries, 2000 resamples:
CONSTRAINT +0.106 [+0.029, +0.183], NAME-ONLY +0.091 [+0.034, +0.153],
ANCHORVOC +0.078 [+0.007, +0.151] — all significant; ANCESTRY +0.035, not.

**This does not show the structural scorers work** — the best is 0.612, not a
usable certifier, and Stage 9 measured 0.410 on *real* misfilings. **It shows
Stage 12's comparison was invalid.** *"Plain cosine beat every structural
alternative tried against it"* has been this track's summary of the entire
certification effort; it is true of the injection policy, not of the problem.

**Why:** geometry scores *distance moved* and degrades as the error gets subtler
(E14: displacement 0.125 → 0.029). CONSTRAINT asks whether a discrete field is
attested elsewhere in the domain — a question that does not soften.

**Second finding:** NAME-ONLY beats COSINE by +0.091. Hints help on easy errors
and hurt on hard ones, which connects to Stage 13's authorial-format result.

**Next, and it is the most valuable thing open:** score all five on the corpus's
**real** misfilings using the Stage 11 adjudication artifacts
(`cell_adjudication.tsv`, `refiling_gate0.tsv`) — the only non-synthetic ground
truth in the project. If CONSTRAINT holds its margin there, this track has a
certification signal it spent a year concluding it did not have.

### E16 — the battery on REAL misfilings, and E15's reopening withdrawn

`research/mapper/E16_REAL_MISFILINGS.md`. Ground truth: Stage 11's blind
re-filing (`refiling_gate0.tsv`) — 582 entries a rater re-filed while blind to
the existing cell, judging against `GRID_AXES.md`. MOVED = the old filing was
rejected. Base rate 80.1%.

| scorer | random inj. | nearest inj. | **real** | vs COSINE (95% CI) |
|---|---:|---:|---:|---:|
| COSINE | 0.882 | 0.506 | **0.519** | — |
| NAME-ONLY | 0.866 | 0.597 | 0.508 | −0.010 [−0.067, +0.042] |
| CONSTRAINT | 0.675 | **0.612** | 0.536 | +0.017 [−0.066, +0.097] **ns** |
| **ANCHORVOC** | 0.654 | 0.584 | **0.664** | **+0.145 [+0.072, +0.219]** |
| ANCESTRY | 0.627 | 0.566 | 0.534 | +0.025 [−0.069, +0.121] |

**E15's conclusion is withdrawn.** CONSTRAINT won by +0.106 on plausible
synthetic errors and gains +0.017 on real ones, CI straddling zero.
Plausible-synthetic is closer to real than random-synthetic, and still not it.

**Everything geometric is at chance on real misfilings** (0.508–0.519) —
Stage 9's 0.410 reproduced on a larger, better-controlled sample. Confirmed
three ways now.

**The scorer that wins on real data won on neither synthetic task.** ANCHORVOC
was fourth under both injections. Had the track only ever run injections it
would have been discarded.

**One confound blocks banking it.** ANCHORVOC measures lexical overlap with the
cell's authored anchor hints — and the blind rater judged entries against those
same anchors. It may be detecting "shares vocabulary with the text the rater
read" rather than "is misfiled". Two cheap resolutions are specified in the
report: the 76 flagged non-blind entries as a control, and a held-out-half-of-
hints probe.

**Net after E14–E16:** the track's central negative stands — no tested mechanism
detects a real misfiling — but the reason it stood was wrong for years, and one
candidate has surfaced that was never fairly tested.

### E17 — ANCHORVOC's confound: three tests, two pass, one fails

`research/mapper/E17_ANCHORVOC_CONFOUND.md`.

| test | result | |
|---|---|---|
| **1.** does it predict the rater's *choice* of new cell? | **PASS** | rater's cell at median rank **17 of 70** (random 35); if the rater ran ANCHORVOC it would be 1–3. But 33% top-5 vs 7.9% random — shared information, not a shared procedure |
| **2.** survives half the anchor's hints? | **PASS** | 0.664 → **0.598**, still +0.079 [+0.007, +0.150] over cosine |
| **3.** behaves the same where blindness failed? | **FAIL** | ANCHORVOC **rises** 0.664 → **0.871** on the 56 hint-leaking entries while cosine falls. Fails the criterion the script pre-registered |

**Net:** not circular in the strong sense, but hint-dependent in a way a
filing-independent signal would not be. The account that fits all three: *an
entry sharing no vocabulary with its cell's anchor really is more likely
misfiled — and that is also part of what the rater used.* Real signal, not
independent evidence. **A lead, not a validated certifier.**

**The test that settles it:** one adjudication pass by a rater who sees only the
axis definitions, never the anchor hints. The Disposer Bench already has the
shape; it needs its display changed to withhold hints.

*(This script's first verdict read only Test 1, found it passed, and declared the
confound resolved — ignoring that Test 3 failed a criterion the same script had
pre-registered. A confound check that stops at the first reassuring answer is
not a check.)*

### E18 — ANCHORVOC closed, and the port checked

`research/mapper/E18_ANCHORVOC_CLOSED.md`. Three questions, one pass.

**Port validation.** CONSTRAINT r = +0.943 and ANCESTRY r = +0.976 against
experiment38's Rust values — faithful. **COSINE r = +0.197 — divergent, and
expectedly so:** experiment38 embeds with MiniLM, this corpus with BGE.
Structural scorers are embedder-independent; geometry is not. So E15/E16's
COSINE is the **stronger** BGE baseline (E1: +0.132 over MiniLM), meaning the
structural scorers were compared against a harder baseline than Stage 12 used.

**ANCHORVOC is an entry-length artifact.**

| arm | AUC | vs COSINE |
|---|---:|---:|
| COSINE | 0.519 | — |
| ANCHORVOC (E16) | 0.664 | +0.145 sig |
| **CONTROL — entry length only** | **0.619** | **+0.100 sig** |
| CONTROL — random cell's anchor | 0.565 | +0.046 ns |
| **normalised ANCHORVOC vs length-only** | — | **+0.048 [−0.019, +0.116] ns** |

Entry length alone reaches 0.619 of the 0.664, and **no vocabulary effect
survives controlling for it.**

**It also fails to replicate** on `cell_adjudication.tsv` — the audit of a
filing the rater did *not* produce — where cosine beats it 0.691 vs 0.544 (NO
only) and 0.739 vs 0.662 (NO + MARGINAL).

**Verdict: ANCHORVOC is not a certification signal.**

### Where E14–E18 leave it

*No tested mechanism detects a real misfiling.* The central negative stands and
now stands for the right reason: before E14 it rested on a comparison decided by
an injection policy that inverted the ranking; it now rests on three scorer
families measured against two real ground truths with the length and vocabulary
confounds separated. **The human-in-the-loop conclusion is the finding, not a
fallback from a failed search.**

*(The arc is kept in the reports because the process is the point: E15 reopened
the question, E16 narrowed it to one scorer, E17 found its confound tests
disagreed, E18 closed it. Four experiments each correcting the last, ending at
the position the track held before E14 — for entirely different and much better
reasons.)*

### The one constraint everything converges on

**The corpus is too small, and that is now measured rather than argued.**

- E5: 22 of 57 cells hold fewer than 5 entries; macro-F1 0.318 against accuracy 0.447.
- E7: a well-populated mode cut to 3 training entries scores **F1 0.06** — so the
  F1 = 0.00 modes are empty, not ill-defined. The curve is a forecast:
  **n ≈ 20 → F1 0.26–0.49**, and most of the gain is bought by the first 20.
- E8: the pre-flight STOPs the *current* grid on sparsity — 35 of 57 classes
  below 6 training entries. Much of this track has been measuring sparsity
  rather than its stated hypothesis.
- E10: the mechanism under test fired in under 3% of generations.

**Order of operations: populate first, expand second.** See
`research/mapper/EXPANSION_SPEC.md` for the deficit table, forecast and
acceptance test, and `research/mapper/E11_CANDIDATE_MINER.md` for the worklist.

**The worklist exists (E11).** WordNet noun synsets, filtered by lexicographer
file to the categories a physis entry actually *is* (act, cognition, state,
process, relation…) and dropping concrete taxa: 13,958 candidates. **Validated
before its output was read** — hold out 30% of existing entries, hide them among
the synsets, build each cell's target from train members and anchor only, and
rank the pool of 14,155:

| arm | r@25 | **r@100** |
|---|---:|---:|
| target from members + anchor | **0.325** | **0.497** |
| CONTROL shuffled cell targets | 0.005 | 0.086 |
| CONTROL random ranking | 0.000 | 0.025 |
| *chance* | | *0.0071* |

Half the held-out entries surface in the top 100 — 70× chance, 5.8× the shuffled
control. Output is usable (`ideation`, `groupthink`, `divergent thinking` for
BRAINSTORM; `guidance`, `leadership`, `fostering` for GUIDE).

**Known weakness:** the CONSTRUCT domain pulls concrete artifacts (`crane`,
`hoist` for CONSTRUCT/LIFT; `transporter`, `freight elevator` for
CONSTRUCT/MOVE) — machines that lift and move, not the mode. **When mining
WordNet for CONSTRUCT cells, filter concrete artifacts harder.** (E12 tested
whether this meant the anchor itself was broken. It does not — see below.)

### E12 — the anchor audit, which refuted the fix it was designed to prescribe

`research/mapper/E12_ANCHOR_AUDIT.md`. E11's CONSTRUCT observation looked like
Stage 11's problem, so the plan was Stage 11's fix: regenerate anchors from
`docs/GRID_AXES.md`. **Both halves were wrong.**

**Anchors work, unevenly.** Mean precision@n **0.306** over 45 measurable cells
against chance under 0.11. Worst: STUDY/PLAY, STUDY/PLAN, HEAL/MOVE,
STUDY/CREATE, FABRICATE/MAINTAIN all at **0.00**, and **STUDY/GUIDE puts its
typical entry at rank 327 of 658** — worse than corpus median.

**CONSTRUCT is not the weak domain — HEAL is.** HEAL 0.257 < STUDY 0.279 <
CONSTRUCT 0.284 < FABRICATE 0.342 < BOND 0.371. E11's observation was about what
the anchor retrieves *from WordNet*, not about anchor quality; correctly scoped
it is a filter problem in the candidate pool.

**The Stage 11 rewrite would have caused harm:**

| probe | mean prec@n | mean median rank |
|---|---:|---:|
| authored anchor (name + hints) | **0.306** | **64** |
| written axis definitions | 0.115 | 152 |

Definitions win in 1 cell of 45 and lose in 31. **A definition is for the
author; an anchor is for the index** — Stage 11 measured definitions helping
*people file consistently* (misfiling 31.8% → 16.2%), which is a different job
from helping a machine retrieve, where abstraction costs −0.19 precision.
Conflating the two is what made the rewrite look obvious.

### E13 — and the rewrite queue does not survive either

`research/mapper/E13_ANCHOR_CEILING.md`. E12 ended by prescribing a hand-rewrite.
E13 measured what a rewritten anchor could achieve, on held-out entries only:

| arm | prec@n | median rank |
|---|---:|---:|
| authored anchor (ships today) | **0.350** | **25** |
| written axis definitions | 0.122 | 58 |
| **centroid of the cell's own entries** | **0.353** | 27 |
| top TF-IDF terms of those entries | 0.275 | 35 |
| authored anchor + those terms | 0.388 | 22 |

**Headroom over a probe built from the cell's own entries is +0.004.** The
centroid *is* "written in the entries' idiom", and the authored anchors already
match it — so hand-rewriting is an exhausted move. **E12's queue should not be
worked**, which cancels the recommendation made one experiment earlier.

**The augmentation arm looked like the replacement finding and is not.** +0.038
mean, but it **beats the anchor in 6 of 21 cells and loses in 10** — a mean
carried by a few large gains over many small losses, nowhere near significant on
a sign test. Shipped, it would read as an aggregate improvement while degrading
half the cells it touched. Same shape as six earlier positives in this track.

**Jointly established by E12+E13:** anchors are approximately as good as this
representation allows. Three consecutive attempts to improve them — from
definitions, by hand, by augmentation — are all refuted, two against the very
evidence that suggested them. **Anchor work is finished** until something other
than retrieval precision motivates it.

**Where the error actually lives:** class sparsity (E5/E7/E8 — 35 of 57 cells
below 6 training entries) and ~16% label noise (Stage 11). Both corpus problems.
Neither is fixed by writing.

### Standing rules, each earned

1. **Treat any positive as artifact until a construction-matched control says
   otherwise.** Seven positives have died to one, most recently E7 refuting E6's
   "cluster" against random four-mode subsets.
2. **Report kappa, never accuracy against a majority floor**, on any problem with
   many unbalanced classes. Merging cells raises accuracy 0.431 → 0.645 while
   kappa stays flat.
3. **Run the pre-flight** (`research/mapper/preflight.py`) before an experiment,
   and honour it. It refutes algebraically-redundant arms and underpowered
   designs. It has been overridden once, by its own author, in E10 — recorded
   there because a control the author ignores is not a control.
4. **Coverage filters and ranks discovery candidates; it does not verify that a
   candidate is correct.** Sell the filter, not the judgement.


---

# Detail: E19–E29 (added 2026-09-09)

Two tracks ran on the same day. **E19–E20** in `research/convergence/` tested an
external brief about cross-domain abstraction. **E21–E29** in `research/mapper/`
turned the whole certification question around and got the first positive this
project has recorded.

## E19 — cross-domain structural convergence: **NOT SUPPORTED**

`research/convergence/E19_CROSS_DOMAIN_CONVERGENCE.md`. Does presenting a model
five structurally *congruent* domains improve its ability to abstract the shared
relational pattern? Generate a schema from a stimulus, then apply it — blind to
the arm — to 24 pre-labelled probes in 12 matched pairs.

Replicated on **four models**, which closes the experiment's own first-listed limit:

| model | NULL | ORACLE | band | C5 (five congruent) |
|---|---:|---:|---:|---:|
| qwen3:4b | +0.361 | +0.667 | 0.306 | +0.467 |
| qwen2.5:7b | +0.250 | +0.500 | 0.250 | +0.450 |
| qwen3:8b | +0.194 | +0.556 | 0.361 | +0.567 |
| ornith-1.5-9b | +0.194 | +0.750 | 0.556 | +0.600 |

**The ceiling does not climb with scale and C5 never separates from one domain**
(4B: C5 − A1 = +0.067; 7B: +0.017). What five items buy is *permissiveness* —
positive acceptance 0.77 → 0.97, negative 0.37 → 0.50 — the brief's own §17
trivial universal.

**The load-bearing finding needs no between-arm comparison.** C4, differentiation
— that interaction can *produce* difference rather than erase it — is the worst
clause for every arm. Four of five C5 schemas never state it, and ORACLE, handed
the exclusion in writing, accepts **every** convergence decoy while rejecting all
four identity-destruction decoys. The model detects interaction that *destroys*
difference and fails on interaction that *reduces* it.

### E19b — three controls, and a transferable finding about controls

`research/convergence/E19b_CONTROL_REBUILD.md`.

**S5 was never a control.** Described as "C5's tokens with congruence destroyed",
it is C5's five bodies **byte-identical, cyclically shifted against five identical
headings** (tf-cosine 1.000). Congruence among them is intact, and the generation
prompt tells the model to discard exactly the part that moved. E19's "decisive
test" is withdrawn; the verdict rests on A1 and A5, which are valid.

Two rebuilds, both recorded:

| arm | design | outcome |
|---|---|---|
| B5 | five domains chosen to share nothing | confounded — all five were asymmetric one-way relations |
| B5b | five surface-matched, heterogeneous | **falsifier fired at 4B, 7B and 9B** — converged on ¬C1 every time |
| B5c | five coupled, each failing a different clause | fires at 4B (¬C2), **valid at 9B** |

At 9B, where the control is admissible, C5 +0.600 vs B5c +0.233 = +0.367 against
a band of 0.556 — inside it, so the brief is not supported where a valid control
exists.

**Transferable: a control arm's validity is a property of the stimulus *and the
model* together, and must be checked per model.** B5b converges at 9B while B5c
scatters — same model, same prompt, same seeds.

**Also recorded:** the negation space of a target structure is dominated by
absence of coupling, so any set assembled to be "structurally unrelated" drifts
into sharing ¬C1 unless coupling is held fixed deliberately.

## E20 — the n-gram sieve's floors, measured before the table exists

`research/convergence/E20_FLOORS.md`. **The 51GB Qwen3.8-Flash-Next n-gram table
is not on the fleet** and E20 has not run. Its three declared controls need no
table, so they were measured first.

| scorer | forced choice | AUC |
|---|---:|---:|
| length (words / chars) | 0.250 / 0.417 | 0.378 / 0.420 |
| frequency (mean / min / rare-n) | 0.583 / 0.333 / 0.250 | 0.535 / 0.521 / 0.427 |
| **embedding LOPO** | **0.917** | **0.757** |

Length and frequency are at chance — **the ANCHORVOC confound is absent from this
probe set.** But an off-the-shelf embedding separates **C3 4/4 and C1 3/3**, so
E20's "expected honest positive" (separating C3) is *already the floor*, and its
pre-registered negative (C1 is causal, not lexical) is **false**. The only
headroom is C4 at 0.67 — on three pairs. **The blocker on E20 is not the download;
it is that the probe set needs more C4/C5 pairs.**

## E21 — geometry as a proposer: the first positive that survives its null

`research/mapper/E21_PROPOSER.md`. **Every refutation in this project refuted
unsupervised, zero-shot judgement.** COSINE, NAME-ONLY, CONSTRAINT, ANCESTRY and
ANCHORVOC are fixed formulas with nothing fitted to a human decision. The
supervised, human-in-the-loop version had never been measured — and the question
a person filing 582 entries has is not *"is the machine right"* but *"can it
narrow 57 cells to 3 so I choose instead of search."*

Ground truth: the rater's **revised** cell in `refiling_gate0.tsv`, 582 blind
entries, 57 targets.

| at 436 decisions seen | top-1 | top-3 | top-5 |
|---|---:|---:|---:|
| **geometry** | **0.438** | **0.667** | **0.755** |
| prior `old`→`new` lookup | 0.184 | 0.368 | 0.474 |
| majority | 0.089 | 0.255 | 0.359 |
| **permuted (the null)** | 0.036 | 0.136 | 0.233 |
| length only | 0.010 | 0.036 | 0.071 |

**Top-1 0.438 essentially reproduces E5's 0.447. The geometry did not get
better** — a number filed as a failure is the same number that makes a usable
proposer once the output is three candidates instead of one. Learning curve still
climbing at 436 (top-3: 0.385 → 0.508 → 0.595 → 0.667).

**Open, and it decides whether this is worth building on: a second rater.** The
Gate 0 rater was Claude, so this may have learned a policy or the corpus.

## E22–E29 — the mode axis, closed from five directions

| # | question | result |
|---|---|---|
| **E22** | what do the confusions say the axes should be? | domain: FABRICATE ↔ CONSTRUCT confuse **0.22/0.21 symmetric**, one distinction wearing two names. Mode is **three problems**: a four-way blur (WORK/CREATE/MAINTAIN/SENSE, 68% of entries, ample population), a definition failure (WALK, n = 27, top-1 **0.18**), and five genuinely unmeasured classes (BRAINSTORM n = 1) |
| **E24** | how much should hints weigh? | **α = 0.5 beats both α = 0 and the shipped α = 1, on both axes.** Top-3 0.712 vs 0.658. Refutes a per-axis split: one unimodal curve, same peak |
| **E25** | is a compound `M + H` or `T_M · H`? | **geometry is additive** — offset coherence +0.697 vs a +0.036 null, no head asymmetry. **But roles are asymmetric**: mode sits almost entirely in the head (+0.251 over floor vs the modifier's +0.054), and the true compound beats its own additive reconstruction |
| **E26** | are domain and mode perpendicular? | **No** — 46.1° mean, and orthogonalising *hurts both*: the overlap is shared signal. **Domain compresses to 4-d losslessly (−0.003); mode is not a subspace (−0.034)** |
| **E27** | can merging make mode coherent? | **No.** At matched granularity mode-merged-to-5 is −0.187 against domain's −0.007. Merging makes it *worse* |
| **E28** | does the split proposer ship? | **No, −0.119.** Factorising costs −0.028 (the axes are not independent) and head-only a further −0.091 |
| **E29** | is there a better carve to discover? | **No.** Clustering the blur recovers *subject matter*; the current four beat every discovered partition (−0.014 vs −0.049/−0.082/−0.073) |

### Two corrections made in-flight, recorded rather than folded in

**E22's axis asymmetry was a two-point artifact.** It measured hints helping
domain (+0.022) and hurting mode (−0.034) and read an axis asymmetry into it.
E24's full sweep shows one unimodal curve peaking at 0.5 for *both* — hints are
simply over-weighted, more so for mode. The measurements stand; the prescription
of two representations does not.

**E27's prescription contradicted E25's own limits.** E27 recommended building
the mode axis from the head, because head-only halves mode's *incoherence*. E28
measured the cost in *prediction*: **−0.091 top-3**. E25's limits section had
already said *"not a prescription to drop the modifier."*

> **Compression asks whether an axis is a coherent object. Held-out top-k asks
> whether a representation predicts. A diagnostic criterion is not an objective
> function** — and this track has now conflated them once.

### Where the mode axis stands

**Mode is not underdetermined by its labels. It is absent from the
representation.** Every computational route is closed, each for a different
reason: coarser granularity (E27), better representation (E28), factorised
decoding (E28), a stronger embedder (E21, +0.065 and no closure), unsupervised
discovery (E29). An entry here is a name plus subject-matter hints — what
something is *about*. Mode asks what is being *done*, and nothing in the corpus
ever observed anything being done.

### E23 — where supervision may come from

`research/mapper/E23_SUPERVISION_SOURCES.md`. Testing whether machine-generated
supervision could replace the human decisions E21 depends on. On 528
conventional-commit subjects — the label *is* an activity, the text *is* about it,
prefixes stripped:

| scorer | top-1 |
|---|---:|
| **TF-IDF (lexical)** | **0.628** |
| embedding | 0.575 |
| majority | 0.451 |
| permuted | 0.222 |

The reading was fixed before the run: *if TF-IDF matches the embedder, the signal
is lexical convention, not activity semantics.* **It beats it.**

**So machine supervision is real where a convention or an exact log carries the
label, and is not a route to inferring causes nobody wrote down.** Hence the
standing rule: **causes are user-assessed unless an exact error log is provided.**
An exact log needs no semantics — it *is* a structured label.

## What ships from all of this

**One line: halve the hints.** `v = normalise(v_name + 0.5·(v_hints − v_name))`,
used by a single joint proposer over cells. Top-3 **0.705** against the shipped
representation's 0.651, *t* = +12.13, better in **24 of 24** paired splits
(E28 variant B). Every more elaborate design built on top of it lost.

## Proposed: quantitative event streams (2026-09-17)

[Detailed research specification](QUANTITATIVE_EVENT_STREAMS.md): Q1–Q8, data
contracts, baselines, walk-forward validation, nulls, costs and kill criteria.
Status: NOT MEASURED. Linked to PLAN.md Phase 27 and existing MS packets;
no new engine, implementation, worker dispatch or production promotion.
