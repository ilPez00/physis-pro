# E83 — a dictionary rule for contradiction: high precision, 14% recall

`WHAT_WE_KNOW.md` files a settled negative: cosine does not separate agreement
from contradiction (C2, precision 0.667 at perfect recall). E82 showed the
mechanism — bare geometry is at chance on synonym versus antonym — and that
WordNet holds 7983 antonym pointers the embedder does not. E83 asks whether
those pointers buy the detector that cosine could not give.

## Two fixtures, and why the second one exists

**Generated (526 pairs).** Contradictions made by putting a word and its WordNet
antonym into the same frame, agreements by putting a word and its synonym,
plus unrelated pairs and negation contradictions.

**Held out (12 pairs, hand-written).** Contradictions that carry *no* antonym:
numeric (`12 minutes` / `40 minutes`), modal (`must` / `must not`), role swap
(`the valve feeds the pump` / `the pump feeds the valve`), quantifier (`every
operator` / `no operator`), implication (`passed on the first attempt` / `needed
three attempts`), plus paraphrase pairs that must *not* fire.

The second fixture exists because the first one is circular: a rule that looks
for antonyms, measured on pairs built by inserting antonyms, is the generator
grading itself.

## Results

Generated fixture:

| arm | AUROC | precision | recall | F1 |
|---|---:|---:|---:|---:|
| cosine distance | **0.336** | — | — | — |
| antonym rule | 0.832 | 1.000 | 0.664 | 0.798 |
| negation rule | 0.666 | 1.000 | 0.332 | 0.498 |
| antonym + negation | 0.998 | 1.000 | 0.996 | 0.998 |

Cosine at 0.336 is **below chance**: contradictions are *closer* than
non-contradictions, because a contradiction is the same sentence with one word
changed. That is C2's negative, reproduced from a different direction.

The 0.998 is the circularity, not a result.

Held-out fixture:

| arm | precision | recall | F1 | cosine AUROC |
|---|---:|---:|---:|---:|
| antonym + negation | **1.000** | **0.143** | 0.250 | 0.514 |

**One of seven contradictions caught** — the only one with a lexical antonym
(`above` / `below`). Missed: the numeric pair, the modal pair, the role swap,
the quantifier pair, the implication pair, and the repository's own fixture
(`mandatory … forbidden` against `forbidden … required`), where the antonym rule
is silent because both sentences contain both words and cosine distance is
**0.0104** — they are nearly the same point in the field.

No false positives on the five paraphrase pairs. Cosine on the held-out set is
0.514, chance.

## What this establishes

* **A dictionary rule is a precision instrument, not a detector.** 1.000
  precision on both fixtures, 0.143 recall on the non-circular one. As a filter
  that never cries wolf it is usable today; as a contradiction detector it
  misses six sevenths of the cases.
* **The geometry is not merely weak here, it is inverted.** AUROC 0.336 means
  ranking by distance would put contradictions *first* among candidate
  agreements.
* **Contradiction is mostly not lexical.** Five of the seven held-out
  contradictions turn on number, modality, quantifier scope or argument order —
  none of which a synonym/antonym dictionary can see, and none of which the
  embedding distance separates either.
* **Generated fixtures flatter rules.** The same rule scored 0.998 and 0.143 on
  two fixtures for the same task. Any figure from a fixture built by the same
  process as the rule is an upper bound and should be labelled one.

## What would have to come next

The missing cases are structured, not semantic: comparatives and units, modal
polarity, quantifier scope, and predicate-argument order. Those are the shape
that TimeML-style annotation and a dependency parse address, and they are the
first place where a parser would earn its cost in this pipeline — the previous
experiments (E81, E82) did not need one.

An NLI model is the other arm, and this session's memory records it beating
cosine cleanly at 120B while collapsing at 7B. That comparison is not run here;
what is run is the offline, zero-inference arm, and its ceiling is now measured.

## Limits

Twelve held-out pairs is a small denominator: 0.143 recall is 1 of 7, and one
more lexical case would move it to 0.286. The pairs are written by the same
person running the experiment, on the same subject matter as the repository's
own corpus. The negation rule is brittle by construction — it requires the token
after the negator to be a content word the other sentence asserts, which is why
`must not be requested` slips past it.
