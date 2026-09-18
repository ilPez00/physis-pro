# E91-A — Archaeology: reuse points, caller audit, avalanche data gap

Scope: does Physis already contain the machinery E91 needs to test whether
*structural change* is sparse/avalanche-like and barrier-constrained? This file
pins the reuse points, proves there is no second invariant engine, and states
the data gap honestly. No production Rust was changed for this deliverable; the
only write is this file.

Repo root for every path: `/home/gio/physis_pro`.

---

## 1. REUSE MAP

`holds` = what already exists and can be reused without a new engine.
`change-set observable?` = whether the mechanism emits a durable, inspectable
set of changed units (the raw material an avalanche-size measurement needs).

| mechanism | path:line | holds | perturbation | mutates | change-set observable? |
|---|---|---|---|---|---|
| `propagate_delta` | `src/dependency/mod.rs:49` | affectedness flood over `DependencyGraph`; returns `Vec<(node, score, PropagationTrace)>` | a `SemanticDelta` at a source node | local only — builds `affected`/`traces`, never touches graph | yes in memory (traces), **no** persistence |
| `PropagationTrace` | `src/dependency/mod.rs:184` | per-hop record: source, target, relation, incoming, relevance, overlap, resulting, decayed | — | immutable value | yes — the closest thing to an event row |
| decay rule | `src/dependency/mod.rs:80` | `decayed_score = score * 0.7` (30% per hop) | — | — | — |
| threshold rule | `src/dependency/mod.rs:83` | skips when `score < threshold`; note it tests `score`, not `decayed_score` | — | — | — |
| frontier order | `src/dependency/mod.rs:60` | `queue.pop()` => **LIFO/DFS**, despite the comment at `:55` saying BFS | — | — | — |
| `DependencyGraph` | `src/dependency/mod.rs:205` | outgoing+incoming adjacency, claims, depends_on | `add_edge` `:228` | yes (add only, no prune) | n/a |
| `DependencyEdge` | `src/dependency/mod.rs:8` | source, target, relation, strength, confidence, provenance | `new` `:24` | — | n/a |
| `SemanticDelta` | `src/semantic_delta/mod.rs:80` | full lexical+semantic change record | — | — | **no live producer** (see §2) |
| `LexicalDiff` | `src/semantic_delta/mod.rs:4` | char/word change counts, new/removed words | — | — | — |
| `PolarityDelta` / `ScopeDelta` / `ModalityDelta` | `src/semantic_delta/mod.rs:27,:34,:42` | delta enums | — | — | — |
| `QuantityDelta` | `src/semantic_delta/mod.rs:48` | old/new value + direction | — | — | — |
| `EntityChangeType` / `EntityDelta` | `src/semantic_delta/mod.rs:71,:63` | substitution/add/remove/negation | — | — | — |
| `RelationDelta` | `src/semantic_delta/mod.rs:347` | relation-type change record | — | — | — |
| `compute_semantic_distance` | `src/semantic_delta/mod.rs:143` | cosine distance old vs new text via any `Embedder` | — | — | scalar only |
| `StalenessState::classify` | `src/dependency/mod.rs:305` | affectedness -> operational state enum | — | — | scalar only |
| duplicate classifier | `src/staleness/mod.rs:9` | byte-for-byte same logic as `:305` | — | — | **duplicate, not a second engine** |
| `classify_affected_nodes` / `explain_staleness` | `src/dependency/mod.rs:353,:380` | batch classify + human explanation | — | — | no callers outside module |
| `RepairRegion` / `select_repair_regions` | `src/repair_regions/mod.rs:5,:91` | groups stale nodes into regions | — | returns new vec | yes (region membership), but collapses to one |
| single-region collapse | `src/repair_regions/mod.rs:112-116` | "In a full implementation, this would split" — always `vec![region]` | — | — | region count is always 0 or 1 |
| `compute_heaviness` | `src/orchestration/mod.rs:91` | weighted score for region difficulty | region + delta | pure | scalar |
| placeholders inside heaviness | `src/orchestration/mod.rs:100,:116,:136` | node text len `100`, centrality `0.0`, failure penalty `0.0` | — | — | score has two live terms only |
| `choose_worker` | `src/orchestration/mod.rs:143` | routes cheap vs strong by heaviness > 0.4 | — | returns boxed worker | **never called** (compiler warning; `logs/auto_update.log:7809`) |
| `EditMetrics` | `src/metrics/mod.rs:8` | per-edit counters incl. `max_dependency_depth` | `from_edit` `:43` | pure | yes as struct, not persisted |
| hardcoded metrics | `src/metrics/mod.rs:86,:93` | `edit_waste = 0.1`, `repair_precision = 0.85` | — | — | fabricated constants |
| depth bug | `src/metrics/mod.rs:130` | `max_dependency_depth = trace.decayed_score.floor()` — a 0..1 score cast to usize is dimensionally wrong (always 0) | — | — | depth is unusable as written |
| `DebugDashboard` | `src/metrics/mod.rs:175` | snapshot bundle | — | pure | no producers |
| `CausalLedger` / `CauseEvent` | `src/ledger.rs:36,:22` | append-only why-made history per tenant | `record` `:60`, `record_at` `:73` | yes | yes, and persisted |
| lineage / descendants / purposes | `src/ledger.rs:98,:122,:148` | object<-input chain walk | — | reads only | graph, not magnitude |
| sled persistence | `src/ledger.rs:207` save / `:218` load | per-tenant rows `ledger:{uid}:{seq}` | — | append-only | durable |
| `ImpactAnalysis` / `CellDelta` / `Transition` | `src/planner.rs:114,:103,:32` | counterfactual before/after cell deltas | `derive` `:131`, `simulate` `:51` | pure | yes, from saved simulations |
| `QualityTracker` | `src/ported/quality.rs:50` | failure records + per-cell penalty/boost | `report_failure` `:77`, `penalize_cell` `:140`, `report_success` `:147` | yes | failures persisted; penalties partial |
| `cell_standing` / `adjust_score` | `src/ported/quality.rs:184,:191` | live cell multiplier | — | reads only | scalar |
| persistence loss | `src/ported/quality.rs:250-263` | `load_or_new` rebuilds `cell_penalties` only; `cell_boosts` and `penalize_cell` history are **lost on reload** | — | — | partial observability |
| `SalienceStore` / `Accrued` | `src/salience.rs:101,:84` | per-tenant/context accrued evidence, decayed on read | `record` `:163`, `score` `:188`, `bias` `:211`, `cell_bias` `:225`, `ranked` `:244` | yes | scalar evidence, persisted to `salience.json` |
| `DreamEngine` | `src/dream.rs:38` | generate batches; contract review | `generate_dreams` `:63`, `with_context` `:67`, `review` `:412`, `evaluate_dream` `:498` | yes | dream set, graded/ungraded |
| `communities::label_propagation` / `detect_communities` | `src/communities.rs:87,:139` | deterministic label propagation + cohesion | — | input read-only | assignment + cohesion |
| studio duplicate | `src/ported/studio_communities.rs:58,:99` | same algorithm re-implemented against core types | — | input read-only | assignment + cohesion |
| `compare_structures` / `compare_node_lists` / `search_structures` | `src/structsim.rs:289,:301,:369` | node-by-node cosine matches, missing/extra | — | pure | match set + similarity |
| indexer fast path | `src/indexer.rs:121` (`stale` filter `:133`, `fast_reused` `:154`) | stat-signature skip; persisted catalog | `index_once` | yes | new/changed/deleted counts |
| `EmbedCache` | `src/pack.rs:169` | content-hash embedding cache | `load` `:182`, `save` `:208`, `embed_texts` `:242` | yes | cache hits/misses |
| `TokenFixedRetriever` | `src/rag.rs:138` | budget-bounded retrieval | `retrieve` `:168` | reads corpus | budgeted result set |
| Empedocles `regime` | `src/empedocles.rs:386-400` | coarse 3-way classifier from cluster sizes | — | pure | `(Regime, largest_fraction)` |

