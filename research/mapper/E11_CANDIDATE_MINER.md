# E11 — Candidate mining that works, validated before it was read

`research/mapper/run_e11.py` · `physis-core/examples/mapper_wordnet_export.rs`

The expansion spec recorded a negative result: **mining free prose for ontology
entries fails**, because an entry is a short domain concept and prose contains
none in extractable form. The stated fix was *"a source that already contains
concept-shaped items — a glossary, an index, a taxonomy."*

WordNet was in `models/` the whole time. Its noun synsets are a lemma plus a
gloss, which is exactly a physis entry's name-plus-hints shape.

## The source, and a filter that is not arbitrary

Physis entries are processes, conditions, fields and acts — not concrete
objects. WordNet's **lexicographer file id** already partitions nouns that way,
so the export keeps `act, attribute, cognition, communication, event, feeling,
motive, phenomenon, process, relation, state, time` and drops
`animal, artifact, body, food, group, location, object, person, plant,
possession, quantity, shape, substance`.

**13,958 synsets** after filtering, embedded with BGE like every other arm.

## Validation — run before the output was looked at

The previous three miners were shipped straight to output and all three were
useless. This one was tested against ground truth first:

> Hold out 30% of the **existing** entries. Drop them into a pool with the
> WordNet synsets as distractors — 14,155 candidates total. Build each cell's
> target from its **train members and authored anchor only**. Rank the whole
> pool per cell, and ask where that cell's own held-out entries land.

| arm | n held-out | r@1 | r@5 | r@25 | **r@100** |
|---|---:|---:|---:|---:|---:|
| **target from members + anchor** | 197 | **0.091** | **0.183** | **0.325** | **0.497** |
| CONTROL shuffled cell targets | 197 | 0.000 | 0.005 | 0.005 | 0.086 |
| CONTROL random ranking | 197 | 0.000 | 0.000 | 0.000 | 0.025 |
| *chance* | | | | | *0.0071* |

**Half of the held-out entries surface in the top 100 of 14,155** — 70× chance
and 5.8× the shuffled-target control. A third are in the top 25. The miner finds
entries it is known to want, which is the precondition for trusting it on
entries nobody has written yet.

## What it proposes for the starved cells

| cell | top candidates |
|---|---|
| BOND/BRAINSTORM | **ideation**, talk, **groupthink**, **free association**, **divergent thinking** |
| CONSTRUCT/DESTROY | **wrack**, **ruin**, **attrition**, **annihilation**, battering ram |
| BOND/PLAY | **sharing**, party, **phatic speech**, cordial reception, **chat**, **friendship** |
| BOND/GUIDE | **guidance**, **leadership**, **fostering**, guidepost, morale builder |
| BOND/LIFT | love, **attachment**, **closeness**, **alliance**, confidence |
| BOND/MOVE | **liaison**, introduction, **public speaking**, **correspondence** |

These are usable. `ideation / groupthink / divergent thinking` for BRAINSTORM and
`guidance / leadership / fostering` for GUIDE are the kind of thing a person
would have written by hand.

## The residual weakness, stated plainly

**The CONSTRUCT domain pulls concrete artifacts**, and the lexfile filter did not
catch it:

- CONSTRUCT/LIFT → *crane, lifting device, hoist, derrick, beam*
- CONSTRUCT/MOVE → *transporter, freight elevator, dock, trailer*
- CONSTRUCT/GUIDE → *drill site, carpenter's rule*

Those are machines that lift and move, not the *mode* of lifting or moving. The
BOND cells have no such problem — they return relations throughout.

**This is informative rather than merely annoying.** `docs/GRID_AXES.md` defines
CONSTRUCT as *"structure — the durable arrangement other work happens inside or
on top of"*, but its anchor evidently embeds toward physical construction. That
is the same anchor-quality gap E6 measured: the authored anchors recover the
domain axis at 39.9% gain over chance and the mode axis at 18.7%. **The
CONSTRUCT anchor is pulling the miner toward a building site.**

So the miner's failures are a probe of anchor quality, which is a second use for
it beyond producing the worklist.

## How to use it

`wordnet_worklist.json` — 40 deduped candidates per starved cell, with glosses
and similarity. It is a **proposal list, not verified entries**: this track's
standing result is that coverage filters and ranks discovery candidates but does
not verify that any candidate is correct. The Disposer Bench pattern applies —
the machine ranks, a person disposes.

Two practical notes:

1. **Read BOND, HEAL, STUDY and FABRICATE cells first.** CONSTRUCT's list needs
   the anchor fixed before it is worth reading.
2. **Twenty per cell is the target**, from E7's curve. The list gives 40 so
   there is room to reject half.

## Process

Four miners. The first three failed and each failure named the next fix:
similarity-summed terms returned Italian stop words (English-only stop list);
contrastive scoring returned codebase identifiers (circular — mining physis's
source for physis's ontology); document-library-only returned prose vocabulary
that was correctly contrastive and still not ontology entries. **The fourth
works because the third's negative result specified what the source had to be.**

The validation is the difference. Three miners shipped output that looked
plausible and was useless; this one was measured against held-out ground truth
with two controls before a single candidate was read.
