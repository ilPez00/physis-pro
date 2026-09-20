# Experiment 53 Spec — Singular-Series Fit (twin-prime constant)

Status: SPEC ONLY, NOT RUN. No numbers below are results.
Design mirrors E48–E51 conventions; frozen before any held-out look.

## Objective

Fit the empirical twin-prime ratio R(k) against the full
Hardy-Littlewood singular series at N=1e6 and N=1e7. Target: the twin
constant 2C2 ≈ 1.32032. Valid outcomes: agreement / partial / miss.

## Frozen observable, splits, nulls (same as E52 family)

- Observable: binned twin-pair density ratio R(k) over log-spaced
  k-windows; one scalar per window, binning frozen in this spec.
- Splits: discovery windows vs held-out windows, frozen by index
  before any run; no re-binning after seeing values.
- Nulls: Cramer random primes at density 1/ln x + geometric-gap null
  (same as E52) + a shuffled-twin null (order destroyed).
- Estimator sees primes only; no zeta zeros, no critical-line input.

## Gates (frozen before held-out)

- G1 specificity factor: discovery R(k) must separate from EVERY null
  by at least the frozen margin.
- G2 holdout tolerance: held-out windows must reproduce the discovery
  fit within the frozen per-window tolerance.
- G3 constant agreement band: fitted constant inside the frozen band
  around 2C2 ≈ 1.32 to claim agreement; outside = miss.
- Never tune holdout thresholds after seeing held-out values.

## Statistics

- Bootstrap 95% CI over independent k-windows (moving-block where
  windows correlate); resampling unit predeclared per split.
- Effect size reported alongside every claim.
- No mechanical tests without pairing: McNemar / Wilcoxon only where
  pairing holds; otherwise descriptive CI + effect size only.
- Record ties, missing windows, low-sample warnings honestly.

## Relation to 1/2

MUST state: no 1/2 appears in generation or scoring. Series, R(k),
nulls, and all three gates are defined without the critical exponent.
A fitted value near 1/2 is a recorded numerical coincidence, never
support for or against RH.

## Artifacts (to be produced on run; absent now)

- `experiment53_singular_series.rs` example (primes to R(k) to fit).
- JSON output: windows, R(k), null values, fitted constant, CI, gates.
- Results doc `EXPERIMENT53_RESULTS.md` with discovery/held-out tables.
- Run: `cargo run -p physis-core --release --example
  experiment53_singular_series` at N=1e6 then N=1e7.

Status repeated: SPEC ONLY, NOT RUN. Nothing above is evidence.