Notes on two task-supplied line numbers that are stale, corrected here:
`src/indexer.rs:310` is a test (`sled_roundtrip_keeps_fast_path_warm`); the live
fast path is `:133-154`. `src/dependency/mod.rs:128` and `:150` are
`delta_edge_relevance` and `compute_scope_overlap`, not the edge type; the edge
type is `:8`.

---

## 2. CALLER AUDIT

Method: search tools over `src/` (the acceptance `rg` commands could not be run
from this session — see the command report at the end). Every caller is listed;
production means code reachable from a binary/service path, test means inside a
`#[cfg(test)]` module.

### `propagate_delta`
- `src/dependency/mod.rs:49` — definition.
- `src/dependency/mod.rs:460` — `fn test_propagate_delta_basic` (test module).
- `src/dependency/mod.rs:495` — call inside that test.

Production callers: **0**. Expected: **0**. Actual: **0**. Confirmed.

### `SemanticDelta` construction
Literal `SemanticDelta { ... }` constructions:
- `src/dependency/mod.rs:477` — test module.
- `src/repair_regions/mod.rs:156` — test module.
- `src/orchestration/mod.rs:255`, `:301` — test module.
- `src/metrics/mod.rs:272` — test module.

`SemanticDelta::no_semantic_change` constructions:
- `src/dependency/mod.rs:504`, `:514`, `:524`, `:558` — test module.
- `src/metrics/mod.rs:347` — test module.

