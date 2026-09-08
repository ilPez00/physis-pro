# Thesis Evaluation Report — Post-Experiment JEPA × Qwen × Physis

Evaluated against `physis-core` **0.1.21** and `physis-pro` at `5dea6ac`, using
the repository as ground truth rather than the thesis's description of it.
Every experimental figure below is recovered from `FINAL_REPORT.md`,
`RESEARCH_LOG.md` or a raw artifact in this directory; anything not found is
labelled **RESULT NOT RECOVERED**.

---

## 1. Executive Verdict

**REVISE & RESUBMIT.**

The thesis correctly identifies its own starting point. Cross-embedder
*agreement* has been tested three ways in this repository and failed three
times — clustering agreement at ARI ≈ 0.10 (Iteration 18), corroboration as a
noise filter at zero aggregate improvement with recall falling 5/5 → 4/5
(Iteration 19), and split agreement (Iteration 24). Proposing to use
*disagreement* instead is a genuine inversion of a settled negative, not a
restatement of it, and no experiment in the record tests whether `Δcos`
predicts any downstream outcome. That claim is **NOT YET TESTED** and is worth
testing.

Three things block acceptance as written.

**The multimodal branch is misdescribed.** Physis-pro already has a
contrastively aligned text–image space (`src/embed/clip.rs`, `jina.rs`). The
thesis proposes JEPA as the grounding modality, but the only JEPA in this
repository is a text-only rank-K predictor in `examples/`, falsified at scale
(Iteration 4), and a JEPA latent is *not* aligned to text by construction. The
thesis proposes an unaligned instrument where an aligned one already ships.

**The Qwen component has no implementation and no defined quantity.** `Q_n` is
undefined in the source. The only n-grams present are SHA-256 character hashes
in the *fallback* embedder (`embed.rs:34`), and the only Qwen is a label on a
pre-computed vocabulary file that scored 0.575 AUC.

**The metric is not a certainty.** `S = [1−(cos₁−cos₂)]·Qₙ` ranges over
[−1, 3], is directionally asymmetric with no reference/challenger ordering
defined, and has no calibration evidence.

The architecture is defensible; the claims exceed the evidence.

---

## 2. Existing Experimental Evidence

