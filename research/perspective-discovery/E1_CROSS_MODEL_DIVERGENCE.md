# E1 Cross-Model Divergence Report

`physis-core/examples/experiment47_cross_model_divergence.rs`
Run 2026-09-08. Full console output: reproduce with

```
cargo run -p physis-core --features embed-onnx --release \
  --example experiment47_cross_model_divergence
```

---

## 1. Experimental question

> Does cross-model representational divergence contain predictive epistemic
> information that existing Physis mechanisms do not already provide?

Three prior experiments failed to make cross-embedder *agreement* useful
(Iteration 18, ARI ≈ 0.10; Iteration 19, FPR gain 0 and recall 5/5 → 4/5;
Iteration 24, split agreement fails backwards). E1 inverts that assumption and
measures *disagreement* directly against an independently observable outcome.

The primitives are kept separate, and none of them is called certainty:

```
support_1 = cos(e1, centroid_1(cellmates, excluding self))
support_2 = cos(e2, centroid_2(cellmates, excluding self))
delta     = support_1 - support_2
D         = |delta|
```

**Answerable on this harness:** Q1 (does D correlate with actual errors), Q4
(discrimination over baseline), Q5 (signed vs absolute), Q6 (absolute support
vs disagreement), Q7 (joint vs single), Q8 (swap invariance).

**Not answerable on this harness, and not answered:** Q2 (contradictions) and
Q3 (hypothesis failure). Stage 12 carries no proposition-level ground truth —
its only outcome is an injected misfiling. Per §14 of the brief, divergence is
not contradiction, and inventing a proposition-level label to fit the metric is
exactly the redesign §3 forbids. Q2/Q3 belong to E2 and were left open.

## 2. Existing baseline

Stage 12's COSINE scorer, unchanged: `1 - cos(e_MiniLM, cell centroid)`, higher
= more suspect, scored by rank-sum AUC with ties averaged.

**Mean AUC 0.733 over 22 configurations.** E1 reproduces the Stage 12 baseline
*exactly* — all 22 per-configuration values match `sweep_regenerated.txt`
digit-for-digit (0.712, 0.686, 0.738, 0.744, 0.759, 0.739, 0.767, 0.747, 0.827,
0.789, 0.775, 0.767, 0.725, 0.722, 0.752, 0.742, 0.745, 0.746, 0.581, 0.559,
0.757, 0.745). The published configuration (stride 7, mult 13, half=all) gives
n = 558, 74 injected, cosine 0.739 — identical to Stage 12.

## 3. Experimental setup

Reused from `experiment38_soundness_audit.rs` **without modification**:

| component | source |
|---|---|
| corpus | `OntologyLoader::load_all()` classification domains, mode anchors excluded — 658 entries |
| gate | WordNet noun-DAG anchoring; only anchored entries are scored — 571/658 |
| ground truth | deterministic stride/mult misfiling injection, character for character |
| split | `test_entries` from `cell_vocab_AB.json` — 318 held-out entries |
| peer rule | cellmates under the *perturbed* assignment, self excluded, `peers.len() >= 2` |
| centroid | mean of peer vectors, renormalised, computed without the entry itself |
| metric | `auc()`, rank-sum with ties averaged |
| configurations | the same 11 stride/mult pairs × {test, all} = 22 |
| strata procedure | the same 8-stratum within-cosine rank test |

Three things were added, and nothing else:

