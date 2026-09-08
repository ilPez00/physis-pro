# E16 — The battery on real misfilings: E15's reopening was premature

`research/mapper/run_e16.py`

E15 showed the Stage 12 ranking inverts once the *injected* error becomes
plausible, and concluded the certification question reopens. A plausible
synthetic error is still synthetic. This is the same battery on the only
non-synthetic ground truth in the project.

## Ground truth

`research/perspective-discovery/refiling_gate0.tsv` — Stage 11's Gate 0
regeneration. Every entry was re-filed by a rater who saw name, category and
hints, was **blind to the existing cell**, and judged against the written
contract in `docs/GRID_AXES.md`.

- **positive** = MOVED — a blind judge rejected the old filing
- **negative** = same — a blind judge endorsed it

The 76 entries whose hints narrate their own old cell are excluded; the file
flags them so exactly this can be done. **582 entries, 466 positive, base rate
80.1%.**

*Caveat from the artifact's own header: the rater was Claude, and the same rater
produced the replacement filing. Blindness to the old cell makes the disagreement
genuine, but this is not an independent human panel and is not claimed as one.*

## Result

| scorer | AUC | vs COSINE (95% CI) | |
|---|---:|---:|---|
| COSINE | 0.519 | — | |
| NAME-ONLY | 0.508 | −0.010 [−0.067, +0.042] | |
| **CONSTRAINT** | **0.536** | **+0.017 [−0.066, +0.097]** | **not significant** |
| **ANCHORVOC** | **0.664** | **+0.145 [+0.072, +0.219]** | **significant** |
| ANCESTRY | 0.534 | +0.025 [−0.069, +0.121] | |

For comparison, the same scorers on synthetic errors:

| | COSINE | CONSTRAINT | ANCHORVOC |
|---|---:|---:|---:|
| random injection (Stage 12's) | 0.882 | 0.675 | 0.654 |
| nearest injection (E15) | 0.506 | **0.612** | 0.584 |
| **real misfilings (E16)** | **0.519** | 0.536 | **0.664** |

## What this settles

**CONSTRAINT's E15 advantage does not transfer.** It won by +0.106 on plausible
synthetic errors and gains +0.017 on real ones, with a confidence interval
straddling zero. **E15's conclusion that "the certification question reopens" was
premature and is withdrawn.** Plausible-synthetic is closer to real than
random-synthetic, and still not the same thing.

**Everything geometric is at chance on real misfilings** — COSINE 0.519,
NAME-ONLY 0.508 — which is Stage 9's 0.410 reproduced on a larger and
better-controlled sample. That conclusion is now confirmed three ways.

**The scorer that wins on real data won on neither synthetic task.** ANCHORVOC
was mid-pack under both injections (0.654, 0.584) and is the only significant
result here. Had the track only ever run injections, it would have been ranked
fourth and discarded.

## The confound that must be resolved before ANCHORVOC is believed

**ANCHORVOC measures lexical overlap between an entry and its cell's authored
anchor hints. The blind rater judged entries against those same anchors.**

So ANCHORVOC may be detecting *"this entry shares vocabulary with the text the
rater used to decide"* rather than *"this entry is misfiled"*. It shares an input
with the ground truth, which is precisely the shape of circularity this track has
learned to check for.

**This is not a reason to dismiss it — it is a reason not to bank it yet.** Two
cheap tests would separate the readings:

1. **The 76 non-blind entries** (45 moved, 31 same) are flagged in the same file.
   The rater's procedure was identical there but blindness failed. If ANCHORVOC's
   margin is about the rater's procedure it should persist; if it is about real
   misfiling it should behave differently.
2. **Score ANCHORVOC against a held-out half of each anchor's hints**, using one
   half to build the probe and the other to score. If the effect survives, it is
   about the cell's vocabulary rather than about the specific strings the rater
   read.

## Where this leaves the track

| claim | status |
|---|---|
| Geometry certifies filings | **NO** — 0.519 on real errors, three independent measurements |
| Stage 12's refutations were valid | **NO** — measured under an injection policy that inverts the ranking (E14/E15) |
| Structural scorers certify filings | **NO** — CONSTRAINT +0.017, ns |
| Lexical anchor overlap certifies filings | **OPEN** — +0.145 significant, one serious confound unresolved |

The honest net after E14–E16 is that the track's central negative stands — *no
tested mechanism detects a real misfiling* — but the reason it stood was wrong
for years, and one candidate has appeared that was never fairly tested because
every earlier evaluation used an injection policy that ranked it fourth.