| # | Hypothesis | Method | Baseline | Result | Statistical evidence | Conclusion | Relevance to this thesis |
|---|---|---|---|---|---|---|---|
| It. 1 | Δ(x,r) magnitude reveals reference-conditioned structure | analytic | cosine | identical to cosine ranking for unit vectors | proof, not statistics | **FALSIFIED** | Δ-magnitude alone is not a new signal |
| It. 2 | Δ-direction clustering beats similarity search | clustering, n=21 | naive cosine | loses on mean F1 at both granularities | n=21 | **FALSIFIED** | — |
| It. 3–4 | Trained predictive latent (JEPA-style) beats cosine | rank-K `pred(r)=V(Uᵀr)` | raw cosine | toy win was a supervision confound; chance-level at scale on 730 entries | shuffled-label control | **FALSIFIED at scale** | **directly relevant — the JEPA leg** |
| It. 17 | JEPA rebound to discovered centroids (stop-gradient) | self-supervised | oracle centroids | **INCONCLUSIVE** — held-out sets of 2–5 items | sample-size limit, not a finding | **UNTESTED at scale** | the one JEPA claim still open |
| It. 17b | Supervised metric learning at real scale | converged (loss 1.609→0.558) | 0.50–0.56 raw cosine | 0.534 held-out, 0.277 generalization gap | — | **FALSIFIED** | architecture has a real ceiling |
| It. 18 | Structure survives an embedder swap | MiniLM vs BGE | — | **ARI ≈ 0.10** | near-random | **NEGATIVE — foundational** | the thesis's premise |
| It. 19 | Cross-embedder corroboration filters noise | two embedders | single embedder | **zero aggregate FPR improvement**; recall 5/5 → 4/5 | — | **NEGATIVE** | agreement fails; disagreement untested |
| It. 19b | (qualitative) | spot-check | — | for broad entries the true domain is often the *least* corroborated | qualitative | anti-correlation | echoes Stage 9 |
| It. 24 | Cross-embedder split agreement certifies splits | — | — | fails | — | **NEGATIVE** | third agreement failure |
| It. 20/25 | Density peaks; capacity-constrained loss | — | — | unstable / fails | — | **NEGATIVE** | — |
| It. 26 | ONNX embedding is nondeterministic | probe | — | **disproved**; cause was `HashMap` iteration order | verified by sabotage | **fix shipped** | determinism is not a confound |
| It. 30 | Near-neighbour midpoints propose real gaps | hold-out rediscovery | manifold-matched control | 1.44×, **z = +4.87**, ~13% precision | — | **SUPPORTED** | proposer works |
| It. 33 | The disposer can be a machine (filter) | 5-fold CV, 2000 candidates | existing-entry verbatim | duplicate rescues **0/2000**; **Welch t = +12.11** | — | **SUPPORTED, shipped** | `coverage` |
| It. 33b | The disposer prefers It. 30's proposer | construction-matched | medium-rank midpoints | 0.073 vs 0.070, **t = +0.29** | — | **NOT SUPPORTED** | 4th positive killed by a control |
| It. 35 | Independent-source substructure corroborates | WordNet glosses | hint-derived | **z = +6.64** vs +0.62 | strengthened under tighter control | **SUPPORTED** | **an independent instrument did work once** |
| Stage 7 | The corpus itself is sound | 60-row deterministic audit | — | **43.3% clearly misfiled** | deterministic sample | **NEGATIVE — invalidated the evaluation** | — |
| Stage 9 | Physis can triage its own misfilings | AUC vs 60 adjudications | chance | **0.410**, CI [0.251, 0.569] | chance inside CI | **NEGATIVE** | machine cannot judge belonging |
| Stage 10 | The disposer ranks, not just filters | 1638 proposals × 6234 records | 4 controls | **precision@25 = 0.480** vs 0.132 base | exact *p* = 2.3e-5 | **SUPPORTED, shipped 0.1.20** | ranking works |
| Stage 11 | Gate 0 — regenerate the corpus | blind re-filing of 661 entries | old filing | misfiling proxy **31.8% → 16.2%** | rater-independent | **SUPPORTED, shipped 0.1.21** | the clean corpus this thesis inherits |
| Stage 12 | The mechanisms were only underpowered | 22-config sweep, clean corpus | plain cosine | CONSTRAINT **2 wins / 22**, mean Δ −0.063 | **paired t = −5.58** | **REFUTED** | certification is closed |
| Stage 13 | Crowded cells contain splittable substructure | permutation null + stability | mixture scale | separation collapses 20–150× on names alone | 100%/0% prose-hint split | **REFUTED — format artifact** | **a single embedder reads house style as meaning** |

Noise floor for this harness: **0.074 AUC**. Single-run margins below it are not
interpretable.

**RESULT NOT RECOVERED** for: any calibration study (reliability, Brier, ECE) of
any Physis score; any evaluation of the `delta_engine` shadow-frame propagation
against outcomes (it has 5 unit tests, no experiment); any RAG ablation of the
MMR objective; any Discovery Studio human-audit study; any multimodal
text↔image verification benchmark.

---

## 3. What Previous Experiments Already Established

### Established
- Embedding geometry does **not** yield ontology structure stable across
  embedders (ARI ≈ 0.10) or across corpus growth (ARI 0.198 on +25% data).
- Cross-embedder **agreement** does not filter noise (It. 19, 24).
- An operational completeness check works and is shippable (`coverage`,
  *t* = +12.11), and it **ranks** as well as filters (precision@25 = 0.480).
- Structural soundness checking is **refuted**: plain cosine beats every
  structural alternative tried, on clean and dirty corpora alike.
- The machine **cannot** judge whether an entry belongs where it sits (0.410).
- A single embedder can be fooled by authorial formatting (Stage 13).
- Determinism is fixed and is not a confound.

### Strongly suggested
- Independent-source corroboration can work when the source is genuinely
  independent — the WordNet gloss result (z = +6.64) is the only such positive
  and it *strengthened* under tighter control.
- Ambient label noise attenuates detection roughly uniformly: fixing the corpus
  lifted CONSTRAINT and cosine by ~0.085 AUC each, together.

### Negative findings
- Twelve mechanisms across nine geometric families, one trained predictor, and
  three cross-embedder agreement designs.

### Still unknown
- Whether cross-model **disagreement** predicts any downstream outcome.
- Whether a **multimodal** grounding signal reduces false text↔media matches.
- Whether self-supervised JEPA binding works at adequate scale (It. 17's
  inconclusive result is a sample-size limit, not a finding).
- Whether subword/morphological structure adds anything to a contextual
  embedding.
