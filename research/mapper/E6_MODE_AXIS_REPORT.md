# E6 — Where the mode axis is weak, and why

`research/mapper/run_e6.py` · `physis-core/examples/mapper_anchor_export.rs`

E5 found the two halves of the grid are not equally learnable — **domain kappa
0.552, mode kappa 0.418** — and noted that Stage 11 gave the *domain* axis a
written definition and a regenerated corpus while the mode axis never received
either. E5 called that "the most actionable finding". **E6 shows that was
overstated**, and locates what is actually wrong.

## A. Per-mode breakdown, held out

| mode | n | precision | recall | F1 | most confused with |
|---|---:|---:|---:|---:|---|
| WORK | 150 | 0.60 | 0.42 | 0.49 | MAINTAIN |
| CREATE | 117 | 0.62 | 0.50 | 0.55 | WORK |
| SENSE | 93 | 0.58 | 0.61 | 0.59 | WORK |
| MAINTAIN | 77 | 0.46 | 0.52 | 0.49 | SENSE |
| LEARN | 53 | 0.42 | 0.53 | 0.47 | SENSE |
| PLAN | 37 | 0.43 | 0.67 | 0.52 | SENSE |
| REST | 31 | 0.50 | 0.67 | 0.57 | MAINTAIN |
| WALK | 28 | 0.27 | 0.27 | **0.27** | PLAN |
| MOVE | 25 | 0.30 | 0.38 | **0.33** | PLAN |
| GUIDE | 14 | 0.00 | 0.00 | **0.00** | LEARN |
| LIFT | 13 | 1.00 | 0.33 | 0.50 | LEARN |
| PLAY | 13 | 0.75 | 0.50 | 0.60 | CREATE |
| DESTROY | 4 | 0.00 | 0.00 | **0.00** | — |
| BRAINSTORM | 3 | 0.00 | 0.00 | **0.00** | — |

## B. Population, not just definition

**Pearson r(log class size, F1) = +0.718** across the 14 modes.

The three modes scoring **F1 = 0.00** are BRAINSTORM (n = 3), DESTROY (n = 4)
and GUIDE (n = 14). A mode with three entries cannot survive a 70/30 split
regardless of how well its definition is written.

**This corrects E5.** The recommendation there was "write a mode-axis
definition". Roughly three quarters of the variance in per-mode F1 is explained
by how many entries the mode holds. Writing prose will not put entries in
BRAINSTORM.

## C. The Stage 11 test — do the authored anchors define their axis?

Stage 11's root cause for the domain axis was that its anchors did not actually
separate it. The same test, on both axes, with the domain axis as the control
because it *has* had a written definition since 0.1.21. For each entry, find the
nearest of the 70 authored anchors and ask whether that anchor's axis value
matches:

| anchor text | axis | recovered | largest class | gain over chance |
|---|---|---:|---:|---:|
| name + hints | **domain** | 0.562 | 0.272 | **39.9%** |
| name + hints | **mode** | 0.372 | 0.228 | **18.7%** |
| hints alone | domain | 0.540 | 0.272 | 36.7% |
| hints alone | mode | 0.350 | 0.228 | 15.7% |

**The mode anchors define their axis roughly half as well as the domain anchors
define theirs** — 18.7% against 39.9% gain over chance, and the ordering is
unchanged when the cell names are stripped and only the authored hints are used.

So the definition gap is real. It is simply not the whole story, and it is not
evenly spread across the axis — which is what D shows.

## D. The confusion is concentrated, and it has a sink

| pair | count |
|---|---:|
| CREATE ↔ **WORK** | 11 |
| SENSE ↔ **WORK** | 8 |
| MAINTAIN ↔ **WORK** | 7 |
| WALK ↔ **WORK** | 4 |
| CREATE ↔ LEARN | 4 |
| MAINTAIN ↔ SENSE | 4 |
| LEARN ↔ SENSE | 3 |
| CREATE ↔ WALK | 3 |
| PLAN ↔ SENSE | 3 |
| MAINTAIN ↔ REST | 3 |

**WORK is a sink.** It appears in four of the top five confusion pairs and
absorbs 30 of the errors listed. *(E7 sized the effect: removing WORK from the
label space lifts the other thirteen modes by **+0.043 kappa**, and WORK
captures 7.8% of non-WORK held-out entries. Real, and worth fixing — but a third
of what the junk-drawer comparison below implies.)* Its definition in `docs/GRID_AXES.md` is
*"execute, labour, do the routine doing"* — which excludes almost nothing.
Creating something is doing something; maintaining something is doing something;
sensing something is doing something. WORK has the junk-drawer shape Stage 13
found at `CONSTRUCT/CREATE`, one axis up.

The rest of the confusion is *not* diffuse either: SENSE ↔ MAINTAIN ↔ LEARN ↔
PLAN form a cluster of observation-and-upkeep modes that shade into each other.
That pattern — a few semantically adjacent pairs swapping repeatedly — is the
signature of a definition problem rather than a representation problem.

> **Refuted by E7.** Restricting the problem to those four modes gives kappa
> 0.605, against a size-matched random-four-mode control whose range is
> 0.454–0.800 with mean 0.629. The cluster sits *inside* the control range and
> slightly below its mean: those four are no less separable than any arbitrary
> four. The reading above was pattern-matching on raw confusion counts, which
> are dominated by class size. See `E7_FIX_TESTS_REPORT.md`.

## What follows, ranked by the evidence behind it

1. **Populate the starved modes.** BRAINSTORM (3), DESTROY (4), GUIDE (14),
   PLAY (13), LIFT (13). Five modes hold 47 entries between them and three of
   them score exactly zero. This is the largest and most certain effect
   (r = +0.718), and it is a corpus task, not a writing task.
2. **Define WORK by exclusion.** Not "what counts as WORK" but "what does *not*
   count, and where it goes instead." A definition that does not exclude cannot
   stop a sink from filling. This is one mode, not the axis.
3. ~~**Then re-test the SENSE/MAINTAIN/LEARN/PLAN cluster.**~~ **Dropped.** E7
   tested it against a random-four-mode control and it is not a cluster.

**What E6 did not establish, and E7 then measured:** whether either fix would
help. E7 confirms the population diagnosis and turns it into a forecast (a mode
at n≈20 should reach F1 0.26–0.49), sizes the WORK fix at +0.043 kappa, and
refutes the cluster. See `E7_FIX_TESTS_REPORT.md`.
