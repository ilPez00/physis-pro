# Final adversarial review

Four simulated reviewers per live manuscript, run 2026-09-25 against HEAD-era
artifacts. For each headline claim the test is: *what current artifact could
contradict this exact sentence?* Where one exists the sentence was fixed; where
none supports the claim it was narrowed or marked exploratory.

## C — Operational Failure Priors Are Pair-Specific (paper1_failure_priors)

| Claim | Contradicting artifact? | Action |
|---|---|---|
| "Two pairs transfer, in both directions" (cline↔opencode 0.665/0.635) | None. `p1_all_targets_coldstart.json` lists exactly those in `pairs_where_borrowing_helps`. | Keep. |
| "No positive-direction pair survives Holm" | None. `holm_survivors_above_chance` is empty. | Keep. |
| "(an unqualified) no pair shows positive transfer" | **Yes** — the same artifact sets `no_pair_benefits: false`. | Removed; archived P1a carries it, and the validator now fails the build if it returns. |
| "shrinkage stops beating target-global by N≈3–5" | None. Artifact medians: +0.0116, +0.0056, +0.0040, −0.0005, −0.0014. | Keep. |

**Statistics reviewer:** satisfied by the explicit separation of partition
frequency from family-wise rejection, and the worked example (a pair that passes
the first, fails the second).
**Hostile reviewer:** will ask whether Cline/OpenCode reciprocity is an agent
property or an artifact of both targeting the same repositories. **This is the
weakest unresolved assumption and it is not addressed by any current artifact.**

## D — Propose, Do Not Certify (paper4_propose_certify)

| Claim | Contradicting artifact? | Action |
|---|---|---|
| geometry is a useful proposer, uninformative certifier | None. | Keep. |
| "certifier stays at chance under both protocols" | **Yes** — the certifier has one measurement, no sequential rerun. | Removed this pass. |
| "+0.576" | **Yes** — arithmetic from no sentence; verified gaps are +0.402/+0.531/+0.522. | Removed; checker bug that blessed it also fixed. |
| cross-rater κ = 0.202 | None (artifact). | Keep, and now calibrated against published annotation agreement, where it is *low*. |
| "human decision leverage" | Framing risk — raters are models. | Qualified to decision-support potential; model≠human stated in Limits. |

**Reproducibility reviewer:** the join key (`n`, not name) and the first-five-split
reproduction of the published E21 row are both disclosed. Good.
**Domain reviewer:** asks for a human rater. None exists; the paper says so.

## F — Empedocles (paper10_empedocles)

| Claim | Contradicting artifact? | Action |
|---|---|---|
| "no positive control was run" | **Yes** — `p10_controls.json`, 12/12 detected. | Replaced with the control battery this pass. |
| "finite-size analysis remains future work" | **Yes** — `p10_fss.json`, N=100–3000. | Replaced with the FSS section this pass. |
| a critical exponent is available | **Yes** — peak at sweep boundary at every N, χ/N slope 0.997, width broadening. | Explicitly **no exponent reported**, with the three blocking diagnostics named. |
| "the true distribution is lognormal" | Not claimed. Controls show the pipeline cannot separate the families here. | Language checked to stay narrower than the evidence. |

**Hostile reviewer:** the instrument is now shown to be false-positive prone, so
could the refutation be circular? **No** — the bias runs toward finding power
laws, which would have *helped* the original claim; the paper states this as
conservatism rather than proof.

## A — Executable Falsification (paperM_methodology)

| Claim | Contradicting artifact? | Action |
|---|---|---|
| "nine invariants, 0 failures" | None — but the snapshot records commit `8232042`, behind HEAD. | Recorded as a named gap (provenance exists ≠ current). |
| "the answer changed sign" ×4 | Outcomes are heterogeneous: artifact / underpowered / protocol violation / reclassification. | Reworded to "overturned, reclassified, or rendered non-promotable". |
| the workflow "catches" violations | **Yes** — Case 12 shows it records them. | Already stated honestly in the paper. |

**Reproducibility reviewer:** satisfied by I8's stripped-record probe — the suite
is shown to be capable of failing, which is what makes "0 failures" meaningful.
**Domain reviewer:** the new section is now positioned against the verification-gap
survey (83% release code, 38% release traces, 38% report novelty verification).

## B — Multi-Harness Corpus (paper9_dataset)

| Claim | Contradicting artifact? | Action |
|---|---|---|
| "the corpus is valuable" on size grounds | **Yes** — SWE-chat is ~10× more sessions. | Conceded in the abstract, related work, and conclusion. |
| action-kind granularity is load-bearing (AUC 0.627–0.751 vs 0.446–0.525) | None. | Keep. |
| per-action outcome labels | None; comparators confirmed to label at turn/run/task level. | Narrowed novelty claim to this. |

**Related work:** TraceLab could not be located; recorded as an omission rather
than silently dropped.

## E — Semantic Trajectories (paper2_trajectories)

**Blocked.** The external corpus exists (10 repos) but the analysis artifact does
not. No claim in this manuscript is currently supported by a result file, so it
is not reviewed here; it must not be submitted until `p2_analysis.py` completes
and its output is inspected against the A–E decision rule.

## Cross-cutting
- Every headline number in C, D, F and M traces to a stored artifact.
- Remaining unverifiable numbers: E5's 0.447 and E24's 0.712, both disclosed as
  unreproducible in P4's provenance section.
- No manuscript in the live set asserts a claim whose contradicting artifact
  exists, except E (no artifact at all) and the P1 reciprocity assumption
  (unaddressed by any experiment).
