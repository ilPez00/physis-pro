# Research to do — the open conceptual problems

Written 2026-09-13, after problems 1 and 4 were measured and closed enough to
say what they are. This is the read-and-run list for what is left. Each item is
an experiment with a corpus, a control, and a result that can come back
negative. Nothing here is a task list; if an item cannot fail it is not on it.

Ordering is by *information per hour*, not by importance. Item A is first
because it is a two-hour experiment that could invalidate a whole architectural
direction, and item E is last because it is a month and might not be answerable.

---

## A. The hybrid: does token evidence rescue polarity without losing topic?

**Status:** open, cheapest, and the direct consequence of today's result.

The polarity arm measured the two embedders failing in opposite directions on
the same eight pairs:

| | finds the topic | reads the verdict |
|---|---|---|
| bge-base-en-v1.5 | paraphrase recall 0.875 | discrimination **0.250** |
| random-projection | paraphrase recall **0.000** | discrimination 0.750 |

Neither dominates, and nothing in the crate combines them.

**The experiment.** Score each claim as `α · cosine(semantic) + (1−α) · lexical`,
where the lexical leg is any cheap token-overlap statistic — BM25 over the
claim statements is the obvious first choice and needs no model. Sweep α from
0.0 to 1.0 in steps of 0.1 and plot both existing metrics against it: paraphrase
recall@5 (the topic leg) and polarity discrimination (the verdict leg).

**The prediction to record before running it.** There is an α that beats both
endpoints on both metrics — because the two legs fail on *different* pairs, not
on the same ones with different severity. If instead the curve is monotone
between the two endpoints, the legs are redundant and the hybrid is dead.

**The control.** α = 1.0 and α = 0.0 are the two published columns; they must
reproduce 0.875/0.250 and 0.000/0.750 exactly, or the harness is not measuring
what it did yesterday.

**Why this is first.** Every other item in this file assumes the retrieval leg
can be made to read a verdict. If the hybrid curve is flat, the answer is that
retrieval *cannot* carry polarity and the discrimination has to come from the
structural identity of item C — which changes what C is for, from an elegance
argument into the only remaining option.

**Cost:** one afternoon. No model, no service, no data collection.

---

## B. The relevance floor: `act` warns on half of all unrelated commands

**Status:** open, found today by the cost arm, larger than the problem it was
built to measure.

`act` takes the top five by cosine and prints whichever are refuted. There is no
floor. Measured: **4 of 8 commands with nothing to warn about get a warning
anyway** (`act_recall`, `top 5 · relevance` row). On a real ledger of hundreds
of claims that number goes up, not down, because there is always *something* at
rank five.

This is the failure mode the survey attributes to Nepomuk and to every semantic
desktop that died: the system is *felt*, constantly, for nothing.

**The experiment.** Add an absolute or leader-relative floor below which no
warning is printed, and sweep it against the two arms already built:
false-alarm rate on `UNRELATED_COMMANDS` (must fall) and `target_in_top` on
`PAIRS` (must not). The floor that maximises the gap is the answer; if no floor
separates them, cosine relevance is not the right gate and the gate has to be
something else — the same conclusion as A, reached from the other side.

**The harder version, and the one worth doing.** A floor tuned on eight
synthetic commands is tuned on nothing. The real question is the *distribution*:
on a ledger of N real claims and a stream of real commands, what fraction of
commands have any claim above the floor? That needs a real ledger, which the
dogfood loop (`just dev-loop`) has been accumulating. Measure it there before
picking a number.

**Cost:** an afternoon for the synthetic sweep; a day for the real-ledger
distribution, most of it extracting the ledger.

---

## C. Problem 2 — structural claim identity

**Status:** open. The design is published; the decision is not.

A claim is identified by its sentence, and every relation that matters —
contradiction, dedup, supersession, `act`'s recall — runs on the embedding of
that sentence. The graphiti state-hash experiment measured entities, topology
and temporal boundaries as byte-identical across a 7B and a 120B interpreter,
with the prose differing across models *and across two runs of one model*. So
the relations are computed on the one layer shown to be noise.

### Read first, in this order

1. **TOKI** (arXiv 2606.06240) — bitemporal operator algebra. Valid time and
   transaction time as separate dimensions; contradiction defined as
   *overlapping valid intervals asserting incompatible facts*. Its important
   move is that it keeps **both** identity mechanisms — structural keys and
   embedding similarity — arguing each catches what the other misses. That is a
   stronger position than the plan's "demote prose to rendering", and the first
   research question is whether their argument survives physis's own graphiti
   result. **Their numeric tables did not survive PDF extraction here** — get
   them before citing.
2. **STALE** (arXiv 2605.06527) — 400 conflict scenarios, three probes. Its
   *Implicit Conflict* mode is a later observation invalidating an earlier
   memory with no explicit negation, which is precisely the case today's
   polarity arm shows the embedder cannot see. Its third probe, Implicit Policy
   Adaptation, is a description of what `act` does.
3. **NeuSymMS** (arXiv 2605.17596) — automated truth maintenance in CLIPS,
   retracting stale facts as new arrive. A working version of what
   `hypothesis.rs` does by hand.
4. **AGM** (Alchourrón, Gärdenfors & Makinson, 1985) and **Doyle's TMS** — the
   ancestry the crate's `truth-maintenance` keyword claims. Read once, cite
   once, stop reinventing the postulates.

### The experiments

