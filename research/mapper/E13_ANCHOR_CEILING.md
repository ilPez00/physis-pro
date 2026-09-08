# E13 — How much better could an anchor get? Enough to cancel E12's advice

`research/mapper/run_e13.py` · `physis-core/examples/mapper_embed_texts.rs`

E12 ended with a rewrite queue and an instruction: *rewrite the eight worst
anchors by hand in the entries' idiom, STUDY/GUIDE first.* It never established
what a rewritten anchor could achieve. This measures that first.

Five ways of writing an anchor, all scored identically and **all on held-out
entries**, so an anchor built from train entries is never tested against its own
inputs. 21 cells have ≥3 held-out entries and are measurable.

| arm | prec@n | median rank |
|---|---:|---:|
| **A authored anchor (ships today)** | **0.350** | **25** |
| B written axis definitions (E12: refuted) | 0.122 | 58 |
| **C centroid of the cell's TRAIN entries** | **0.353** | 27 |
| D top TF-IDF terms of TRAIN entries, as text | 0.275 | 35 |
| E authored anchor **+** those terms | **0.388** | 22 |

## The rewrite queue is low-value, and that is the solid result

**Headroom over a probe built from the cell's own entries is +0.004.**

Arm C is the literal form of "write it in the entries' idiom" — it *is* the
entries. The authored anchors already match it. So rewriting an anchor to sound
more like the entries it should retrieve is an exhausted move: there is nothing
there to win, whoever does the writing.

**E12's rewrite queue should not be worked.** That cancels the recommendation I
made one experiment ago, and the cost of finding out was one afternoon of
compute rather than one of yours.

## Arm E looked like the replacement finding. It is not.

Combining the authored anchor with the cell's own vocabulary gives **+0.038 mean
precision** and beats even arm C — which would mean the anchor and the corpus
carry independent information and augmentation is free improvement.

**The per-cell distribution says otherwise: E beats A in 6 of 21 cells, ties 5,
and loses in 10.**

The mean is carried by a few large gains offsetting many small losses. Over the
16 non-tied cells, 6 wins is nowhere near significant on a sign test.
**Augmentation is not supported either** — shipped, it would look like an
improvement in aggregate while degrading half the cells it touched.

This is the same shape as six earlier positives in this track: an aggregate that
survives until the per-unit breakdown is read.

## Power, stated rather than discovered later

Only **21 cells** have enough held-out entries to score. This design cannot
resolve a per-cell effect of the size arm E claims. The right reading is not
"augmentation fails" but "this experiment cannot license it, and the aggregate
alone would have licensed it wrongly."

## What E12 and E13 jointly establish

1. **Do not regenerate anchors from the axis definitions** — 0.350 → 0.122 (E12).
2. **Do not hand-rewrite anchors toward the entries' idiom** — headroom +0.004.
3. **Do not ship anchor augmentation on the aggregate** — wins 6 of 21.
4. **The anchors are approximately as good as this representation allows.**
   Anchor quality is not where the remaining error lives.

That fourth point is the useful one. Three consecutive attempts to improve the
anchors — rewrite from definitions, rewrite by hand, augment mechanically — are
all refuted, two of them against the very evidence that suggested them. Anchor
work is finished until something other than retrieval precision motivates it.

**Where the error actually lives, from the rest of the track:** class sparsity
(E5, E7, E8 — 35 of 57 cells below 6 training entries), and ~16% label noise
(Stage 11). Both are corpus problems. Neither is fixed by writing.

## Correction to E12

E12's worst-cell table and E13's are not directly comparable: E12 scored all 658
entries across 45 cells, E13 scores held-out entries only across the 21 cells
with enough of them. `STUDY/GUIDE`, E12's standout failure at median rank 327,
is not in E13's measurable set at all. Neither table is wrong; they answer
different questions, and the E12 queue should be read as "worst on the full
corpus" rather than "worst on held-out data".
