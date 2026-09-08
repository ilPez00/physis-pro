# E14 — The 0.866 belongs to errors nobody would make

`research/mapper/run_e14.py`

Two numbers in this track were never reconciled:

- **Stage 9: AUC 0.410** detecting the corpus's *own* misfilings — chance inside
  the interval.
- **E1: AUC 0.866** detecting *injected* misfilings, same corpus, same scorer.

That gap has been carried for the whole track, and **0.866 is the number I have
repeatedly cited as the best scorer this work has produced.**

## The hypothesis

Stage 12's injection moves an entry to a **uniformly random** other cell. A real
misfiling is a near-miss — a careful author confuses adjacent cells, not distant
ones. So the injected task may be easy for a reason unrelated to the scorer.

## Result — plausibility explains the whole gap

Same corpus, same scorer, same injection rate. Only *where* a misfiled entry
goes changes.

| injection policy | AUC |
|---|---:|
| random cell — **Stage 12's original** | **0.880** |
| same DOMAIN, different mode | 0.827 |
| same MODE, different domain | 0.771 |
| **the entry's NEAREST alternative cell** | **0.506** |
| *Stage 9, the corpus's own misfilings* | *0.410* |

**AUC falls from 0.880 to 0.506 — to chance — purely by making the injected
error one a careful author might actually make.** The residual gap to Stage 9's
0.410 is −0.096.

The mechanism is visible in how far an entry moves:

| policy | similarity to the cell it lands in | to its own cell | deficit |
|---|---:|---:|---:|
| random | 0.664 | 0.789 | **+0.125** |
| same-mode | 0.684 | 0.789 | +0.105 |
| same-domain | 0.678 | 0.789 | +0.110 |
| **nearest** | **0.760** | 0.789 | **+0.029** |

**The scorer detects distance moved, not wrongness.** Random injection drops an
entry 0.125 below its home similarity; a plausible error drops it 0.029, and the
detector cannot see it.

## What this caveats, precisely

This does not overturn any experiment's internal logic. It changes what the
numbers are *about*.

1. **"Plain cosine at 0.866 is the best certification scorer in this track"** —
   which I have written repeatedly — should read **~0.51 on realistic errors**.
   There is no working certification scorer. That is Stage 9's original
   conclusion, now with a mechanism.
2. **Stage 12's refutations need re-testing, and I am not claiming they fall.**
   CONSTRAINT (0.685), ANCESTRY (0.604) and NON-LATTICE (0.459) were all
   measured against cosine's 0.733 *under random injection*. Under realistic
   injection cosine drops to ~0.51. Whether the structural scorers drop as far
   is **unmeasured** — they may be structural precisely where geometry is not.
   This is the single most valuable re-test the track now has.
3. **E1's absolute numbers are task-specific**; its mechanism finding is not.
   That `D = |s1 − s2|` is a folded read-out of `s2` is arithmetic and holds
   under any injection policy. The 0.866 and the 22/22 sweep are about
   implausible errors.
4. **The Disposer Bench's gold items are random-injected**, so its machine
   precision@50 of 0.640 is inflated for the same reason. A human adjudicating
   that bench is being shown easier errors than the corpus actually contains,
   and their measured recall will overstate what they would achieve in practice.

## What it does *not* caveat

- **E5, E7, E8, E13** measure cell *prediction* and *retrieval*, not misfiling
  detection. They never used injection and are unaffected.
- The corpus-sparsity conclusion is untouched and remains the binding constraint.

## What was honest about the original design, and what was not

Stage 12's header says it plainly: *"the ground truth is INJECTED, so there is
no argument over what the right answer is"* and *"a synthetic perturbation is a
lower bound on difficulty — real misfilings are the ones a careful author already
thought were right."*

**The caveat was written down from the beginning and then not propagated.** Every
subsequent citation of 0.733 and 0.866 — including mine, repeatedly, this
session — treated them as measures of the scorer rather than of the injection.
E14 is not a new insight so much as the original caveat finally quantified:
the lower bound on difficulty turns out to be the entire signal.

## The re-test that matters

Run the Stage 12 structural scorers (CONSTRAINT, ANCESTRY, NON-LATTICE) under
`nearest`-cell injection. Cosine falls to 0.506 there. If a structural scorer
holds above it, the Stage 12 refutations were an artifact of the injection policy
and the whole certification question reopens — on the side of structure, which is
where this track spent most of its effort and found nothing.

If they fall too, then nothing detects a plausible misfiling, and the
human-in-the-loop conclusion becomes the only one available. Either result is
worth more than anything else currently open.
