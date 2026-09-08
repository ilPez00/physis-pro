# E9 — Is there a discrete signifier structure worth formalising?

`research/mapper/run_e9.py`

The proposal: stop deciding membership by geometry — cosine to a centroid, which
E8 showed sits **+0.003** above a random sample for the largest cells — and give
each entry a **value** instead. Membership becomes a decidable predicate on that
value. The value is a signifier, not the thing. Tokens are the obvious alphabet.

That is a real proposal, and Lean would earn its keep on it: consistency of an
assignment is machine-checkable, and propagation becomes inference with provable
properties. But **formalising noise is worse than not formalising**, so the prior
question is empirical — does a discrete address carry the ontology at least as
well as the geometry it would replace?

**Standard to beat: geometry reaches 0.447 held-out cell accuracy, kappa 0.416.**
H(cell) = 5.05 bits.

---

## A. Dyadic address — recursive bisection, membership is a prefix test

Each node is split on its own top principal direction at the median, so an entry
gets a bit-path and membership at any level is a prefix test. Self-similar by
construction, which is what "fractal" means operationally.

| depth | groups | I in-sample | norm | bits/level | **held-out acc** | unseen |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2 | 0.304 | 0.060 | 0.30 | — | — |
| 4 | 16 | 1.426 | 0.282 | 0.49 | — | — |
| **6** | **64** | 2.631 | 0.520 | 0.65 | **0.244** | — |
| 8 | 256 | 4.017 | 0.795 | 0.67 | 0.221 | 0.11 |
| 12 | 256 | 4.017 | 0.795 | 0.00 | 0.208 | 0.15 |

### The fractal guess is supported

Bits gained per level, over the eight depths before the splitter saturates:

```
0.30  0.34  0.30  0.49  0.55  0.65  0.72  0.67
first half 0.356 · second half 0.648 · ratio 1.82
```

**Information per level is constant-to-rising, not decaying.** That is what
self-similar structure looks like: each additional bit of address buys about as
much as the last one did. Against a random address at matched granularity the
dyadic code is consistently ahead (depth 8: 4.017 against 3.436), so the
structure is real rather than an artifact of having many groups.

> **A correction to this experiment's own first verdict.** The first run reported
> "information per level decays — the fractal guess is not supported". That was
> wrong, and it was my splitter's fault: nodes below four members stop dividing,
> so past depth 8 the gain is structurally zero, and averaging those zeros in
> produced a decay that measured the stopping rule rather than the data. Bits per
> level are now computed only over unsaturated depths.

## B. Token address — the rarest Qwen tokens used directly as the code

Each entry's code is its *k* rarest tokens by **merge rank** — Qwen's own
frequency ordering, so "rare" means rare in Qwen's training corpus rather than in
these 658 entries. No embedding is consulted anywhere.

| k tokens | groups | I in-sample | norm | **held-out acc** | unseen |
|---:|---:|---:|---:|---:|---:|
| 1 | 575 | 4.893 | 0.968 | **0.091** | 0.82 |
| 2 | 653 | 5.054 | **1.000** | **0.010** | 0.99 |
| 3 | 657 | 5.054 | **1.000** | **0.000** | 1.00 |

**The in-sample columns are a trap, and they are printed to show it.** At k = 2
there are **653 groups for 658 entries**: the code *names* each entry, so
normalised MI reads a perfect 1.000 while the code generalises to nothing. The
held-out column collapses to 0.010, and 99% of held-out entries carry a code
never seen in training.

**"Use tokens directly" fails as an exact code because the alphabet is too
fine.** Qwen's vocabulary is 151,643 symbols and these entries are ~22 tokens
long; any exact conjunction of rare tokens is a fingerprint. Tokens can only work
as signifiers if they are **coarsened** — binned by merge rank, or folded through
the merge DAG — not used at full resolution.

## C. What this says about building the Lean layer

| scheme | held-out cell accuracy |
|---|---:|
| **geometry (BGE k-NN)** | **0.447** |
| best dyadic address (depth 6, 64 groups) | 0.244 |
| token address, k = 1 | 0.091 |
| token address, k ≥ 3 | 0.000 |

**A decidable address currently carries about half of what the geometry it would
replace carries.** The gap between the best address's in-sample normalised MI
(0.520) and its held-out accuracy (0.244) is exactly the memorisation a
fine-grained code can fake, and it is why in-sample MI must never be the
reported number here.

**Recommendation: do not build the Lean layer yet — but the direction is not
refuted.** Two findings pull in opposite directions and both are real:

- *Against, for now:* every discrete address tested generalises worse than
  cosine. Formalising one today would encode a structure that predicts less than
  what it replaces, and the consistency guarantees Lean provides are guarantees
  about an assignment that is itself half-wrong.
- *For, and specifically:* the space **is** hierarchically structured with
  roughly constant information per level. That is the property an address scheme
  needs in order to be worth having, and it was not obvious in advance.

**The concrete corrective:** the failure is alphabet resolution, not the idea.
The token code fails by being too fine (653 groups / 658 entries); the dyadic
code peaks at 64 groups and degrades past it. So the next construction to try is
a **coarse** symbolic alphabet — merge-rank bins rather than token identities, or
the dyadic path truncated at the depth where held-out accuracy peaks — and the
bar it has to clear is 0.447, not 1.000.

**What E9 does not establish:** that no discrete address can reach geometry's
0.447. Two schemes were tested and both are natural first choices; the
self-similarity result is a reason to expect a better one exists.
