# MANUSCRIPT SYNCHRONIZATION STATE

## Current Commit
6462fd5 P4: cross-rater proposer arms -- shared at the axis, rater-specific at the cell

## Papers Found (with latest artifact per paper)

| Paper | .tex | .pdf | Result JSON | Status |
|-------|------|------|-------------|--------|
| paper1_agent_priors | research/papers/paper1_agent_priors.tex | research/papers/paper1_agent_priors.pdf | research/p1_proposer_results.json | P1+P1B merge needed |
| paper1b_priors_pair_specific | research/papers/paper1b_priors_pair_specific.tex | research/papers/paper1b_priors_pair_specific.pdf | research/p1_shrinkage_prior.json | P1+P1B merge needed |
| paper4_propose_certify | research/papers/paper4_propose_certify.tex | research/papers/paper4_propose_certify.pdf | research/e21_cross_rater_results.json | Second-rater results integrated |
| paper2_trajectories | research/papers/paper2_trajectories.tex | research/papers/paper2_trajectories.pdf | research/impossible_machine/... | External replication pending |
| paper3_ontology_instability | research/papers/paper3_ontology_instability.tex | research/papers/paper3_ontology_instability.pdf | research/empedocles_*.json | Mark superseded |
| paper10_empedocles | research/papers/paper10_empedocles.tex | research/papers/paper10_empedocles.pdf | research/e15_e18_rerun.json | Update controls + FSS |
| paper9_dataset | research/papers/paper9_dataset.tex | research/papers/paper9_dataset.pdf | research/p1_all_targets_coldstart.json | Literature audit |
| paperM_methodology | research/papers/paperM_methodology.tex | research/papers/paperM_methodology.pdf | research/methodology_validation.json | Verifier falsification evidence |

## Stale Manuscript Sections

- paper1_agent_priors: "no regime borrowing helps" global claim (now qualified pair-specific)
- paper1b_priors_pair_specific: opposite framing retained as archived superseded
- paper4_propose_certify: withdrawn "+0.576" number, withdrawn "curve still climbing", qualified certifier wording
- paper10_empedocles: states "no positive control was run" and "finite-size analysis remains future work" — both now false
- paper2_trajectories: "software objects are trajectories" / "2–3 dimensional" provisional claims
- paper3_ontology_instability: historical ontology story, should be marked superseded

## Superseded Papers (archive, do NOT delete)

- paper1_agent_priors (old global-negative framing) → archive/superseded/paper1_global_negative/
- paper1b_priors_pair_specific (old pair-specific framing) → archive/superseded/paper1_pair_specific/
- paper3_ontology_instability → mark with front-page "HISTORICAL RESEARCH LINEAGE — SUPERSEDED BY 'PROPOSE, DO NOT CERTIFY'"

## Running Experiments
- None currently active. P2 process not found. P4 second-rater analysis complete.

## Blockers
- P2 external replication (10 public repos, hundreds of files, Nomic embeddings) — not running; will be addressed in step 7
- P1/P1B merge requires reconciling opposite framings into canonical paper
- paper10 controls/finite-size analysis requires regenerating from artifacts
- paperM methodology evidence (ledger invariants, verifier falsification) must be integrated

## Planned Changes (execution order per the MISSION)

1. Merge P1/P1b into canonical paper "Operational Failure Priors Are Pair-Specific Across Coding-Agent Harnesses"
2. Archive old P1 variants
3. Update P4 from canonical second-rater artifacts (cross-rater generalization)
4. Mark P3 superseded
5. Update P10 from controls + finite-size scaling
6. Update paperM with ledger/verifier failure evidence
7. Update P9 literature positioning against current trace corpora
8. Wait/check P2 (not running; skip or complete externally)
9. Standardize authorship across all papers
10. Update result registry
11. Run all build/consistency gates
12. Adversarial review of each live manuscript
13. Rebuild final PDFs
12. Commit all changes with clear research commits

## Canonical P1 Result (from merge)

