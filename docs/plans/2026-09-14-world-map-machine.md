# Physis as a machine for a continuously revisable map of a world

Design note, 2026-09-14. Top-down first, then backward to the smallest thing
that can kill it. Nothing here is a claim about what the code does today;
`docs/WHAT_WE_KNOW.md` holds those, and every line below that touches them cites
the experiment number.

## 1. The object

A world map is not a corpus and not an index. It is a structure that answers
four questions that a file browser and a search box cannot:

1. **What is there?** — records, and the entities they are about.
2. **What is related to what?** — links that are not similarity: same entity,
   caused by, supersedes, part of, produced by this action.
3. **What changed, in what order?** — transitions between states, ordered by
   position in the log rather than by clock.
4. **What could have been, and what may yet be?** — the alternatives that were
   live at a position, and the ones still live now.

Written as a machine:

```text
  W = ⟨ O, A, R, T, Π ⟩

  O   observations      append-only, positioned         observe.rs      built
  A   actions+outcomes  intent → argv → exit → effect    system.rs run   built
  R   relations         entity links, mutual-kNN, cells  worldstate.rs   built
  T   transitions       set operations over log order    worldstate.rs   E56
  Π   possible worlds   hypotheses, validity intervals   hypothesis.rs   inert
```

Four of the five exist and run. Π exists as schema and is exercised by nothing
while you work — `WHAT_WE_KNOW.md` lists "the revision loop" under *Open:
believed, not measured*. That is the gap this design is organised around, not
the retrieval quality of any single leg.

**Continuously revisable** is the load-bearing word. A map that is rebuilt from
scratch on every question is a cache; a map that can never be wrong is a
decoration. Revision means: a record arrives, some belief's validity interval
narrows, and a later answer differs *because of it* — traceably, with the
superseded version still readable.

## 2. Language is a controller, not the world

The measured negatives already force this. Embedding geometry does not yield
stable structure (cross-embedder ARI ≈ 0.10, E12–E18); cosine does not separate
agreement from contradiction (precision 0.667 at perfect recall, C2); a word
table matches a 7B interpreter on real text at 1/2400th the build cost (E63,
E64). None of that says text is useless. It says text is the **control signal**:
the thing that selects which part of the map to bring into view, ranks
candidates, and names what was found. It is not the substrate the map is made
of.

The consequence for design is concrete. Similarity may **propose**; only
relations, order and outcomes may **assert**. That is the same `judge`/`propose`
split the research track already paid for (§4 of `WHAT_PHYSIS_IS.md`: top-3
0.712 against a 0.136 null for proposal, chance for certification), applied one
level up: language proposes the region, the world model asserts the structure.

## 3. The ultimate test

> Given an agent's behaviour, can physis infer the representational geometry
> that best explains it, instantiate a surrogate agent from that geometry, and
> predict the agent's future choices?

This is the right north star because it is the only test of a world model that
cannot be satisfied by paraphrase. A map that merely *describes* a trace can be
graded only by someone's judgement of the description. A map that **predicts the
next choice** is graded by the world.

Stated so it can fail:

- Behaviour is a sequence `B = (c_1 … c_n)` of choices — which file, which
  command, which target — each with the state that preceded it.
- A geometry `G` is anything that induces a ranking over candidate next choices
  from the prefix: a lexical field, a co-access graph, a directory metric, a
  transition table over world states, a learned embedding.
- **Inference:** pick `Ĝ = argmax_G  P(c_{k+1..n} | c_{1..k}, G)` on held-out
  continuations.
- **Instantiation:** the surrogate is `π̂(next | prefix) ∝ rank_Ĝ`.
- **Prediction:** score `π̂` against the agent's actual later choices.

It is refuted if no `G` beats the recency-plus-frequency baseline. It is
uninteresting if every `G` ties, because then the fitted geometry carries no
information the trivial one does not.

## 4. Backward, layer by layer

Each layer is a question with a control that can fire. A layer that fails stops
the ones above it, which is the point of ordering them.

