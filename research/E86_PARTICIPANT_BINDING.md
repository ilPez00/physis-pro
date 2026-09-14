# E86 — participant binding: the rules keep their recall and lose the false positives

> Follows `E85_ADVERSARIAL_FIXTURES.md`: four rules fired wrongly on consistent
> pairs built from their own cue shapes, and the defect was named — class cues
> matched without binding them to the pair's participants. E86 adds one guard
> per broken rule (`benchmarks/conatus/e86_participant_binding.py`), tuned on
> the E85 adversarial set and the E84 holdout **only**. The validation fixture
> (`fixtures/adversarial-consistent-2.json`) was written before the repaired
> rules were first run against it.

## The guards

| rule | binding added |
|---|---|
| modal | first content noun must match on both sides (the modal's owner is what differed) |
| comparative | content words *before* the matched comparison word must match (a re-addressed subject changes the prefix) |
| quantifier | universal vs negative-universal suppressed when the predicates are antonyms (WordNet, the domain table, or a derivational `un-`/`dis-` pair) — "all passed" *is* "none failed" |
| antonym | subject-head match + the antonym must occupy the same slot (right content neighbour equal or both absent); fires when *any* matched pair binds |
| domain (reinstated) | E84's table with the same quantifier-absorption guard |

## Result — three fixtures, no fixture graded its own repair

| fixture | precision | recall | notes |
|---|---|---|---|
| holdout (E84) | **1.000** | **0.840** | identical to E84 — the binding cost no recall |
| adversarial (E85) | — | — | **4 false+ → 0** |
| validation (E86) | **1.000** | 0.833 | 5/6 genuine contradictions caught; 0 false+ |

The single validation miss — "The filter was clogged / The filter was clear" —
is a WordNet dictionary gap (no `clogged`/`clear` antonymy), the same class as
E84's fourth miss, not a binding failure.

## What this changes

* **E85's collapse is repaired without paying recall.** The instrument's
  measured profile is now: precision 1.000 on benign holdout *and* on unseen
  adversarial input, recall 0.840/0.833 across both — no fixture flattered the
  repair, because the validation set predates it.
* **The parser's cost claim is weakened, honestly.** E85 justified a dependency
  parse by the binding failure. First-order binding (head nouns, slot
  neighbours, predicate antonymy) recovers precision with surface machinery
  only. What a parser would still buy: the `implication` class (0/3, unchanged
  since E83) and dictionary gaps derivational logic cannot bridge.
* **The remaining ceiling is lexical coverage, not structure.** Both miss
  classes left after E86 are "the dictionary does not know these two words are
  opposed" — which is R3's three-way discriminator problem (PLAN.md E4), not a
  parsing problem.

## Limits

Still one author, one subject matter, small denominators (25/15/14). The
quantifier guard uses the E83 `_stems` crude stemmer; a plural it mishandles
would silently pass the antonymic test. And "fires when any pair binds" is the
generous reading — a pair with two antonym matches, one binding and one not,
now fires where E84 also fired; a stricter reading is untested.

*Script, fixtures, raw results: `benchmarks/conatus/e86_participant_binding.py`,
`fixtures/adversarial-consistent-2.json`, `e86-results.json`.*