A. IN-CORPUS SIGNAL: Per-action-kind failure priors carry useful signal within several harnesses.

B. SIMPLE CROSS-HARNESS TRANSFER: Under session-level inference and Holm correction over 12 directional tests — no positive-direction pair survives Holm; three below-chance pairs survive Holm; Cline→OpenCode is nominally positive / BH-dependent but not family-wise established.

C. REPEATED COLD-START PARTITIONS: Across leak-free repeated partitions — Claude Code target from OpenCode source does not benefit; local target history dominates quickly; the previous single-split cold-start claim was not an artifact after correction for that pair.

D. ALL TARGET-SOURCE PAIRS: Cline←OpenCode and OpenCode←Cline are the only reciprocal pair showing borrowed priors above chance in a majority of partitions; most other pairs do not benefit.

E. SHRINKAGE: Cross-agent information as shrinkage provides at most a marginal benefit around N=1 and stops beating target-global shrinkage by roughly N=3–5.

Statistical language preserves the distinction between point estimate, bootstrap interval, nominal p-value, BH result, Holm result, and repeated-partition frequency. Do not conflate "above chance in a majority of partitions" with "statistically established after family-wise correction." Holm rank-5 threshold = 0.05/8 = 0.00625.

Cold-start figure from leakage: label withdrawn/leaked, preserve only as error history. Prefer new figures from leak-free repeated-partition results.

## P4 Canonical Interpretation

- geometry works as a proposer
- not as a certifier
- random split inflates absolute score
- sequential split remains above its matched null
- crossed-rater performance is substantially lower
- most fine-grained 57-cell structure is rater-specific
- some coarser axis-level signal appears to transfer
- second rater is another language model, not a human
- state that clearly; do not imply model-model agreement estimates human agreement

## P10 Update Required

The manuscript currently states:
- "no positive control was run" — FALSE; positive control detected 12/12
- "finite-size analysis remains future work" — FALSE; N=100/300/1000/3000 analysis completed

Must incorporate:
A. Positive control (critical Poisson branching; exponent ~1.5)
B. Second positive/physical control (percolation if present)
C. Negative controls (wide-lognormal false positives; instrument sensitive but not sufficiently specific)
D. Empedocles refutation strengthens: method biased toward identifying power-law-like behavior in some heavy-tailed non-power-law controls; Empedocles still never produces a significant power-law win; therefore the refutation is conservative w.r.t. this bias; result: "the original power-law/SOC claim is unsupported and does not survive the fitting/control battery"

## P3 Supersession

Must add front-page status: "HISTORICAL RESEARCH LINEAGE — SUPERSEDED BY 'PROPOSE, DO NOT CERTIFY'"
Create research/P3_SUPERSESSION.md explaining what was superseded, which numbers are stale, and where canonical results live.

## P2 Decision

External replication not running; when/if it completes, inspect artifact before modifying P2. Decision rule: A-E based on results (semantic embeddings vs text-overlap, identity vs direction, TF-IDF/edit distance explanation, intrinsic dimension, practical extrapolation value).

## Figure Hygiene

- transfer matrix: verify row/column direction explicitly
- coverage-vs-AUC: label correlation as exploratory, note n=12
- old cold-start curve: label WITHDRAWN/LEAKED
- all figures must correspond to current results; stale figures must be marked

## Literature Audit Needed

For each live paper, create research/papers/<paper>/RELATED_WORK_AUDIT.md:
- P1: operational failure prediction, cross-agent transfer, tool-use reliability, agent telemetry
- P4: human/model-in-the-loop ontology classification, embedding-based candidate generation, inter-rater reliability, representation instability
- P9: coding-agent traces / trajectory datasets
- P10: SOC inference, heavy-tail model comparison, finite-size scaling, false identification of power laws
- M: autonomous research agents, AI Scientist systems, automated verification/falsification, preregistration / research provenance, scientific-agent verification gap

## Canonical Portfolio

