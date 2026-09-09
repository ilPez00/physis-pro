# E24 — one weight, not two representations

**Result: hints should be halved, not split per axis.** The proposal was to
filter hints out of `mode` and propagate them further in `domain`. Half of it
holds — down-weighting helps — and the per-axis asymmetry does not.

`run_e24.py`. `v(α) = normalise( vec_name + α·(vec − vec_name) )`, where α = 1 is
the shipped vector and α = 0 is name-only. 582 blind entries, 75% train, 24
shuffles.

| α | domain top-1 | mode top-1 |
|---:|---:|---:|
| 0.00 hints filtered out | 0.607 ± 0.042 | 0.508 ± 0.032 |
| 0.25 | 0.640 | 0.533 |
| **0.50** | **0.648 ± 0.048** | **0.538 ± 0.030** |
| 0.75 | 0.638 | 0.523 |
| 1.00 *shipped* | 0.617 ± 0.040 | 0.487 ± 0.034 |
| 1.50 | 0.559 | 0.430 |
| 2.00 hints propagated further | 0.503 | 0.394 |
| 3.00 | 0.436 | 0.355 |

**α = 0.5 is optimal for both axes.** A single unimodal curve, same peak, so the
per-axis split is not justified by this evidence and the added complexity should
not be paid for. **Propagating hints further hurts domain too** — 0.617 → 0.503
by α = 2 — so "more domain signal for the domain axis" is not what hints supply
past a point; they swamp the name.

At the deployable 57-cell level:

| α | top-1 | top-3 | top-5 |
|---:|---:|---:|---:|
| 0.00 | 0.406 | 0.667 | 0.748 |
| **0.50** | **0.447** | **0.712** | **0.791** |
| 1.00 *shipped* | 0.412 | 0.658 | 0.759 |

**+0.054 top-3 over the shipped representation, for a one-line change.**

## What this corrects in E22

E22 reported hints helping `domain` (+0.022) and hurting `mode` (−0.034) and
read that as an axis asymmetry. Both measurements stand — they are α = 1 against
α = 0 — but **the two-point comparison misread the shape.** The curve is one
hump peaking at 0.5 on both axes; hints are simply over-weighted, and more
over-weighted for mode because mode has less signal in the name to begin with.
The E22 conclusion that *"the corpus has been enriched in the direction that
makes mode worse"* survives; the implied prescription of separate per-axis
representations does not.

## Limits

1. `vec − vec_name` is a **proxy** for the hint contribution. The two BGE vectors
   were embedded and normalised separately, so their difference is not exactly
   "what the hints added". A clean version embeds hints alone and combines. The
   curve is smooth and the peak is far from both endpoints, so the conclusion is
   unlikely to be an artifact of that — but α = 0.5 should not be read as a
   precise constant.
2. Tuned and evaluated on the same 582 entries. The peak is broad (0.25–0.75 all
   beat shipped on both axes), so this is not a knife-edge fit, but a held-out
   corpus has not confirmed the exact value.
