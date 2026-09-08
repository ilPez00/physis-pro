# E5 — Is the 70-cell grid the dominant constraint?

`research/mapper/run_e5.py` · `physis-core/examples/mapper_physis_feedback.rs`

`FINAL_REPORT.md` concluded that *"the taxonomy, not the embedding or the
calibration heuristic, [is] the dominant constraint"*. That inference rested on
three things, one of which E1 has since revised: **the embedder swap was not
modest.** BGE beat MiniLM on 22 of 22 configurations, +0.132 AUC. So the
taxonomy claim has to be tested on its own rather than inherited.

Three open items are answered here: a proper supervision comparison, the
granularity question, and integration with physis's own coherence/quality
machinery.

**Setup.** 658 entries, **57 occupied cells of 70**, 461 train / 197 held out
(the E5 split, shared with the Rust experiment via `split.json`). Cell sizes:
median 7, max 69, and **22 cells hold fewer than 5 entries** — 59 entries sit in
cells too small to learn from a 70% split.

---

## A. Proper supervision comparison — kappa, not a majority floor

Cohen's kappa corrects for chance agreement given the actual class marginals.
On a 57-way problem whose largest class is 10%, "beats the majority floor" is a
very low bar and flatters every arm.

| classifier | accuracy | **kappa** | macro-F1 |
|---|---:|---:|---:|
| k-NN k=5 | **0.447** | **0.416** | 0.318 |
| nearest centroid | 0.431 | 0.404 | **0.384** |
| softmax regression | 0.437 | 0.399 | 0.318 |
| k-NN k=15 | 0.411 | 0.375 | 0.258 |
| k-NN k=1 | 0.396 | 0.365 | 0.321 |
| majority-class floor | 0.102 | **0.000** | 0.004 |

**Kappa ≈ 0.41 — moderate agreement, not poor.** Against the label-noise
ceiling: Stage 11 put the post-0.1.21 misfiling proxy at 16.2%, so if ~16% of
held-out labels are themselves wrong, a perfect classifier scores about **0.84**.
The representation reaches roughly **half the achievable accuracy**.

The gap between accuracy (0.447) and macro-F1 (0.318) is the tell: performance
is very uneven across classes. Large cells are handled, small cells are not.

*(A methodology note kept rather than buried: the first run of this table used a
400-iteration plain-gradient softmax that scored 0.218 — badly underfit — and it
would have been reported as evidence that the taxonomy was crushing a fitted
classifier. It was fixed to an adaptive-step optimiser and reached 0.437, in
line with the others.)*

## B. Granularity — the decisive test of the taxonomy claim

The same representation and classifier, scored at three levels of the grid:

| level | classes | accuracy | floor | **kappa** | gain over floor |
|---|---:|---:|---:|---:|---:|
| cell (domain × mode) | 57 | 0.431 | 0.102 | **0.404** | 36.7% |
| mode | 14 | 0.487 | 0.218 | **0.418** | 34.4% |
| domain | 5 | 0.645 | 0.264 | **0.552** | 51.7% |

**If the 70-cell grid were the binding constraint, collapsing 57 classes to 14
should produce a large jump in chance-corrected performance. It produces
+0.014.** Cell-level and mode-level kappa are effectively identical. The fine
granularity is not what is costing the accuracy.

**The interesting asymmetry is elsewhere: the domain axis (kappa 0.552) is
markedly more learnable than the mode axis (0.418).** That lines up exactly with
Stage 11, which found the domain axis had no written definition, wrote one in
`docs/GRID_AXES.md`, and regenerated the corpus against it. **The mode axis never
got that treatment.** On this evidence the weaker half of the grid is the mode
axis, and it is the half that has never been defined.

## C. Merge curve — does collapsing confusable cells rescue anything?

Cells merged by average-linkage agglomeration on **train** centroids, labels
relabelled to the merged groups, classifier retrained.

| groups | accuracy | floor | **kappa** | largest group |
|---:|---:|---:|---:|---:|
| 57 | 0.431 | 0.102 | **0.404** | 10.5% |
| 50 | 0.533 | 0.457 | **0.444** | 43.5% |
| 35 | 0.635 | 0.731 | 0.365 | 72.5% |
| 25 | 0.751 | 0.827 | 0.352 | 82.8% |
| 18 | 0.832 | 0.898 | 0.266 | 91.6% |
| 12 | 0.919 | 0.944 | 0.282 | 95.9% |
| 8 | 0.954 | 0.970 | 0.237 | 98.0% |
| 5 | 0.954 | 0.975 | 0.237 | 98.6% |