- Calibration of anything in the system.

---

## 4. Mathematical Evaluation

Proposed: `S = [1 − (cos₁ − cos₂)] · Qₙ`.

With `cos₁, cos₂ ∈ [−1, 1]`, the bracket ranges over **[−1, 3]**:

| case | cos₁ | cos₂ | bracket |
|---|---|---|---|
| maximum disagreement, direction A | 1 | −1 | **−1** |
| equal support | c | c | **1** |
| maximum disagreement, direction B | −1 | 1 | **3** |

Three defects, in order of severity.

**1. It is not a certainty and must not be named one.** It is unbounded below
by 0, exceeds 1, and no calibration evidence exists anywhere in this repository
(see RESULT NOT RECOVERED, §2). Per the brief's own rule, rename. **Model
Divergence** or **Representational Tension** are accurate; `certainty` is not.

**2. The asymmetry is unjustified as specified — but do not symmetrize
reflexively.** The asymmetry is legitimate *only* if a reference/challenger
ordering exists. In physis-pro one plausibly does: BGE-base is the production
embedder and any second model is the challenger. If that ordering is adopted it
must be stated and the models must not be interchangeable in the code. If it is
not adopted, use `D = |cos₁ − cos₂|`. **The repository defines no such
ordering today**, so as written the sign of `S` depends on an arbitrary
argument order — a latent bug, not a design.

**3. Multiplying by `Qₙ` conflates three quantities.** Per §7 of the brief,
disagreement `D`, agreement `A = 1 − |cos₁−cos₂|/2`, and absolute support must
stay separate. A product collapses "both models strongly support, and disagree
slightly" with "both models reject, and disagree slightly". Physis needs these
distinguished: the first is a confident candidate, the second is noise. Keep
them as separate fields on the evidence record.

**Recommended form:** emit `(support₁, support₂, D)` as three numbers and let
the hypothesis layer combine them. Any scalar collapse should be *learned
against an outcome*, not asserted.

---

## 5. Model Divergence — what `Δcos` actually is

On the evidence: **an evidence differential, provisionally; representational
tension at best; certainly not certainty.**

The record contains one directly relevant observation, from Iteration 19's
spot-check: for broad entries, *the entry's own true domain is often the least
corroborated label*. That is an **anti-correlation** between cross-model
agreement and correctness, and it rhymes with Stage 9, where the fit score was
anti-correlated with real misfiling (mean fit 0.196 correct vs 0.213 misfiled).

Two readings follow, and the thesis must choose one and test it:

- **Weak reading (supported by It. 19):** disagreement marks entries that are
  *broad or polysemous*, not entries that are *wrong*. Useful for routing to
  human review; useless as a truth signal.
- **Strong reading (untested):** disagreement predicts downstream failure —
  contradiction, prediction miss, coherence degradation.

The strong reading is the thesis. Nothing in the record supports or refutes it.
It is the single most valuable experiment available (see §10, E1).

---

## 6. Qwen Contribution

**Status: NOT IMPLEMENTED, and the quantity is undefined.**

What is actually in the repository:

- `physis-core/src/embed.rs:34` — `hash_ngrams`, SHA-256 hashes of character
  1-grams and 2-grams. This feeds the **`RandomProjectionEmbedder` fallback**,
  the degraded path that the classifier explicitly reports as `degraded` and
  abstains on. It is not a Qwen artifact and carries no learned semantics.
- `experiment38_soundness_audit.rs` — `VOCAB-GEN (qwen3:4b)`, a per-cell
  vocabulary generated offline and consumed as JSON. Measured **AUC 0.575**
  against COSINE's 0.739 on the regenerated corpus, and its companion
  VOCAB-FREQ at 0.590. **Both figures are currently uninterpretable**: the
  vocabulary was authored against the pre-0.1.21 cell composition and 77.6% of
  entries have since moved.
- `physis-pro/src/ai/provider.rs:725` — a `Qwen/Qwen2.5-72B-Instruct` model id
  in a provider table. A generation endpoint, not a representation source.

There is no `Qₙ`. The thesis must define which of frequency, count, normalized
frequency, transition probability, hashed sequence, or learned representation
it means, because these have different failure modes and only some are
falsifiable.

**Prior evidence bearing on it.** The n-gram discretization result in
`FINAL_REPORT.md` is marked *"promising, not yet validated"*: exact 3-gram
recurrence 11–83% by term, and — importantly — **a naive variation allowance
makes it worse** (skip-grams below exact adjacency on every term). So the one
n-gram finding in the record is that loosening the n-gram hurts.

