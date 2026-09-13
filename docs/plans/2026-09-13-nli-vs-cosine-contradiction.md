# NLI beats the shipped contradiction rule — at 120B, and only at 120B

2026-09-13. Measured, with the incumbent as an arm and a null that had to be
beaten. Raw data: `~/dev/computer-remake-research/experiments/nli/`.

## The question

`map.rs` decides contradiction with
`lex >= 0.5 && (cos < 0.55 || na != nb || number_clash)` — cosine finds
topically-similar candidates, a symbolic discriminator makes the call.
Contradiction recall has sat at 0.50 (RESCOPE §6 item 7), and the miss was
recorded as "a map defect rather than a retrieval one".

That reading was right, and the defect is sharper than "a map defect":

```
extra-window-a  Operator access window is mandatory from 06:00 and night access is forbidden.
extra-window-b  Operator access window is forbidden from 06:00 and night access is required.
```

A **swap**, not a negation. Same words, same numbers, cosine 0.984, and
`na == nb == 0` because each text contains exactly one prohibition. Counting
negation markers cannot see a swap at any threshold — the difference is *which
argument each polarity binds to*. Adding "forbidden" to `NEGATION_MARKERS`
would not help: both sides would then score 1.

## Design of the test

Over the **86-pair candidate set**, not all 435 pairs: `map.rs` skips
`lexical_overlap < 0.5` before considering anything, so scoring 435 would
measure a system physis does not have. Both gold pairs are inside the candidate
set (lex 1.000 and 0.750), so what is under test is the **discriminator**.

Recall cannot settle this. The null — flag every pair above a cosine threshold
— scores recall 1.00 by construction, because contradictions are topically
similar. **Precision decides it.**

## Result

| arm | precision | recall | flagged | FP |
|---|---|---|---|---|
| INCUMBENT `map.rs` (pre-PH-022) | **1.000** | 0.50 | 1 | 0 |
| INCUMBENT `map.rs` (post-PH-022) | **1.000** | 0.50 | 1 | 0 |
| NULL cosine ≥ 0.95 | 0.054 | 1.00 | 37 | 35 |
| NULL cosine ≥ 0.90 / 0.85 / 0.80 | 0.023 | 1.00 | 86 | 84 |
| NLI — local `qwen2.5:7b` | **0.054** | 1.00 | 37 | 35 |
| NLI — `nemotron-3-super-120b` | **1.000** | **1.00** | 2 | **0** |

86 pairs, 0 errored, 5 × HTTP 503 recovered via `Retry-After`.

**At 120B, NLI strictly dominates the incumbent** — same perfect precision,
double the recall. It flagged exactly the two gold pairs out of 86 and nothing
else, including `window-a/b`, which the incumbent structurally cannot reach.

## The finding that matters most is the model-size threshold

**7B is not a degraded 120B. It is identical to the null.** Same prompt, same
schema, same corpus, same candidate set: 37 flagged, 35 false positives,
precision 0.054 — indistinguishable from "flag everything above cosine 0.95".
It says "contradiction" to 43% of topically-similar pairs.

This is the second time in one day that a 7B silently produced confident
rubbish where a large model was clean; the first was graphiti's extraction
rewriting English facts into mangled Chinese
(`computer-remake-research/experiments/graphiti/RESULTS.md`). The pattern is
worth naming: **small models do not fail loudly at semantic judgement, they
fail agreeably.**

## What PH-022 bought, honestly: nothing measurable here

The `number_clash` dead branch fixed this morning now fires correctly —
`num_fixed=True` on `spec-A/B` (2.6 vs 4.2) where `num_old=False`. But that
pair was **already** caught by negation asymmetry (`na=0, nb=3`), so the
incumbent's numbers are identical pre- and post-fix. A real defect repaired
with no measured benefit on this corpus. Recorded rather than quietly dropped:
the fix is still correct, and the corpus simply does not contain a pair that
only a measurement clash would catch.

## What this does not license

- **It does not ship.** Physis is offline-first and the local arm *failed*.
  "NLI works" licenses nothing about what physis can distribute.
- **n = 2 gold pairs.** Recall 1.00 means "found both of two". The precision
  half is much stronger: 84 true negatives correctly left alone.
- **The prompt is authored.** I wrote the instruction that tells the model when
  to say contradiction, including an explicit "different instances are neutral"
  clause. That is an authored control, the weak kind. A rule-generated arm set
  would be better.
- **The gold-label assumption penalises NLI** — an unlabelled real contradiction
  it found would score as a false positive. It produced zero, so the assumption
  did not bite, which strengthens rather than weakens the precision number.

## Next, in order

1. **A dedicated NLI cross-encoder, locally.** The untested and most promising
   option: DeBERTa-v3-MNLI class is ~400 MB and purpose-built for exactly this
   three-label task, against a general 7B chat model that was never trained for
   it. The local arm's failure is evidence about `qwen2.5:7b`, not about local
   NLI.
2. **Then wire it as the discriminator**, never the candidate generator — the
   cross-encoder runs on within-cluster pairs only (86, not 435), so the cost
   is bounded by the clustering that already exists.
3. **Emit an `Observation`, not a boolean.** `machines::Observation` with
   `machine: "nli"` and `ProofStatus::Numerical` (PH-021). An NLI model is an
   interpreter; physis's thesis is that interpreters are replaceable and
   therefore scoreable. A contradiction certified by an unrecorded model is the
   graphiti failure mode with a different noun.
4. **A bigger gold set.** Two pairs cannot carry this. Rule-generated contrast
   pairs — swap, negate, quantifier, double-negation, conditional — with the
   correct label known by construction, is the same shape as
   `benchmarks/direction/run.py` and nobody authors the answer.
