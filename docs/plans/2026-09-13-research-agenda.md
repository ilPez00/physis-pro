# Research to do — the open conceptual problems

Written 2026-09-13, after problems 1 and 4 were measured and closed enough to
say what they are. This is the read-and-run list for what is left. Each item is
an experiment with a corpus, a control, and a result that can come back
negative. Nothing here is a task list; if an item cannot fail it is not on it.

Ordering is by *information per hour*, not by importance. **A and B were run
the same day and are closed here with their results**, including one prediction
that failed and produced a better fix than the one it predicted. C and D are
open. The core/Pro boundary that was item E has been dropped from this file:
it is a business decision, not an experiment, and it was cluttering a list of
experiments.

---

## A. The hybrid — CLOSED 2026-09-13, prediction wrong, fix found anyway

Predicted an interior mixing weight beating both endpoints. **There is none.**
From alpha 0.60 to 1.00 the mixture is identical to cosine alone — the lexical
term is too small to reorder anything — and below 0.60 topic recall collapses
before polarity improves. The crate's own `rrf@60` lands in the same dead zone.
One score cannot do two jobs.

The legs *are* complementary; the endpoints say so (BM25 alone reads the verdict
at 0.875/0.625 where cosine reads it at 0.250/0.125). Mixing them into one score
was the mistake. Retrieval and ranking are different problems.

`Selection::CascadeRerank` — cosine retrieves the pool, BM25 orders it — gives
paraphrase recall 0.875 (unchanged, and unchanged *by construction* at
`pool = top`, since recall is set membership) with polarity 0.250 → 0.875 and
0.125 → 0.625. Combined 1.000 → 1.500. Through the cost arm: identical to
cosine at top 5, and at top 1 removes 4 of 5 reassurances for 2 false alarms —
2.00 harm removed per alarm, twice the warning reserve's best. Shipped in the
CLI; `act::bearing_on` keeps the cosine order as the library's frozen baseline.

Found by noticing `rag::Bm25Index` and `rag::fuse_rrf` have existed since G5,
are advertised in `lib.rs` as "G5 hybrid retrieval", and were called by nothing
in the ledger.

---

## B. The relevance floor — CLOSED 2026-09-13, and it is absolute

The floor exists. The leader-relative rule inherited from `WarningReserve` does
**nothing** at 0.80/0.90/0.95, because the leader is high for both classes of
command. `examples/floor_spread` shows the classes separating on the absolute
score instead: commands with a refutation bottom out at cosine **0.717**,
commands with nothing to say top out at **0.643**. Disjoint, 0.074 apart.

`cascade@5 + floor 0.66` takes false alarms from 4/8 to **0/8** at both cuts
while keeping every real warning — strictly better than the current default on
every column measured. 0.70 already drops real warnings, so the usable window is
exactly the measured gap, which is the check that the number is the corpus's and
not the author's.

Embedder-specific, and that is the cost: on random-projection the two
distributions overlap completely and no floor exists, so the CLI applies none
there.

**What is still open from B:** every number above is from eight synthetic
commands. The real question is the *distribution* on a real ledger — of N real
claims and a stream of real commands, what fraction clear 0.66? The dogfood loop
(`just dev-loop`) has been accumulating one. Measure there before trusting 0.66
outside this corpus.

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

**C1 — score against STALE. BLOCKED 2026-09-13: no public dataset found.**
The paper is up (arXiv 2605.06527, HF papers page) and no dataset or code
repository was findable by search. Until one appears, physis cannot score
against it. Two things to carry anyway:

- STALE's own headline is that **the best evaluated model reaches 55.2%
  overall accuracy**, and its stated gap is "between retrieving updated
  evidence and acting on it" — which is precisely the gap `act` sits in. That
  number is the external context for anything physis reports here.
- Recheck for a release before doing more self-authored corpus work. It is the
  single highest-value unblock on this list. `MemoryAgentBench`
  (github.com/HUST-AI-HYZ/MemoryAgentBench, arXiv 2507.05257) *is* public and
  is the nearest substitute if STALE stays unavailable.

**C2 — RUN 2026-09-13. The prose identity is wrong a third of the time it
speaks.** `src/claim_identity.rs`, `examples/claim_identity`, artifact in
`benchmarks/results/claim-identity.json`. Rather than the ground truth's two
contradiction pairs, it uses the twin ledger's eight, plus eight *agreeing*
restatements as the decisive negatives. 64 claims, 2016 pairs:

| identity | found | false + | precision | recall | F1 |
|---|---|---|---|---|---|
| `embedding@0.90` | 8/8 | 4 | 0.667 | 1.000 | 0.800 |
| `structural@0.30` | 3/8 | **0** | **1.000** | 0.375 | 0.545 |

The embedding finds every contradiction *and* flags half the agreements as
contradictions: similarity cannot separate "says the same thing about this"
from "says the opposite thing about this". The structural identity — a bag of
content words plus a scoped polarity, no parser — is **never wrong when it
speaks** and silent on five of eight pairs.

