# E12 — Auditing the 70 anchors, and a rewrite that would have made things worse

`research/mapper/run_e12.py` · `physis-core/examples/mapper_defs_export.rs`

E11's miner pulled `crane`, `hoist` and `freight elevator` for CONSTRUCT cells,
and I read that as *"the CONSTRUCT anchor embeds toward a building site"* and
proposed regenerating anchors from `docs/GRID_AXES.md` — the Stage 11 move, the
one intervention in this track that demonstrably worked.

**Both halves of that were wrong, and measuring first is what caught it.**

## A. Per-anchor quality

For each cell, rank all 658 entries by similarity to its anchor and count how
many of that cell's own entries land in the top *n*. Chance is under 0.11 for
every cell.

**Mean precision@n = 0.306 across the 45 cells with enough entries to measure**
— roughly 3× chance, so the anchors work, unevenly.

The worst, which is the rewrite queue:

| cell | n | prec@n | median rank | anchor |
|---|---:|---:|---:|---|
| STUDY/PLAY | 3 | **0.00** | 154 | Thought Experiment |
| STUDY/PLAN | 4 | **0.00** | 122 | Design the Inquiry |
| HEAL/MOVE | 6 | **0.00** | 51 | Transit & Locomotion |
| STUDY/CREATE | 4 | **0.00** | 37 | Theorise & Write |
| FABRICATE/MAINTAIN | 4 | **0.00** | 28 | Keep the Line Able |
| BOND/CREATE | 17 | 0.06 | **213** | Form the Connection |
| STUDY/GUIDE | 7 | 0.14 | **327** | Teach & Explain |
| HEAL/WALK | 9 | 0.11 | 86 | Steady Condition |

And the best, for contrast: STUDY/LIFT 0.80 (*Intensive Focus*), BOND/REST 0.75
(*Quiet Together*), BOND/LEARN 0.67 (*Learn to Relate*).

**`STUDY/GUIDE` is the standout failure** — its typical entry sits at rank 327
of 658, worse than the median of the whole corpus. *"Teach & Explain"* is not
finding the entries filed under teaching.

## B. The CONSTRUCT hypothesis — refuted

| domain | cells | mean prec@n |
|---|---:|---:|
| **HEAL** | 9 | **0.257** |
| STUDY | 11 | 0.279 |
| **CONSTRUCT** | 7 | 0.284 |
| FABRICATE | 9 | 0.342 |
| BOND | 9 | 0.371 |

**CONSTRUCT is middle of the pack.** HEAL is the weakest domain. E11's
observation does not generalise from its one example to the domain.

**But E11's observation is not wrong — it was narrower than I framed it.** E11
measured what the anchor retrieves *from WordNet*; E12 measures what it
retrieves *from the corpus*. The CONSTRUCT anchor can pull concrete machinery
out of a general lexicon while still finding its own entries adequately. The
correct statement is about the **candidate pool**, not about anchor quality:
when mining WordNet for CONSTRUCT, filter harder. The claim I made — that the
anchor is broken — did not survive the test.

## C. Anchors vs written definitions — the intervention, refuted

Composed each cell's probe from the two written axis definitions
(`"CONSTRUCT structure — the durable arrangement… WORK execute, labour…"`) with
nothing hand-authored per cell, and ran the identical retrieval test.

| probe | mean prec@n | mean median rank |
|---|---:|---:|
| **authored anchor (name + hints)** | **0.306** | **64** |
| written axis definitions | **0.115** | 152 |

**The definitions win in 1 cell of 45, tie in 13, and lose in 31.** Regenerating
anchors from `GRID_AXES.md` would have cut retrieval precision by nearly two
thirds and pushed the median entry from rank 64 to rank 152.

Where the authored anchor is most decisively better: BOND/REST (0.75 → 0.00),
STUDY/LIFT (0.80 → 0.20), HEAL/LIFT (0.50 → 0.00), FABRICATE/LEARN (0.57 → 0.14).

### Why, and the distinction it forces

The axis definitions are abstract and generic — *"structure — the durable
arrangement other work happens inside or on top of"*. The anchors are concrete
and written in something closer to the entries' own idiom — *"Peak Exertion:
heavy lifting, max effort, push to the limit"*. `gen_cell_vocab.py` recorded this
exact problem for the anchors themselves; the definitions are one step further
from the entries again.

**Stage 11's success and this failure are not in tension, because they are
different uses of a definition:**

- A **written definition helps a person file consistently** — that is what Stage
  11 measured, and the misfiling proxy went 31.8% → 16.2%.
- A **concrete anchor helps a machine retrieve** — that is what E12 measures, and
  abstraction hurts it.

Conflating them is what made me propose the rewrite. **A definition is for the
author; an anchor is for the index.** They should not be the same text, and
generating one from the other is a downgrade in this direction.

## What to actually do

1. **Do not regenerate anchors from `GRID_AXES.md`.** Measured cost: −0.19 mean
   precision@n.
2. **Rewrite the eight anchors in section A**, by hand, in the entries' idiom —
   not from the axis definitions. `STUDY/GUIDE` first, at median rank 327.
3. **When mining WordNet for CONSTRUCT cells, filter concrete artifacts harder.**
   That is E11's real finding, correctly scoped.
4. **HEAL is the weakest domain at 0.257** and no one had looked; it deserves the
   attention CONSTRUCT was about to get on the strength of a single anecdote.

## Process note

This experiment was designed to confirm a hypothesis and to prescribe a fix.
It refuted the hypothesis and showed the fix would have caused measurable harm.
Both were cheap to discover because the intervention was measured against the
thing it was meant to improve *before* being applied — which is the only reason
the −0.19 shows up here rather than in the corpus.
