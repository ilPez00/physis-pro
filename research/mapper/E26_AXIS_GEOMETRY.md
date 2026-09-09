# E26 — domain is a 4-dimensional object; mode is not an object

**Testing: "geometry need not be parallel or additive between domain and mode,
if the two are perpendicular."** They are not perpendicular. But the reason that
does not matter is more useful than the premise would have been.

`run_e26.py`, 582 blind entries, 768-d BGE. Subspaces are spanned by centred
class centroids and **fitted on the train split only** — fitting on everything
would manufacture the improvement being looked for.

## 1. Not perpendicular, and orthogonalising makes it worse

Principal angles between the domain span (4-d) and the mode span (13-d):

| | | | | |
|---|---|---|---|---|
| **35.8°** | 41.9° | 52.3° | 54.5° | mean **46.1°** |

Neither perpendicular (90°) nor collinear (0°). And removing one axis's subspace
before classifying the other **hurts both**:

| | domain top-1 | mode top-1 |
|---|---:|---:|
| raw | 0.634 ± 0.038 | 0.491 ± 0.042 |
| project the other axis out | 0.580 ± 0.041 | 0.475 ± 0.038 |
| **delta** | **−0.055** | **−0.015** |

**The overlap is not interference — it is shared signal both axes use.**
Projection does not purify one axis; it deletes directions the other was
legitimately reading. This also explains why E24's α = 0.5 helped while this
hurts: down-weighting attenuates a noisy *feature* uniformly, whereas projection
surgically removes a *subspace* that carries signal for both.

## 2. The result that matters: domain compresses losslessly, mode does not

| | domain top-1 | mode top-1 | dims |
|---|---:|---:|---|
| full vector | 0.619 ± 0.043 | 0.477 ± 0.044 | 768 |
| **own axis subspace only** | **0.616 ± 0.046** | 0.443 ± 0.040 | **4 / 13** |
| delta | **−0.003** | **−0.034** | |

**Domain survives a 192× compression with no measurable loss.** Four numbers are
the whole of it — and 4 is exactly *k−1* for 5 classes, so domain is a clean
linear 5-class structure with no residual. It is a genuine low-rank object.

**Mode is not.** 13 dimensions is also *k−1* for its 14 classes, yet mode loses
0.034 when restricted to them: its class structure is **not** captured by the
span of its own centroids. Mode needs directions its own classes do not define,
which is what "not a linear object" means.

## 3. This converges with two independent measurements

| method | on domain | on mode |
|---|---|---|
| E21 proposer | top-2 **0.837**, saturated against a stronger embedder (+0.003) | top-2 0.678, still moving (+0.065) |
| E22 confusion | one symmetric merge pair (FABRICATE/CONSTRUCT); BOND clean at 0.76 | a four-way blur over 68% of the corpus |
| **E26 geometry** | **4-d, lossless** | **not a subspace** |

Three methods, three angles, one answer: **domain is a small well-formed
structure and mode as currently defined is not a coherent object.** E22 reached
that from confusions, E26 from rank. Neither knew about the other.

## 4. What this says for the interface

The point that prompted this — *the vector collapses dog-tooth, but the interface
need not* — holds, with the axes assigned differently than expected:

- **Domain can be an interface coordinate.** Four numbers, carried explicitly,
  losslessly, and cheap to expose, store and compare.
- **Mode cannot be**, and no projection or reweighting will make it one. It is
  not that mode needs a better embedder — it does not compress because the
  distinctions do not form a linear structure. This is the same conclusion the
  confusion analysis reached about WORK/CREATE/MAINTAIN/SENSE, and it is why
  `PLAN.md` has the machine proposing domain and deferring mode.

**So the interface carries what is structured and defers what is not**, which is
the architecture already recorded — the machine proposes, the person disposes —
now with a measured boundary rather than an intuited one.

## Limits

1. **Centroid spans are a crude subspace estimate.** A supervised discriminant
   (LDA with shrinkage) might separate the axes better than centroid spans do,
   and the 46° figure would move. The compression result is more robust: it says
   domain's own class span suffices and mode's does not, which is a statement
   about each axis alone and does not depend on the between-axis estimate.
2. **Nearest-centroid throughout**, matching E21 so the numbers are comparable. A
   stronger classifier could extract more from mode's 13-d projection, which
   would weaken §2's claim — the honest form is "mode is not linearly captured by
   its class centroids", not "mode is unlearnable".
3. Single embedder, single corpus, 582 entries.