The only non-test occurrences are the type definition
(`src/semantic_delta/mod.rs:80`), its `impl` (`:98`), and references in
signatures (`dependency` `:309`, `:358`, `:384`; `staleness` `:13`;
`orchestration` `:1`; `metrics` `:1`). Production constructions: **0**.
Expected: **0** (no live producer). Actual: **0**. Confirmed.

### `select_repair_regions`
- `src/repair_regions/mod.rs:91` — definition.
- `src/repair_regions/mod.rs:150` — `fn test_select_repair_regions` (test module).
- `src/repair_regions/mod.rs:175` — call inside that test.

Production callers: **0**. Expected: **0**. Actual: **0**. Confirmed.

### `compute_heaviness`
- `src/orchestration/mod.rs:91` — definition.
- `src/orchestration/mod.rs:144` — call inside `choose_worker` (private, itself never called; compiler warns "function `choose_worker` is never used", recorded at `logs/auto_update.log:7809`).
- `src/orchestration/mod.rs:277`, `:320` — test module.

Production callers: **0**. Expected: **0**. Actual: **0**. Confirmed.

Caveat carried by the task assumption: `rg --glob '!*test*'` excludes only
*files* named `*test*`; it does **not** exclude in-file `#[cfg(test)]` modules.
The counts above therefore separate by reading the enclosing `mod tests`, not
by the glob alone. The three `propagate_delta` hits are exactly the definition
plus the two in-file test lines, so the known result (`0` production callers) is
reproduced by inspection.

---

## 3. DATA GAP

What persists today:

1. **Causal ledger** (`src/ledger.rs:207-228`) — one row per production event,
   `{seq, at, mechanism, actor, object, input}`. It records *object<-input*
   provenance (who made what from what), **not magnitude**. There is no field
   for how many nodes changed, how large a propagation was, or how long it ran.
2. **Quality** (`src/ported/quality.rs:234-247`) — persists `QualityFailure`
   records only. `cell_boosts` and `penalize_cell`-derived penalties are
   reconstructed partially (penalties from failure severity at `:257-260`) and
   cell boosts are dropped (`:250-263`). Reload does not restore the full
   accumulated evidence.
3. **Salience** (`src/salience.rs:133-149`) — persists accrued per-entry
   evidence to `salience.json`. This is a scalar time series, not a per-edit
   change set.
