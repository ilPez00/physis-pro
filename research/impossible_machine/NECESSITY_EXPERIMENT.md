# Experiment 48: The Exponent Necessity Machine

## The wall

Every machine in the Impossible Machine bottoms out in the same place:

> **WHY 1/2?**

All four candidate answers (self-ratio, scale stability, fixed locus,
Love/Strife balance) are either conditional or heuristic.

The critical line's NECESSITY for the actual zeros remains unproven.

## The new experiment

Instead of asking "why are the zeros on 1/2?", ask:

> What happens to every structural invariant of the system when I
> parameterize the critical exponent as sigma, and which invariants become
> impossible to satisfy unless sigma = 1/2?

## Method

1. Construct the prime-density field from arithmetic data (pi, theta, psi)
2. Build scale-space representations
3. For each candidate sigma, compute constraints C_i(sigma)
4. Find the intersection of all constraints
5. Determine the set S of admissible sigma

## Outcomes

Any of: S = {1/2}, S larger, S excludes 1/2, no constraint, ill-posed.
All are valid scientific outcomes.

## Implementation

`physis-core/examples/experiment48_exponent_necessity.rs`

Run:

cd physis-core && cargo run -p physis-core --features embed-onnx --release --example experiment48_exponent_necessity


## Status: IMPLEMENTED AND RUN

- **E48** (naive estimators): LEVEL 1 — see EXPERIMENT48_RESULTS.md
- **E49** (self-consistent, controls-certified): REPRESENTATION-DEPENDENT —
  see EXPERIMENT49_RESULTS.md. E_psi converges to d≈1.40 (stable across 8
  scale windows, distinct from both nulls) but E_theta does not converge;
  no invariant dimension claim possible; 1/2 not among candidate laws.
