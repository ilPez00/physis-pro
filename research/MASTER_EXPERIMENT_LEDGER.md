# MASTER EXPERIMENT LEDGER — Human-Readable Version

Complete inventory of all identifiable experiments in the Physis research program.
Canonical IDs resolve the original E-number collision problem (different tracks reused E63, E64, etc.).
NO DELETIONS — failed experiments are research evidence.

Research tracks:
- PD = perspective-discovery (ontology induction, JEPA)
- MAP = mapper (n-gram dictionaries, taxonomy)
- CON = convergence (cross-domain abstraction)
- BEH = behaviour (coding-agent failure prediction)
- REF = reference-spectrum (semantic trajectories)
- ON = ontology (directory ontology, contradiction, retrieval)
- WM = world-map / cheap computation
- IM = impossible-machine / primes
- STR = structural (graph/link prediction)
- C3 = conatus (self-revision, temporal semantics)

---

## PH-PD-E1 — Cross-model divergence
- Date: 2026-09-08 | Commit: 96735b4
- Question: Does |s1-s2| carry epistemic signal?
- Result: REFUTED. |Δ| is a folded read-out of s2, folding costs ~0.10 AUC.
- Status: REFUTED | Reproducible: yes

## PH-PD-E1b — Sign/magnitude decomposition
- Date: 2026-09-08 | Commit: e17e604
- Result: REFUTED. Δz is a linear reweighting; s1+s2 still dominates.
- Status: REFUTED | Reproducible: yes

## PH-PD-E1d — Non-parametric bound
- Date: 2026-09-08 | Commit: e17e604
- Result: SUPPORTED. 2-D estimator reaches 0.866 vs s2's 0.870 — bounds every combiner.
- Status: SUPPORTED | Reproducible: yes

## PH-MAP-E2 — Mapper hypothesis (Qwen n-grams)
- Date: 2026-09-08 | Commit: e280eb3
- Question: Can Qwen3-8B n-gram structure be a groundable map?
- Result: REFUTED. No disproportionate sparse gain; propagation worse than ignoring the map.
- Status: REFUTED | Reproducible: yes

## PH-MAP-E3E4 — Density diagnosis (24x text)
- Date: 2026-09-08 | Commit: b957c23
- Result: REFUTED. Every level got worse with 24x data. L2 falls below majority floor.
- Status: REFUTED | Reproducible: yes

## PH-MAP-E5 — Taxonomy constraint
- Date: 2026-09-08 | Commit: dfff20d
- Result: NOT SUPPORTED. Collapsing 57->14 classes gains only +0.014 kappa.
- Status: NOT SUPPORTED | Reproducible: yes

## PH-MAP-E6 — Mode axis weakness
- Date: 2026-09-08 | Commit: cf89d89
- Result: SUPERSEDED by E7. Population explains it: r = +0.718 with log class size.
- Status: SUPERSEDED | Reproducible: yes

## PH-MAP-E7 — Mode cluster structure
- Date: 2026-09-08 | Commit: 4b06690
- Result: REFUTED. Apparent clusters do not survive random-subset control.
- Status: REFUTED | Reproducible: yes

## PH-MAP-E8 — Hypertrophy
- Date: 2026-09-08 | Commit: f2c2cde
- Result: SUPPORTED. Hypertrophy index -0.644; pre-flight STOPs on sparsity.
- Status: SUPPORTED | Reproducible: yes

## PH-MAP-E9 — Signifier address
- Date: 2026-09-08 | Commit: 20492d1
- Result: PARTIAL. Best address 0.244 vs geometry's 0.447. Fractal structure confirmed.
- Status: PARTIAL | Reproducible: yes

## PH-MAP-E10 — Selective error / viability
- Date: 2026-09-08 | Commit: 43ee885
- Result: NOT SUPPORTED. +0.01 identity for -5.8 depth. Register survives.
- Status: NOT SUPPORTED | Reproducible: yes

## PH-MAP-E11 — Candidate miner (WordNet)
- Date: 2026-09-08 | Commit: 8379b5c
- Result: SUPPORTED. r@25 0.325 vs 0.005 shuffled; r@100 0.497 vs 0.086.
- Status: SUPPORTED | Reproducible: yes