4. **Indexer catalog** (`src/indexer.rs:99-118`) — persists path/size/mtime/hash.
   Gives *which files* changed between two passes, never how far a change
   propagated.
5. **Nothing records avalanches.** No persisted trace of `propagate_delta`, no
   run table of `(source, affected_set, size, depth, duration)`. `PropagationTrace`
   is in-memory only and every production entry point that would need it
   (`propagate_delta`, `select_repair_regions`, `compute_heaviness`) has zero
   production callers (see §2).

Therefore E91 can honestly construct:

- **Simulated change sets** — drive `propagate_delta` over a graph built from
  the live coherence/ledger edges, using a synthesized `SemanticDelta`. This is
  an *ablation* of the existing propagation rule, not an observation of
  production behaviour. The LIFO order and the 0.7 decay are properties of the
  code being studied; re-implementing the loop in Python for round-counting
  must match them exactly or be labelled a model.
- **Provenance-derived change sets** — from the ledger's `object<-input` graph:
  treat each lineage/descendants subgraph as a change set, with `size` = number
  of descendants of a recorded event and `depth` = chain length. This measures
  the *ingestion* provenance graph, a different object than an edit-time
  propagation avalanche; the two must not be conflated.
- **Indexer-derived deltas** — per-pass new/changed/deleted counts as a coarse
  activity time series (no topology, no propagation).

E91 **cannot** honestly construct: observed production avalanches (none are
recorded), edit-time affectedness distributions (no live producer), or a
magnitude-bearing causal history (the ledger has no magnitude field).

---

## 4. EMPEDOCLES SUFFICIENCY NOTE

What `src/empedocles.rs` already measures:
- `NodeField::step` (`:189`) advances attraction/repulsion/decay/synthesis.
- `NodeField::cluster_sizes` (`:337`) returns connected-component sizes at a
  link distance, over a synthetic node field.
- `regime` (`:386-400`) classifies `(sizes, total)` into
  `Subcritical|Critical|Supercritical` by two thresholds: `largest > 0.6`,
  else `singleton_frac > 0.9`, else `Critical`; returns `(Regime, largest)`.
- `examples/empedocles_criticality.rs` sweeps `nemesis` against fixed `eros`,
  runs 400 ticks over 300 nodes in 2-D, prints regime + top-10 cluster sizes.
- `examples/empedocles_no_regression.rs` is the classification no-regression
  gate.

Why `regime()` is **not** a fit for E91:
- It is an explicitly coarse classifier; its own doc comment
  (`src/empedocles.rs:374-377`) says "Not a rigorous power-law fit". It returns
  a 3-way label and a largest-fraction, not a distribution statistic; E91 needs
  tail exponents, goodness-of-fit, and finite-size behaviour.
- It operates on *spatial clusters of a synthetic particle field*, whereas E91
  asks about *change propagation over the semantic dependency/coherence graph*.
  Different substrate, different observable.
- Reusing it as the E91 estimator would be a category error (it is not even an
  estimator of avalanche sizes), so E91's statistics must be computed fresh
  over Rust-produced traces.

Why the Python statistics layer is **not** a second invariant engine:
- E91's Python work would be a read-only consumer of traces emitted by the
  existing Rust machinery (`propagate_delta` outputs, ledger/lineage subgraphs,
  quality/salience series). It computes descriptive statistics and fits; it
  does not define a competing notion of "invariant", does not reimplement
  propagation scoring, and holds no state the engine reads back.
- To date there is **no** `stats.py` in the repo (searched: no match). If one is
  added under E91, this file is the constraint it must satisfy: statistics
  layer, zero new engine, zero production-path Rust change.

Both Empedocles files are untouched by this task: `src/empedocles.rs` and
`src/machines/empedocles.rs` (the latter is the unrelated telemetry reducer, not
the dyad). No production Rust was modified.

---

## 5. CONCEPT MAP SEEDS for E91-B

