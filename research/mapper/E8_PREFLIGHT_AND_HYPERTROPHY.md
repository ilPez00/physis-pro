# E8 — A pre-flight checker, and whether ontologies grow hypertrophically

`research/mapper/preflight.py` · `research/mapper/run_e8.py`

Three questions, taken in order: can experiments be refuted before they are run;
do cells grow by making distinctions or by absorbing whatever comes near; and
should the grid be expanded.

---

## 1. What a verifier can decide here, and what it cannot

A proof assistant verifies **deductive** claims. *"Will this signal predict the
outcome?"* is empirical and no verifier answers it. But a specific and expensive
class of failure **is** deductive, and this track produced several:

- **Span.** E1's standardised divergence arm could not beat a model containing
  `s1` and `s2`, because `Δz = 8.40·s₁ − 20.41·s₂` lies in their span. A theorem,
  decidable before any run.
- **Well-posedness.** AUC is undefined with no positives; kappa degenerates when
  one class holds 95%; a merge that collapses to one group makes every metric
  meaningless. Decidable from the label vector alone.
- **Power.** Whether a design can resolve an effect of the claimed size is a
  computation.
- **Leakage.** Whether a feature is a near-deterministic function of the label is
  decidable by fitting it.

**On Lean specifically:** the span lemmas would be a few lines each, and a rank
computation gives the same answer with less ceremony. These are implemented
directly rather than in a proof assistant. The claim is not "the research is
formalised" — it is "the deductively-decidable failures are now checked first."

### The checker, validated against arms whose outcome is already known

A feasibility checker that cannot recover known failures is worthless; one that
also refuses arms that *worked* is worse than nothing. The checker sees only the
features, never the results.

| arm | measured outcome | checker | R² on baseline |
|---|---|---|---:|
| signed Δz | 0.775 — **lost** to `s1+s2`'s 0.866 | **REFUTED** | **1.0000** |
| absolute \|D\| | 0.602 — **lost** | passes | 0.8657 |
| support_2 alone | 0.866 — **won 22/22** | passes | 0.1174 |

It refutes the arm that was algebraically doomed, passes the arm that won, and
**cannot** kill `|D|` — that one carries genuine variance the baseline lacks and
simply turned out to be weak. **That is the honest boundary: the checker catches
algebraic redundancy, not weak signal.** It would have saved the E1c sweep. It
would not have saved E1 itself.

## 2. Hypertrophy — the hypothesis, and it is confirmed

> *Does the same approach check whether ontologies propagate undifferentiatedly
> and hypertrophically?*