## PH-MAP-E12 — Anchor audit
- Date: 2026-09-08 | Commit: 67f5f20
- Result: REFUTED. Definitions win in 1 cell, lose in 31. Centroid matches authored anchor.
- Status: REFUTED | Reproducible: yes

## PH-MAP-E13 — Anchor ceiling
- Date: 2026-09-08 | Commit: d6712d8
- Result: REFUTED. Headroom over centroid is +0.004. Anchor work is finished.
- Status: REFUTED | Reproducible: yes

## PH-MAP-E14 — Injection realism
- Date: 2026-09-08 | Commit: 04401f8
- Result: SUPPORTED. Random injection AUC 0.880, nearest-cell 0.506, real 0.410.
- Status: SUPPORTED | Reproducible: yes

## PH-MAP-E15 — Structure survives
- Date: 2026-09-08 | Commit: 3ad8468
- Result: SUPERSEDED. CONSTRAINT wins by +0.106 under nearest injection. E16 withdrew this.
- Status: SUPERSEDED | Reproducible: yes

## PH-MAP-E16 — Real misfilings battery
- Date: 2026-09-08 | Commit: 03d7ea4
- Result: PARTIAL. All geometric at chance (0.508-0.519). ANCHORVOC wins but has confound.
- Status: PARTIAL | Reproducible: yes

## PH-MAP-E17 — ANCHORVOC confound
- Date: 2026-09-08 | Commit: 25ddff0
- Result: PARTIAL. Test 3 FAILS — ANCHORVOC rises on hint-leaking entries.
- Status: PARTIAL | Reproducible: yes

## PH-MAP-E18 — ANCHORVOC closed
- Date: 2026-09-08 | Commit: bbf2700
- Result: SUPPORTED. Entry length alone reaches 0.619 of 0.664. Not a certification signal.
- Status: SUPPORTED | Reproducible: yes

## PH-CON-E19 — Cross-domain convergence
- Date: 2026-09-09 | Commit: 76a0ff9
- Question: Does five congruent domains improve abstraction?
- Result: NOT SUPPORTED. Ceiling does not climb with scale. C4 differentiation is worst clause.
- Status: NOT SUPPORTED | Reproducible: yes

## PH-CON-E19b — Control rebuild
- Date: 2026-09-09 | commit: 9274cbe
- Result: S5 was never a control (byte-identical, tf-cosine 1.000). B5c fires at 4B, valid at 9B.
- Status: SUPPORTED (transferable finding about controls) | Reproducible: yes

## PH-CON-E20 — N-gram sieve floors
- Date: 2026-09-09 | commit: 76ff22d
- Result: Length and frequency at chance. Embedding LOPO separates C3 4/4. Pre-registered negative is false.
- Status: PARTIAL | Reproducible: yes

## PH-MAP-E21 — Geometry as proposer
- Date: 2026-09-09 | commit: 325f1e8
- Result: SUPPORTED. Top-3 0.712 vs permuted 0.136. Learning curve still climbing at 436.
- Status: SUPPORTED | Reproducible: yes

## PH-MAP-E22 — Axis structure from confusions
- Date: 2026-09-09 | commit: ca71437
- Result: Mode is three problems: four-way blur, definition failure, five unmeasured classes.
- Status: SUPPORTED | Reproducible: yes

## PH-MAP-E23 — Supervision sources
- Date: 2026-09-09 | commit: eb25d25 (actually afd42bc)
- Result: TF-IDF (0.628) beats embedding (0.575) on commit subjects. Signal is lexical convention.
- Status: SUPPORTED | Reproducible: yes

## PH-MAP-E24 — Hint weight
- Date: 2026-09-09 | commit: afd42bc
- Result: alpha=0.5 beats both alpha=0 and alpha=1. Top-3 0.712 vs 0.658.
- Status: SUPPORTED | Reproducible: yes

