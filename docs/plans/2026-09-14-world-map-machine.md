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
