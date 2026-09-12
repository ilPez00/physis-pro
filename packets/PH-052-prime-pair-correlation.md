---
id: PH-052
track: physis
phase: 2
status: done
result: partial-positive — Hardy-Littlewood small-gap correction is strongly arithmetic-specific (D~12x above independence nulls) and out-of-sample stable (held-out D 1.258 reproduces discovery 1.222, twin ratio 1.45 in both halves). First object-level property in E50/E51 line that is both prime-specific and stable. See physis-core/research/impossible_machine/EXPERIMENT52_RESULTS.md.
rung: 2
depends: [PH-051]
gate: "cd physis-core && cargo run --release --example experiment52_pair_correlation 2>&1 | grep -q 'experiment52_results.json'"
commit: "branch agent/PH-001; conventional commits; per PROTOCOL §6"
---

# PH-052 — Prime Pair-Correlation (E52): first test for a representation-covariant arithmetic law

## Why (the failure this prevents)

E50/E51 showed the geometric-ontological and probability-level structure of the
primes is **representation-bound**: what transfers across representations is
generic cumulative-counting behaviour, and what is arithmetic-specific does not
transfer. The negative conclusion is only as strong as the failure to find ONE
object-level law that is both arithmetic-specific AND out-of-sample stable. E52
is the named successor: it tests whether the **Hardy–Littlewood singular
series** — the sign-definite deviation of small prime-gap frequencies from the
Cramér independence model — is a genuinely representation-covariant arithmetic
law. If it is not (no deviation, or deviation that evaporates out-of-sample), it
would weigh strongly toward closing the geometric-ontology hypothesis at this
resolution.

## Context

- E51 (`physis-core/examples/experiment51_probability_laws.rs`) returned a clean
  failure: 0/5 candidate laws clear transfer AND specificity AND held-out.
- Gaps/potential laws that are "distributional" are null-equivalent because a
  Cramér 1/ln x process reproduces them. The one known deviation that survives
  IS the small-gap correlation: the twin-prime ratio R(2) ≈ 2C₂ ≈ 1.32, with a
  singular-series profile over even gaps. Cramér independence predicts R(k) ≈ 1.
- Measured facts to verify in code: R(k) profile over even k at 200 000, and its
  stability between the first and second halves of the range.

## Steps

1. `physis-core/examples/experiment52_pair_correlation.rs`:
   - Sieve primes ≤ 200 000 (reuse build_base machinery from E51).
   - Represent the gaps three ways (all the same object — the transfer gate
     measures how faithfully each view recovers the law): **π / θ jumps** (gaps),
     and **ψ / von-Mangoldt jumps** (prime powers included — a strict superset).
   - Observable: empirical-to-Cramér ratio R(k) = N(k)/E(k) for even k, where
     E(k) is the independence (geometric-density) expectation at the actual prime
     positions. Deviation D = RMS of |R(k) − 1| over small even k ∈ {2..40}.
   - Nulls: (N1) Cramér random primes through the same pipeline, (N2) shuffled
     increments of the gap series, (N3) iid exponential gaps with the same mean.
   - Blind held-out: D measured on the first half (discovery) is frozen; the
     second half must reproduce it within a pre-fixed margin.
   - Verdict: a law survives iff specific (D ≫ nulls) AND held-out-surviving
     (halves agree) AND largely transfer-free across the π/θ/ψ views.
   - 1/2 never appears in candidate generation, scoring, or ranking.
2. Write `research/impossible_machine/EXPERIMENT52_RESULTS.md`.
3. Flip PH-052 to done in the packet header.
4. Append `packets/reports/PH-052.md`.

## Tests you must add

- Determinism: two runs produce byte-identical `experiment52_results.json`.
- Gate: example builds cleanly and writes the JSON.

## Gate

```bash
cd physis-core && cargo run --release --example experiment52_pair_correlation 2>&1 | grep -q 'experiment52_results.json'
```

## Commit strategy

- Branch `agent/PH-001` (parent) + the physis-core submodule. Conventional
  commits referencing PH-052. Scoped adds only — never `git add -A`.

## Report

Append `packets/reports/PH-052.md` per PROTOCOL §5.

## Never

- Do not manufacture a connection to 1/2.
- Do not claim representation-independence where the three views are the same
  primes and therefore trivially agree; report the deviation honestly as a
  specificity + out-of-sample-stability test, with the π/θ/ψ agreement measured
  and stated as a caveat, not a discovery.