**The strongest available argument for a subword layer is not semantic.** It is
Stage 13: the embedder was fooled by hint formatting. A form-level instrument
that is *explicitly about form* would let the system model that channel instead
of being deceived by it. That is a real motivation, and it is not the
motivation the thesis gives.

---

## 7. JEPA Contribution

**Status: the strongest part of the thesis and the most misdescribed.**

**Alignment.** §13 of the brief is right that `cos(e_text, e_JEPA)` is invalid
without demonstrated alignment, and this is decisive here: JEPA latents are
trained by predicting *masked latent targets within a modality*. There is no
mechanism making them commensurable with a text embedding. Equal dimensionality
is not alignment.

**And physis-pro already has the aligned alternative.** `src/embed/clip.rs` and
`src/embed/jina.rs` provide contrastively trained text↔image spaces — alignment
is what CLIP's objective *is*. The README's claim that "text, images and video
land in the same embedding space" is architecturally true for these and would
be false for a JEPA latent.

So the honest framing is:

- **For grounding text against media today: use CLIP/Jina.** It is aligned,
  shipped, and needs no projection.
- **JEPA's distinctive contribution is predictive, not comparative** — it
  models what an observation *will look like*, which is a genuinely different
  evidence type and maps onto Physis's existing `Prediction` primitive
  (`hypothesis.rs:127`) rather than onto similarity scoring.

**Prior evidence.** Iterations 3/4 falsified a JEPA-style predictor *as a
similarity scorer* — beaten badly by cosine on 730 entries, with the toy win
traced to a supervision confound. Iteration 17's self-supervised rebinding is
**INCONCLUSIVE at n = 2–5 held out**, explicitly a sample-size limit. That
inconclusive result is the only live JEPA claim, and it is text-only.

**The multimodal branch is therefore NOT YET TESTED** — genuinely new. It is
also the only leg whose failure so far can be blamed on the corpus rather than
the instrument.

---

## 8. Physis Integration

**Existing primitives the thesis should reuse, not rebuild:**

| primitive | location | note |
|---|---|---|
| `Hypothesis`, `Evidence`, `EvidencePolarity`, `Prediction`, `Revision`, `FitnessBreakdown` | `hypothesis.rs` | the epistemic layer already exists |
| `Contradiction`, `ContradictionParty` (with `authority`, `confidence`) | `contradiction.rs` | contradictions are *claims*, not vectors |
| `CoherenceProfile` | `coherence_dimensions.rs` | see correction below |
| `OntologyMutation`, `EvaluationContext`, shadow frame, `compute_coherence`, `build_adjacency` | `delta_engine.rs` | shadow evaluation exists; **untested against outcomes** |
| MMR retrieval under token budget | `rag.rs:133` | the brief is right that the baseline is not greedy-cosine |
| `coverage::{uncovered, candidate_gain, rank_candidates, CandidateGain::lift}` | `coverage.rs` | filter **and** ranker, both measured |
| CLIP / Jina multimodal | `physis-pro/src/embed/` | the aligned space |
| Grounding gate (token-match against verbatim source) | `physis-pro/src/grounding.rs` | anti-hallucination precedent for admitting LLM output as evidence |

**Correction to the brief.** §16 lists the coherence dimensions as *Semantic,
Ontological, Logical, Empirical, Process* and instructs that none be invented.
The implementation (`coherence_dimensions.rs:52`) has **six**: `semantic`,
`logical`, `temporal`, `causal`, `procedural`, `empirical`. There is **no
ontological dimension**. Any thesis text mapping divergence onto "Ontological
coherence" is mapping onto something that does not exist; `semantic` is the
nearest real target.

**New adapter required (small):** a second `VectorEmbed` instance plus an
`Evidence` producer emitting `(support₁, support₂, D)`.

**New Physis primitive required: none.** Divergence enters as `Evidence`
attached to a `Hypothesis`. This is the brief's §8 and it is correct — the
existing architecture already separates similarity from support, and the
thesis should not bypass it.

**Claims unsupported by the current implementation:** `Qₙ` (no definition, no
code); JEPA multimodal grounding (no JEPA outside `examples/`, and the
multimodal space that exists is CLIP/Jina); "dictionary-independent semantics"
(the defensible claim is *no static dictionary at inference time* — and note
WordNet **is** a dictionary and is used in the audit harness).

