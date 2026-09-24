# P4 — independent raters (model-based, secondary evidence)

STATUS: complete for the model-rater arm. Human rater still unavailable.

## HYPOTHESIS

If the 57-cell filing is a property of the entries, independent raters should
agree well above chance. If it is a property of the rater's policy, agreement
will be low — and then P4's geometry result is a result about one policy, not
about the cells.

## KILL TEST

Low inter-rater agreement kills "geometry captures something shared across
raters". This is that test.

## NULL

Chance agreement under the two raters' own marginals:
pe = sum_c p1(c) * p2(c).

## DATA

`research/perspective-discovery/refiling_gate0.tsv`, 661 entries, 57 cells.
Rater 1 = the existing Claude filing (`new_cell`).

| rater 2 | model | n | sample |
|---|---|---|---|
| A | `mistralai/mistral-nemotron` | 39/57 | stratified, 1 per cell |
| B | `deepseek-chat` | **661/661** | **full, natural distribution** |

Both **blind**: the prompt builder takes only (entry, source, cells) and has no
parameter for either label, verified on the real payload (228 checks, 0 leaks).

## RESULT

| | rater A (small) | rater B (frontier) |
|---|---|---|
| n | 39 | **661** |
| parse rate | 39/57 (18 transport failures) | **661/661, 0 failures** |
| observed agreement | 0.128 | **0.2330** |
| chance agreement pe | 0.0178 | 0.0358 |
| Cohen's kappa | 0.112 ("slight") | **0.2045 ("fair")** |
| 95% CI on agreement | — | [0.202, 0.267] |

Rater-B behaviour (the check separating "task is ambiguous" from "rater broken"):

| | distinct cells | entropy | most used |
|---|---|---|---|
| rater 1 | 57 of 57 | 3.51 | CONSTRUCT/CREATE 69, FABRICATE/WORK 61, STUDY/WORK 51 |
| rater 2 | 43 of 57 | 3.07 | STUDY/LEARN 103, BOND/MAINTAIN 69, CONSTRUCT/CREATE 52 |

Rater 2 is **concentrated, not uniform**: it uses 43 of 57 cells and puts 16% of
all items on one cell. It restricts itself to a coarser vocabulary than rater 1
— that is a strategy, not noise. Agreement is higher where rater 1 used a common
cell (0.261) than a rare one (0.196), consistent with rater 2 favouring default
cells.

## VERDICT

**Two frontier-class models agree on 23.3% of blind filings — about one in four**
(kappa 0.205, "fair"; ~6.5x chance). Three filings in four differ.

This **settles the capability confound in the right direction**: the smaller
model agreed on 12.8% (kappa 0.112), the frontier model on 23.3% (kappa 0.205).
Agreement roughly doubles with rater capability, so the low figure from rater A
was partly a capability artifact, as flagged.

**But it does not rescue the shared-structure reading.** A ceiling of ~23%
means the filing is substantially rater-specific: roughly three-quarters of the
variance in a filing is rater, not entry. Any geometry accuracy above this
ceiling is capturing a *policy*, and the paper must keep saying
"rater-assigned cell", never "correct cell".

## LIMITATIONS

1. **This is a model ceiling, not a human one.** Two models agreeing is
   agreement between two policies. It bounds what can be claimed about raters
   in general only loosely — the human rater is still the missing measurement.
2. **Rater 2 restricts its own vocabulary** (43/57 cells, 16% on STUDY/LEARN), so
   part of the disagreement is a coarseness mismatch rather than disagreement
   about the entries. A rater that used the full grid might agree more.
3. Rater A's stratified sample forced rater 1 to be uniform, depressing pe to
   ~1/57 and *inflating* its kappa. Rater B used the full natural sample, so
   0.2045 is a neutral estimate — this is why both are reported.

## REPRODUCTION

    python3 research/papers/p4_second_rater.py --n 100000 --backend deepseek --timeout 120

Artifacts: `research/p4_rater2_deepseek.json` (n=661),
`research/p4_rater2_nvidia.json` (n=39).

## ARTIFACT-INTEGRITY NOTES

Defects found by reading the artifacts rather than their summaries:

- `rater2_raw` was stored as `raw[:80]`. Labels were parsed from the full text
  so they are sound, but the evidence for them was destroyed — a row whose
  stored raw contained no cell name still carried a cell, which is impossible if
  the stored raw is what was parsed. Now stores `raw[:2000]` plus `raw_len`.
  Rater A's rows predate this fix; **rater B's do not**, so only rater B's
  labels are auditable.
- `_load_partial` counted `REQUEST-FAILED` rows as done, so a resume silently
  skipped items never asked. Now excluded.
- The artifact and partial paths were shared across backends. The resume is
  keyed on item number, so a second rater would have **skipped items the first
  one rated and reported them as its own** — agreement between raters that is
  really one rater copied twice. Both paths are now namespaced by backend.

## GIT COMMIT

Recorded at the commit that adds this file.