**Accuracy must rise as groups merge — the floor rises with it, faster.** Kappa
is the column that matters, and it peaks at 0.444 (50 groups, +0.040 over the
full grid) then falls steadily to 0.237. Merging the grid does not rescue
performance; past a small amount it makes the problem worse in chance-corrected
terms while the accuracy column climbs reassuringly toward 0.95.

The "largest group" column is printed so degeneracy cannot be mistaken for a
result — by 35 groups the agglomeration has swallowed 72.5% of the corpus into
one group.

*(The first version of this curve used maximum-linkage, chained everything into
one mega-cluster, and produced accuracy exactly equal to the floor with kappa
0.000 at every level. That is a degenerate merge, not a finding, and it was
fixed to average linkage before anything was reported.)*

## D. Integration with physis's own coherence / quality machinery

E2 measured emendation with a hand-rolled relabelling loop. `QualityTracker` is
the **shipped** machinery for exactly this, so the honest ablation runs the loop
through it, using the production path copied from `src/web/coherence.rs`:
`penalize_cell(wrong cell, PIN_DISSENT_SEVERITY = 0.25)`,
`report_success(right cell)`, and scoring by `adjust_score` applied to the
`CellScore`s that `classify` produces.

Baseline held-out cell accuracy **0.350**; 128 of 197 held-out entries are
misclassified and form the correction pool. Every arm is scored **only on
entries that were not themselves corrected**.

| corrections | naive re-seed | Δ | physis QualityTracker | Δ |
|---:|---:|---:|---:|---:|
| 0 | 0.350 | +0.000 | 0.350 | +0.000 |
| 10 | 0.364 | −0.005 | 0.123 | **−0.246** |
| 25 | 0.384 | −0.017 | 0.134 | **−0.267** |
| 50 | 0.442 | −0.027 | 0.211 | −0.259 |
| 100 | 0.629 | −0.082 | 0.485 | −0.227 |

Read the Δ column, not the accuracy column: as corrections remove known-wrong
entries from the evaluation pool, the pool gets easier and the *baseline* rises
faster than either arm.

**Both mechanisms fail to propagate, and physis's is much worse.** Naive
re-seeding costs −0.005 to −0.082; the QualityTracker path costs −0.23 to −0.27.

**Why, and it is a granularity mismatch rather than a defect.**
`penalize_cell` applies a **global, per-cell** penalty. But the cells that get
chosen wrongly most often are the *largest* ones — they attract. Penalizing
`CONSTRUCT/CREATE` because one entry was misfiled into it damages every entry
that genuinely belongs there. Per-entry evidence applied as a per-cell weight
cannot do anything else.

**Stated fairly:** `QualityTracker` was built for interactive quality complaints
and pin dissent, where a handful of corrections accumulate slowly under a
human's eye. A hundred batched corrections is outside its design envelope, and
this result should not be read as "the shipped feature is broken". It should be
read as: **the existing machinery is not a correction-propagation mechanism, and
using it as one makes classification worse.**

*(A bug in this experiment, found and fixed before reporting: arm C originally
scored `adjust_score` over `cell_centroids` — a mean-of-entries scorer — while
arm A used `classify`, which uses max. Arm C read 0.431 against arm A's 0.350 at
**zero** corrections, which is how the mismatch was caught. With the same base
scorer both arms now agree exactly at zero.)*

---

## Verdict

```
TAXONOMY-AS-DOMINANT-CONSTRAINT: NOT SUPPORTED
```

The grid's granularity is not what is limiting performance. Chance-corrected
performance is essentially identical at 57 cells and at 14 modes (kappa 0.404 vs
0.418), and merging confusable cells raises kappa by at most 0.040 before
degrading it.

**What the evidence points to instead**, in order of the support behind it:

1. **The mode axis is the weak half of the grid.** Domain kappa 0.552 against
   mode 0.418 — and the domain axis is the one Stage 11 gave a written
   definition and a regenerated corpus. The mode axis has never had either.
   This is the most actionable finding in E5.
2. **Label noise.** ~16% by the Stage 11 proxy, capping achievable accuracy near
   0.84 against the observed 0.447.
3. **Cell sparsity, not cell count.** 22 of 57 cells hold fewer than 5 entries;
   macro-F1 0.318 against accuracy 0.447 says the small cells are simply not
   learned. That is a *population* problem, and it is the opposite prescription
   from merging: those cells need entries, not amalgamation.

**What does not follow:** that the grid is well-designed. E5 tests whether
granularity limits *predictability*, and it does not. Whether the 5×14 grid
carves the domain the way its users need is a different question that no
accuracy number answers.