Live:
A. Executable Falsification for AI-Assisted Research
B. Multi-Harness Coding-Agent Action Corpus
C. Operational Failure Priors Are Pair-Specific Across Coding-Agent Harnesses
D. Propose, Do Not Certify
E. Semantic Trajectories — only if external replication supports a paper
F. Empedocles — standalone only if expanded control/finite-size analysis is substantial enough

Archive/supersede:
- P3
- old P1 variants
- older failed manuscripts
- withdrawn versions

NO DELETIONS.

## Build / Validation Gates

After manuscript updates, run:
- results registry validator
- ledger invariant validator
- release/privacy audit
- paper consistency validator
- LaTeX build for all live papers
- broken citation check
- undefined reference check
- stale-number check
- superseded-result check

Fail the build if a known-withdrawn claim returns.

## Adversarial Review

For each live manuscript, simulate statistics reviewer, domain reviewer, hostile reviewer, reproducibility reviewer. For every headline claim ask: "What current artifact could contradict this exact sentence?" If one exists: fix the sentence. If no artifact supports the claim: remove or mark exploratory. Do not defend prose against data.

## Deliverables (to create)

- research/MANUSCRIPT_SYNC_STATE.md (this file)
- research/MANUSCRIPT_SYNC_REPORT.md
- research/CANONICAL_PORTFOLIO.md
- research/CROSS_PAPER_CONSISTENCY_V3.md
- research/SUPERSEDED_PAPERS.md
- research/FINAL_CLAIMS_REGISTRY.md

Rebuild PDFs into research/papers/final/ (or equivalent). Do not overwrite historical PDFs without retaining them.

## Execution Order

1. inspect live state ✅ (done)
2. verify whether P2 is running ✅ (not running)
3. do NOT disturb P2 ✅
4. merge P1/P1b
5. archive old P1 variants
6. update P4 from canonical second-rater artifacts
7. mark P3 superseded
8. update P10 from controls + finite-size scaling
9. update M with ledger/verifier failure evidence
10. update P9 literature positioning
11. wait/check P2
12. if P2 finished, analyze and update it
13. standardize authorship
14. update result registry
15. run all build/consistency gates
16. adversarial review
17. rebuild final PDFs
18. commit all changes with clear research commits

Suggested commits:
- papers: merge failure-prior manuscripts into canonical P1
- paper4: integrate crossed-rater generalization
- paper3: mark historical lineage superseded
- paper10: integrate controls and finite-size refutation
- methodology: add verifier-falsification evidence
- paper9: sharpen dataset positioning against current trace corpora
- paper2: integrate external replication
- research: synchronize canonical claims and manuscript registry
- papers: rebuild submission portfolio

============================================================
STATUS AFTER THIS PASS (2026-09-25)
============================================================

DONE:  P1+P1b merged -> papers/paper1_failure_priors.tex (12pp, gate green 37/0)
       P1 variants archived via git mv; all live refs repointed (build_all.sh,
       verify_paper1.py, validate.py, 9 registry claims)
       P4 wording contradictions removed; abstract qualified
       P3 front-page supersession banner + P3_SUPERSESSION.md
       authorship standardised across all 7 live papers
       all 7 live papers compile clean (0 errors)
       deliverables: MANUSCRIPT_SYNC_REPORT.md, CANONICAL_PORTFOLIO.md,
       CROSS_PAPER_CONSISTENCY_V3.md, SUPERSEDED_PAPERS.md,
       FINAL_CLAIMS_REGISTRY.md, P3_SUPERSESSION.md

OPEN:  P10 rewrite (controls + FSS) -- manuscript still denies experiments that exist
       PaperM verifier-falsification section
       P9 literature positioning; 5x RELATED_WORK_AUDIT.md
       P2 untouched (corpus built, no analysis artifact, no process running)
       figure hygiene (fig2 is a leaked figure still on disk)
       adversarial review; research/papers/final/
       ledger provenance behind HEAD (8232042 vs 6462fd5)

SUPERSEDED: paper3_ontology_instability -> banner added; not maintained.