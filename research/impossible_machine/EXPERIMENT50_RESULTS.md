# Experiment 50 Results — Representation Equivalence

**Verdict: FAILED — cleanly, and with the failure precisely localized.**

**The question:** what do the representations have no choice but to agree
about? **The answer: only what is generic. Nothing that is prime-specific
survives a change of representation.**

## Pipeline integrity

- Estimator machinery re-certified on all four positive controls (E49 core
  reused verbatim; conventions untouched).
- E49 baseline reproduced: E_psi spread 0.712 ≤ δ 0.820.
- Discovery representations: E_psi, E_theta (+ shuffled and Cramér nulls).
- Held-out representations (constructed blind, frozen before evaluation):
  **E_pi** (pi − li) and **Mertens M(u)/√x**.
- 1/2 absent from the entire discovery process.

## Discovery phase: 5/5 candidates survive (psi, theta)

| candidate | complexity | discovery | null | verdict |
|---|---|---|---|---|
| C1 rank-corr of d(s) profiles | 2 | **0.952** | −0.405 | SURVIVES |
| C2 drift-diff dd/dlog s | 1 | 0.004 | 0.006/0.013 | SURVIVES |
| C3 null-ordering agreement | 1 | 0.000 | 1.000 | SURVIVES |
| C4 raw-Hurst rank-corr | 2 | 0.738 | −0.595 | SURVIVES |
| C5 estimator-order Kendall | 2 | 1.000 | 0.571 | SURVIVES |

The (psi, theta) pair agrees about *everything* at first glance. Then:

## Held-out phase: the agreement evaporates

| candidate | psi–pi | theta–Mertens | pi–Mertens | held-out |
|---|---|---|---|---|
| C1 rank-corr | −0.262 | −0.190 | 0.500 | **FAILS** |
| C2 drift-diff | 0.047 | 0.013 | 0.039 | **survives** |
| C3 null-ordering | 0.000 | NaN | NaN | incomplete |
| C4 raw-Hurst rank | −0.548 | 0.381 | −0.286 | **FAILS** |
| C5 estimator-order | 0.667 | 0.619 | 0.667 | **FAILS** (frozen margin) |

## Arithmetic specificity phase: the mirror image

| candidate | prime | shuffled | Cramér | specific? |
|---|---|---|---|---|
| C1 | **0.952** | −0.405 | 0.214 | YES |
| C2 | 0.004 | 0.006 | 0.013 | **NO — null-equivalent** |
| C3 | 0.000 | 1.000 | NaN | YES |
| C4 | **0.738** | 0.000 | −0.595 | YES |
| C5 | **1.000** | 0.571 | 0.400 | YES |

## The failure, precisely localized

The two properties are **anti-correlated**:

- **C2** (profile flatness) transfers across every representation — and is
  indistinguishable in the nulls. It is a property of *cumulative-counting
  series in general*, not of primes.
- **C1, C4, C5** (rank structure of scale profiles, raw increment-Hurst
  ordering, estimator ordering) are strongly arithmetic-specific — and
  collapse the moment the representation changes. Notably C5 stays positive
  on all held-out pairs (0.62–0.67) but below the frozen margin; the
  estimator *ordering* is partially preserved while its *content* is not.

**Phase IX conditions 2+3 combined: no candidate is BOTH
representation-robust AND arithmetic-specific. The geometric ontology is
representation-bound exactly where it is arithmetic, and generic exactly
where it is not.** This is the strongest form of the negative answer to the
central question — and a direct sharpening of E49's conclusion.

## The four conclusions (A–D)

**A. Dimension invariance: NO.** E49's conclusion stands and deepens — d is
not even approximately the same quantity across representations.

**B. Structural invariance: NO** (for arithmetic structure). The only
transferring structure (C2, flatness) carries no arithmetic content.

**C. Representation dependence: YES — essential, not incidental.** The
discovery-pair agreement (5/5!) was pair-specific, not object-level. Without
the held-out test this would have been a false positive of the exact kind
the blind protocol exists to catch.

**D. Arithmetic specificity: REAL but representation-locked.** Primes are
genuinely distinguishable from both nulls (4/4 specific candidates) — but
only from within a fixed representation. The arithmetic signal does not
survive translation.

## Phase X — relation to 1/2

None of the candidate values is within 0.05 of 1/2.
**Recorded honestly: 1/2 does not emerge. No connection manufactured.**

## Final adversarial question, answered

> Does recursive ontology refinement converge toward an invariant?

**Not from this direction.** What the object *refuses to let you change*
turns out to be, so far: (i) null-separation itself (primes are always
distinguishable from randomized arithmetic), and (ii) the flatness of
d-scale profiles (generic). What it lets you change freely: every
arithmetic-bearing geometric quantity. The invariant, if it exists, is not
made of graph-geometric estimates of cumulative fluctuations.

## Next experiment

The failure mode names its own successor: an ontology whose primitives are
**representation-covariant by construction** — e.g. measure-theoretic
(measures, return times, gap distributions as probability laws — which are
representation-invariant by definition) rather than graph-geometric
(estimator-dependent). Ask not what dimension the primes have, but which
probability laws are the same no matter which arithmetic function you
sample. If those laws also fail to be shared, the geometric-ontology
hypothesis dies with a much smaller corpse.