---

## 9. Semiotic Evaluation

The proposed mapping —

```
signifier = Qwen n-gram structure
signified = cross-model divergence
referent  = JEPA representation
```

— is **not defensible as stated**, for one structural reason: a *referent* is
mind-independent, and a JEPA latent is a model-dependent sign. Calling it the
referent smuggles in exactly the equivalence the rest of the thesis is careful
to avoid (representation ≠ truth). It also inherits Saussure's dyad while
adding a third term, which is a category error: Saussure's sign has two parts
by construction.

**Peirce is the right frame and the repository already uses it.**
`semiotic_ontology.json` carries the full trichotomy vocabulary — Qualisign,
Sinsign, Legisign, Icon, Index, Symbol, Rheme, Dicisign, Argument, and
Firstness/Secondness/Thirdness. In Peircean terms the architecture maps as:

| Peirce | this system | why |
|---|---|---|
| **representamen** (sign vehicle) | subword/token form | the observable form |
| **object** | the source observation | what the sign is about |
| **interpretant** | the Physis hypothesis | the interpretation the sign produces, which is itself a further sign |

This is a better fit and it makes the epistemics honest: the interpretant is
explicitly *another sign*, never truth, which is precisely Physis's own
architecture. Divergence is then **not** a sign component at all — it is a
property of two interpretants of the same object, which is what makes it
interesting.

The brief's §15 alternative (FORM / INTERPRETATION / GROUNDING OBSERVATION) is
the same structure in plainer words and is preferable to either the Saussurean
version or unexplained Peircean jargon.

**One caution the record supports.** Stage 13 found the semiotic entries
separating from the corpus in embedding space — and it was prose formatting,
not meaning. Semiotic vocabulary in this project has already produced one
convincing illusion. Terminology should follow the computational architecture,
per the brief's own instruction.

---

## 10. Incremental Experimental Program

Five delta experiments, each the smallest decisive test of one unresolved
claim, ordered by information per unit of work. Each must beat the existing
pipeline, not a strawman.

### E1 — Does divergence predict anything? *(do this first; the thesis dies or lives here)*
**Claim tested:** `D = |cos₁ − cos₂|` predicts a downstream outcome.
**Design:** reuse the Stage 12 harness unchanged. Add BGE-base as embedder 1
and a genuinely different model as embedder 2. Ground truth already exists:
81 injected misfilings per configuration, 22 configurations.
**Baseline:** COSINE, mean AUC **0.733** on the regenerated corpus.
**Bar:** the noise floor is 0.074 AUC; report paired *t* across 22
configurations, as Stage 12 did.
**Kill criterion:** if `D` does not beat 0.5 alone, or adds nothing over cosine
in a stratified analysis (the harness already computes "within cosine strata"),
the strong reading of §5 is refuted and the thesis reduces to the weak reading.
**Cost:** one afternoon. Everything needed already exists.

### E2 — Model diversity is a variable, not a constant *(the brief's §25)*
**Claim tested:** two models disagree for reasons other than shared priors.
**Design:** three pairings at increasing independence — two checkpoints of one
family; two different architectures on similar corpora; a model with a
different tokenizer and training corpus. Measure whether E1's effect scales
with independence.
**Why it matters:** if the effect does not grow with diversity, "independent
corroboration" is correlated agreement and the epistemic claim fails even if
E1 is positive.

### E3 — Is the form channel real? *(the honest motivation for the Qwen leg)*
**Claim tested:** an explicit form-level feature captures the variance that
fooled the embedder in Stage 13.
**Design:** the Stage 13 fixture is the ground truth, already built and already
published. Predict the prose/keyword label from `Qₙ` alone; then test whether
conditioning on `Qₙ` removes the spurious split.
**Bar:** must beat the trivial baseline (mean hint length), which is already
100% separating.
**Note:** if `Qₙ` cannot beat mean hint length, it is redundant frequency
information and §11 of the brief says remove it. This is the cheapest possible
falsification and it should be run before any Qwen integration work.

### E4 — Multimodal grounding, against the aligned baseline
**Claim tested:** JEPA reduces false text↔media matches.
**Design:** the comparison must be **JEPA vs CLIP/Jina**, not JEPA vs nothing.
CLIP is aligned by construction and already shipped; a new unaligned instrument
must earn its projection layer.
**Metric:** false-match rate at fixed recall on text–image pairs.
**Kill criterion:** if JEPA + learned projection does not beat CLIP zero-shot,
the multimodal branch is complexity without value.