**C1 — score against STALE instead of the ledger.** The strongest thing
available and the least inventive. Everything measured today is on a corpus this
project authored, which is a ceiling and not an estimate. STALE is external,
expert-validated, and its probes line up with what physis claims. Run it, report
the three probe scores, and stop quoting self-authored numbers as the headline.
*This should be done before C2, not after: it is the baseline C2 has to beat.*

**C2 — does structural identity change contradiction recall?** The pre-existing
number is **0.50** — one of two known pairs missed, unchanged by the embedder,
which RESCOPE §6 item 7 already calls a map defect rather than a retrieval one.
Extract (subject, relation, object, interval) for each claim, compute
contradiction on the tuple, and compare. The control is the current
embedding-only path on the same corpus. If it does not move 0.50, the elegance
argument is all that is left and it is not enough.

**C3 — the interpreter-swap test, run on the ledger rather than on graphiti.**
Repeat the state-hash experiment against physis's own claims: same evidence,
two interpreters, hash the entity set, the edge topology, the intervals, and the
prose. The graphiti result predicts the first three are stable and the fourth is
not. If that reproduces here, C2's design is settled by measurement rather than
by argument. If it does not reproduce, the whole premise of problem 2 is weaker
than stated and should be written down as such.

**Cost:** C1 two days, mostly harness. C2 a week. C3 two days and it needs both
model endpoints available (see `fleet-llm-endpoints`).

---

## D. Problem 3 — the ontology is the one belief that cannot be contradicted

**Status:** open, and it is the item with **no published design to copy.**

Every other belief in the engine can be supported, contradicted, superseded and
replayed. The 70-cell grid cannot. It is authored in advance and sits underneath
the machinery that exists to make beliefs falsifiable, exempt from it — while
E5 already found that collapsing 57 → 14 classes *gains* +0.014 kappa and E8's
pre-flight STOPs on sparsity at every level.

The nearest thing in the literature is **LKD-KGC** (surveyed in arXiv
2510.20345): adaptive embedding-based schema integration, where equivalent
entity types are merged by clustering and schema alignment *emerges from the
data* rather than being authored. That is the same move from the construction
side. It is not the same argument, and the falsifiability argument appears to be
unoccupied.

### The experiments

**D1 — give the grid a validity window and a fitness, and see what happens to
E5.** The mechanism already exists: `TemporalValidity`, `narrow_until`, the
revision history. Treat each cell as a hypothesis, let a mis-classification
count as contradicting evidence, and replay the existing corpus through it. The
question is not whether it is elegant; it is **whether any cell's fitness
actually falls**. If every cell keeps a fitness near 1.0 after a full replay,
the mechanism is decorative and should not ship.

**D2 — the merge that E5 already justifies, done as evidence rather than as an
edit.** `FABRICATE` ↔ `CONSTRUCT` confuse at 0.22/0.21, symmetric — the cleanest
merge candidate in the grid (E22). Rather than editing the ontology, record the
confusion as contradicting evidence against the two cells and let the mechanism
from D1 produce the merge. If the mechanism cannot produce a merge the evidence
plainly supports, it is not a belief system, it is a fitness display.

**D3 — the honest negative to look for.** A grid that revises itself can drift
into incoherence one observation at a time, and nothing in the crate would
notice. Before D1 ships, define the null: a *label-permuted* replay, where the
same evidence is fed against shuffled cell labels. If fitness falls the same way
under permutation, the mechanism is responding to volume rather than to fit.
This control is not optional and it is the reason D1 is a week and not a day.

**Cost:** D1 a week including D3. D2 two days on top.

---

## E. The core/pro boundary — a strategy question wearing a tidiness costume

**Status:** open, unscheduled, and the only item here that is not an experiment.

`rag.rs` exists in both crates and the physis-pro copy is the one the `pack` CLI
uses, so the MMR fix landed in core and did not reach the CLI until it was made
twice (PH-012). `machines.rs` went to core this session. `direction.rs` is in
core. There is no stated rule for what belongs in the Apache-2.0 core versus
Pro, and the two are diverging by accident.

There is no experiment for this. It needs a written rule, one paragraph, and
then a sweep for everything already on the wrong side of it. The rule the
evidence suggests: **the core holds what is measured and published; Pro holds
what is integrated and supported.** `act_recall`, `direction` and `map` have
measurements in the open and belong in core by that rule. Whether the ledger
does is the actual decision, and RESCOPE §0 already says the ledger is the
differentiator — which argues the opposite way. That tension is the thing to
resolve, and it is a business call, not a technical one.

---

## What is closed, so nobody reopens it

- **Problem 1 — `act`'s recall.** Measured: 1.000 lexical / 0.875 paraphrase at
  top 5 on a semantic embedder, 0.000 paraphrase on the fallback. The
  pessimistic 2×2 reading is withdrawn. What replaced it is the polarity result
  (discrimination 0.250, inverted) and the false-alarm finding in item B — both
  larger than the thing originally suspected.
- **The reserve fix.** Implemented, measured, declined: at best one harm removed
  per false alarm created, and at the default cut it is pure cost. Do not
  re-propose it without a corpus where the trade looks different.
- **Problem 4 — capability sweep.** `physis-check capabilities` ships, with the
  manifest at `physis-core/.physis-capabilities`. 12 of 13 pass. The red row is
  a genuine finding, not a broken check.

---

## The rule that applies to all of it

Every item above names its control. An item whose control was never run is not
a result, and the two defects the capability sweep shipped with today — a check
that read its own input, and a heuristic that marked live code dead — were both
found by asking one question: *what would this look like if it were broken?*
Ask it before reporting anything from this file.
