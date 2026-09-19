# JEV judgement of every component, and the improvement plan — 2026-09-19

Method: one self-contained judgement prompt per component, judged by JEV (the
OpenCode Zen transport, model `oczen/nemotron-3.5-lightning-free`). Each prompt
described the component's contract and evidence so the judge did not have to
explore; exploration was permitted but not required.

**Caveats, stated up front.** Six of fourteen components returned a verdict
before the run was stopped. Two replies (`s1`, `s5`) were truncated mid-JSON, and
`cli` returned an empty result — the adapter appends its own reply contract, and
when it collides with the judge's requested schema the model can follow the
wrong one. A truncated reply is itself a finding about the transport, not about
the component. The scores below are JEV's, judged from descriptions rather than
from reading the code, so they measure how well each component *communicates and
holds together* as much as how it is built.

| component | concept | impl | notes |
|---|---|---|---|
| S0 object/action registry | 7 | 6 | tight coupling to the nav index |
| S1 agent world model | — | — | reply truncated |
| S2 adapters | 7 | 6 | transport pinned to one provider/model |
| S3 routing ladder | 7 | 6 | policy boundary ambiguous |
| S4 task graph | 7 | 6 | fold cost, no concurrency model |
| S5 `@` grammar | — | — | reply truncated |
| S6 HUD agent surface | **8** | **7** | best-scoring; UI timing coupling |
| S7 intent, S8 attention, S9 decision, grid, settings, web, cli | — | — | not returned |

---

## 1. The recurring themes

Across every verdict that returned, the same four criticisms recur. They are the
plan's spine, because a flaw four judges find independently is structural, not a
detail.

1. **Coupling to a concrete instance.** Objects are tied to the nav index;
   the JEV transport is tied to OpenCode Zen and a pinned free model;
   `adapter_for` has no stated selection criteria. The system is honest about
   *what* it reached but not *why that one*.
2. **No stated concurrency or lifecycle model.** Concurrent task claims,
   the UI's channel-vs-draw timing, a log that grows without retention, a task
   graph that folds the whole log on every read. Each is correct at today's size
   and has no stated ceiling.
3. **Policy boundaries are ambiguous.** The clearest example: an `@agent`
   override "wins even when its cost is outside policy" — JEV called this out
   directly. Which authority wins at the boundary is not written down.
4. **Production readiness is asserted by configuration, not by evidence.** The
   transport defaults to a model that was out of funds; the free tier is a batch
   transport at 43 s–3 min; five capabilities have five tests.

---

## 2. Plan — conceptual

Ordered by what unblocks the most.

### C1. Measure the reworked grid (blocks honesty everywhere)
The nine-mode axis was reworked *on* a measurement of the fourteen-mode axis and
has never been measured itself. Every doc now says so. Run the same 731 entries
through the same size-preserving permutation null for DOMAIN (5), MODE (9) and
the joint cell (45), and publish the table next to the pre-rework one.
**Until this runs, the grid's headline numbers belong to a grid that no longer
exists.**

### C2. Write down the authority order
One page, one list: for any decision, which layer wins. Intent > explicit
override > route table > policy > ladder? Today the answer is scattered across
`choose()`, `permitted()` and the docs, and JEV found the seam. Make it a table
with the boundary cases (override outside policy, offline with a pinned agent,
a route naming an unavailable agent) as the worked examples.

### C3. Name the unit of "agent" and the unit of "route" separately
`agent:<id>` is a registry object, a roster entry and a routing candidate. That
conflation is why "host/device placeholder ambiguity" keeps surfacing. Decide
whether an agent is an object with capabilities or a transport with permissions,
and let the other be a facet of it.

### C4. State the growth ceiling for every unbounded thing
The observation log, the task graph fold, the pack walk, the agent call
deadline. For each: the current cost, the size at which it hurts, and the
intended fix. "Unbounded" is acceptable only when written down (the pack walk
was exactly this, and it was fixed only after it was measured at 10+ minutes).

### C5. Make the transport a choice, not a default
JEV's strongest criticism of S2. The contract is "a Jev-compatible endpoint";
the implementation is one pinned free model on one provider. Add the endpoint as
first-class configuration (already partly done via `providers.jev.endpoint`) and
document the three transports (Zen text, direct decision endpoint, local) as
peers with their measured latency and cost.

---

## 3. Plan — implementation

Ordered by risk retired per unit of work.

### I1. The log is the substrate: give it retention and a reader
`observations.jsonl` is appended by every surface and folded on every read.
Add (a) rotation or compaction with a stated policy, (b) an indexed read so
`TaskGraph::fold` is not O(n) per call, (c) a schema version on `TaskEvent`
before the first incompatible change. Evidence: JEV on S4, twice.

### I2. Concurrency, or an explicit "single writer"
Either add optimistic versioning to task records, or state in one place that
the log has a single writer and enforce it. Right now neither is true. Evidence:
JEV on S4 ("no concurrency model") and S6 ("timing coupling between model
response and UI refresh").

### I3. Finish the adapter error surface
JEV found the deadline race (spawn vs kill), the missing SIGKILL fallback, and
that `guard_prompt`'s leading-`-` check is not a full option check. Also:
`agent call` now prints the reply on a schema miss, but the judge's own schema
collision shows the adapter should not append its contract to every prompt —
make it opt-in per call. Evidence: JEV on S2, and this run's `cli` empty result.

### I4. Raise the floor on coverage
"5 capabilities, 5 tests" is a ratio, not a gate. Add the cases the verdicts
name: unknown object, unknown kind, a capability with an empty verb list, a
malformed `actions.json`, a route table naming a missing agent, an intent edited
concurrently. Evidence: JEV on S0, S3.

### I5. HUD: buffer the channel, and pin the snapshot's meaning
Per-agent buffers so a long turn cannot stall the drain, a channel timeout, and
a note on what `--snapshot` does and does not capture (glyphs and layout, not
colour). Evidence: JEV on S6 — the highest-scoring component, with the clearest
concrete fixes.

### I6. Make the web surface the graphical TUI it now is
`/impossible` renders what the terminal HUD renders, clickably. The HUD still
draws the task graph as text. Either point the HUD at the page for graphs, or
accept the split and say which surface is canonical. Evidence: this session's
own gap list.

---

## 4. What is NOT a problem

Worth recording so it is not "fixed" later: the acceptance gate (reported ≠
completed), the deny-by-default permissions, the total `@` parser, the
provenance-stamped intent, the readout that cannot disagree with its score, and
the similarity labelling that refuses to call a word match a meaning match. Each
was judged and none drew a flaw. They are the parts of the system that already
say what they do.