| # | question | control that could kill it | status |
|---|---|---|---|
| L1 | Are actions and outcomes ingested at all, from a real agent, without anyone remembering to? | count of records against count of tool calls | built, unmeasured (`clients/opencode-physis-observer`) |
| L2 | Does log order carry recoverable transitions? | shuffled order | **passed**: 0.486 vs 0.291, z = +4.88 (E56) |
| L3 | Do relations beat order-blind similarity where links are good, and lose where they are bad? | order-blind cosine | **passed and bounded**: 0.700 vs 0.013 (E57), 0.209 vs 0.349 (E60) |
| L4 | Does an agent's choice sequence carry *any* geometry? | recency+frequency, shuffled order | **not run — this is the next experiment** |
| L5 | Does a surrogate built from the best geometry predict held-out choices? | the same nulls, plus the geometry fitted on shuffled labels | blocked on L4 |
| L6 | Does revising a belief change a later answer, traceably? | the same query before and after, with the superseded version still readable | blocked on Π being exercised at all |

The north star is L5. The smallest thing that can falsify the whole programme
is L4, and it needs no new subsystem.

## 5. The smallest falsifiable experiment (E67)

**Claim under test.** An agent's file-touch sequence is better predicted by some
geometry over the workspace than by recency and frequency alone.

**Data, already on disk.** 165 Claude Code transcripts; 25 sessions carry ≥ 10
file touches, 1938 touches in total, across four projects. No new collection,
no model, no licence.

**Task.** Leave-one-session-out. For each touch `c_{k+1}`, rank the candidate
files given the prefix `c_{1..k}`, score top-1 and top-5.

**Arms.**

| arm | ranking signal |
|---|---|
| `recency` | most recently touched first (the null that usually wins) |
| `frequency` | most touched in this project first |
| `recency+frequency` | their rank fusion — **the arm to beat** |
| `lexical` | BM25 of the prefix's file contents against candidates (`rag.rs`) |
| `co-access` | graph of files touched within the same session window |
| `tree` | directory-path distance from the last touch |
| `transition` | world-state transition table over log order (`worldstate.rs`) |
| `shuffled` | the best arm, fitted on order-shuffled prefixes |

**Pre-registered thresholds.** With 1938 events over 25 sessions, only a large
effect is detectable; that is stated now rather than discovered later. A
geometry counts as carrying signal if it beats `recency+frequency` by ≥ 0.05
top-1 with a bootstrap CI over sessions that excludes zero, and simultaneously
beats its own `shuffled` arm. Anything smaller is reported as *underpowered*,
not as a trend.

**What each outcome means.**

- *No arm beats recency+frequency.* Behaviour at this scale carries no geometry
  a world model can recover, and L5 is dead until either the data is richer
  (the observer log, with outcomes) or the choice space is harder than "which
  file next". Publish the negative; it is the cheapest possible refutation of
  the north star.
- *`lexical` alone wins.* The controller is doing the work and the map is
  decoration — consistent with E63/E64, and an argument for spending on the
  cheap word table rather than on structure.
- *`co-access`, `tree` or `transition` wins.* There is structure in behaviour
  that similarity does not carry. That is the first evidence for L5, and the
  surrogate experiment becomes worth building.

**Cost.** One script against existing files, hours. It reuses `rag.rs` for the
lexical arm and `worldstate.rs` for the transition arm, and it adds nothing to
the engine.

## 6. What E67 deliberately does not test

It does not test belief revision (L6), possible worlds, or outcomes — a file
touch has no success label. Those need the observer log, where an action carries
an intent and an exit status, and that log is days old and small. The honest
order is: find out whether behaviour carries geometry at all before building the
machinery that would exploit it.

It also does not test transfer. Every number in this repository so far comes
from one machine's log, one week, mostly one repository
(`WHAT_WE_KNOW.md`, *Open*), and E67 inherits that limit exactly.

## 7. If E67 passes, the next two

- **E68 — surrogate.** Instantiate `π̂` from the winning geometry and score
  whole held-out continuations (trajectory edit distance, not just next-step),
  against the same nulls and against a surrogate fitted on a *different* agent's
  sessions. A surrogate that predicts everybody equally well has learned the
  repository, not the agent.
- **E69 — revision changes an answer.** Take a query whose answer depends on a
  belief with a validity interval; append the record that narrows it; re-run.
  The control is the same query against the same store with the record withheld.
  Until that fires, "continuously revisable" is a design word, not a property.

---

# Part II — the full form, then subtraction

