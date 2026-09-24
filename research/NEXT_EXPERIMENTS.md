# Next Experiments — Ordered by Research Value per Unit Cost

Cheap falsifiers run first. Each experiment is ordered by expected information gain divided by effort.

---

## P0 — Freeze E74 method (immediately)
- **Expected gain:** Establishes the baseline all future papers reference
- **Dependencies:** None
- **Compute:** None
- **Wall-clock:** 1 hour
- **Human work:** Low — document the frozen algorithm
- **Risk:** None
- **Unlocks:** Papers 1, 9

## P1 — Reproduce E74 from committed redacted corpus
- **Expected gain:** Verifies the 75,933-action corpus is reproducible
- **Dependencies:** P0
- **Compute:** CPU, <1 hour
- **Wall-clock:** 2 hours
- **Human work:** Low
- **Risk:** Low
- **Unlocks:** Papers 1, 9

## P2 — Acquire public external coding-agent dataset
- **Expected gain:** External validation of E74's cross-agent transfer claim
- **Dependencies:** None (can run in parallel with P0/P1)
- **Compute:** Download + extraction
- **Wall-clock:** 1-2 days
- **Human work:** Medium — find and acquire SWE-bench trajectories or OpenHands data
- **Risk:** Dataset may not have compatible labels
- **Unlocks:** Paper 1 (external replication)

## P3 — Write converter for external dataset to E74 format
- **Expected gain:** Enables blind external replication
- **Dependencies:** P2
- **Compute:** CPU
- **Wall-clock:** 4-8 hours
- **Human work:** Medium
- **Risk:** Label mismatch between datasets
- **Unlocks:** Paper 1

## P4 — Blind external E74 replication
- **Expected gain:** The decisive test — does E74's method work on data it has never seen?
- **Dependencies:** P1, P3
- **Compute:** CPU, <1 hour
- **Wall-clock:** 2 hours
- **Human work:** Low
- **Risk:** If it fails, Paper 1's external validity claim dies
- **Unlocks:** Paper 1 (the key experiment)

## P5 — Run cold-start curve
- **Expected gain:** Characterize the crossover point where borrowed > own prior
- **Dependencies:** P1
- **Compute:** CPU, <1 hour
- **Wall-clock:** 2 hours
- **Human work:** Low
- **Risk:** Low
- **Unlocks:** Paper 1 (cold-start law)

## P6 — Coverage-transfer regression
- **Expected gain:** Model coverage as a predictor of transfer success
- **Dependencies:** P1
- **Compute:** CPU, <1 hour
- **Wall-clock:** 1 hour
- **Human work:** Low
- **Risk:** Low
- **Unlocks:** Paper 1 (coverage mechanism)

## P7 — Baseline suite
- **Expected gain:** Compare against simple baselines (global rate, logistic regression)
- **Dependencies:** P1
- **Compute:** CPU, <1 hour
- **Wall-clock:** 2 hours
- **Human work:** Low
- **Risk:** If simple baselines match E74, the contribution is weakened
- **Unlocks:** Paper 1

## P8 — Generate all figures for Paper 1
- **Expected gain:** Transfer AUC matrix, cold-start curve, coverage-transfer plot, calibration curves
- **Dependencies:** P4, P5, P6, P7
- **Compute:** CPU
- **Wall-clock:** 4 hours
- **Human work:** Medium
- **Risk:** None
- **Unlocks:** Paper 1

## P9 — Draft Paper 1
- **Expected gain:** Complete draft ready for internal review
- **Dependencies:** P8
- **Compute:** None
- **Wall-clock:** 3-5 days
- **Human work:** High
- **Risk:** None
- **Unlocks:** Paper 1 (submission)

---

## P10 — Expand E78 to 100+ files across 10 public repos
- **Expected gain:** Test whether trajectory directionality generalizes beyond 6 files
- **Dependencies:** E78 instrument (already built)
- **Compute:** CPU + embedding, ~4 hours
- **Wall-clock:** 1-2 days
- **Human work:** Medium
- **Risk:** If directionality doesn't generalize, Paper 2 dies
- **Unlocks:** Paper 2

## P11 — Random-walk control for E78
- **Expected gain:** Kill condition — if random walks produce same directionality, result is geometry
- **Dependencies:** P10
- **Compute:** CPU, <1 hour
- **Wall-clock:** 1 hour
- **Human work:** Low
- **Risk:** If it kills the result, Paper 2 dies
- **Unlocks:** Paper 2 (or its refutation)

