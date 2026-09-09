# E22 — what the corpus says the axes should be

`run_e22.py`. A re-brainstorm from a blank page repeats whatever intuition
produced the current axes. This asks the 582 blind re-filings instead, and
**proposes only** — `GRID_AXES.md` is not touched by any script here.

Same human/machine division as E21, one level up: **the machine proposes the
grid revision, the person decides.**

---

## 1. The finding that reframes the mode problem

`corpus.json` ships two vectors per entry: name alone, and name + hints. Paired
over 40 identical splits, 75% train, held-out top-1:

| axis | hints − name-only | *t* | hints better in |
|---|---:|---:|---:|
| domain | **+0.022** | +3.13 | 27/40 splits |
| **mode** | **−0.034** | **−5.39** | **6/40 splits** |

**The corpus's own enrichment degrades the mode axis.** Hints are subject-matter
vocabulary — *"endpoint, contract, version, schema, protocol"* — so they are
**domain** signal. Adding them pulls every entry toward what it is *about* and
away from what is being *done*.

E6/E7 explained the weak mode axis by population (r = +0.718 with log class
size). That stands, but it is not the whole account: **part of the mode axis's
weakness is representational, and the corpus has been enriched in the direction
that makes it worse.** Nobody was going to find this by adding entries.

Corroborating, from E21's embedder comparison — a stronger embedder buys most
where the representation is the binding constraint:

| | BGE | nomic | Δ |
|---|---:|---:|---:|
| domain top-2 | 0.837 | 0.840 | **+0.003 — saturated** |
| mode top-1 | 0.460 | 0.525 | **+0.065** |

Domain top-2 does not move with a better embedder; what is left there is label
noise or real ambiguity. Mode moves, so mode still has representational headroom.

## 2. DOMAIN — 5 classes, all well populated

| domain | n | held-out top-1 |
|---|---:|---:|
| FABRICATE | 140 | 0.64 |
| STUDY | 137 | 0.61 |
| CONSTRUCT | 129 | 0.49 |
| HEAL | 94 | 0.67 |
| BOND | 82 | **0.76** |

Confusions (rate, with the reverse rate):

| pair | → | ← | reading |
|---|---:|---:|---|
| **FABRICATE ↔ CONSTRUCT** | **0.22** | **0.21** | **mutual and symmetric — the corpus does not distinguish these at all** |
| CONSTRUCT ↔ HEAL | 0.18 | 0.10 | mutual |
| STUDY → FABRICATE | 0.15 | 0.07 | one-way |
| BOND → CONSTRUCT | 0.14 | 0.03 | one-way; CONSTRUCT is the default sink |

**Proposal.** FABRICATE and CONSTRUCT are one distinction wearing two names, at
0.22/0.21 symmetric confusion — the cleanest merge candidate anywhere in the
grid. Either merge them (5 → 4 domains) or write the sentence that separates
making-a-thing from building-a-structure and re-file against it. CONSTRUCT is
also the sink that BOND leaks into one-way, which usually means its definition
is too broad rather than that BOND is wrong. BOND at 0.76 is the axis's
best-defined class and should be the model for the others.

## 3. MODE — 14 classes, and three different problems

| mode | n | top-1 | |
|---|---:|---:|---|
| WORK | 133 | 0.44 | |
| CREATE | 107 | 0.53 | |
| SENSE | 84 | 0.54 | |
| MAINTAIN | 73 | 0.39 | |
| LEARN | 36 | 0.66 | |
| PLAN | 35 | 0.64 | |
| WALK | 27 | **0.18** | **adequate n, terrible accuracy** |
| MOVE | 25 | 0.42 | |
| REST | 21 | 0.58 | |
| GUIDE | 13 | 0.14 | starved |
| PLAY | 12 | 0.48 | starved |
| LIFT | 11 | 0.48 | starved |
| DESTROY | 4 | 0.44 | starved |
| BRAINSTORM | **1** | 0.00 | starved |

**These are three separate problems and they need three different responses.**

**(a) The core blur — 68% of the corpus.** WORK, CREATE, MAINTAIN and SENSE hold
397 of 582 entries and are mutually confusable: WORK↔CREATE 0.18/0.13,
WORK↔MAINTAIN 0.12/0.09, WORK↔SENSE 0.11/0.13, MAINTAIN↔SENSE 0.18/0.10. WORK
is at 0.44 and MAINTAIN at 0.39 *with ample population*. **This is where the
mode axis actually fails**, and it is not a sparsity problem — it is four
definitions that do not carve anything apart. Any re-brainstorm that does not
start here is decorating.

**(b) A definition failure, not a population failure.** WALK has n = 27 —
comparable to LEARN (36, 0.66) and PLAN (35, 0.64) — and scores **0.18**. E7's
population account predicts WALK should be around 0.5–0.6. It is not. WALK is
the one class where the corpus has given the definition a fair trial and it
failed. It leaks to SENSE both ways (0.06/0.12). **Redefine or retire it**, and
unlike the starved classes, the corpus has earned the right to say so.

**(c) Genuinely unmeasured — do not judge these.** GUIDE 13, PLAY 12, LIFT 11,
DESTROY 4, BRAINSTORM **1**. E7 showed a well-populated mode cut to 3 entries
scores F1 0.06, so these numbers say nothing about the distinctions. Populate
each to n ≈ 20 or drop it, but **do not read their accuracy as evidence**.
BRAINSTORM at n = 1 is not a class; it is a note.

One exception with a real signal: **GUIDE → LEARN at 0.48 one-way** (reverse
0.01). Half of everything filed GUIDE lands in LEARN and nothing comes back.
Even at n = 13 that asymmetry is informative: as written, GUIDE reads as a
special case of LEARN. It needs either a definition that is not teaching, or a
merge.

## 4. The brief this hands to the re-brainstorm

1. **Fix the WORK/CREATE/MAINTAIN/SENSE blur first.** 68% of entries, ample
   population, and the four definitions do not separate. Everything else is
   smaller than this.
2. **FABRICATE vs CONSTRUCT**: merge, or write the distinguishing sentence.
3. **WALK**: the corpus tested it fairly and it failed. Redefine or retire.
4. **GUIDE**: as written it is a kind of LEARN.
5. **Populate or drop** PLAY, LIFT, DESTROY, BRAINSTORM. Do not argue about them
   on current evidence.
6. **Stop enriching entries with subject-matter hints and expecting mode to
   improve.** It measurably does the opposite (§1). Mode needs text about what
   is being *done*, which is a different modality — see the supervision note in
   `E23_SUPERVISION_SOURCES.md`.