It does, and they do. Two indices, both against **size-matched random controls**
(the same control that killed Stage 13's positive):

```
differentiation(cell) = mean intra-cell similarity
                      − mean similarity of a size-matched random sample
hypertrophy index     = r( log cell size , differentiation )
```

| mode | n | intra-sim | vs random | reading |
|---|---:|---:|---:|---|
| WORK | 150 | 0.559 | **+0.003** | absorbing — barely above random |
| CREATE | 117 | 0.561 | **+0.003** | absorbing |
| SENSE | 93 | 0.592 | +0.034 | weakly differentiated |
| MAINTAIN | 77 | 0.561 | **+0.003** | absorbing |
| LEARN | 53 | 0.604 | +0.046 | weakly differentiated |
| PLAN | 37 | 0.621 | +0.064 | differentiated |
| REST | 31 | 0.608 | +0.049 | weakly differentiated |
| WALK | 28 | 0.558 | **+0.002** | absorbing |
| MOVE | 25 | 0.575 | +0.018 | absorbing |
| GUIDE | 14 | 0.564 | +0.006 | absorbing |
| PLAY | 13 | 0.573 | +0.017 | absorbing |
| LIFT | 13 | 0.618 | +0.060 | differentiated |
| DESTROY | 4 | 0.710 | **+0.150** | differentiated |
| BRAINSTORM | 3 | 0.631 | +0.073 | differentiated |

### HYPERTROPHY INDEX = **−0.644**

**The bigger a mode gets, the less it differentiates.** Growth here is
absorption, not distinction — which is the phenomenon the question named, now
measured rather than suspected. The four largest modes sit within 0.003 of a
random sample of the corpus: as groups of meanings, they are barely groups.

**And the periphery is where it happens.** Re-measuring differentiation on each
mode's core 75% (by distance to centroid) and comparing:

| mode | core 75% | full | dilution |
|---|---:|---:|---:|
| WORK | +0.022 | +0.003 | **+0.019** |
| CREATE | +0.027 | +0.003 | **+0.024** |
| SENSE | +0.063 | +0.034 | +0.030 |
| MAINTAIN | +0.025 | +0.003 | **+0.022** |
| PLAN | +0.095 | +0.064 | +0.032 |
| WALK | +0.028 | +0.002 | **+0.026** |

Every large mode is 4–10× better differentiated once its outer quarter is
removed. **The outer quarter is the sink**, and it is a concrete, reviewable list
rather than a diagnosis — which makes it actionable in a way "WORK is too broad"
was not.

## 3. Should the grid be expanded? Refuted before running

The proposal is more domains and modes — more cells over the same 658 entries.
Running the checker on it:

| grid | verdict | why |
|---|---|---|
| current 5 × 14 | **STOP** | 35 of 57 classes below 6 training entries; forecast F1 **0.07** |
| proposed 2× (140 cells) | **STOP** | **140 of 140** below 6, even assuming perfect balance |
| proposed 4× (280 cells) | **STOP** | **280 of 280** below 6 |

The expansion arms are checked under **perfect balance**, which no real grid
achieves — the current one has 22 cells below 5 entries while its largest holds
69. The forecast is therefore optimistic and the verdict still STOP.

**The current grid already fails this check.** That is the uncomfortable
retrospective finding: 35 of 57 cells are too small to learn, so a good deal of
this track has been measuring sparsity rather than the hypothesis under test.
E7's F1(n) curve is what makes the forecast quantitative.

## 4. "Machines categorize better" — true, but only coarsely

A taxonomy must survive resampling or it is not a taxonomy. The authored grid
has stability 1.0 for free; a discovered partition has to earn it. k-means on the
same vectors, adjusted Rand index between partitions fitted on disjoint 80%
resamples:

| k | ARI across resamples | reading |
|---:|---:|---|
| 5 | **0.463** | moderately stable |
| 14 | 0.277 | unstable |
| 57 | 0.213 | unusable as a taxonomy |
| 140 | 0.202 | unusable as a taxonomy |

For scale, Iteration 18 measured cross-embedder ARI at 0.10 and this track
treated that as fatal to discovered structure.

**Machine partitions are viable at coarse granularity and not at fine.** At
k = 5 a discovered carving is about as stable as this data supports; by k = 57 —
the granularity the grid actually uses — a re-run on 80% of the corpus produces a
substantially different partition. So the position is half-supported: machines
can carve, but not at the resolution the proposal wants, and not with 658
entries.

---

## Synthesis: the corpus is the bottleneck, not the grid

Three independent lines now converge on the same constraint, none of which is
the taxonomy's design:

1. **E5** — collapsing the grid never helps (kappa flat 0.376–0.444 from 57
   groups down to 5). Granularity is not what limits performance.
2. **E7** — the modes that fail are the *empty* ones; a well-populated mode cut
   to three training entries scores F1 0.06.
3. **E8** — expansion is refuted on sparsity at every level, and machine
   partitions are unstable at fine granularity.

**Expanding the grid divides 658 entries across more cells and is predicted, by
this track's own measurements, to make every number worse.** The order of
operations is the other way round: **populate first, expand second.** E7 gives
the target — roughly 20 entries per cell before a cell is learnable at all — so
a 140-cell grid needs on the order of **2,800 entries** against the current 658.

What *is* supported right now, from §2: the outer quarter of WORK, CREATE,
MAINTAIN and WALK is where the differentiation is lost. That is a specific,
reviewable set of entries, and re-filing or re-defining around it is worth
+0.043 kappa by E7's measurement — available today, without touching the grid's
shape.
