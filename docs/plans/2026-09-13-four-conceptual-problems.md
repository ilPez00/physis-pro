# Four conceptual problems, each with the measurement that exposed it

Written 2026-09-13, after a session that ran the §3 2×2 for the first time,
tested graphiti against its own claims, and shipped PH-018/019/020/021.

These are not bugs and they are not a backlog. Each is a place where the
architecture says one thing and the code or the measurements say another.
Ordered by how much they cost if left alone.

---

## 1. The differentiator routes through the commodity, and the commodity does not work yet

`RESCOPE.md` §0 and §7 settle that the context compiler is the **commodity**
half — several exist, one is MIT-licensed and beats physis on physis's own
terms — and that the **ledger** is the half nothing else ships.

`act.rs` is the ledger's sharpest claim: it is the only implementation, in a
twenty-one system survey, of *acting while checking what is already believed*
(`computer-remake-research/gaps.md` Gap 8). Here is how it finds what is
already believed:

```rust
relevance: crate::models::cosine_sim(&q, &h.embedding)   // act.rs:146
```

Cosine over the command text against claim statements. That is the commodity
half, and it is the only path by which the differentiator reaches its data.

The 2×2 the same day measured that retrieval family at **0–1/7 against a
whole-file ceiling of 5/7** (`2026-09-13-2x2-first-run.md`). So in the normal
case `act` will not find the contradiction it exists to surface. Gap 8 is
closed in architecture and open in practice.

`act.rs` already says its matcher is weak, in its own "Honest limits". What is
missing everywhere — RESCOPE included — is the structural consequence: **the
part called the differentiator has a hard dependency on the part called the
commodity.** "The compiler is the commodity" is true about the market and
false about the build order.

**What would settle it:** measure `act`'s recall directly. Seed a ledger with
N claims, issue commands that contradict k of them, and count how many
contradictions are surfaced. There is a construction-matched null available —
surface k random claims of the same age — so the result can fail.

---

## 2. A claim's identity is its prose, and prose is the layer measured as noise

```rust
pub struct Hypothesis {
    pub id: String,
    pub statement: String,        // human-readable statement
    pub embedding: Vec<f32>,      // embedding OF THE STATEMENT
    ...
}
```

Contradiction, deduplication, supersession and `act`'s recall all match on that
embedding. A claim *is* its sentence.

The graphiti experiment measured what survives an interpreter swap
(`computer-remake-research/experiments/graphiti/RESULTS.md`,
`state_hash.py`). Same evidence, two interpreters — a 7B local model and a
120B hosted model:

```
[STABLE ] L1 entities        9f783e434055  9f783e434055
[STABLE ] L2 + topology      4adb32e62daa  4adb32e62daa
[STABLE ] L3 + temporal      ff6f278a7cb4  ff6f278a7cb4
[CHANGED] L4 + fact prose    a9a2bd89e883  f438d5586e88
```

and the control — the *same* model, twice, on the *same* input — moved L4 as
well. Prose is not even an interpreter signature. It is noise.

So the ledger computes its defining relations on the one layer shown to be
interpreter-dependent, while the layers shown to be evidence-determined
(entities, topology, interval) are not part of a claim's identity at all.

This is also the sharpest instance of the thesis physis argues elsewhere: *the
model must be a replaceable interpreter, not the memory*. A claim keyed by the
sentence a model happened to emit is a claim keyed by the interpreter.

**Direction:** a claim needs a structural identity — subject, relation, object,
interval — with the sentence demoted to attributed rendering stored beside it,
never the thing a contradiction is computed against. `machines::Observation`
(PH-021) already carries `machine` as provenance for exactly this reason.

**What would settle it:** does structural identity change contradiction recall
on the existing corpus? Contradiction recall is currently 0.50 — one of two
known pairs missed, unchanged by the embedder, which RESCOPE §6 item 7 already
calls "a map defect rather than a retrieval one". That is the pre-existing
number to beat, and it is a cheap experiment.

---

## 3. The ontology is the one belief that cannot be contradicted

The survey's own conclusion (`computer-remake-research/notes.md` §3, "What to
reject") is: **a fixed ontology authored in advance is only as good as its fit**
— and names localsearch, still shipping Nepomuk's, as the cautionary case.

Physis ships a 70-cell semiotic grid, authored in advance. E5 found that
collapsing 57 → 14 classes *gains* +0.014 kappa. E8's pre-flight STOPs on
sparsity at every level.

There is no mechanism by which evidence revises the grid. Every other belief in
this engine can be supported, contradicted, superseded and replayed. The
ontology cannot. It sits underneath the machinery that exists to make beliefs
falsifiable, exempt from it.

That is a coherence problem before it is a performance problem: the system
holds a principle it does not apply to itself.

**What would settle it:** the grid is a hypothesis like any other. Give it a
`TemporalValidity` and a fitness, and let E5's own result — 57→14 gains kappa —
be recorded as evidence against the current cell count rather than as a
research note.

---

## 4. Schema without a write path — three instances in one day

| capability | schema | path that exercises it |
|---|---|---|
| bi-temporal validity | `valid_from`/`valid_until`/`expired_at` since G1 | **none** — only a constructor ever wrote `valid_until` (fixed: PH-019) |
| structural machines | `ProofStatus`, `Observation`, `StructuralMachine` | **none reachable** — defined inside one example (fixed: PH-021) |
| continuous observation | `Observation {source, kind, body, duration_ms, produced_by}` | **none** — no window, process or accessibility watcher exists |

The pattern is the same each time: the representation is built, carefully and
correctly, and the path that would exercise it is not. The capability then
reads as present in every document that lists capabilities.

`physis-skill` §1 is the right check — *declared ≠ called*, a symbol that
compiles and exports and has test call-sites can still be called by nothing —
and it operates on symbols. Nothing runs it on **capabilities**. Two of the
three above were found by accident this session; the third is still open.

**What would settle it:** extend `physis-check` with a capability sweep. For
each claimed capability, name the write path and the read path, and fail when
either is absent. That is the same check, one level up, and it is the cheapest
of the four items here.

---

## Lower priority, but real: the core/pro boundary is drifting

`rag.rs` exists in **both** crates, and the physis-pro copy is the one the
`pack` CLI uses — so the MMR fix landed in core and did not reach the CLI until
it was made twice (PH-012 owns this). `machines.rs` went to core this session.
`direction.rs` is in core. There is no stated rule for what belongs in the
MIT core versus Pro, and the two are diverging by accident rather than by
decision. That is a licensing and strategy question, not a tidiness one.

---

## What this list is not

It is not a plan, and none of the four is scheduled. Each names the measurement
that would settle it, and three of the four have a construction-matched null
available, which is the standard this repository holds itself to and the reason
these are worth writing down rather than arguing about.
