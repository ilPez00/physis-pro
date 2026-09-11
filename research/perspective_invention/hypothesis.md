# Hypothesis — perspective invention in Physis

> **Pre-registration.** This file was written before the experiment was run.
> The decision rules below were frozen before any score was observed. Any
> deviation is recorded in `failures.md` with the reason.

## The claim under test

> A sufficiently coherent recursive representation system can discover a new
> perspective on data — not merely classify data within perspectives supplied
> to it — by detecting relational structure that its first, strongest
> organization leaves unexplained, constructing candidate re-representations
> from that residual, and re-organizing the data under them.

Narrow experimental form (Section 13 discipline — no "intuition", no "mind"):

> Can Physis autonomously discover a representation under which previously
> weakly organized relations become more coherent and more useful, with the
> improvement confirmed on held-out data and reproduced by an independent
> discovery route?

## Formal object: perspective

A perspective is a first-class, serializable object (not a metaphor):

```text
P = ( id, parent, generator,
      reference set R        — texts or embedding-space centroids,
      relational operator    — cosine-to-references (the only operator used),
      transformation T_P     — x ↦ [cos(x, r) for r in R]  (re-representation),
      scoring function       — k-means with internal k, silhouette-selected,
      discovered structure D(P) — the partition induced on items,
      lineage + before/after bookkeeping per Section 7 of the mission )
```

`T_P` is deliberately *not* a geometric rotation. "Perspective inversion"
means reversing the direction of inference: the representation is not chosen
upstream and applied; it is *inferred from relational residual* and validated
by whether re-organization under it becomes more coherent.
## Sources of candidates (all derived from observed structure, none labeled)

| generator | residual it consumes | independent because |
|---|---|---|
| `residual-edge` (`P_edge`) | cross-cluster edges of the residual graph | partitions a graph of *discarded* relations, never the raw similarity matrix |
| `soft-membership` (`P_soft`) | multi-modal soft memberships under the baseline | operates on membership *profiles*, an object the baseline cannot express |
| `residual-reference` (`P_ref`) | items of maximal residual degree | reference-conditioning, but with references *chosen by residual structure* (prior track chose them arbitrarily / by domain and failed) |
| `random` (`P_rand`, control) | none — Gaussian random projection | must fail; if it does not, hypothesis false |
| `permuted` (`P_perm`, control) | none — `P_edge` labels shuffled | must fail; keeps cluster sizes, destroys alignment |
| `oracle` (evaluation-only) | latent labels | upper bound, never used in selection |

## The dataset (decisive experiment, mission Section 15)

Synthetic, seeded, surface structure ≠ latent organizing principle:

- 4 surface domains (workshop, garden, harbor, kitchen) built from real
  English nouns — the dominant lexical signal, easy for any embedder.
- 3 latent "flows" — the true organizing principle — expressed only through
  (a) a shared pseudo-word per flow (sub-lexical cue, no pretrained semantics)
  and (b) mild shared phase vocabulary, with inclusion probabilities
  calibrated so the latent structure is *difficult but not impossible*.
- ~5% bridge items carrying two flows (multi-membership ground truth).
- Generation-side difficulty calibration uses latent labels (dataset
  authoring, declared); **discovery never sees any label.**
- Split: train 240 / test 120 (fresh combinations, same grammar), plus a
  transfer set of 120 with *different* surface domains and the same latent
  grammar.

## Pre-registered tiers (mission Section 19, with exact thresholds)

All thresholds are absolute deltas on the train/test splits defined in
`methodology.md`; bootstrap 95% CIs (1000 resamples) reported alongside.

- **Tier 0 — FAILED**: candidates generated but no candidate beats `P_rand`
  on any internal or held-out measure.
- **Tier 1 — INTERESTING BUT INSUFFICIENT**: the internally selected winner's
  discovered train partition has latent ARI ≥ baseline-ARI + 0.10 AND
  raw-space silhouette ≥ baseline silhouette − 0.01 AND latent ARI ≥
  `P_rand` + 0.10.
- **Tier 2 — SUPPORTED**: Tier 1 AND frozen-winner held-out latent 1-NN
  accuracy ≥ raw-cosine 1-NN + 0.05 AND frozen-winner same-flow link AUC
  (cross-domain held-out pairs) ≥ raw-cosine AUC + 0.05, with `P_rand` and
  `P_perm` below the same margins.
- **Tier 3 — STRONG STRUCTURAL EVIDENCE**: Tier 2 AND at least two
  independent generators (`P_edge`, `P_soft`, `P_ref`) each individually
  reach the held-out margins AND mutual partition ARI between two of them
  ≥ 0.5 (permuted-size-matched null < 0.2), OR cross-embedder route
  convergence (MiniLM route vs BGE route on the same generator) ARI ≥ 0.5 —
  against the known cross-embedder partition-identity null of ≈ 0.10.
- **Tier 4 — REPRESENTATION DISCOVERY**: Tier 3 AND transfer: winner frozen
  on dataset 1 improves held-out latent 1-NN on dataset 2 (new surface
  domains) by ≥ +0.05 over raw cosine 1-NN.
- **Tier 5 — HIGH-ORDER GENERATIVE REPRESENTATION**: recursive level-2
  perspectives improve held-out prediction AND transfer simultaneously, with
  the level-1 gain not lost.

Verdict vocabulary per claim: OBSERVED / SUPPORTED / PARTIALLY SUPPORTED /
FALSIFIED / UNRESOLVED. Negative results are preserved verbatim in
`failures.md`.

## Primary (internal, label-free) vs evaluation (label-using) measures

- Internal (drives all selection/recursion/stopping): raw-space silhouette of
  the discovered partition, coherence in P-space, stability under bootstrap.
  **No latent label ever enters selection.**
- Evaluation (reported after freezing): latent ARI/NMI/purity, bridge dual
  margins, held-out accuracy/AUC, transfer deltas, compression curve.

## Falsification conditions stated in advance

- If `P_rand` matches the winner on held-out measures → hypothesis falsified
  (improvement is a property of any re-representation, not of residual-derived
  ones).
- If internal selection picks a candidate whose held-out performance is at or
  below baseline while an unselected candidate is above → internal-selection
  claim falsified (echoes the settled certification failures).
- If candidates converge on each other but not on any structure that improves
  held-out prediction → convergence is a shared artifact of the generators,
  not discovered latent structure (report as PARTIALLY SUPPORTED at best).
