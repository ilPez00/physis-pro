# E7 — Do E6's proposed fixes hold up?

`research/mapper/run_e7.py`

E6 ranked three remedies for the mode axis and stated plainly that none had been
tested. Recommending work without testing the recommendation is how a research
track accumulates plausible-sounding debt, so each is measured here on the same
held-out split. One is confirmed and quantified, one shrinks, one is refuted.

---

## A. Population — confirmed, and now quantified

Each well-populated mode has its **training** entries subsampled to *k*, every
other mode keeps all of its own, and that mode's held-out F1 is re-measured (12
replicates). This reads the F1(n) curve off modes that actually have data.

| mode | n | k=3 | 4 | 6 | 10 | 20 | 40 | all |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| WORK | 150 | **0.02** | 0.01 | 0.07 | 0.13 | 0.26 | 0.38 | 0.49 |
| CREATE | 117 | **0.04** | 0.04 | 0.11 | 0.16 | 0.34 | 0.45 | 0.55 |
| SENSE | 93 | **0.04** | 0.09 | 0.18 | 0.29 | 0.49 | 0.56 | 0.59 |
| MAINTAIN | 77 | **0.04** | 0.06 | 0.13 | 0.25 | 0.36 | 0.43 | 0.49 |
| LEARN | 53 | **0.17** | 0.27 | 0.39 | 0.41 | 0.45 | — | 0.47 |

Against the starved modes at their real sizes: BRAINSTORM (n=3) **0.00**,
DESTROY (n=4) **0.00**, GUIDE (9 train entries) **0.00**.

**Starvation is a sufficient explanation.** A well-populated mode cut to three
training entries scores F1 = 0.06 on average — WORK, a concept with 150 entries
behind it, manages 0.02. The zeros are not a sign that BRAINSTORM or DESTROY are
ill-defined. They are what three entries produces.

**And the curve is a forecast, which is the useful part.** Populating a starved
mode to n ≈ 20 should put it around **F1 0.26–0.49**; to n ≈ 40, around
**0.38–0.56**. That is a falsifiable prediction rather than "add more entries",
and it says where the returns flatten: most of the gain is bought by the first
20, and LEARN shows the curve is nearly flat from 20 to 53.

## B. WORK as a sink — real, but a third of what E6 implied

Remove WORK from the label space entirely and re-score the other 13 modes on the
same 154 held-out non-WORK entries:

| | accuracy | kappa | macro-F1 |
|---|---:|---:|---:|
| WORK present as a class | 0.506 | 0.441 | 0.399 |
| WORK removed entirely | 0.552 | **0.484** | 0.411 |

WORK captures **12 of 154** non-WORK held-out entries — 7.8%.

**WORK is a sink, and the effect on its neighbours is +0.043 kappa.** That is
real and worth fixing, but E6 described it as absorbing "30 errors" and reading
as the junk-drawer shape of `CONSTRUCT/CREATE`; the recoverable effect on the
rest of the axis is modest. Defining WORK by exclusion is worth roughly +0.04
kappa on the other thirteen modes, not a transformation of the axis.

## C. The SENSE / MAINTAIN / LEARN / PLAN cluster — refuted

E6 read the residual confusion as *"a cluster of observation-and-upkeep modes
that shade into each other"* and called that "the signature of a definition
problem rather than a representation problem". Restricting the problem to those
four and comparing against size-matched random four-mode subsets:

| subset | accuracy | kappa | n test |
|---|---:|---:|---:|
| **SENSE / MAINTAIN / LEARN / PLAN** | 0.724 | **0.605** | 76 |
| random 4-mode subsets (40 draws), mean | — | **0.629** | — |
| their range | — | 0.454–0.800 | — |

**The cluster sits inside the control range, slightly below the mean.** Those
four modes are no less separable from each other than any random four modes are.
E6's reading was pattern-matching on raw confusion counts, which are dominated
by class size rather than by conceptual overlap.

This is the third time in this track that a plausible structural reading has
died to a size-matched control, and it did not need a new mechanism to catch —
only the discipline of asking what four *arbitrary* modes would have scored.

---

## Revised recommendations

| # | action | evidence | expected effect |
|---|---|---|---|
| 1 | **Populate BRAINSTORM, DESTROY, GUIDE, PLAY, LIFT to ~20 entries each** | F1(n) curve, 12 replicates across 5 modes | those modes move from 0.00 to roughly **0.26–0.49** |
| 2 | **Define WORK by exclusion** | removing WORK lifts the other 13 modes | **+0.043 kappa** on the rest of the axis |
| 3 | ~~Re-test the SENSE/MAINTAIN/LEARN/PLAN cluster~~ | kappa 0.605 against a 0.454–0.800 control range | **dropped — not a real cluster** |

Item 1 is a corpus task: five modes hold 47 entries between them, and the curve
says the first twenty per mode buy most of what is available. Item 2 is a
writing task worth a measured but modest amount. Item 3 should not be worked on.

**What E7 does not establish:** that populating the starved modes will raise
*overall* mode kappa by any particular amount. The curve predicts each mode's
own F1; the axis-level effect depends on how many entries those modes end up
holding relative to the rest, which is a decision rather than a measurement.