1. **A second embedder.** BGE-base-en-v1.5, 768-d, **mean** pooling — the same
   pooling as MiniLM, so the only variable is model identity. (Iteration 18
   measured mean 0.488 vs CLS 0.489 LOO-1NN for BGE; the formats are
   interchangeable on this data, so matching MiniLM's is the cleaner choice.)
2. **The sweep runs inside one process.** experiment38 was re-invoked per
   configuration. The corpus does not vary across configurations — only the
   injection does — so embedding 731 texts twice per invocation would cost ~20×
   for identical vectors. The injection loop became a function; no scored
   quantity changed, and the exact reproduction of all 22 baseline values is the
   evidence for that.
3. **A logistic combiner for the joint arms**, fit on the TRAIN half and scored
   on TEST only. A single monotone signal needs no fit, so its held-out AUC is
   unaffected; the joint arms are the only ones that could overfit, and they are
   the only ones that were fit.

The `minimal()` / non-lattice computation was dropped — E1 uses no structural
scorer. WordNet loading was kept because the anchored gate defines *which*
entries are scored, and dropping it would change n.

## 4. Configurations

All scores oriented so **higher = more suspect** (Stage 12's baseline already
scored distance, not similarity). Signed Δ is the exception and is reported raw.

| arm | score |
|---|---|
| A baseline | `1 - support_1` |
| B support_1 | `-support_1` (the baseline restated; AUC identical by construction) |
| C support_2 | `-support_2` |
| D absolute | `\|support_1 - support_2\|` |
| E signed | `support_1 - support_2` |
| F sum | `-(support_1 + support_2)` |
| G joint | logistic(support_1, support_2, D), fit on train, scored on test |
| Dr control | `\|support_1 - support_2_deranged\|` |

## 5. Results

### Mean over the 22 configurations

| Signal | Mean AUC | vs baseline | sd | paired *t* | wins/22 |
|---|---:|---:|---:|---:|---:|
| A baseline (`1-s1`) | **0.733** | — | — | — | — |
| B support_1 | 0.733 | +0.000 | 0.000 | +0.00 | 0 |
| C support_2 | **0.866** | **+0.132** | 0.034 | **+18.54** | **22** |
| D absolute \|Δ\| | 0.602 | −0.131 | 0.058 | −10.53 | **0** |
| E signed Δ | 0.574 | −0.159 | 0.075 | −10.01 | **0** |
| F sum | 0.812 | +0.079 | 0.015 | +24.17 | 22 |
| Dr random pairing | 0.411 | −0.322 | 0.065 | −23.33 | 0 |

### Held-out: fit on TRAIN, scored on TEST, per configuration

| Model | mean held-out AUC | vs `s1` alone | paired *t* |
|---|---:|---:|---:|
| `s1` alone | 0.725 | — | — |
| **`s1 + D`** | **0.650** | **−0.076** | **−3.24** |
| `s1 + s2` | **0.866** | +0.141 | +12.46 |
| `s1 + s2 + D` | 0.859 | +0.133 | +11.12 |
| `D` alone | 0.513 | −0.213 | −4.97 |

Adding D to the baseline makes the baseline **worse**, on held-out data, and
adding D to `s1 + s2` makes that worse too (0.859 < 0.866).

### Published configuration, bootstrap CI (2000 resamples, paired rows)

| arm | half=test | half=all |
|---|---|---|
| C support_2 | +0.103 [+0.041, +0.162] | +0.143 [+0.095, +0.191] |
| D absolute | −0.188 [−0.309, −0.069] | −0.155 [−0.250, −0.060] |
| E signed | −0.219 [−0.364, −0.077] | −0.151 [−0.265, −0.031] |
| F sum | +0.066 [+0.038, +0.095] | +0.086 [+0.064, +0.111] |
| Dr random | −0.348 [−0.461, −0.230] | −0.351 [−0.442, −0.262] |

Every divergence arm's confidence interval on the baseline difference lies
**entirely below zero**, on both halves.

### Non-linear combinations (published config, half=all)

| combination | AUC |
|---|---:|
| support_2 alone | **0.881** |
| min(s1, s2) | 0.825 |
| mean(s1, s2) | 0.825 |
| max(s1, s2) | 0.775 |
| baseline (`1-s1`) | 0.739 |

Nothing built from both models beats simply using the better one.

## 6. Controls

### Model swap

| metric | MiniLM as reference | BGE as reference |
|---|---:|---:|
| baseline `1-s` | 0.739 | 0.881 |
| absolute \|Δ\| | 0.584 | 0.584 |
| signed Δ | 0.588 | 0.412 |

`|Δ|` is invariant, as it must be. Signed Δ is directional: AUC → 1 − AUC under
the swap. **A signed arm is therefore only statable once a reference model and a
challenger model are named**, and this implementation has no canonical ordering.
The joint linear arm is permutation-invariant in its features by construction.

The swap also produces the report's most consequential number: **the baseline
itself is +0.142 AUC better when the reference model is BGE.**

### Random pairing

Model-2 vectors deranged across entries, so entry *i* is scored in BGE space
using another entry's vector against *i*'s own cell centroid — marginals
preserved, per-entry pairing destroyed.

| | real pairs | random pairs |
|---|---:|---:|
| mean \|D\| | 0.1000 | 0.1378 |
| AUC of \|D\| | 0.584 | 0.388 |

`D(real) ≠ D(random)`. Genuinely paired representations disagree *less* than
unpaired ones, and the real divergence is 0.20 AUC more informative than the
deranged one. **So cross-model geometry does carry real shared structure** — a
useful qualification on Iteration 18's ARI ≈ 0.10, which read as near-total
independence. That structure simply turns out not to be epistemically useful,
which is the next section.

### Held-out evaluation

Every joint arm is fit on the train half and scored on the test half only. The
single-signal arms are reported for both halves separately in the 22-config
table; no arm's sign changes between halves.

### The Stage 12 strata control, pointed at D

This is experiment38's own "does structure add anything beyond cosine"
procedure — the identical 8-stratum within-rank test that scored ANCESTRY at
0.553 and NON-LATTICE at 0.480.

| conditioned on | absolute \|D\| | signed Δ | support_2 | support_1 |
|---|---:|---:|---:|---:|
| **support_1** (the baseline) | 0.698 | 0.764 | 0.800 | — |
| **support_2** (the better model) | **0.423** | 0.474 | — | 0.554 |
| support_1, random-paired Dr | 0.555 | — | — | — |
| support_2, random-paired Dr | 0.422 | — | — | — |

**This table is the finding.** Conditioned on the baseline, D looks like it
carries something (0.698, well above ANCESTRY's 0.553). Conditioned on the
better model instead, D drops to **0.423 — below chance — and becomes
indistinguishable from the deranged null (0.422).**

The mechanism is arithmetic and checkable. Within a stratum of nearly-constant
`s1`, `D = |s1 − s2|` is a monotone folding of `s2`, and signed `Δ = s1 − s2` is
very nearly `−s2` plus a constant. That predicts signed Δ within `s1` strata
should approach support_2's own within-strata score: predicted ≈ 0.800,
**measured 0.764**. Divergence is not a signal. It is a lossy re-encoding of the
second model's support, and folding it (taking the absolute value) is what costs
the 0.10 between 0.764 and 0.698.

## 7. Interpretation

> **Does cross-model disagreement contain useful epistemic information?**

**No.** Not on its own (0.602 vs 0.733, 0/22 configurations, *t* = −10.53), not
added to the baseline (held-out 0.650 vs 0.725, *t* = −3.24), and not added to
the two supports jointly (0.859 vs 0.866). Every confidence interval on the
baseline difference sits entirely below zero. And once the analysis conditions
on the *stronger* of the two supports rather than the weaker, the residual
information in D is statistically identical to a random derangement of the model
pairing.

Q6 asked whether either model's absolute support contains information that
disagreement does not. It emphatically does, and that is where the value of a
second model lies: **support_2 alone beats the baseline by +0.132 AUC on 22 of
22 configurations** (*t* = +18.54, held-out 0.866 vs 0.725). The second embedder
is worth having. It is worth having as a *better support*, not as a
disagreement detector.

One further consequence deserves stating plainly, because it is uncomfortable:
`support_1` contributes **nothing** on top of `support_2` — 0.554 within
support_2 strata, and the fitted `s1 + s2` model reaches exactly the same
held-out 0.866 as `s2` alone. On this task the weaker model is not a second
opinion. It is redundant.

## 8. What this establishes

1. **The divergence hypothesis (H2) is refuted on this benchmark**, by the same
   harness, ground truth, split, metric and configuration count that refuted the
   structural mechanisms in Stage 12.
2. **Why it fails is known, not merely observed.** D is a folded read-out of
   support_2; the folding loses ~0.10 AUC, and the unfolded quantity was already
   available as a plain support.
3. **Cross-model geometry is not noise.** Real pairs diverge less than deranged
   pairs (0.1000 vs 0.1378) and their divergence is 0.20 AUC more informative.
   Iteration 18's ARI ≈ 0.10 understated the shared structure; it just is not
   structure that predicts filing errors.
4. **Signed Δ is directional and currently unstatable.** With no canonical
   reference/challenger ordering in the implementation, `metric(A,B) ≠
   metric(B,A)` (0.588 vs 0.412). Any future signed arm must name the roles.
5. **Stage 12's refutations get stronger, not weaker.** CONSTRAINT (0.685),
   ANCESTRY (0.604) and NON-LATTICE (0.459) were measured against a 0.733
   baseline. The available baseline is 0.866. The gap they must close roughly
   doubles.
6. **The best certification scorer in this whole track is still plain cosine —
   just to a better embedder.** 0.866 mean AUC, from a drop-in model swap, no
   new mechanism. Nothing invented across Stages 0–13 comes close.

## 9. What remains unproven

- **Q2 and Q3 are untouched.** Whether divergence predicts contradictions or
  hypothesis failure cannot be asked of a harness whose only outcome is an
  injected misfiling. This is a limit of the benchmark, not a result.
- **One benchmark, one task.** Misfiling detection on a 5×14 facet grid. A
  negative here does not refute divergence for retrieval, calibration, or
  outcome prediction — it refutes it for the task E1 was specified on.
- **One model pair.** MiniLM/BGE, both general-purpose English text encoders
  with mean pooling. A pair chosen to be *architecturally* divergent (text vs.
  multimodal, contextual vs. structural) might behave differently. That is the
  only version of H2 this report leaves standing, and it is a weak prior given
  the mechanism in §6.
- **Whether the second model's +0.132 transfers.** It was measured on injected
  misfilings, which Stage 12 already flags as a lower bound on difficulty: a
  synthetic perturbation is easier than a misfiling a careful author believed
  was right.
- **H3 (Stage 13) is untouched by this.** The two findings coexist without
  tension: Stage 13 showed a single embedder reads authorial format as a
  semantic stratum; E1 shows a second embedder's *disagreement* does not fix
  that. Both point the same way — at the instrument, not at the geometry between
  instruments. Notably, E1's confound is one Stage 13 would have caught: 13% of
  entries carry prose hints, and the two models may simply agree on style.
  E1 did not control for that, and should before H2 is called closed everywhere
  rather than closed here.

## 9b. Follow-up: the sign/magnitude decomposition (E1b, E1c)

Raised after the first pass: *adding divergence subtracts, but using a modulus
with a separate sign study, ranked by coherence ±1.* E1 tested `|Δ|`, signed Δ,
and a **linear** combiner over `(s1, s2, D)`. A linear model already spans signed
Δ, but it cannot express "partition by which model is more supportive, then rank
by how far apart they are" — and the strata procedure, which is exactly that kind
of conditional rank test, is where `|D|` looked strongest. So the decomposition
was a genuine gap in E1's coverage, not a restatement of it.

Define `coherence = +1 when support_1 ≥ support_2, else −1` and
`magnitude = |Δ|`, and rank lexicographically: coherence block first, magnitude
within it.

### E1b — the decomposition as stated, and why it tested an artifact

| arm | AUC | vs baseline 0.739 |
|---|---:|---:|
| coherence sign alone | 0.513 | −0.226 |
| modulus \|D\| alone | 0.584 | −0.155 |
| lex: +1 block, then \|D\| | 0.589 | −0.150 |
| lex: −1 block, then \|D\| | 0.564 | −0.175 |
| signed Δ | 0.588 | −0.151 |

**The sign is +1 on 478 of 558 rows — 86%.** A near-constant variable cannot
discriminate whatever it means, so E1b measured a scale artifact rather than the
idea: BGE's cosines live in a narrower band (sd 0.049) than MiniLM's (sd 0.119),
so `sign(s1 − s2)` mostly reports which model you are in, not whether this item
is coherent. Rejecting the proposal on that number would have been sloppy.

### E1c — the same decomposition with each model's scale removed

Standardise each support within its own distribution (no labels involved), then
take the sign and magnitude of `Δz = z1 − z2`. The split becomes 315 / 243.

| arm | AUC | vs baseline 0.739 |
|---|---:|---:|
| z coherence sign alone | 0.673 | −0.066 |
| z modulus \|Δz\| alone | 0.656 | −0.083 |
| **z lex: +1 block, then \|Δz\|** | **0.773** | **+0.034** |
| z lex: −1 block, then \|Δz\| | 0.426 | −0.313 |
| z signed Δz | 0.775 | +0.036 |

**The proposal was right about E1's arm.** Collapsing to `|Δ|` threw away real
structure, and standardising before decomposing lifts the divergence family from
0.584 to 0.775 — the first divergence arm anywhere in E1 to beat the MiniLM
baseline. Held out, `|Δz| + z sign` reaches 0.761 against the raw version's
0.538. That is a large correction and it belongs in the record.

**It does not move the verdict, and the reason is again arithmetic.** With
sd₁ = 0.119 and sd₂ = 0.049,

```
Δz = z1 − z2 = 8.40·support_1 − 20.41·support_2 + constant
```

so the standardised signed divergence is a *linear* combination of the two
supports, weighted 2.4× toward support_2. It is inside the span of the `s1 + s2`
model by construction — which is why that model dominates it (held out 0.866 vs
0.775) and why adding the decomposition to it still subtracts (0.861,
paired *t* = −1.88, 4 wins in 22). Standardising did not find a second channel;
it found better weights on the channel that was already there.

### The E1 control, applied to the decomposition

| conditioned on support_2 | real | random-paired null |
|---|---:|---:|
| z coherence sign | 0.595 | 0.584 |
| z modulus \|Δz\| | 0.449 | 0.427 |

The gap between the genuine pairing and a derangement is **+0.011** on the sign
and +0.022 on the modulus. This is the same verdict E1 reached, reached again by
a different route: conditioned on the stronger support, the decomposition is
indistinguishable from noise. (One incidental: the raw, unstandardised sign
scored *below* its own null — 0.595 against 0.604 — which is the clearest
possible statement that E1b was measuring scale, not structure.)

The lex arms also confirm E1's directionality finding: `+1 block first` scores
0.773 and `−1 block first` 0.426. The sign axis is directional, so it remains
unstatable until a reference model and a challenger are named.

## 10. Thesis consequence

```
DIVERGENCE HYPOTHESIS NOT SUPPORTED
```

Unchanged by E1b/E1c. The decomposition improved the arm by 0.19 AUC and still
loses to a model built from the two supports alone, and still equals its own null
once conditioned on the stronger support.

Per §18 of the brief, this is the FAIL branch: **stop the JEPA/Qwen expansion,
and reassess the thesis around Stage 13.** E2–E5 were not implemented.

## 11. Next experiment

E1's stopping rule says not to propose an extension of the divergence track, and
this report does not. The one thing E1 *did* produce that justifies work is not
an extension of H2 at all:

**Re-run the Stage 12 certification comparison with BGE as the baseline
embedder.** It is a configuration change, not a mechanism. It costs one sweep.
It moves the number every structural scorer in this track has been measured
against from 0.733 to 0.866, and it should be done before any further
certification mechanism is proposed, because several of this track's "close"
negatives are not close against the real baseline.

The second candidate, and the reason it is second: **control Stage 13's format
confound inside the Stage 12 harness** (score the entry names alone, hints
dropped) and check whether the baseline's 0.733 — and BGE's 0.866 — survive it.
Stage 13 showed the embedder reads house style. Stage 12's ground truth is
injected, so a format effect cannot create the labels; but it can inflate how
separable the cells look. That number is currently unknown.
