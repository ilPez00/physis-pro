# E27 — can the mode axis be made coherent?

**Not by merging. Partly by representation.**

E26 supplied a criterion that is not a matter of taste: an axis is a coherent
object when it compresses into the span of its own class centroids without loss.
Domain does (−0.007). Mode does not (−0.029). `run_e27.py` searches for a
granularity or a representation where it does.

## 1. Merging fails, and the pre-registered falsifier is why

Greedy merge of the most-confused mode pair, held-out at every step:

| k | kappa | compression | merged |
|---:|---:|---:|---|
| 14 | +0.394 | −0.039 | — |
| 12 | +0.401 | **−0.025** | CREATE + WORK |
| 9 | +0.472 | −0.054 | GUIDE + LEARN |
| 7 | +0.507 | −0.078 | the whole WORK/CREATE/SENSE/MAINTAIN blur |
| 5 | +0.478 | −0.207 | |
| 3 | +0.512 | −0.388 | |
| 2 | **+0.533** | −0.344 | |

`run_e27.py` stated the falsifier before running: *"if compression loss stays
large at every k, mode is not a linear structure at any granularity and no merge
fixes it."* **It stayed large at every k**, and got dramatically worse as
merging proceeded.

**The confound, removed.** Fewer classes also means fewer dimensions, so the
column above is not comparable down its own length. At **matched granularity**:

| labelling | k | dims | compression |
|---|---:|---:|---:|
| **domain** | 5 | 4 | **−0.007** |
| **mode, merged to k = 5** | 5 | 4 | **−0.187** |
| mode, native | 14 | 13 | −0.029 |

**27× more lossy than domain at identical k and identical dimension** — and
merging to k = 5 is *worse* than leaving mode at 14. Coarsening does not
concentrate mode into a structure; it smears it.

## 2. The two criteria disagree, and that is the methodological point

Kappa rises monotonically toward k = 2 (+0.533). Compression collapses over the
same range (−0.344). **Optimising kappa alone would have produced a two-class
mode axis** — excellent by the number, useless to a person choosing a filing, and
maximally incoherent as an object.

E5 recorded that collapsing 57 → 14 classes gained +0.014 kappa. That stands, and
this is the other half of it: **kappa rewards coarseness for free, so it cannot
be the criterion for deciding a grid's shape.** House rule 2 says report kappa
instead of accuracy against a majority floor; E27 adds that kappa is still the
wrong instrument for choosing granularity.

## 3. What does help: drop the modifier

E25 found mode lives in the head of a compound. Testing that as a
*representation* rather than a finding, on the 498 compound entries:

| representation | domain | mode |
|---|---:|---:|
| full compound name | −0.016 | −0.061 |
| **head word only** | **+0.006** | **−0.024** |

**Mode's incoherence more than halves when the modifier is dropped** (−0.061 →
−0.024), and domain loses nothing (+0.006, i.e. no measurable loss). The
modifier is not neutral for mode — it is the noise that stops mode being an
object, which is the same conclusion E22 reached from hints and E24 from the
weight sweep, now in the geometry.

Mode at −0.024 is still three to four times less coherent than domain at
+0.006. **Improved, not solved.**

## 4. So: can mode be made coherent?

**The route is representation and redefinition, not granularity.**

1. **Do not merge modes to fix this.** It makes the axis less of an object, and
   the kappa gain that suggests otherwise is the free kind.
2. **Build the mode axis from the head/verb, not the whole entry.** Measured:
   more than halves the incoherence at no cost to domain.
3. **The remaining work is definitional, not computational.** E22 localised it:
   WORK/CREATE/MAINTAIN/SENSE hold 68% of the corpus with ample population and do
   not separate. No representation fixes four definitions that carve nothing
   apart — and E27 now shows merging them is not the answer either, so they have
   to be *rewritten*.
4. **Meanwhile the interface should keep deferring mode**, per `PLAN.md`. Nothing
   here changes that; it sharpens why.

## Limits

1. Compression is measured with nearest-centroid throughout, for comparability
   with E21/E26. The claim is that mode is not linearly captured by its class
   centroids, not that mode is unlearnable.
2. The k = 5 mode merge is the one the confusion search happened to reach. A
   different merge path could give a different k = 5 partition, and a
   hand-designed one could do better than a greedy confusion merge — this refutes
   *mechanical* merging, not every possible five-mode grid.
3. The head-only result is on the 498 compound entries only; 84 single-word
   entries have no head/modifier split and are excluded.
