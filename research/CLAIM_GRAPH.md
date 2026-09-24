# Claim Graph ~ Physis Research Program ★

Graph: hypothesis -> evidence -> contradiction -> revision -> surviving claim

---

## Track A ~ Coding-Agent Behaviour (strongest chain)

```
H1: "File-touch sequences carry recoverable geometry"
    -> E67: structure (co-access+transition+tree) beats recency+frequency (+0.070)
    -> BUT: order-dependent features tie shuffled null
    -> REVISION: "Behaviour is predicted by workspace structure, not touch order"
    -> STATUS: PARTIAL (co-access clears shuffle, but from a base too low to matter)

H2: "Deformation (online learning) beats a frozen prior"
    -> E70: deform beats frozen (MRR +0.063, CI excludes zero)
    -> BUT: every arm gets worse within a session (-0.044)
    -> REVISION: "Deformation allocates quickly then holds while task drifts"
    -> STATUS: PARTIAL

H3: "The world model can anticipate action outcomes"
    -> E71: frozen prior AUC 0.645 vs 0.500 base-rate
    -> BUT: deform ties frozen on Brier, loses on AUC (-0.057)
    -> REVISION: "Anticipation is possible via a prior, not via within-session learning"
    -> STATUS: SUPPORTED (narrowed)

H4: "The prior transfers across machines and agents"
    -> E73: feather->pansa 0.644 vs in-corpus 0.642
    -> E74: 4 harnesses, 75,933 actions, cold-start rule
    -> STATUS: SUPPORTED (one user's fleet, externally unvalidated)

SURVIVING CLAIM: "Cross-agent failure priors transfer when target history is thin
and coverage is high, with a measurable cold-start crossover point."
```

---

## Track B ~ Ontology / Geometry Instability (settled negative)

```
H1: "Cross-model divergence carries epistemic signal"
    -> E1: |Δ| = 0.602 vs baseline 0.733 (0/22 configs)
    -> E1d: 2-D estimator bounds every combiner at 0.866 vs s2's 0.870
    -> VERDICT: REFUTED. D is a folded read-out of s2.

H2: "Embedding geometry gives stable ontology structure"
    -> E1-E18: cross-embedder ARI ~0.10
    -> E88: anisotropy 0.53-0.56, slope -0.489
    -> E90: rank intervention fails (anisotropy is covariate, not lever)
    -> VERDICT: REFUTED. Geometry is unreliable for certification.

H3: "A cheap signal can certify discovered structure"
    -> E14: random injection AUC 0.880, nearest-cell 0.506
    -> E16: real misfilings ~ all geometric at chance (0.508-0.519)
    -> E18: ANCHORVOC closed as entry-length artifact (0.619 of 0.664)
    -> VERDICT: REFUTED. No tested mechanism detects a real misfiling.

H4: "Geometry can propose (not certify)"
    -> E21: top-3 0.712 vs permuted 0.136
    -> E24: halving hints improves to 0.705
    -> VERDICT: SUPPORTED. The first positive that survives its null.

SURVIVING CLAIM: "Geometry is unreliable for certification but useful for
proposal, with the propose/certify gap as the measurable finding."
```

---

## Track C ~ Contradiction / Relation Typing

```
H1: "Cosine distance separates contradiction from agreement"
    -> E82: bare words at chance (AUC 0.488)
    -> E83: cosine on generated fixture 0.336 (below chance!)
    -> VERDICT: REFUTED. Cosine detects relatedness, not relation polarity.

H2: "Dictionary rules can detect contradiction"
    -> E83: precision 1.000, recall 0.143 on held-out
    -> E84: 5 rules lift recall to 0.840, precision stays 1.000
    -> E85: adversarial fixtures collapse precision to 0.000
    -> E86: participant binding repairs to 1.000
    -> VERDICT: PARTIAL. Rules are precision instruments with narrow recall.

H3: "NLI extends recall on implication pairs"
    -> E89: NLI at 1.000/0.960 holdout, worse than rules on adversarial
    -> VERDICT: PARTIAL. NLI helps but independently reproduces binding failures.

SURVIVING CLAIM: "Relation polarity is not contained in proximity; symbolic
binding and neural inference fail in complementary ways."
```