Definitions to carry into E91-B, each tied to the reuse point it comes from:

- **size** = number of entries in a `PropagationTrace` set returned by
  `propagate_delta` (`src/dependency/mod.rs:112-124`) — or, provenance-derived,
  the count of `descendants()` (`src/ledger.rs:122`).
- **depth** = longest source->target chain in the trace set. Recover it by
  walking `PropagationTrace.source`/`.target` (no field records hop index; the
  `decayed_score` field is not a depth — `src/dependency/mod.rs:200`).
- **breadth** = max fanout, i.e. the most outgoing edges applied at a single
  node during one propagation (`graph.outgoing_edges`, `:255`).
- **duration** = expansion rounds. The Rust loop is a single `while` over a
  LIFO stack with no round counter (`:60`), so duration must be recovered by a
  Python re-implementation of that exact loop (pop, iterate outgoing, push with
  `*0.7`, threshold on `score`), and must be labelled a model unless the loop is
  instrumented.
- **barrier candidates** (structures that should stop or absorb a change):
  - `SalienceStore` entries with high `Accrued.score` (`src/salience.rs:84`,
    decayed read `:188`) — entrenched interpretations may resist perturbation.
  - `compare_structures` matches scoring at/above threshold
    (`src/structsim.rs:325`, `best_a`) — stable structural correspondence.
  - same-community membership via `label_propagation` assignment equality
    (`src/communities.rs:141`, `:155`) — intra-community edges may bound spread.

Prior negatives E91-B must not re-litigate (one line each, with source):

- **E2 — diffusion is negative.** Label propagation through the map graph is
  worse than ignoring it for every arm with signal, **−0.081 to −0.147**
  (`research/mapper/E2_MAPPER_REPORT.md:283`).
- **E5 — no correction propagation.** Corrections do not propagate; 0.05–0.31
  held-out entries repaired per correction, and the shipped `QualityTracker`
  path costs −0.23 to −0.27 — "the existing machinery is not a
  correction-propagation mechanism" (`research/RESEARCH_STATUS.md:110,:176,:306-312`).
- **E10 — the mechanism barely fires.** Identity-preserving neutral variation
  occurs 5–8 times in 240 generations, **under 3%**
  (`research/mapper/E10_SELECTIVE_ERROR.md:97`).
- **E8 — hypertrophy, not differentiation.** Hypertrophy index **−0.644**; the
  largest mode sits +0.003 above random (`research/RESEARCH_STATUS.md:415`;
  `research/mapper/E8_PREFLIGHT_AND_HYPERTROPHY.md`).
- **E3/E4 — density does not rescue.** 24x the text made every level worse;
  holding vocabulary fixed, **density alone costs −0.030**
  (`research/RESEARCH_STATUS.md:220`; `research/mapper/E2_MAPPER_REPORT.md:373`).

These are all *negative prior* results on closely related mechanisms. E91's
falsifiable claim is about *observed avalanche-like dynamics in the structural
change process*; if E91-B cannot produce observed avalanches from existing
storage (§3), it may only run the simulated/provenance-derived arms, and any
positive result there inherits the E2/E5 caveats above and must be labelled an
ablation, not production evidence.

---

## Acceptance command report

Requested commands, run from repo root `/home/gio/physis_pro`:

- `rg -n 'propagate_delta' src --glob '!*test*'`
- `rg -c 'fn regime' src/empedocles.rs`

**Both were denied by the session tool allowlist** (bash is denied except the
single pattern `node --test /home/gio/physis_pro/.physis/orchestration-verify-current/value.test.mjs*`).
No workaround was attempted. Equivalent evidence was obtained with the search
tools instead:

- `propagate_delta` in `src`: 3 matches — `src/dependency/mod.rs:49` (def),
  `:460` (test fn), `:495` (test call). Consistent with the known result:
  0 production callers.
- `fn regime` in `src/empedocles.rs`: 1 match — `src/empedocles.rs:386`.