## PH-MAP-E25 — Composition
- Date: 2026-09-09 | commit: 03992f3
- Result: Geometry is additive. Mode sits in head (+0.251), modifier adds little (+0.054).
- Status: SUPPORTED | Reproducible: yes

## PH-MAP-E26 — Domain-mode perpendicularity
- Date: 2026-09-09 | commit: 03992f3
- Result: NOT SUPPORTED. 46.1 deg mean. Orthogonalising hurts both.
- Status: NOT SUPPORTED | Reproducible: yes

## PH-MAP-E27 — Mode coherence through merging
- Date: 2026-09-09 | commit: 4dd6824
- Result: NOT SUPPORTED. Merging makes mode less coherent (-0.187).
- Status: NOT SUPPORTED | Reproducible: yes

## PH-MAP-E28 — Split proposer
- Date: 2026-09-09 | commit: 27f0236
- Result: NOT SUPPORTED. Factorising costs -0.028. Head-only costs -0.091 top-3.
- Status: NOT SUPPORTED | Reproducible: yes

## PH-MAP-E29 — Mode discovery
- Date: 2026-09-09 | commit: 4a3cb3c
- Result: NOT SUPPORTED. Clustering recovers subject matter. Current four beat every discovered carve.
- Status: NOT SUPPORTED | Reproducible: yes

## PH-BEH-E67 — Behaviour geometry (file touches)
- Date: 2026-09-14 | commit: 4356276
- Result: Directory tree explains next-file. Order-dependent features tie shuffled null.
- Status: PARTIAL | Reproducible: yes

## PH-BEH-E70 — Behaviour geometry (deformation)
- Date: 2026-09-14 | commit: 4356276
- Result: Deform beats frozen (MRR +0.063). But every arm gets worse within a session.
- Status: PARTIAL | Reproducible: yes

## PH-BEH-E71 — Anticipating outcomes (failure prediction)
- Date: 2026-09-14 | commit: 0108720
- Result: SUPPORTED. Frozen prior AUC 0.645. Order ties shuffled (third time). Deny loses.
- Status: SUPPORTED | Reproducible: yes

## PH-BEH-E72 — Workspace state features
- Date: 2026-09-14 | commit: db23c7d
- Result: NOT SUPPORTED. State features are ties at 45 sessions. More data moved toward zero.
- Status: NOT SUPPORTED | Reproducible: yes

## PH-BEH-E73 — Prior transfer across machines
- Date: 2026-09-14 | commit: b5b28b3
- Result: SUPPORTED. feather->pansa 0.644 vs in-corpus 0.642. Coverage is the gate.
- Status: SUPPORTED | Reproducible: yes

## PH-BEH-E74 — Four harnesses, three machines
- Date: 2026-09-14 | Commit: ed89319
- Result: SUPPORTED. 75,933 actions, 4 harnesses. Cold-start rule: borrowed > own when thin.
- Status: SUPPORTED | Reproducible: yes

## PH-REF-E75 — History storage vs path dependence
- Date: 2026-09-14 | commit: 53f1ed5
- Result: REFUTED. Order spread: 0.000000 over six orderings at f32 precision.
- Status: REFUTED | Reproducible: yes

## PH-REF-E76 — Perturbation resistance
- Date: 2026-09-14 | commit: 53f1ed5
- Result: NOT DEMONSTRATED. Perturbation added node; dream declined to act. Coherence drifted down.
- Status: NOT DEMONSTRATED | Reproducible: yes

## PH-REF-E77 — Temporal semantics
- Date: 2026-09-14 | commit: 477e7b1
- Result: NOT DEMONSTRATED. Replay indexed by write time, not content date.
- Status: NOT DEMONSTRATED | Reproducible: yes

## PH-REF-E78 — Reference spectrum (trajectories)
- Date: 2026-09-14 | commit: 5bbd59f
- Result: SUPPORTED. 6 files, 5/6 temporal direction. 2D spectral concentration (0.569-0.817).
- Status: SUPPORTED | Reproducible: yes

## PH-REF-E79 — Granularity and position
- Date: 2026-09-14 | commit: cde78bb
- Result: Genre-dependent. Developmental docs show direction, reference docs don't. Sub-document identity dissolves.
- Status: PARTIAL | Reproducible: yes

