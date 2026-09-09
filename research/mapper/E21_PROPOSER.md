# E21 — geometry as a proposer, not a judge

**The first positive in this track that survives its construction-matched
control.** Nothing about the geometry changed. What changed is what was asked
of it.

## The move

Every refutation here — Stage 12, E12–E18 — refuted **unsupervised, zero-shot
judgement**. COSINE, NAME-ONLY, CONSTRAINT, ANCESTRY and ANCHORVOC are fixed
formulas with nothing fitted to a human decision, and on real misfilings they
sit at chance (0.519–0.536). The **supervised, human-in-the-loop** version was
never measured.

And the question was never quite the right one. "Can the machine judge whether
an entry belongs?" is settled **NO**, seven times over. The question a person
filing 582 entries actually has is:

> **Can the machine narrow 57 cells to 3 so that I choose instead of search?**

That is standing rule 5 — *sell the filter, not the judgement* — applied to
filing rather than to discovery.

## Method

`run_e21.py`. Ground truth is `refiling_gate0.tsv`, Stage 11's blind Gate 0
re-filing: 582 entries, 57 distinct target cells, the rater's **revised**
`new_cell` as the target rather than the old broken corpus. The 76 non-blind
entries are excluded, as in E16. Vectors are the shipped `bge-base-en-v1.5`
768-d from `corpus.json`; no inference is run.

Nearest-centroid over cells built from the decisions already made, scored as
**top-k containment of the rater's own cell**, on a learning curve over how many
decisions the rater has made. Five shuffles per point.

## Result

Top-k containment, by number of decisions seen:

| decisions seen | top-1 | top-3 | top-5 |
|---:|---:|---:|---:|
| 58 | 0.211 | 0.385 | 0.465 |
| 145 | 0.307 | 0.508 | 0.591 |
| 291 | 0.368 | 0.595 | 0.689 |
| **436** | **0.438** | **0.667** | **0.755** |

Controls at 436 decisions seen — house rule 1:

| control | top-1 | top-3 | top-5 | what it rules out |
|---|---:|---:|---:|---|
| **GEOMETRY** | **0.438** | **0.667** | **0.755** | — |
| PRIOR `old_cell`→`new_cell` | 0.184 | 0.368 | 0.474 | the rater moving systematically; a lookup table |
| MAJORITY | 0.089 | 0.255 | 0.359 | the base rate |
| **PERMUTED** | 0.036 | 0.136 | 0.233 | **the construction-matched null** |
| LEN-ONLY | 0.010 | 0.036 | 0.071 | E18's length confound |

Geometry beats its permuted null by **+0.531 top-3**, and beats the lookup table
by **+0.299**. The learning curve is **still climbing at 436** — more human
decisions were still paying at the point the data ran out.

## Which half of the filing the machine can carry

The 57 cells are `domain × mode`. Scored per axis, at 436 decisions seen:

| axis | classes | top-1 | top-2 | permuted | majority | lift over majority |
|---|---:|---:|---:|---:|---:|---:|
| **domain** | 5 | **0.626** | **0.837** | 0.174 | 0.205 | **3.05×** |
| mode | 14 | 0.460 | 0.678 | 0.134 | 0.233 | 1.97× |

**The machine can carry the domain axis and cannot carry the mode axis** — 0.837
top-2 over five domains against 0.678 over fourteen modes, and the gap survives
normalising for class count (3.05× vs 1.97× lift). That is the same asymmetry
E6/E7 found and explained by population rather than by missing definition, now
stated as a division of labour: **the machine proposes the domain, the person
picks the mode.**

## What this does NOT show, stated before anyone quotes the table

1. **Top-1 is not new.** 0.438 essentially reproduces E5's 0.447 accuracy on the
   old labels. **The geometry did not get better.** A number this track recorded
   as a failure — "geometry classifies at 0.45, useless" — is the same number
   that makes a usable proposer once the output is three candidates instead of
   one. The finding is about the question, not the representation.
2. **It is not certification.** Nothing here says an entry *belongs* in the
   proposed cell. E14–E18 stand untouched. A proposer that is right two times in
   three at top-3 is useful precisely because a person checks it.
3. **It predicts a policy, not a truth.** The rater was Claude, and the same
   rater produced the replacement filing (E16's caveat). Under "is the machine
   right" that is a confound. Under this framing it is the target — the artifact
   exists to pre-sort for an arbiter — but **whose** policy it learned is
   untested, and a second rater is the experiment that would settle it.
4. **Supervision is doing the work, and supervision has to come from somewhere.**
   436 decisions is most of a corpus. The honest claim is "geometry converts
   human decisions into leverage over the remaining ones", not "geometry
   replaces them". The learning curve is the thing to read: at 58 decisions
   top-3 is 0.385, already 1.5× the majority floor.

## Where it goes

- **A second rater** on a sample, to separate "learned the policy" from "learned
  the corpus". This is the one that decides whether the result is worth building on.
- **The mode axis is the constraint, again.** Every route into it has landed on
  population (E7: n ≈ 20 → F1 0.26–0.49). A proposer that carries domain and
  defers mode is deployable *now* and does not wait on that.
- **Does a stronger embedder move it?** Yes, and unevenly:

| | BGE-base (shipped) | nomic-embed-text | Δ |
|---|---:|---:|---:|
| overall top-1, 436 seen | 0.438 | 0.452 | +0.014 |
| **mode top-1** | 0.460 | **0.525** | **+0.065** |
| domain top-1 | 0.626 | 0.660 | +0.034 |
| **domain top-2** | 0.837 | 0.840 | **+0.003 — saturated** |

  Domain top-2 does not move: what limits it is label noise or real ambiguity,
  not encoding. Mode does move, so mode still had representational headroom —
  which E24–E29 then spent, without closing it.

---

## What came after this, and what it changed

| | result | effect on this report |
|---|---|---|
| **E24** | hints halved (α = 0.5) beats the shipped vector | **supersedes the numbers above.** Top-3 rises 0.651 → **0.705**, *t* = +12.13, better in 24/24 paired splits (E28) |
| **E22** | hints are domain-loaded; the mode axis is three separate problems | explains the domain/mode gap found here |
| **E26** | domain compresses to 4-d losslessly; mode is not a subspace | gives the gap a mechanism — domain is a linear object, mode is not |
| **E27** | merging modes makes the axis *less* coherent | closes the obvious fix |
| **E28** | the split proposer loses 0.119; **coherence ≠ predictive power** | corrects a wrong prescription in E27 |
| **E29** | clustering the mode blur recovers *subject matter*, and the current labels beat every discovered carve | mode is **absent from the representation**, not underdetermined by its labels |

**The shipping configuration is E28's variant B**: one joint proposer over cells,
on the α = 0.5 vector. The split-by-axis design proposed in this report's own
"where it goes" section was built and **failed**.
