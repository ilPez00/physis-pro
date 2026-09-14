# E85 — adversarial fixtures: the E84 rules fire wrongly when invited to

> Follows `E84_STRUCTURED_RULES.md`, which reached precision 1.000 / recall
> 0.840 on the dictionary-free holdout and named its own limit first: precision
> 1.000 without adversarial pairs is untested confidence. E85 supplies the
> adversarial set. Script and harness are E84's, unchanged — only the fixture
> differs.

**Run:**
`python3 benchmarks/conatus/e84_structured_rules.py --fixture benchmarks/conatus/fixtures/adversarial-consistent.json --out benchmarks/conatus/e85-adversarial-results.json`

Fixture: 15 pairs, **0 contradictions**, each constructed on the same surface
shape as a holdout contradiction but consistent — same measurement frame with
different (sequential or cross-line) numbers, opposed comparison words with the
subjects re-addressed, same modal with opposed polarity over *different*
objects, universal against negative-universal that *agree*, predicate-argument
order swapped into the passive, dictionary antonyms across a time boundary, and
agreement phrased as negation.

## Result

**Precision 0.000. 4 of 15 consistent pairs fired; every firing was wrong.**

| rule | false+ | wrong pair |
|---|---|---|
| quantifier | 2 | "All sensors passed calibration" ≡ "No sensor failed calibration"; "Every pallet was scanned" ≡ "No pallet left unscanned" — the rule reads universal vs negative-universal as opposed polarity, but with an antonym predicate they are *equivalent* |
| modal | 1 | "The valve must stay open" / "The drain valve must stay closed" — opposed polarity over **different objects**; the rule never checks that the modal's subject matches |
| comparative | 1 | "older press runs slower / new press runs faster" — a paraphrase; opposed comparison words around a shared quantity, with the subjects re-addressed |
| antonym | 1 | "Morning shift heats / Afternoon shift cools" — the antonym spans the pair, but across a time boundary |
| negation | 0 | survived: both agreement-by-negation pairs correctly did not fire |
| numeric | 0 | survived: sequential and cross-line number differences correctly did not fire |
| roles | 0 | survived: passive paraphrases correctly did not fire |

## What this changes

* **E84's verdict is now conditional and the condition is written down.** The
  structured rules are a **high-recall, precision-unknown-on-adversarial-input
  instrument**: 0.840 recall on benign holdout, 4 wrong firings in 15 invited
  cases. Both numbers are needed; either alone is the fixture flattering the
  rule, which is the same lesson E83 recorded about generated fixtures.
* **The failure has one named shape.** All four false positives are the same
  defect: the rules match *class cues* without binding them to the pair's
  *participants*. Equivalent-polarity quantifiers, modals over different
  objects, comparisons with re-addressed subjects, antonyms across time —
  each needs participant identity, which is exactly what a dependency parse
  provides. The parser's cost is now justified by a *measured* failure mode,
  not by E83's gap.
* **Numeric and roles rules passed their first adversarial test** (frames
  distinguished sequential vs simultaneous readings; passive swaps did not
  fire). Negation survived too. The instrument is not uniformly brittle — the
  brittleness localizes to quantifier, modal, comparative, antonym.

## What would have to come next

1. **Participant-binding rules** (cheap first): require the modal's head noun,
   the comparative's subject, or the quantified noun phrase to overlap between
   the two sides before firing. Repairable on the same fixtures in minutes —
   but the repair must be validated on a *second* adversarial set, not the one
   that suggested it (E83's generated-fixture lesson, again).
2. **The NLI arm on implication pairs** stands unchanged from E84; E85 adds
   that NLI's precision on the adversarial set is also worth one run, since
   the rules' collapse there is now the baseline to beat.

*Fixture and raw results: `benchmarks/conatus/fixtures/adversarial-consistent.json`,
`benchmarks/conatus/e85-adversarial-results.json`.*
