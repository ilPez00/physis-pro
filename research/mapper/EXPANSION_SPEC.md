# Corpus expansion — what to add, how much, and what it buys

`research/mapper/expansion_spec.py` → `expansion_spec.json`

Every experiment in this track converged on one constraint: **658 entries is too
few for a 70-cell grid.** This turns that into a worklist with a forecast and an
acceptance test.

## The target, and where it comes from

**20 entries per cell.** Not a guess — E7 subsampled well-populated modes and
read the F1(n) curve off modes that actually have data:

| training entries | 3 | 6 | 10 | **20** | 40 | all |
|---|---:|---:|---:|---:|---:|---:|
| per-mode F1 | 0.06 | 0.10 | 0.21 | **0.38** | 0.46 | 0.52 |

Most of the available gain is bought by the first twenty; the curve is nearly
flat from 20 to 53. Twenty is where the returns stop justifying the authoring.

## Deficit, worst first

| mode | entries | F1 now | needed | F1 at target | gain |
|---|---:|---:|---:|---:|---:|
| **BRAINSTORM** | 3 | 0.06 | 97 | 0.52 | **+0.46** |
| **DESTROY** | 4 | 0.06 | 96 | 0.52 | **+0.46** |
| **PLAY** | 13 | 0.21 | 87 | 0.52 | **+0.31** |
| **LIFT** | 13 | 0.21 | 87 | 0.52 | **+0.31** |
| **GUIDE** | 14 | 0.21 | 86 | 0.52 | **+0.31** |
| MOVE | 25 | 0.38 | 75 | 0.52 | +0.14 |
| WALK | 28 | 0.38 | 72 | 0.52 | +0.14 |
| REST | 31 | 0.46 | 69 | 0.52 | +0.06 |
| PLAN | 37 | 0.46 | 63 | 0.52 | +0.06 |
| LEARN | 53 | 0.46 | 61 | 0.52 | +0.06 |
| MAINTAIN | 77 | 0.52 | 33 | — | +0.00 |
| SENSE | 93 | 0.52 | 33 | — | +0.00 |
| CREATE | 117 | 0.52 | 37 | — | +0.00 |
| WORK | 150 | 0.52 | 22 | — | +0.00 |

**The five starved modes hold 100 entries between them.** They are where the
whole gain is: +0.31 to +0.46 F1 each, against +0.00 for the four largest.

## Order of work

1. **BRAINSTORM and DESTROY first** — 7 entries between them, and the largest
   forecast gain. They currently score F1 = 0.00, which E7 showed is what three
   entries produces rather than evidence of a bad definition.
2. **PLAY, LIFT, GUIDE next** — 40 entries between them, +0.31 each.
3. **Stop there and re-measure.** Do not populate MAINTAIN / SENSE / CREATE /
   WORK: they are already past the curve's flat region and the forecast gain is
   zero. Adding entries there costs authoring and buys nothing measurable.

## Acceptance test — fixed before the work, so it cannot be moved after

Re-run `run_e5.py` and `run_e6.py` on the expanded corpus. The expansion
succeeded if:

- **per-mode F1 for each populated mode reaches ≥ 0.26** (the bottom of E7's
  forecast band at n ≈ 20);
- **mode-level kappa rises above 0.418**, its current value;
- **cell-level kappa rises above 0.404**;
- and the **hypertrophy index (E8, currently −0.644) does not fall further** —
  if newly added entries are absorbed into cells without differentiating them,
  the expansion has added volume rather than structure, and that is the failure
  mode to watch for.

The last one matters most. E8 showed the largest modes sit +0.003 above a random
sample of the corpus; an expansion that makes the starved modes look like the
large ones has not helped.

## What did NOT work: mining candidates from the corpora

Three attempts, each fixing a real defect, and the output remained unusable:

1. **Similarity-summed terms** returned Italian stop words for every mode
   (`della`, `delle`, `degli`) — the stop list was English-only and the scoring
   rewarded frequency.
2. **Contrastive scoring** fixed the frequency problem and then returned terms
   distinctive of the *codebase* (`ontologyloader rand`, `seedablerng`,
   `commit branch`). Mining physis's own source for physis's ontology is
   circular by construction.
3. **Document library only** returned prose vocabulary (`parentesi`, `pistol`,
   `alchemist`, `vspace`) — real terms, correctly contrastive, and not ontology
   entries.

**Recorded as a negative result: free prose is the wrong source for ontology
entry candidates by this method.** An entry is a short domain concept
("Thermodynamics", "Peak Exertion"); philosophy prose and source code contain
neither in extractable form. A working miner would need a source that already
contains concept-shaped items — a glossary, an index, a taxonomy — or a
generative step rather than an extractive one.

The deficit table above does not depend on the miner and stands on its own.

## A privacy defect found and fixed

The third mining run surfaced `autorizzo trattamento dati personali` — GDPR
consent boilerplate from a CV filed under `Philosophy_and_Books`, where neither
the directory exclusion nor the filename filter could catch it.

**Nothing sensitive reached a committed file** — `docs_corpus.json` carries only
`i, src, cell, score, len`, and a grep for the boilerplate across committed
artifacts returns zero. The extraction working file is gitignored.

`extract_docs.py` now applies a **content-level** filter — consent formulas, CVs,
fiscal and bank identifiers — because a document stored somewhere unexpected can
only be caught by what it says, not by where it sits.
