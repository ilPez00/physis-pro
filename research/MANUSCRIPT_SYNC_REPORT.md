# Manuscript Sync Report

Pass date: 2026-09-25 · HEAD at start: `6462fd5` · Branch: `agent/P-037-substrate`

## What this pass changed (all verified against artifacts)

### 1. P1 + P1b merged into one canonical paper  ✅ DONE
- **Inspection found P1b was already the merged paper in substance**: it carried
  both readings, the unresolved tension, the corrected Holm denominator
  (`0.05/8 = 0.00625`), the WITHDRAWN-flagged leaked cold-start figure, the
  all-pairs experiment, and the shrinkage test. P1a was the one-sided
  global-negative variant.
- **Action (small diff on the right file):** promoted `paper1b_priors_pair_specific`
  → `paper1_failure_priors.tex`, retitled to *"Operational Failure Priors Are
  Pair-Specific Across Coding-Agent Harnesses"*, and rewrote the abstract to
  lead with the balanced canonical interpretation covering all five facts
  (in-corpus signal, Holm/BH, repeated partitions, all-pairs reciprocity,
  shrinkage). Removed the now-false "companion manuscript reverses that
  emphasis" sentence.
- Both originals archived with `git mv` (history preserved):
  `archive/superseded/paper1_global_negative/`,
  `archive/superseded/paper1_pair_specific/`.
- Every live reference repointed: `build_all.sh`, `verify_paper1.py`,
  `validate.py`, and 9 claims in `results_registry/claims.json`.
- **Gate: 37 checks / 0 failures**, including "paper1 states the
  no-positive-transfer result" and "paper1 no longer asserts an established
  pair". Compiles clean, 12 pages.

### 2. P4 wording contradictions removed  ✅ DONE
- `"Ground truth is the rater's revised cell"` → `"The target label is the
  rater-assigned revised cell"`.
- `"That is a statement about human decision leverage"` → qualified to
  decision-support potential for a filer whose policy resembles the training
  rater's, explicitly **not** a claim about human decision leverage in general.
- Remaining instances of "truth"/"human" are deliberate disclaimers ("not
  evidence about ontology truth", "a human rater remains the experiment that
  would settle it") and were verified as such, not removed.
- Compiles clean, 7 pages.

### 3. P3 marked superseded  ✅ DONE
Front-page boxed banner: "HISTORICAL RESEARCH LINEAGE — SUPERSEDED BY 'PROPOSE,
DO NOT CERTIFY'", plus `research/P3_SUPERSESSION.md` with the stale-number table.

### 4. Authorship standardised  ✅ DONE
`Giovanni Pezzin \\ Physis Research Program` across all 7 live papers. Three
papers previously read `\author{Giovanni Pezzin}` with no program.

### 5. Deliverable docs  ✅ DONE
`MANUSCRIPT_SYNC_STATE.md`, `MANUSCRIPT_SYNC_REPORT.md`,
`CANONICAL_PORTFOLIO.md`, `CROSS_PAPER_CONSISTENCY_V3.md`,
`SUPERSEDED_PAPERS.md`, `FINAL_CLAIMS_REGISTRY.md`, `P3_SUPERSESSION.md`.

## What was verified (artifact → claim)

| Artifact | Verified |
|---|---|
| `p1_multiplicity_v2.json` | family_size 12; **0** above-chance Holm survivors; **3** below-chance; `0.05/8 = 0.00625` rank-5 logic present |
| `p1_all_targets_coldstart.json` | cline←opencode **0.665**, opencode←cline **0.635**; `pairs_where_borrowing_helps` = exactly those two; `no_pair_benefits: false`; mixing helps for **no** pair |
| `p1_shrinkage_prior.json` | shrinkage−global median **+0.0116** (N=1) → **+0.0040** (N=3) → **−0.0005** (N=5) → **−0.0014** (N=30) |
| `p10_controls.json` | positive branching **12/12**, α **1.4968**; positive percolation **10/12**; **3** negative-control false positives; sensitive=True, specific=False |
| `p10_fss.json` | N = 100/300/1000/3000; χ/N slope **0.9967**; width slope **+0.7939**; peak at sweep boundary at every N; classifier says `NO TRANSITION LOCATED` |
| `methodology_validation.json` | **9** invariants, **0** failures |
| `e21_cross_rater_results.json` | κ **0.2019** [0.168, 0.235]; agreement **0.2302**; cross-rater top-3 **0.367/0.319** vs same-rater **0.673/0.608** |

## NOT done — carried to the next pass (with evidence already in hand)

| # | Item | Why it was not done here |
|---|---|---|
| 1 | **P10 rewrite** (controls + FSS + no-exponent explanation) | Manuscript still asserts "no positive control was run" and "finite-size analysis remains future work" — both now **false**. All numbers are verified and sitting in `FINAL_CLAIMS_REGISTRY.md`; the rewriting itself is a substantial manuscript job. |
| 2 | **PaperM verifier-falsification section** | Same — evidence verified (9/0 invariants, classifier_version 2 fixing the false "consistent with critical scaling" verdict), section not yet authored. |
| 3 | **P9 literature positioning** | Requires a live literature pass against SWE-bench/SWE-agent/SWE-chat/TraceLab; not started. |
| 4 | **P2** | External corpus **is built** (`trajectories_external/corpus_manifest.json`, 10 repos) but **no analysis artifact exists** and no process is running. Per the decision rule, P2 was left untouched. Claims remain unproven. |
| 5 | **Figure hygiene** | `fig2_cold_start_curve.pdf` is still a leaked figure on disk; needs an in-caption WITHDRAWN mark. `fig1`/`fig3` captions need inference-unit/n=12 labels. |
| 6 | **`RELATED_WORK_AUDIT.md` × 5** | Not started. |
| 7 | **Adversarial review** | Not performed. |
| 8 | **`research/papers/final/`** | Not created; historical PDFs retained in place. |

## Two things a hostile reviewer should know

1. **The P1 "merge" was not a rewrite.** P1b already integrated both framings
   and the tension; writing a third merged manuscript would have been
   duplication. The change was a retitle, a rewritten abstract, archival of the
   one-sided variant, and repointing every reference.
2. **`p1_all_targets_coldstart.json` sets `no_pair_benefits: false`.** The
   archived P1a headline ("no pair shows positive transfer") is therefore
   contradicted by the canonical artifact — which is exactly why the canonical
   paper may not lead with it, and why the validator now fails the build if that
   framing returns.