F1 says prose wins; F1 is the wrong number, because item B measured that
`act`'s larger defect is warning when it should be silent.

**Two things this run established that change what C2 means:**

1. The first attempt had to be discarded. Without the agreeing restatements the
   eight contradiction pairs are also the eight most *similar* pairs, so cosine
   scored F1 1.000 for a reason unrelated to contradiction. Any future
   contradiction corpus needs near-duplicate *agreements* as negatives or it
   measures nothing.
2. The structural ceiling is not the lexicon. Recall is capped at 0.375 because
   five pairs negate a predicate no verdict lexicon contains — *"the release
   build finishes … the LTO stage never runs out of memory"*. Scoping a
   negation over an arbitrary predicate is what a parser is for. **So the
   poorest structural identity already has perfect precision and its only limit
   is exactly what the real design would supply.** That is the strongest
   argument available for building the tuple — and it is an argument, not a
   result.

**What is still open:** the tuple itself. A bag has no subject/object asymmetry
and carries no interval, so "A supersedes B" and "B supersedes A" are identical
to it. Building the real (subject, relation, object, interval) needs an
extractor, which needs an interpreter — and the moment there is one, C3 becomes
runnable in the same pass.

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

## D. Problem 3 — the ontology as proposal, not as decree

**Status:** open, and the framing changed 2026-09-13.

Every other belief in the engine can be supported, contradicted, superseded and
replayed. The 70-cell grid cannot. It is authored in advance and sits underneath
the machinery that exists to make beliefs falsifiable, exempt from it — while E5
already found that collapsing 57 → 14 classes *gains* +0.014 kappa and E8's
pre-flight STOPs on sparsity at every level.

### The design

Stated by the author, and it dissolves the objection that made this hard:

> Ontologies could be **proposals**, and variance discovered from **similarity
> to top-down ontologies** plus **human or AI approval**.

The reason this is better than "let the grid drift on evidence" is that it
supplies the thing a self-revising grid otherwise lacks — an anchor. A grid that
revises itself on its own corpus can walk anywhere one observation at a time and
nothing notices. A grid whose proposals are scored *against established
top-down ontologies* has an external reference, and the distance from it is the
falsifiable quantity. Three states rather than two:

| the proposal | the reading |
|---|---|
| matches a top-down ontology | the grid rediscovered something known; cheap confidence |
| **varies** from it, coherently | the interesting case — a real distinction this corpus has and the reference does not |
| varies incoherently | drift; the anchor caught it |

Approval is what separates the second row from the third, and it is where the
human belongs: the machine proposes and measures the variance, the person or a
second model adjudicates. That is not a fallback for a weak method — see below,
it is what the field found too.

### This is an occupied field, and that is good news

The earlier note that problem 3 had "no published design to copy" was wrong; it
was searched under the falsifiability framing rather than this one. Under
*proposal + reference + approval* there is a whole discipline, with benchmarks:

- **OAEI** (Ontology Alignment Evaluation Initiative) is the long-running
  benchmark for exactly the "similarity to top-down ontologies" leg, with
  reference alignments and settled metrics — precision, recall, F, plus MRR and
  Hits@K for ranking, plus **coherence**, the logical consistency of the output
  alignment. Physis already needs all of those and would not have to invent one.
- **OAEI-LLM** (arXiv 2409.14038) and **OAEI-LLM-T** (arXiv 2503.21813) extend
  it for machine-proposed mappings and — this is the important part — formalise
  the error typology: *missing, incorrect, align-up, align-down,* and
  **disputed**. A disputed mapping is one the proposer makes to a relevant
  entity that the reference does not have, *and which may be more precise than
  the reference*. That is the author's "variance" with a name, a benchmark and a
  measurement protocol already attached. Do not rebuild it.
- **DeepOnto** (arXiv 2307.03067) is the Python package for ontology
  engineering with deep learning — matching, alignment, evaluation — and is the
  harness to borrow rather than write.
- **Open Ontologies: Tool-Augmented Ontology Engineering with Stable Matching
  Alignment** (arXiv 2605.09184) is the closest recent system.
- On the approval leg: LLM-supported collaborative ontology design reports the
  split cleanly — machines are strong at *discovery* and hit a ceiling at
  *validation*, with one study reporting an LLM at F1 0.97 against human experts
  at 0.68 with only 22% unanimous agreement. Read that number carefully in both
  directions: it argues for machine proposal, and it argues that "human
  approval" is not a gold standard either. Whatever adjudicates needs its own
  agreement measured.

### The experiments

**D1 — how far is the grid from a top-down ontology, today?** Before anything
is made revisable, measure the anchor. Align the 5×14 grid against an
established upper ontology (SUMO, DOLCE, or schema.org for the industrial
cells) using the OAEI protocol, and report precision/recall/coherence plus the
count in each OAEI-LLM error class. The **disputed** count is the number that
matters: it is how much of physis's grid is variance rather than error. If it is
near zero the grid is a re-labelling of something standard and the interesting
claim is gone. If it is large, that is the differentiator, stated in a metric
someone else defined.

