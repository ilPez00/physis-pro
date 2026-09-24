# Cross-paper consistency

Date: 2026-09-24
Enforced by: `research/results_registry/validate.py` (16 checks, 0 failures)

This document records every place two manuscripts touched the same quantity,
what the artifact actually says, and what was changed. Numbers here are
canonical because the validator recomputes them; prose in the papers is
secondary.

---

## 1. The bootstrap unit — P1 vs P9

**What was claimed.**
- P1: the action-level bootstrap yields 2 of 12 pairs excluding chance
  (session-level: 1 of 12).
- P9: action-level resampling was "narrow enough to turn four chance-level
  comparisons into apparently significant ones."

**What the artifact says** (`bootstrap_unit_check.json`, recomputed by
`validate.py`):

| quantity | value |
|---|---|
| pairs with CI entirely **above** 0.5, session unit | **1** (`cline→opencode`) |
| pairs with CI entirely **above** 0.5, action unit | **2** (`cline→opencode`, `opencode→cline`) |
| pairs whose significance **verdict** differs between units | **4** |
| — of which gained significance | **3** |
| — of which lost significance | **1** |
| pairs with CI entirely **below** 0.5, session unit | **5** |
| pairs with CI entirely **below** 0.5, action unit | **6** |

The file's own `counts` field records `{"action": 2, "session": 1}`.

**Resolution: P1 was correct, P9 was wrong.** P9 conflated two different
counts — *verdict changed* (4) and *became apparently significant* (3) — and
described the result as if all four were positive-direction findings. In fact
two of the three that gained significance sit **below** 0.5, i.e. the action
bootstrap made them look like worse-than-chance predictors rather than
chance-level ones.

**Both corrected.** P9 now states the verdict-change count (4) and the
direction split (three gained, one lost), and names the artifact. P1's
coverage section was separately rewritten (see §2).

**A finding neither paper had made.** 5–6 of the 12 pairs are significantly
*below* chance. The headline "cross-agent priors do not transfer" is too mild:
most failures are **anti-predictive**. This belongs in P1's abstract and is
logged in the experiment plan.

## 2. P1's coverage-sufficiency claim

P1 asserted "low coverage appears sufficient to predict failure" from a
correlation whose own section reports $p > 0.05$ on $n=12$ with 11 pairs at
chance. The transfer matrix refutes the ordering twice:

- `codex→claude-code` has **lower** coverage than `codex→cline`
  (0.241 vs 0.353) and a **higher** AUC (0.489 vs 0.399).
- `cline→opencode`, the best pair in the matrix (0.534), sits at coverage
  0.375 — 0.022 above the pair cited as failing for low coverage, across
  which AUC differs by 0.135.

Corrected: coverage is reported as a diagnostic, not a gate or predictor.
(Note: 0.022 is **not** the smallest coverage gap in the matrix — that is
0.004, between `claude-code→opencode` and `opencode→claude-code`. An earlier
draft of the correction claimed otherwise and was caught by recomputation.)

## 3. The propose/certify "gap" — P4

P4 reported `top-3 containment − certification AUC` (0.668 − 0.519 = +0.149,
shrinking to +0.022 under a sequential split) as "the measurable finding".

**This subtraction is invalid.** Containment in $\{0,1\}$ over 57 candidate
cells and ROC AUC are different metrics on different scales. Changing the
candidate set from 3 to 10 moves the first term and leaves the second
untouched, so their difference has no stable interpretation.

**Removed.** Both tasks are now stated against their own matched null:

| task | score | own null | margin |
|---|---|---|---|
| proposer (top-3 containment) | 0.668 | 0.159 (permuted labels, 1000 shuffles) | +0.509 |
| certifier (ROC AUC) | 0.519 | 0.5 (chance) | +0.019, indistinguishable from none |

The qualitative claim survives — the proposer clears its null, the certifier
does not — expressed as decision leverage (57 cells → 3 candidates containing
the right one 54–67% of the time). The numerical gap does not. Registry claim
`PROP-003` carries status `superseded`.

**A fabricated number was caught here.** While rewriting this section I wrote
that the sequential protocol has "a matched null of 0.265" and clears it "by
+0.28". No sequential permuted null existed; 0.265 is the *random* split's
top-**5** null, read from the wrong row. Removed from the abstract and
discussion, replaced with an explicit statement that the arm was missing, and
the arm is now implemented and running.

## 4. The permutation null — P4

The published null averaged **5** shuffles. Re-run at **1,000** (same code
path, same construction): 0.136 → **0.159** top-3. The published null
understated the noise, so the gap narrowed accordingly (+0.531 → +0.509).
Paper corrected; table left at five seeds so its rows stay internally
consistent, with the re-estimate stated in text.

## 5. The sequential split — P4

Random-split figures inflated top-3 by ≈ +0.13 (0.668 → 0.541 sequential) and
the learning curve is **not** monotone in file order (0.345 → 0.577 → 0.541).
P4's claim that the curve "is still climbing at 436" was withdrawn.

`refiling_gate0.tsv` has a sequence column but **no timestamps**, so this is a
*sequential* split, not a demonstrated chronological one. Stated in-paper.

## 6. Paper M's case numbers

- E83: recall quoted as 0.998 — that is the F1; actual recall is **0.996**.
  Corrected in text and in the catalogue table.
- E85: prose said "four of fifteen fired" while the artifact
  (`e85-adversarial-results.json`) records **five** rule firings across the
  eight rule types. The artifact has no pair-level count, so the pair figure
  is not recoverable from it; the discrepancy is now disclosed rather than
  tidied away.

## 7. Quantities checked and found consistent

| quantity | P1 | P9 | artifact |
|---|---|---|---|
| actions | 75,933 | 75,933 | 75,933 ✓ |
| sessions | 402 | 402 | 402 ✓ |
| median actions/session | 101 | 101 | 101 ✓ |
| harnesses / machines | 4 / 3 | 4 / 3 | 4 / 3 ✓ |
| distinct kinds | 871 | 871 | 871 ✓ |

Both papers previously said median 137; both corrected.

## 8. Standing rule

Any number appearing in two manuscripts must exist as a claim in
`results_registry/claims.json` with a resolving `json_path`. `validate.py`
fails the build if a claim's stated value drifts from its artifact, or if a
paper reasserts a value that another paper has superseded. Add a claim before
copying a number.
