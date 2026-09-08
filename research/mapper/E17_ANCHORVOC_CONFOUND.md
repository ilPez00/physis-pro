# E17 — Is ANCHORVOC circular? Three tests, and they disagree

`research/mapper/run_e17.py`

E16 found ANCHORVOC the only scorer to beat chance on **real** misfilings
(0.664, +0.145 over cosine, CI [+0.072, +0.219]) — and flagged a confound: it
measures lexical overlap with the cell's authored anchor hints, and the blind
rater judged entries against those same anchors. It shares an input with the
ground truth.

Three tests, each pre-registered with the reading that would follow.

## Test 1 — does ANCHORVOC predict the rater's *choice* of new cell?

The sharpest version of the confound: if the rater's rule is essentially "pick
the cell whose anchor shares most vocabulary", then scoring the old cell is just
measuring the rater's own procedure. Rank all 70 cells by ANCHORVOC for each
moved entry:

| cell | median rank of 70 | top-5 rate |
|---|---:|---:|
| the rater's **NEW** cell | **17** | **33.0%** |
| the rejected OLD cell | 40 | 13.1% |
| a random cell (control) | 35 | 7.9% |

*n = 466 moved entries.*

**PASS.** If the rater were running ANCHORVOC the new cell would sit at rank
1–3. At 17 of 70 it is not the same rule. But 33% top-5 against a 7.9% random
baseline is well above chance: the two **share information**, they do not share
a **procedure**.

## Test 2 — held-out half of each anchor's hints

Build the probe from half of each anchor's hint vocabulary; the rater read all
of it.

| probe | AUC | vs COSINE |
|---|---:|---:|
| all hints | 0.664 | +0.145 |
| **half the hints** | **0.598** | **+0.079 [+0.007, +0.150]** significant |

**PASS.** The effect survives on a reduced vocabulary, so it is not about the
exact strings. The 0.066 drop shows some dependence on the specific hints.

## Test 3 — the 56 entries where blindness failed

The artifact flags entries whose hints narrate their own old cell. The rater's
procedure was identical there; only blindness differs. **Pre-registered
reading: a detector of real misfiling should behave similarly on both; one
tracking the rater's reading of the hints should not.**

| scorer | blind subset | non-blind subset |
|---|---:|---:|
| **ANCHORVOC** | 0.664 | **0.871** |
| COSINE | 0.519 | 0.467 |

**FAIL.** ANCHORVOC *rises* by 0.207 exactly where the hint text leaks the cell,
while cosine falls. By the criterion set before running, that is evidence for
the hint-tracking interpretation. *(n = 56; a flag, not a result.)*

## Net reading

**ANCHORVOC is not circular in the strong sense** — it does not reproduce the
rater's decision (Test 1), and it does not depend on the exact hint strings
(Test 2). **It is hint-dependent in a way a filing-independent signal would not
be** (Test 3).

The most likely account fits all three: **an entry that shares no vocabulary
with its cell's anchor really is more likely misfiled — and that is also part of
what the rater used.** The signal is real and it is not independent evidence.
Both are true, which is why the tests disagree.

**Treat it as a lead worth an independent test, not as a validated certifier.**

## The test that would settle it

Adjudicate a sample with a rater who sees **only the axis definitions**, never
the anchor hints. That removes the shared input entirely. If ANCHORVOC still
separates moved from kept, it is measuring misfiling; if it collapses, it was
measuring the anchors.

That is one adjudication pass over a few dozen entries — the Disposer Bench
already has the shape, and it only needs its display changed to withhold hints.

## Correction to this experiment's own first verdict

The script's first verdict read **only Test 1**, found it passed, and printed
*"the result survives the sharpest available circularity test."* It ignored that
Test 3 failed the criterion the same script had pre-registered two sections
earlier. The verdict now weighs all three and reports the disagreement, because
a confound check that stops at the first reassuring answer is not a check.