Part I worked upward from what runs. That is the safe direction and it has a
failure mode: it describes a better version of what exists. This part starts at
the other end. Describe the whole machine, then strike everything that is not
load-bearing for its core, and see what is left that can be executed this week.

## 8. What kind of thing Physis is

Physis is not a language model, an embedding store, a retriever, an ontology, a
ledger or an agent. Each of those is a *part*, and naming the machine after one
of its parts is the specific error `WHAT_PHYSIS_IS.md` §1 was written to stop.

It is the structure that decides **how the parts change**. A machine for
becoming: its subject matter is not the world's current state but the world's
revision — what deforms, in response to what, and what that deformation makes
possible next.

Five invariants define it. They are stated so that a build can violate them,
because an invariant that cannot be violated is decoration.

- **I1 — Reality has authority.** No answer may outrank the record that
  contradicts it. Precedence is not a heuristic: the later, better-grounded
  record wins, and the machine's job is to say *what changed because of it*.
- **I2 — Mismatch deforms, it does not deny.** A contradiction must alter a
  parameter of the representation. Discarding the datum, or lowering its weight
  to zero, is denial. Cells already carry a fitness rather than a truth flag
  (`grid_fitness.rs`, D4) precisely so that being wrong has somewhere to land.
- **I3 — Nothing is deleted.** Superseded structure stays readable, and a
  retired branch can be re-proposed later when the surroundings change
  (`dream.rs`, G7 invalidate-don't-delete). Failures are inventory.
- **I4 — Aim at stable futures, not next states.** A one-step prediction is a
  reflex. The object of interest is the set of configurations the world can
  settle into, ranked by how stable they are under further evidence — which is
  what a shadow-frame cascade already computes for a single mutation
  (`delta_engine.rs`).
- **I5 — Growth is measured or it is not growth.** Error over time must fall
  faster than a frozen copy of the same machine. Without that comparison,
  "learning from mistakes" is a slogan attached to a random walk.

And one stance rather than an invariant: **human and machine share one
neurosemantic layer.** The same log, the same primitives, the same provenance.
A person's note and an agent's tool call are the same kind of record, written by
different actors — which is already literally true of `physis system remember`
and `clients/opencode-physis-observer`. Language is the control surface over
that layer, not a second, private world.

## 9. The full form, as organs

Six organs, each of which exists in some form today. This is the maximal
machine; §10 subtracts from it.

| organ | what it does | exists as |
|---|---|---|
| **Intake** | positions every observation, action and outcome in one append-only log, with the actor named | `observe.rs`, `system run`, the OpenCode observer |
| **Commitment** | every answer leaves a prediction behind, with a validity interval and a confidence that can be wrong | `hypothesis.rs`, `temporal.rs` |
| **Confrontation** | reality arrives; the mismatch against the standing commitment is computed and kept | `contradiction.rs`, `grid_fitness.rs` |
| **Deformation** | the mismatch edits the representation: a cell's fitness shifts, a link rewires, a counted table updates, a term's sense splits because one became another rather than two coexisting | `grid_fitness.rs`, `becoming.rs`, `ngram_table.rs` |
| **Futures** | simulates cascades in shadow frames and keeps the reachable configurations, ranked by stability under further evidence, not by one-step likelihood | `delta_engine.rs`, `transform.rs` |
| **Reproposal** | brings retired structure back when the surroundings have changed enough to make it live again | `dream.rs` |

The organs are wired in a cycle, and the cycle is the machine:

```text
   intake → commitment → confrontation → deformation → futures ↴
      ↑                                                        │
      └──────────────── reproposal ←───────────────────────────┘
```

What is missing is not an organ. It is the **circulation**: nothing today
carries a mismatch from confrontation into deformation automatically, so the
cycle is six organs on a table. `WHAT_WE_KNOW.md` says this plainly under
*Open*: the revision loop exists as parts and revises nothing while you work.

## 10. Subtraction

Strike everything not load-bearing for the invariants, in order:

- **Strike the ontology grid as a fixed coordinate system.** I2 needs *a*
  deformable parameter, not fourteen modes and five domains. A single weighted
  graph over workspace objects deforms just as falsifiably, and D2a already
  says 6 of 14 modes fail an external inventory.
- **Strike embeddings.** E63/E64 bought the right to: a counted table matched a
  7B interpreter at 1/2400th the cost. The core needs a ranking, not a
  semantics.
- **Strike the LLM entirely.** Nothing in I1–I5 requires generation. An
  interpreter can be added later as an arm that must beat the cheap one.
- **Strike multi-agent orchestration.** One actor is enough to test whether
  mismatch deforms; the shared layer is tested by a second actor writing to the
  same log, which the observer already does.
- **Strike possible-worlds simulation down to one step.** I4 needs *a* notion of
  stability that beats one-step likelihood — the end-state of a session is one,
  and it is observable without any simulator.
- **Keep:** the log, one deformable structure, one prediction with an error
  signal, one deformation rule, and the controls that can kill each.

What survives is four functions and a loop:

```text
  predict(state, prefix)  → ranking
  mismatch(ranking, what actually happened) → error
  deform(state, error)    → state'          # I2, and the arm under test
  settle(state, prefix)   → the configuration this is heading for   # I4
```

## 11. E70 — the smallest executable machine for becoming

E67 (§5) asks whether behaviour carries a geometry *at all*, with the geometry
held still. E70 is E67 with the geometry allowed to move, which is the first
test of the actual thesis. Same data, no new subsystem: 165 transcripts, 25
sessions with ≥ 10 file touches, 1938 touches.

**State.** One weighted graph over the files of a project. Edges start from
what is free: co-access within a session, directory distance, lexical overlap
(`rag.rs`). No embeddings, no model.

**The loop.** Walk a session's touches in order. At each step, rank candidates
(`predict`), record the rank of the file actually touched (`mismatch`), then
update edge weights in proportion to that error (`deform`) — a one-line rule,
deliberately, so that the experiment is about *whether deformation helps*, not
about which clever rule.

**Arms, each a control that can fire.**

| arm | what it isolates |
|---|---|
| `frozen` | the same graph, never updated — **I5's control**: if deformation does not beat this, the growth function is decoration |
| `random-deform` | same magnitude of update, random target — isolates "changing helps" from "changing *this way* helps" |
| `deny` | on a mismatch, drop the offending observation instead of updating — **I2's control**, the denial policy stated as a rival |
| `shuffled` | prefixes in random order — kills any result that is sequence shape |
| `recency+frequency` | the trivial predictor from E67, still the arm to beat |

**Three measurements, one per invariant that can be reached at this size.**

1. **I5, growth.** Cumulative rank error over the session, deformed against
   frozen. Pre-registered: a ≥ 10% reduction in mean reciprocal rank error, with
   a bootstrap CI over the 25 sessions excluding zero. Smaller is *underpowered*,
   not a trend.
2. **I2, deform beats deny.** The `deny` arm on the same sessions. If denial
   ties or wins, invariant I2 is wrong as stated and the design changes, which
   is the outcome most worth having.
3. **I4, stable futures.** From the first 25% of a session, predict the *final
   working set* — the files touched in its last 25% — rather than the next
   touch. Nulls: frequency prior, and the deformed graph's own one-step
   prediction rolled forward. A machine that aims at attractors should beat a
   machine that aims at next states on this question specifically; if it does
   not, I4 is a preference, not a property.

**What failure buys.** Each arm that wins against the thesis names the next
primitive to extract: `frozen` winning says the error signal is too weak and the
loop needs outcomes (the observer log's intent → exit records) rather than mere
choices; `deny` winning says fitness-lowering is the wrong deformation and the
right one is structural (split the node, as `becoming.rs` splits a sense);
`shuffled` winning says the whole result was sequence shape. None of those are
dead ends, and all of them are cheap to reach. **Keep the failure, extract the
primitive, repeat** — and the record of it goes in the log the machine is built
on, `physis-check claim` first, `physis-check verdict` after, so that Physis is
the instrument by which Physis is measured.

**What E70 still does not test.** Reproposal (`dream.rs`) needs a longer
horizon than one week of one machine's log. Multi-actor shared layer needs the
observer running in anger. Full possible-worlds ranking needs a simulator, and
`delta_engine.rs` computes cascades for one mutation, not a frontier. Those are
named here so that the smallest system is not mistaken for the full form it was
subtracted from.
