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

### Done, 2026-09-13 — and the verdict above is withdrawn, then replaced by a worse one

`physis-core act-recall` (`src/act_recall.rs`). Eight commands, a 48-claim
ledger of one age, two nulls: k random claims, and k random *contradicted*
claims. On bge-base-en-v1.5, recall@5 is **1.000** when the command shares the
claim's words and **0.875** when it paraphrases — both clear both nulls at top
1, 3 and 5. So "in the normal case `act` does not find the contradiction" does
not survive a direct measurement and is withdrawn; it came from the 2×2, which
scored a different task on a different corpus. The true statement is narrower:
this is a property of the embedder. On the random-projection fallback the
paraphrase case scores **0.000**, below both nulls, so offline Gap 8 is open.

Then a second arm, added after two papers named a defect in that corpus (arXiv
2603.17580, 2609.01556 — see `computer-remake-research/literature-2026.md` §1):
every distractor differed from its target by *topic*, none by *polarity*. Add
each target's affirmed twin and ask which of the pair ranks first, null 0.500
by construction:

| embedder | arm | refutation first | reassured at top 1 |
|---|---|---|---|
| bge-base-en-v1.5 | lexical | **0.250** | 5 of 8 |
| bge-base-en-v1.5 | paraphrase | **0.125** | 3 of 8 |
| random-projection | lexical | 0.750 | 0 of 8 |

Not a coin flip — inverted. The endorsement is the better match for a command
that asks to *do* the thing, so it outranks the refutation in six of eight
pairs. `is_warning` reads status, so the wrong one is surfaced wearing
`Supported`. At top 5 the list rescues the operator; at `--top 1`, in five of
eight lexical pairs the only claim printed is a reassurance about the exact
thing a Contradicted claim refutes.

**So problem 1's real shape is not recall.** `act` finds the right topic and
cannot read the verdict, and the fallback embedder is the one that *can* read
the verdict (0.750) while failing to find the topic at all. Neither dominates.
The open experiment is a hybrid, and it is ahead of the reranking fix the
literature offers.

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

### Done, 2026-09-13 — and it found a fourth instance immediately

`physis-check capabilities` plus `physis-core/.physis-capabilities`, which
declares thirteen capabilities as a write symbol and a read symbol each. Twelve
have both halves running. One does not:

> **provenance chains** — `add_link` is called only by the test suite, so
> `provenance_chains` is empty in the product and `full_explanation_report`
> renders `unwrap_or_default()` — an empty chain — every time. The only writer
> entry point, `PhysisCore::provenance_chain`, has exactly one call site and it
> is inside `#[cfg(test)]`.

Same shape as the three above, found in the first run rather than by accident,
which is the whole point. The sweep is left red on that row rather than the row
removed.

Verified against the tree before PH-019/PH-021: bi-temporal validity and
structural machines both FAIL there and PASS here, so the check discriminates
rather than agreeing with whatever it is pointed at.

The first run returned 14 of 14 — a green that came from two defects, both now
fixed and both affecting `calls` and `sweep` as well:

- the manifest names every symbol it asks about, so it was counting itself as a
  use site: a capability deleted from the source still scored one. A check that
  reads its own input as evidence cannot fail.
- `impl Trait for Type` was treated as *declaring* the trait, so every
  implementing file was dropped as a declaring file — a trait implemented seven
  times in its one consumer read as dead. Only inherent `impl Foo {` declares.

---

## Lower priority, but real: the core/pro boundary is drifting

`rag.rs` exists in **both** crates, and the physis-pro copy is the one the
`pack` CLI uses — so the MMR fix landed in core and did not reach the CLI until
it was made twice (PH-012 owns this). `machines.rs` went to core this session.
`direction.rs` is in core. There is no stated rule for what belongs in the
MIT core versus Pro, and the two are diverging by accident rather than by
decision. That is a licensing and strategy question, not a tidiness one.

---

## The read-and-run list

What is left of this file is now scheduled in
`docs/plans/2026-09-13-research-agenda.md`: five items, each with a corpus, a
control, and a result that can come back negative, ordered by information per
hour. Problems 1 and 4 are closed there with what replaced them — the polarity
inversion and the false-alarm rate, both larger than what was originally
suspected.

## What this list is not

It is not a plan, and none of the four is scheduled. Each names the measurement
that would settle it, and three of the four have a construction-matched null
available, which is the standard this repository holds itself to and the reason
these are worth writing down rather than arguing about.