## P12 — Multiple encoders for E78
- **Expected gain:** Test whether trajectory directionality is embedder-independent
- **Dependencies:** P10
- **Compute:** CPU + embedding, ~4 hours
- **Wall-clock:** 1 day
- **Human work:** Medium
- **Risk:** If directionality is embedder-specific, scope narrows
- **Unlocks:** Paper 2

## P13 — Second rater for E21
- **Expected gain:** Test whether the proposer learned the corpus or the rater's policy
- **Dependencies:** E21 data (already exists)
- **Compute:** None (human study)
- **Wall-clock:** 1-2 weeks (recruit rater)
- **Human work:** High (recruit + supervise)
- **Risk:** If no transfer, Paper 4's external validity dies
- **Unlocks:** Paper 4

## P14 — Externalize E1-E18 to WordNet/SUMO
- **Expected gain:** Test whether cross-embedder ontology instability generalizes
- **Dependencies:** E1-E18 scripts (already exist)
- **Compute:** CPU + embedding, ~2 hours
- **Wall-clock:** 4 hours
- **Human work:** Medium
- **Risk:** If instability doesn't generalize, Paper 3 narrows
- **Unlocks:** Paper 3

## P15 — Write Paper 5 (failure modes)
- **Expected gain:** Complete methodological paper from existing evidence
- **Dependencies:** None (all evidence exists)
- **Compute:** None
- **Wall-clock:** 3-5 days
- **Human work:** High (writing)
- **Risk:** None
- **Unlocks:** Paper 5

## P16 — Write Paper 8 (self-falsifying systems)
- **Expected gain:** Complete methodological paper
- **Dependencies:** None
- **Compute:** None
- **Wall-clock:** 3-5 days
- **Human work:** High
- **Risk:** None
- **Unlocks:** Paper 8

## P17 — Write Paper 11 (null as instrument)
- **Expected gain:** Complete methodological paper
- **Dependencies:** None
- **Compute:** None
- **Wall-clock:** 2-3 days
- **Human work:** Medium
- **Risk:** None
- **Unlocks:** Paper 11

## P18 — Ethics review + release Paper 9 (dataset)
- **Expected gain:** Publish the 75,933-action corpus
- **Dependencies:** P1
- **Compute:** None
- **Wall-clock:** 1-2 weeks
- **Human work:** High (ethics review, documentation, license)
- **Risk:** Privacy concerns may block release
- **Unlocks:** Paper 9

---

## Priority Summary

| Priority | Experiment | Cost | Unlocks | Run first? |
|----------|-----------|------|---------|------------|
| P0 | Freeze E74 | 1h | 1, 9 | YES |
| P1 | Reproduce E74 | 2h | 1, 9 | YES |
| P2 | Acquire external data | 1-2d | 1 | YES (parallel) |
| P5 | Cold-start curve | 2h | 1 | YES |
| P6 | Coverage regression | 1h | 1 | YES |
| P7 | Baseline suite | 2h | 1 | YES |
| P10 | Expand E78 | 1-2d | 2 | YES |
| P11 | Random-walk control | 1h | 2 | YES (after P10) |
| P13 | Second rater | 1-2w | 4 | YES (start early) |
| P15 | Write Paper 5 | 3-5d | 5 | YES |
| P4 | Blind external repl. | 2h | 1 | After P2, P3 |
| P3 | Write converter | 4-8h | 1 | After P2 |
| P8 | Generate figures | 4h | 1 | After P4-P7 |
| P9 | Draft Paper 1 | 3-5d | 1 | After P8 |
| P12 | Multiple encoders | 1d | 2 | After P10 |
| P14 | Externalize E1-E18 | 4h | 3 | When time permits |
| P16 | Write Paper 8 | 3-5d | 8 | When time permits |
| P17 | Write Paper 11 | 2-3d | 11 | When time permits |
| P18 | Release dataset | 1-2w | 9 | After P1 |

---

## Recommended execution order

**Week 1:**
- Day 1: P0, P1, P2 (parallel)
- Day 2: P5, P6, P7 (parallel, on E74 corpus)
- Day 3: P3, P4 (blind external replication)
- Day 4-5: P8, P9 (figures + draft Paper 1)

**Week 2:**
- Day 1-2: P10 (expand E78 to 100+ files)
- Day 3: P11 (random-walk control)
- Day 4: P12 (multiple encoders)
- Day 5: P15 (write Paper 5)

**Week 3:**
- Day 1-3: P16 (write Paper 8)
- Day 4-5: P17 (write Paper 11)

**Week 4:**
- P13 (second rater — started in week 1, completes)
- P14 (externalize E1-E18)
- P18 (dataset release)

**Total wall-clock:** ~4 weeks to have Papers 1, 2, 4, 5, 8, 9, 11 ready or near-ready.
