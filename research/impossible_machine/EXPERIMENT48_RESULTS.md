# Experiment 48 Results — Prime Field Geometry → Spectral Constraint

**Verdict: LEVEL 1 — arithmetic scaling, no intrinsic dimension, no spectral
constraint. The hypothesis did not survive its own pipeline.**

## What was run

`physis-core/examples/experiment48_exponent_necessity.rs` — the full pipeline
without ever introducing 1/2, d=2, the critical line, or RH:

1. Arithmetic fields from primes only (pi, theta, psi, normalized
   fluctuations, n = 500,000).
2. Scale-space in log coordinates.
3. Three independent dimension estimators (box-counting, DFA, correlation).
4. Three null models (uniform, Cramer random primes, phase-randomized
   surrogate).
5. Only then: a spectral scan over sigma in (0,1), unprivileged, with a
   scrambled-sigma control for the admissibility threshold.

## Results

| estimator | d_eff |
|---|---|
| box-counting | 0.076 (unstable — see limitations) |
| DFA (2−alpha) | 1.059 |
| correlation | 1.557 |

| null model | d_eff | vs prime field |
|---|---|---|
| A: uniform random | 1.000 | distinct |
| B: Cramer random primes | 1.240 | distinct |
| E: phase-randomized surrogate | 1.301 | distinct |

- **The prime field is genuinely arithmetic**: all three nulls are
  distinguishable from it. The fluctuations are not generic noise.
- **No single intrinsic dimension**: the estimators disagree beyond any
  reasonable tolerance. Per the pre-registered rule, they were NOT averaged.
- **No spectral constraint**: the admissible set S is EMPTY under the
  scrambled-control threshold. The prime-field geometry, as measured, does
  not couple to the spectral exponent.
- **sigma = 1/d: undefined** (no d to divide by).

## Answer to the final adversarial question

1. Intrinsic dimension? **NO.**
2. Stable across scales? **Not established.**
3. Genuinely arithmetic (vs nulls)? **YES** — the one positive result.
4. Constrains sigma? **NO.**
5. sigma = 1/d? **Undefined.**
6. If d≈2 derived without RH? **Moot — d was never established.**
7. What relation actually emerges? **None that survives the controls.**
8. Failed assumption: **that a single effective dimension describes the
   prime field.** The failure is at the geometry-construction stage, not the
   spectral-coupling stage.

## Honest limitations (recorded, not hidden)

- The box-counting estimator returned 0.076, which is a symptom of a broken
  estimator (graph of a function should be near 1, not 0), not a discovery.
  The correlation estimator (1.557) is the most credible of the three.
- The compatibility function A(sigma) is one candidate construction among
  many; an empty S may reflect a bad A, not a genuine decoupling.
- 500,000 is a small x-range for asymptotic scaling claims.
- Null coverage is partial (no gap-shuffled Null C/D).

## What this means for the program

Per the pre-registered outcomes, this is valid scientific output: the
prime-density field, measured this way, does **not** constrain Re(rho).
The necessity route (deriving 1/2 from arithmetic geometry) is not dead —
but this implementation does not support it, and the burden now sits on:
(a) a dimension estimator that agrees with itself, (b) a compatibility
construction whose null distribution is calibrated, (c) larger x-ranges.

Never report this as evidence for or against RH. It is neither.