**D2 — proposals from the corpus, scored against the anchor.** Cluster the
corpus into candidate cells (the LKD-KGC move — schema emerging from data,
arXiv 2510.20345), align each proposal to the reference, and sort by the
distance. The output is a ranked list of *proposed* grid changes with a variance
score, not an edited grid. `FABRICATE` ↔ `CONSTRUCT` — confusing at 0.22/0.21,
symmetric, the cleanest merge candidate in the grid (E22) — is the worked
example: does the pipeline propose that merge on its own? If it cannot recover
a change the evidence already plainly supports, it will not find a subtler one.

**D3 — the approval loop, with its own agreement measured.** Route proposals to
a human and to a model independently, and measure **inter-annotator agreement
between them** before either is trusted. The 0.97-versus-0.68 result above says
neither is the oracle. If the two adjudicators disagree more than the proposals
vary, the loop is a coin flip with a person in it.

**D4 — the null, and it is not optional.** A grid that revises itself needs a
**label-permuted replay**: the same evidence against shuffled cell labels. If
fitness falls the same way under permutation, the mechanism is responding to
volume and not to fit. Run this before D2 ships anything.

**Cost:** D1 a week (mostly getting a reference ontology and the alignment
harness up — DeepOnto shortens it). D2 two weeks. D3 depends on how many
proposals D2 emits. D4 two days and it gates the rest.

**The mechanism physis already has:** `TemporalValidity`, `narrow_until` and
the revision history. A proposal is a hypothesis with a validity window; the
variance score is its fitness; approval is supporting evidence and rejection is
contradicting evidence. Nothing new is needed in `hypothesis.rs` — which is the
sign the framing is right.

---

## What is closed, so nobody reopens it

- **Items A and B**, above, with their results. The cascade and the 0.66 floor
  are in the CLI; the warning-reserve policy is in the library, measured and
  declined.
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

## The reading list, in one place

Ordered by when you need it, not by importance. Everything below was found by
search on 2026-09-13; the notes are in
`computer-remake-research/literature-2026.md`.

**For C — claim identity and temporal contradiction**

| what | where | why |
|---|---|---|
| TOKI, bitemporal operator algebra | arXiv 2606.06240 | the design C proposes, published — and it keeps *both* identity mechanisms |
| STALE, memory-validity benchmark | arXiv 2605.06527 | external corpus; its Implicit Policy Adaptation probe is what `act` does |
| NeuSymMS, neuro-symbolic truth maintenance | arXiv 2605.17596 | working retraction, in CLIPS |
| SSGM, governing evolving memory | arXiv 2603.11768 | the risks of a memory that revises itself |
| Memory for autonomous LLM agents (survey) | arXiv 2603.07670 | the map of the field, read first if unfamiliar |
| AGM belief revision | Alchourrón, Gärdenfors & Makinson 1985 | the postulates, so they stop being reinvented |
| Doyle, truth maintenance | 1979 | justifications; a belief lives while one holds |

**For D — ontology as proposal**

| what | where | why |
|---|---|---|
| OAEI | ontologymatching.org | the benchmark and the metrics for the anchor leg |
| OAEI-LLM / OAEI-LLM-T | arXiv 2409.14038, 2503.21813 | the error typology, including **disputed** — the author's "variance", named |
| DeepOnto | arXiv 2307.03067 | the harness; do not write a second one |
| Open Ontologies, stable matching alignment | arXiv 2605.09184 | closest recent system |
| GenOM, matching with generated descriptions | arXiv 2508.10703 | LLM proposals scored against a reference |
| LLM-supported collaborative ontology design | Frontiers in Big Data, 2025 | the approval leg, and the 0.97-vs-0.68 agreement problem |
| LKD-KGC, in the KG-construction survey | arXiv 2510.20345 | schema emerging from data rather than authored |

**Cross-cutting, already used by A and B**

| what | where | why |
|---|---|---|
| Retrieved but not ranked | arXiv 2609.01556 | retrieval ≠ ranking; the reason the cascade beat the score mix |
| Negation is not semantic | arXiv 2603.17580 | why the polarity arm exists |
| Architecture fitness functions / ArchUnit | Ford, Parsons & Kua | the name for the capability sweep, and its upgrade path |

**Two caveats on this list.** The TOKI and negation papers were read as PDFs
whose numeric tables did not survive extraction, so their *designs* are
reported here and their *measurements* are not verified — check before citing a
number from either. And several of these are 2026 preprints found by one search
pass; none has been replicated here.

## The rule that applies to all of it

Every item above names its control. An item whose control was never run is not
a result, and the two defects the capability sweep shipped with today — a check
that read its own input, and a heuristic that marked live code dead — were both
found by asking one question: *what would this look like if it were broken?*
Ask it before reporting anything from this file.