### E5 — Divergence as an ontology-mutation prioritiser *(the highest-value integration)*
**Claim tested:** divergence improves decisions about ontology changes.
**Design:** `delta_engine`'s shadow frame already evaluates a mutation in
isolation and propagates coherence through `build_adjacency`. Rank candidate
mutations by divergence; compare against ranking by `CandidateGain::lift`,
which is the measured incumbent at **precision@25 = 0.480**.
**Why this one matters most:** it is the only proposal that connects the new
signal to a shipped, measured decision rather than to a metric. It also gives
the `delta_engine` its first outcome-based evaluation — currently
**RESULT NOT RECOVERED**.

**Not proposed, deliberately:** anything re-testing cross-embedder *agreement*
(three prior failures), any new geometric certification mechanism (Stage 12
closed this), and any experiment scored against `cell_vocab_AB.json` until that
vocabulary is re-authored against the 0.1.21 filing.

---

## 11. Final Thesis Revision

### Claims that should remain
- Cross-model divergence is worth investigating **because** agreement failed —
  the inversion is sound and the negative results are the thesis's foundation.
- Divergence must enter Physis as **evidence about a hypothesis**, never as a
  replacement for the hypothesis system. The architecture already enforces this.
- Representational divergence is **not** contradiction. The record supports the
  distinction: a contradiction is a relation between *claims*
  (`ContradictionParty` carries a claim string, not a vector).
- Prior negative results belong in the thesis. They are its strongest section.

### Claims that should be weakened
- "Certainty score" → **Model Divergence** or **Representational Tension**,
  until calibration exists. No calibration study exists for any Physis score.
- "Dictionary-independent semantics" → *no static dictionary at inference
  time*. WordNet is a dictionary and is used in the audit harness.
- "JEPA as referent" → JEPA as a **predictive representation**, feeding
  `Prediction`. It is a model-dependent sign, not a referent.
- The multimodal grounding claim → must be stated **relative to CLIP/Jina**,
  which already ship and are already aligned.

### Claims that should be removed
- `S = [1 − (cos₁ − cos₂)]·Qₙ` as a single scalar. It ranges [−1, 3], is
  asymmetric with no defined ordering, and conflates disagreement with support.
- The Saussurean triad. Saussure's sign is dyadic; the Peircean
  representamen/object/interpretant mapping in §9 is both defensible and
  already present in the corpus vocabulary.
- Any mapping onto an "Ontological coherence dimension" — it does not exist.
- Any claim that Qwen n-grams currently contribute anything. Nothing named
  `Qₙ` is implemented.

### Claims now empirically supported (by this repository, not by the thesis)
- Embedding geometry does not give stable structure — ARI ≈ 0.10 across
  embedders, 0.198 across corpus growth.
- Structural certification is refuted, not merely unsupported — CONSTRAINT
  2/22, paired *t* = −5.58, non-lattice below chance.
- The machine can filter and rank discovery candidates but cannot judge
  belonging — *t* = +12.11, precision@25 = 0.480, and AUC 0.410 respectively.
- **A single embedder mistakes authorial form for meaning** (Stage 13). This is
  the strongest argument in the repository for a multi-instrument architecture,
  and the thesis does not currently make it.

### Claims requiring new evidence
- That `Δcos` predicts contradiction, prediction failure or coherence
  degradation — **E1**.
- That two models are epistemically independent rather than correlated —
  **E2**.
- That `Qₙ` adds information beyond a contextual embedding — **E3**.
- That a JEPA latent grounds text better than an aligned CLIP space — **E4**.
- That divergence improves ontology-mutation decisions — **E5**.

---

## Closing note on method

The brief instructs that the thesis must not argue "we combine three systems,
therefore richer representation". The record in this directory is the reason
that instruction is right: **six positives in this track died to controls**, the
most recent because an embedder read two authors' hint-formatting conventions as
a semantic stratum. The thesis's own architecture — an embedder, a form-level
instrument, and a second modality — is a reasonable response to exactly that
failure. But the response has to be demonstrated one leg at a time against the
pipeline that already ships, and on current evidence the dictionary leg carries
no signal (0.553 within cosine strata), the Qwen leg does not exist, and the
JEPA leg has never been evaluated on a clean corpus.

Start with E1. If divergence predicts nothing, the rest is architecture without
a signal.
