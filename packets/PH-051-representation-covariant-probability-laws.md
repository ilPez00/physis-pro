---
packet: PH-051
title: Representation-Covariant Probability Laws (E51)
status: done
result: clean failure — 0/5 candidate laws clear transfer AND specificity AND held-out (E50's failure mode restated at the probability level). See physis-core/research/impossible_machine/EXPERIMENT51_RESULTS.md and packets/reports/PH-051.md.
rung: 2
depends: [PH-100]
---

# PH-051 — Representation-Covariant Probability Laws

## Context
E50 established that arithmetic-specific geometric structures (fractal dimensions, Hurst exponents, embedding dimensions) do not survive a blind change of representation. The geometric ontology is representation-dependent. 

E51 moves one abstraction level upward to **probability, statistics, and recurrence**.

## Core Hypotheses & Rules
1. **No Geometry-First Reasoning**: Primary observables are probabilistic/statistical (distributions, return times, gap distributions, fluctuations, conditionals). Geometry is strictly secondary.
2. **$\frac{1}{2}$ is Absent**: $\frac{1}{2}$ must not appear anywhere in candidate generation, scoring, or ranking.
3. **No Representation Averaging**: Measure each representation independently before comparison.
4. **No Post-Hoc Normalization**: Normalizations must be mathematically justified, fixed beforehand, and generalize to held-out representations.
5. **Critical Contrast**: Candidates must satisfy BOTH cross-representation transfer AND arithmetic specificity against null controls (shuffled, Cramér, synthetic).
6. **Blind Holdout**: Partition into Discovery ($R_1, R_2$) and Held-out ($R_3, R_4$) representations. Freeze scoring/normalization before revealing held-out.

## Deliverables
- `physis-core/examples/experiment51_probability_laws.rs`
- `physis-core/research/impossible_machine/experiments/experiment51_results.json`
