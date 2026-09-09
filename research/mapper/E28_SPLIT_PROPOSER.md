# E28 — the split proposer fails, and one thing works

Assembling E24–E27 into the artifact that ships, tested end to end on top-k
containment over 57 cells. 582 entries, 24 identical splits, 75% train.

| variant | top-1 | top-3 | top-5 | top-3 vs baseline |
|---|---:|---:|---:|---|
| **A** joint, shipped vector (E21's proposer) | 0.413 | 0.651 | 0.759 | — |
| **B** joint, **α = 0.5** | **0.436** | **0.705** | **0.782** | **+0.055, *t* = +12.13, better in 24/24** |
| **C** joint, concat[α=0.5 ∥ head] | 0.379 | 0.642 | 0.732 | −0.009, ns |
| **D** factorised into domain × mode, same space | 0.343 | 0.622 | 0.737 | −0.028, *t* = −2.78 |
| **E** factorised, head-word space for mode | 0.293 | 0.532 | 0.628 | **−0.119, better in 0/24** |

## What ships: B

**E24's hint halving, validated end to end.** +0.055 top-3 over the shipped
representation, unanimous across 24 paired splits, one line of code. Nothing in
E25–E27 improved on it.

## Why the split failed, decomposed

The plan was: read domain from the α = 0.5 vector, read mode from the head word,
multiply the posteriors. It lost 0.119. The ablation separates the two causes:

- **Factorisation costs −0.028** (D vs A, at matched representation). Scoring a
  cell as `P(domain) × P(mode)` assumes the axes are independent. **They are
  not** — E26 measured principal angles of 35.8–54.5° and found that projecting
  one axis out *hurts the other*, because they share signal. The joint classifier
  uses that shared structure; the product of marginals throws it away. E26
  predicted this and the prediction was not carried forward into the design.
- **Head-only costs a further −0.091** (E vs D). This is the larger half, and it
  is the error described below.

## The error: coherence is not predictive power

E27 concluded that head-only halves mode's incoherence (−0.061 → −0.024) and
recommended building the mode axis from the head. **That recommendation was
wrong**, and E25's own limits section had already said why: *"not a prescription
to drop the modifier; head-only mode (0.460) is still worse than the full
compound (0.526)."*

| criterion | question | head-only verdict |
|---|---|---|
| compression (E26/E27) | is this axis a linear *object*? | **better** |
| held-out top-k (E25/E28) | does this representation *predict*? | **worse** |

Both measurements are correct. Dropping the modifier makes mode **tidier and
worse** — the modifier carries real predictive signal even though it is the noise
that stops mode being a linear structure. E27 is corrected inline rather than
rewritten.

**The general lesson, and it is the transferable one:** E26's compression test is
a good instrument for asking *whether an axis is a coherent object* and a bad one
for choosing *what to feed a classifier*. Diagnostic criteria and objective
functions are not interchangeable, and this track has now conflated them once.

## What survives for the mode axis

Nothing computational. Merging fails (E27), head-only fails (E28), factorising
fails (E28), a better embedder helps mode but does not close it (E21). **The
remaining work on mode is definitional** — E22 localised it to
WORK/CREATE/MAINTAIN/SENSE holding 68% of the corpus with ample population and
not separating. That is the one route not yet tried.