## PH-ON-E80 — Directory ontology
- Date: 2026-09-15 | commit: 19e3e02
- Result: File level beats null (0.643). Pooling destroys it (0.400). Bottom-up vote 0.600.
- Status: PARTIAL | Reproducible: yes

## PH-ON-E81 — Ingestion and static rules
- Date: 2026-09-15 | commit: 8031da0
- Result: Anchored cues survive shuffle (ratio 3.57-4.15). Single-word cues at chance (0.92).
- Status: SUPPORTED | Reproducible: yes

## PH-ON-E82 — Synonym/antonym geometry
- Date: 2026-09-15 | commit: c9b8075 (actually 75a2f2e)
- Result: Bare words at chance (AUC 0.488). Sentence frame 0.691. Mis-pairing control 0.536.
- Status: SUPPORTED | Reproducible: yes

## PH-ON-E83 — Contradiction rules
- Date: 2026-09-15 | commit: 75a2f2e
- Result: Precision 1.000, recall 0.143 on held-out. Cosine at chance on generated (0.336).
- Status: SUPPORTED | Reproducible: yes

## PH-ON-E84 — Structured rules
- Date: 2026-09-15 | commit: c70e338
- Result: Precision 1.000, recall 0.840. Implication class missed (0/3).
- Status: SUPPORTED | Reproducible: yes

## PH-ON-E85 — Adversarial fixtures
- Date: 2026-09-15 | commit: 9345ac1
- Result: Precision 0.000 on adversarial. All failures are binding failures.
- Status: SUPPORTED | Reproducible: yes

## PH-ON-E86 — Participant binding
- Date: 2026-09-15 | commit: 9f7e409
- Result: SUPPORTED. Precision 1.000 on adversarial, recall 0.833. Binding repairs without recall cost.
- Status: SUPPORTED | Reproducible: yes

## PH-ON-E87 — MaxSim pooling
- Date: 2026-09-15 | commit: e6939c4
- Result: MaxSim doubles pooled (0.200->0.400) but stays inside null. Vote arm still best.
- Status: SUPPORTED | Reproducible: yes

## PH-ON-E88 — Anisotropy covariate
- Date: 2026-09-15 | commit: 510f38b
- Result: SUPPORTED. Anisotropy 0.53-0.56, slope -0.489. Cross-embedder ARI ~0.10 reproduced.
- Status: SUPPORTED | Reproducible: yes

## PH-ON-E89 — NLI arm
- Date: 2026-09-15 | commit: 57ac570
- Result: NLI at 1.000/0.960 holdout but worse than surface rules on adversarial.
- Status: PARTIAL | Reproducible: yes

## PH-ON-E90 — Rank intervention
- Date: 2026-09-15 | Commit: dbf6011
- Result: REFUTED. Rank space does not recover geometry. Anisotropy is a covariate, not a lever.
- Status: REFUTED | Reproducible: yes

## PH-IM-E48-E52 — Impossible Machine / primes
- Date: 2026-09-16 | commit: 49428fe
- Result: PH-052 prime pair-correlation: partial-positive. PH-051 representation-covariant laws: clean failure.
- Status: PARTIAL | Reproducible: yes

## PH-IM-E53 — Structural analogy / transform
- Date: 2026-09-16 | commit: 2d46f8d
- Result: Pending externalization. May reduce to classical link prediction.
- Status: UNTESTED | Reproducible: yes

## PH-WM-E55-E66 — World-map / cheap computation
- Date: 2026-09-16 | commit: multiple
- Result: Cascade rerank (cosine retrieves, BM25 ranks) gives paraphrase recall 0.875. Floor 0.66 separates classes.
- Status: SUPPORTED | Reproducible: yes

## PH-C3-E75-E77 — Conatus / self-revision
- Date: 2026-09-14 | commit: 53f1ed5, 477e7b1
- Result: Three of four conatus ingredients are inert fields. No path dependence. No temporal semantics.
- Status: REFUTED as implemented | Reproducible: yes
