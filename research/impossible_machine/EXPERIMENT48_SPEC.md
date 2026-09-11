# Experiment 48: Prime Field Geometry → Spectral Constraint

## Status: DESIGN COMPLETE, IMPLEMENTATION PENDING

## The wall

Every machine in the Impossible Machine bottoms out in:

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


## Status: NOT YET IMPLEMENTED — this is the design doc.
