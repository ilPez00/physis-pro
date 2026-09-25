# Final Claims Registry

Every headline claim, its canonical artifact, and its verification status.
Values below were read from the artifact files in this pass, not from prose.

## C. Operational Failure Priors Are Pair-Specific (paper1_failure_priors)

Artifact set: `p1_all_targets_coldstart.json`, `p1_repeated_splits.json`,
`p1_multiplicity_v2.json`, `p1_shrinkage_prior.json`, `e74_replication_results.json`

| Claim | Value | Artifact | Verified |
|---|---|---|---|
| Corpus | 75,933 actions, 402 sessions, 4 harnesses, 3 machines | `e74_replication_results.json` | preamble (registry) |
| Corpus size cross-check | 585 blind lines / 661 rows in gate0 (independent set) | — | n/a |
| Reciprocal pair — cline←opencode | borrowed prior above chance in **0.665** of 200 partitions | `p1_all_targets_coldstart.json` | ✅ read |
| Reciprocal pair — opencode←cline | **0.635** | `p1_all_targets_coldstart.json` | ✅ read |
| Claude Code target from OpenCode | max **0.05** of 200 partitions (prose: 4.2% of 500 in `p1_repeated_splits`) | both | ✅ read |
| Pairs where borrowing helps | `["cline<-opencode","opencode<-cline"]`; `no_pair_benefits: false` | `p1_all_targets_coldstart.json` | ✅ read |
| Pairs where mixing consistently helps | `[]` (none) | `p1_all_targets_coldstart.json` | ✅ read |
| Holm over 12 tests | family_size 12; above-chance survivors **[]**; below-chance survivors **3** | `p1_multiplicity_v2.json` | ✅ read |
| Holm threshold logic | rank 5 of 12 → `0.05/(12-5+1) = 0.05/8 = 0.00625` | `p1_multiplicity_v2.json` | ✅ read |
| cline→opencode | nominal p 0.0094 > 0.00625 → not Holm; BH survivors 6 | `p1_multiplicity_v2.json` | ✅ read |
| Shrinkage vs target-global, N=1 | median **+0.0116** | `p1_shrinkage_prior.json` | ✅ read |
| Shrinkage vs target-global, N=3 / N=5 | **+0.0040** / **-0.0005** | `p1_shrinkage_prior.json` | ✅ read |
| Shrinkage vs target-global, N=30 | **-0.0014** | `p1_shrinkage_prior.json` | ✅ read |

**Strongest surviving claim:** cross-harness transfer is weak, pair-specific, and
generally not robust under family-wise inference; Cline and OpenCode show
reciprocal cold-start structure; cross-agent information as shrinkage gives at
most a marginal benefit around N=1 and is spent by roughly N=3–5.

**Weakest unresolved assumption:** that Cline/OpenCode reciprocity is a property
of the agents rather than of their overlapping task mix (both target the same
repo conventions).

**Last kill test:** leak-free repeated partitions (`p1_repeated_splits.json`, 500
partitions, n_train=30). **Submission blocker:** none — merged and gated.

## D. Propose, Do Not Certify (paper4_propose_certify)

Artifact set: `e21_cross_rater_results.json`, `e21_sequential_split.json`,
`e21_permutation_null_1000.json`, `e15_e18_rerun.json`

| Claim | Value | Artifact | Verified |
|---|---|---|---|
| Same-rater top-3, random split @436 | 0.673 (null 0.159, margin +0.514) | `e21_cross_rater_results.json` | ✅ read |
| Sequential top-3 @0.75 | 0.541 (own null 0.107, margin +0.434) | `e21_sequential_split.json` | ✅ read |
| Cross-rater top-3 @436 (A→B / B→A) | 0.367 / 0.319 | `e21_cross_rater_results.json` | ✅ read |
| Cross-rater margins | +0.240 / +0.209 | `e21_cross_rater_results.json` | ✅ read |
| Exact-cell agreement | 0.2302 | `e21_cross_rater_results.json` | ✅ read |
| Cohen's κ (exact cell) | 0.2019 [0.168, 0.235], n=582 | `e21_cross_rater_results.json` | ✅ read |
| Cell vocab overlap | Claude 57, DeepSeek 42, shared 42, Claude-only 15, DeepSeek-only 0 | `e21_cross_rater_results.json` | ✅ read |
| Coarse axis transfer (mode) | 0.675 / 0.668 vs 0.817 / 0.813 within-rater | `e21_cross_rater_results.json` | ✅ read |
| Certifier | AUC 0.519 vs analytic 0.5 null, n=574, **one protocol only** | `e15_e18_rerun.json` | ⚠️ single measurement; no sequential rerun exists |

**Strongest surviving claim:** embedding geometry is a useful proposer and an
uninformative certifier; the random split inflates the absolute level; most
fine-grained 57-cell structure is rater-specific while coarse axis-level
structure transfers.

**Weakest unresolved assumption:** both raters are language models; model-model
agreement does **not** estimate human agreement, and no human rater exists.

**Last kill test:** the crossed arms themselves. **Submission blocker:** none.

## F. Empedocles (paper10_empedocles)

Artifact set: `p10_controls.json`, `p10_fss.json`, `empedocles_multiseed.json`, `empedocles_fits.json`

| Claim | Value | Artifact | Verified |
|---|---|---|---|
| Positive control (critical branching) | detected **12/12**, median α **1.4968** (theory 1.5) | `p10_controls.json` | ✅ read |
| Second positive control (2D percolation at pc) | detected **10/12** (0.833) | `p10_controls.json` | ✅ read |
| Negative control false positives | **3**: subcritical branching (0.75), wide-lognormal s1.2 (1.0), s2.0 (1.0) | `p10_controls.json` | ✅ read |
| Negative controls correctly quiet | 2: supercritical branching, lognormal s0.6 (0.917 lognormal wins) | `p10_controls.json` | ✅ read |
| Instrument sensitivity / specificity | sensitive **True** / specific **False** | `p10_controls.json` | ✅ read |
| FSS node counts | 100, 300, 1000, 3000 (seeds 16/16/16/8) | `p10_fss.json` | ✅ read |
| Susceptibility scaling | χ/N log-log slope **0.9967** vs N (trivial) | `p10_fss.json` | ✅ read |
| Transition width | slope **+0.7939** (broadens, does not sharpen) | `p10_fss.json` | ✅ read |
| Transition location | peak at sweep boundary at every N → `NO TRANSITION LOCATED` | `p10_fss.json` | ✅ read |
| Critical exponent | **none reported — not interpretable** | `p10_fss.json` | ✅ read |

**Strongest surviving claim:** the original power-law/SOC claim is unsupported
and does not survive the fitting/control battery. The instrument is sensitive
but not specific, so the refutation is *conservative* w.r.t. that bias.

**Weakest unresolved assumption:** fixed interaction radius while density changes
is an unmodelled confound in the sweep.

**Submission blocker:** the manuscript still asserts "no positive control was
run" and "finite-size analysis remains future work" — both now false; not yet
rewritten in this pass.

## A / B / E — status
- **A (paperM_methodology):** 9 ledger invariants, 0 failures
  (`methodology_validation.json`, recorded at commit 8232042 — behind HEAD
  `6462fd5`). Verifier-falsification section not yet written.
- **B (paper9_dataset):** corpus counts in registry; related-work audit not yet run.
- **E (paper2_trajectories):** external corpus **built** (10 repos;
  `trajectories_external/corpus_manifest.json`) but the analysis artifact does
  **not** exist. Claims "software objects are trajectories" and "2–3
  dimensional" remain **unproven** — do not assert.