---

## Track D ~ Reference Spectra / Becoming

```
H1: "References to one object form a tight region"
    -> E78: 6/6 files, within-object distance 4-14x smaller than between
    -> VERDICT: SUPPORTED.

H2: "The region moves with temporal direction"
    -> E78: 5/6 files show direction (Spearman rho up to +0.976)
    -> E78: quality.rs fails (wanders and returns, straightness 0.120)
    -> VERDICT: PARTIAL. Direction exists but fails for wandering/reversion.

H3: "Trajectories are low-dimensional"
    -> E78: 2D spectral concentration 0.569-0.817
    -> VERDICT: SUPPORTED.

H4: "Identity survives at sub-document granularity"
    -> E79: paragraph-level identity dissolves (within 0.379 vs near 0.405)
    -> VERDICT: REFUTED at paragraph level.

SURVIVING CLAIM: "Revisions of a single source file form a low-dimensional
directed semantic trajectory, with temporal direction recoverable from
embeddings alone, but identity dissolves below document granularity."
```

---

## Track E ~ Conatus / Self-Maintenance (refuted)

```
H1: "History constrains the future (path dependence)"
    -> E75: order spread 0.000000 over six orderings at f32 precision
    -> VERDICT: REFUTED. State is a function of the evidence multiset.

H2: "The system resists perturbation"
    -> E76: perturbation added node, coherence drifted down, nothing restored
    -> VERDICT: NOT DEMONSTRATED. Append-only persistence.

H3: "The system has temporal semantics"
    -> E77: replay indexed by write time, not content date
    -> VERDICT: NOT DEMONSTRATED. One current belief, no temporal arrow.

H4: "Conatus is implemented (preference, self-model, temporal state)"
    -> 360 audit: three of four ingredients are inert fields
    -> VERDICT: REFUTED as implemented.

SURVIVING CLAIM: "Conatus as implemented is inert. The fields exist but are
never read. No path dependence, no self-maintenance, no temporal semantics."
```

---

## Track F ~ Retrieval / Cheap Computation

```
H1: "Hybrid (cosine+BM25) beats either alone"
    -> A/B test: no interior mixing weight beats both endpoints
    -> REVISION: "Retrieval and ranking are different problems"
    -> CASCADE: cosine retrieves, BM25 ranks -> paraphrase recall 0.875
    -> STATUS: SUPPORTED (as cascade, not mixture)

H2: "An absolute floor separates warning from reassurance"
    -> B test: commands with refutation bottom out at cosine 0.717
    -> Commands with nothing to say top out at 0.643
    -> Disjoint, 0.074 apart
    -> STATUS: SUPPORTED on 8 synthetic commands (needs real-ledger validation)

H3: "MaxSim fixes pooling"
    -> E87: MaxSim doubles pooled (0.200->0.400) but stays inside null
    -> VERDICT: NOT SUPPORTED. Vote arm is the right statistic.

SURVIVING CLAIM: "Retrieval and ranking are separate problems; a cascade
(cosine retrieves, BM25 ranks) beats any mixture."
```

---

## Cross-Cutting Surviving Claims

1. **Transferable operational failure priors** (Track A) ~ SUPPORTED
2. **Geometry proposes but does not certify** (Track B) ~ SUPPORTED
3. **Relation polarity resists proximity** (Track C) ~ SUPPORTED
4. **Semantic trajectories are low-dimensional and directed** (Track D) ~ SUPPORTED
5. **Conatus as implemented is inert** (Track E) ~ REFUTED
6. **Retrieval != ranking; cascade beats mixture** (Track F) ~ SUPPORTED

---

## Repeated Asymmetries (stronger than any individual result)

1. **Proposal works / certification fails** (E21 vs E14-E18)
2. **Local similarity works / relation identity fails** (E82 vs E83)
3. **Prior transfers / online adaptation fails** (E71-E74)
4. **Fine-grained representation works / pooling destroys it** (E79, E80)
5. **Same-object trajectory survives / sub-object identity dissolves** (E78 vs E79)
6. **Anisotropy is real as correlation / unusable as lever** (E88 vs E90)
7. **Order carries nothing at behavioural scale / carries something at textual scale** (E67 vs E56)
