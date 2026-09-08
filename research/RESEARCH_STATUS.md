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
| Can a cheap signal certify discovered structure? | **NO — refuted, not untested** | CONSTRAINT loses to plain cosine 2 wins in 22, paired *t* = −5.58, on a corpus that is no longer 43% noise; non-lattice below chance (0.459) |
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

