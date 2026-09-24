# P4 — second rater (model-based, secondary evidence)

STATUS: preliminary. Harness verified; result recorded with two caveats that
bound what it can be used for.

## HYPOTHESIS

If the 57-cell filing is a property of the entries, two independent raters
should agree on them at a rate well above chance. If it is a property of the
rater's policy, agreement will be low — and then P4's geometry result is a
result about one policy, not about the cells.

## KILL TEST

Low inter-rater agreement kills "geometry captures something shared across
raters". This is that test, run with a second *model* (the human rater is
unavailable).

## NULL

Chance agreement under the two raters' own marginals:
pe = sum_c p1(c) * p2(c).

## DATA

`research/perspective-discovery/refiling_gate0.tsv`, 661 entries, 57 cells.
Sample: 57 entries, stratified one per rater-1 cell, seed 20260924.
Rater 1 = the existing Claude filing (`new_cell`).
Rater 2 = `mistralai/mistral-nemotron` via an OpenAI-compatible endpoint,
temperature 0, **blind**: the prompt builder takes only (entry, source, cells)
and has no parameter for either label. Verified on the real payload — 228
checks, 0 leaks.

## RESULT

39 of 57 items rated (18 transport failures, spread across the whole item range,
not a rate-limited tail).

    observed agreement   po = 5/39 = 0.128
    chance agreement     pe = 0.0178
    Cohen's kappa        = 0.112          ("slight")

Rater-2 behaviour (the check that separates "task is ambiguous" from "rater 2 is
broken"):

| | distinct cells | entropy | most used |
|---|---|---|---|
| rater 1 | 39 of 39 | 3.66 nats | one item each (by construction) |
| rater 2 | 19 of 39 | 2.65 nats | STUDY/LEARN 9, BOND/MAINTAIN 5, BOND/GUIDE 3 |

Rater 2 is **concentrated, not uniform** — it engages the task with strong
preferences rather than answering at random. So the low agreement is not simply
a broken rater.

## VERDICT

**Two models agree on about one filing in eight.** The filing is substantially
rater-specific, so the ceiling for "geometry captures the cell" is low. This
weakens any reading of P4 as discovering a rater-independent structure, and
supports the paper's existing framing: the target is a *rater-assigned cell*, not
a correct one.

## LIMITATIONS (both must travel with the number)

1. **Capability confound.** Rater 1 is a frontier model; rater 2 is much
   smaller. Part of the disagreement may be rater-2 weakness rather than task
   ambiguity. A fair ceiling test needs a rater of comparable capability — a
   second frontier model, or the human rater. Two models agreeing is agreement
   between two policies; it is not a human ceiling.
2. **The stratified sample inflates kappa.** Drawing one item per rater-1 cell
   forces rater 1 to be uniform, which drives pe down to ~1/57, the minimum
   possible, and therefore makes kappa look *better* than it would on a natural
   sample. kappa = 0.112 is an upper-ish bound, not a neutral estimate.
3. 18 of 57 items failed on transport and are unrated. Failures are spread
   across the item range, so the rated subset is not obviously biased — but the
   arms are unequal, and a rerun should retry them (the harness now does).

## REPRODUCTION

    python3 research/papers/p4_second_rater.py --n 60 --backend nvidia

Artifact: `research/p4_rater2.json`.

## ARTIFACT-INTEGRITY NOTES

Two defects found by reading this artifact rather than its summary:

- `rater2_raw` was stored as `raw[:80]`. Labels were parsed from the full text,
  so they are sound, but the evidence for them was destroyed — a row whose
  stored raw contained no cell name still carried a cell, which is impossible if
  the stored raw is what was parsed. Now stores `raw[:2000]` plus `raw_len`.
  **Rows from the run above predate this fix, so their raws are truncated and
  must not be treated as auditable evidence.**
- `_load_partial` counted `REQUEST-FAILED` rows as done, so a resume silently
  skipped items that were never asked. Now excluded, so a rerun retries them.

## GIT COMMIT

Recorded at the commit that adds this file.
