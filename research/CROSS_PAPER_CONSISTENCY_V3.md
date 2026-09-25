# Cross-Paper Consistency V3

Scope: the 7 live manuscripts in `research/papers/` after the P1 merge.

## 1. Shared facts must agree everywhere
| Fact | Canonical value | Where it appears | Status |
|---|---|---|---|
| Corpus size | 75,933 actions / 402 sessions / 4 harnesses | P1, P9 | consistent (registry claim CORPUS-001) |
| Rater assignments | Claude (rater 1), deepseek-chat (rater 2) | P4 | consistent |
| Cross-rater κ | 0.202 [0.168, 0.235], n=582 | P4 | from artifact |
| Positive control | 12/12, α=1.4968 | P10 | from artifact — **not yet in the manuscript** |
| Ledger invariants | 9 invariants, 0 failures | M | from artifact — **not yet in the manuscript** |

## 2. Withdrawn-claim guards (must never return to prose)
| Withdrawn | Why | Guard location |
|---|---|---|
| `0.05/10` Holm denominator | rank-5 denominator is 8, not 10 | `validate.py` XFER-008 |
| "no pair shows positive transfer" (unqualified) | refuted by 0.665/0.635 reciprocal pair | `validate.py` XFER-005 |
| leaked rising cold-start curve (0.469→0.720) | train/test leakage | `validate.py` XFER-006/007 |
| "+0.576" geometry-minus-null gap | belonged to no sentence; verified gaps are +0.402/+0.531/+0.522 | `verify_paper4.py` |
| "curve still climbing at 436" | withdrawn under the sequential split | P4 caption (fixed) |
| "correct cell" | replaced by "rater-assigned cell" | P4 (fixed), P1 grep clean |
| "certifier absent under both protocols" | certifier measured once only | P4 (fixed) |
| "no positive control was run" | false since `p10_controls.json` | **P10 — STILL PRESENT** |
| "finite-size analysis remains future work" | false since `p10_fss.json` | **P10 — STILL PRESENT** |
| "2–3 dimensional" trajectories | no supporting artifact | **P2/P9 — provisional** |

## 3. Statistical-language discipline (P1)
Distinct quantities, never conflated:
- point estimate (AUC), bootstrap interval, nominal p, BH result, Holm result,
  repeated-partition frequency.

`majority of partitions above chance` (a frequency over 200/500 partitions) and
`survives Holm` (family-wise rejection) answer different questions. P1 now says
so explicitly, and cline→opencode is the worked example of a pair that passes
the first and fails the second.

## 4. Figure hygiene
| Figure | Status | Required caption addition |
|---|---|---|
| `fig1_transfer_matrix.pdf` | live | state row/column = source→target explicitly |
| `fig2_cold_start_curve.pdf` | **LEAKED** | mark WITHDRAWN in-caption; not a result figure |
| `fig3_coverage_vs_auc.pdf` | live | label exploratory, note n=12, no causal claim |

## 5. Known unresolved contradictions (carried forward, not silently fixed)
1. **P10 vs its own artifacts** — manuscript denies controls/FSS that exist.
2. **P2 vs its artifacts** — manuscript asserts trajectory claims with no
   analysis artifact.
3. **PaperM ledger provenance** — `methodology_validation.json` records commit
   `8232042`, five commits behind HEAD `6462fd5`.
