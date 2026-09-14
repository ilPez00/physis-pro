# E89 — the NLI arm: best recall on record, worse than surface rules on adversarial input

> Follows `E86_PARTICIPANT_BINDING.md`, which scoped the model arm to the
> implication class (0/3, the one class no rule touches) and added the
> adversarial-precision comparison as the arm to beat: bound rules at 0 false+
> on E85, 0 false+ on the E86 validation set. E85 also records this session's
> memory of the larger-model NLI comparison — 120B beats cosine cleanly, 7B
> collapses — which is why this run uses neither end: the smallest honest NLI
> cross-encoder with fp32 ONNX weights, `cross-encoder/nli-deberta-v3-xsmall`
> (22M params, MNLI, 272 MB). Threshold 0.5 on P(contradiction). ~10 ms per
> pair on CPU — deployable, no provider needed.

**Run:** `.venv-nli/bin/python benchmarks/conatus/e89_nli_arm.py --out
benchmarks/conatus/e89-results.json` (venv carries onnxruntime + tokenizers;
model weights live in `models/nli-deberta-v3-xsmall`, git-ignored like the
other weights).

## Result

| fixture | NLI (this run) | bound rules (E86) |
|---|---|---|
| holdout | **1.000 / 0.960** (24 caught, 1 missed) | 1.000 / 0.840 |
| adversarial | 0.000 (3 false+) | **0 false+** |
| validation | 0.750 / 1.000 (6 caught, 2 false+) | 1.000 / 0.833 |

- The NLI arm catches 2 of 3 implication pairs at P=0.99 — exactly the class
  the rules measure 0/3 on — and the dictionary-gap antonym
  ("clogged/clear") the rules miss everywhere. Its one holdout miss,
  "passed on the first attempt / needed three attempts" at P=0.02, is a
  temporal-entailment judgement the model confidently gets wrong.
- On adversarial input NLI is **worse than surface rules**: 3 of 15 and 2 of
  14 fire at high confidence (P=0.99, P=0.79). The failure shapes are the
  E85/E86 participant-binding shapes — opposed modals over different owners
  ("Operators may stop the line / may keep it running", P=0.99), opposed
  modals over different objects ("valve must stay open / drain valve must
  stay closed", P=0.99), gauge readings at startup vs shutdown (P=0.52),
  warmup/cooldown antonyms across participants (P=0.79).
- Union accounting: "rules first, NLI only where no rule fires" takes the
  holdout from 21/25 to 23/25 (recall 0.920, precision 1.000) but lets 3
  adversarial pairs through. The NLI arm is a recall extension with a
  precision tax, not a replacement.

## What this changes

* **Routing, not replacement.** The evidence now supports a routed design the
  earlier experiments could only guess at: structured rules first (precision
  shield, 0 false+ across 65 pairs of three fixtures), NLI confined to the
  residual — implication pairs and dictionary gaps. The 120B-vs-7B session
  memory plus this 22M arm puts the size/recall/precision tradeoff on record
  at three points: small NLI extends recall but pays the adversarial tax.
* **The NLI failure is a participant-binding failure too.** At P=0.99 the
  model produces the same verdict the unbound rules produced — same modal
  shape, same missed owner. Slot-binding is a property of the *task*, not of
  the instrument: the planned `jepa-rerank` admission tests (PLAN.md E3, IR/SR
  over paraphrased queries) should take these exact pairs as the negative
  fixture, since both rules and models fail them pre-binding.
* **Cost position is settled.** 272 MB fp32 ONNX, 10 ms/pair CPU, offline,
  no provider — the routed design deploys without the infrastructure the 120B
  comparison assumed. The `.venv-nli` (onnxruntime + tokenizers) stays out of
  the main dependency tree, same as the conatus scripts.

## Limits

MNLI labels are single-sentence-pair judgements without domain context; the
22M model's confidence on industrial modal pairs (0.99 wrong) is not a
calibration failure, it is a distribution failure — fine-tuning or a
threshold-per-class was not attempted and would need its own fixture split.
The implication sub-sample is 3 pairs; "2/3 at 0.99" is a direction, not a rate.

*Script, weights location and raw results: `benchmarks/conatus/e89_nli_arm.py`,
`models/nli-deberta-v3-xsmall` (git-ignored), `e89-results.json`.*
