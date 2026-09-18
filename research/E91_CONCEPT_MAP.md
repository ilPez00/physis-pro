# E91-B — Concept map: SOC terms bound to engine observables

Scope: bind each SOC term E91 needs to exactly one observable this engine
already produces, with the code path pinned. No imported exponents, no neural
analogies, no second engine. Depends on E91-A
(`research/E91_ARCHAEOLOGY.md`); the reuse map, caller audit, and data gap
there are not repeated here except as pinned references.

Repo root for every path: `/home/gio/physis_pro`.

Naming correction carried from E91-A §1: `propagate_delta`
(`src/dependency/mod.rs:49`) is **LIFO/DFS** — `queue.pop()` on a `Vec`
(`:60`) — despite the comment at `:55` saying BFS. Every definition below
assumes stack order. Any Python re-implementation that uses a FIFO queue is a
different dynamical rule and must be labelled a model, not a replay.

---

## 1. Propagation-arm definitions (from `propagate_delta` traces)

One avalanche = one call `propagate_delta(graph, source, delta, threshold)`.
The raw material is the returned `Vec<(node, score, PropagationTrace)>`
(`src/dependency/mod.rs:112-124`). `PropagationTrace` is defined at
`src/dependency/mod.rs:184-201`.

### size

**size = number of entries in the returned trace set** (one entry per
affected node record; the `affected` map at `:57` dedupes by target, keeping
the strongest score at `:88-90`).

- Measurable in memory only. Nothing persists traces (E91-A §3.5).
- The `threshold` argument sets the size scale directly: the skip test is
  `score < threshold` (`:83`), and it tests `score`, **not** `decayed_score`.
  Same graph + same delta + different threshold = different size by
  construction, so every reported size must carry its threshold.

### depth

**depth = longest source→target chain recoverable from the trace set.**

Critical field-level fact: every trace's `source` is the propagation **root**
(`source: source.to_string()`, `:94`) — not the per-hop parent. There is no
hop-index field, and `decayed_score` (`:200`) is not a depth (it is a product
of strengths, relevances, overlaps, and 0.7 decays along one path, and two
different path lengths can yield the same value). Therefore:

- depth is recovered by chaining **targets through the graph**: target A → target
  B iff `B ∈ graph.outgoing_edges(A)` (`:255`), restricted to nodes present in
  the trace set. Depth = longest such chain starting at `source`.
- A cycle in the graph does not produce infinite depth: the `affected` map
  keeps only the strongest score per target (`:88-90`), so the trace set is
  finite and the chain walk terminates on visited-set.
- Unreachable-without-graph case: given traces alone with no graph handle,
  depth is unrecoverable (parent links are not stored). The harness must keep
  the graph reference alongside the trace set.

### breadth

**breadth = max fanout: the largest number of outgoing edges expanded at a
single popped node during one propagation.**

- Read at `graph.outgoing_edges(&node_id)` (`:62`) per pop (`:60`).
- Like duration (§2), breadth is not stored in the trace — it must be counted
  by the instrumented loop / Python re-implementation. Counting edges
  *traversed* (all outgoing of each popped node) vs edges *recorded* (only
  above-threshold hits at `:83-104`) are two different breadths; the harness
  must state which it reports. Default: traversed (the attempt), with recorded
  as a secondary column.

### duration

**duration = expansion rounds = number of `queue.pop()` iterations until the
stack empties**, counted by a Python re-implementation of the exact loop.

The Rust loop (`:60-109`) is a single `while` over a LIFO stack with no round
counter and no wave structure. One pop = one node expansion. The
re-implementation must match these semantics exactly or be labelled a model:

1. stack starts `[(source, 1.0)]` (`:56`);
2. pop from the **end** (LIFO);
3. for each outgoing edge: `score = incoming × strength × relevance × overlap`
   (`:74-77`), `decayed = score × 0.7` (`:80`);
4. skip when `score < threshold` (`:83`) — on `score`, not `decayed`;
5. on `score > previous best` for that target, record trace (`:93-103`);
6. push `(target, decayed)` **unconditionally** — including when the score was
   skipped or not an improvement (`:107` runs regardless of the `:83` and
   `:89` branches). A re-implementation that pushes only on record diverges.

Until the Rust loop is instrumented, duration is a property of the
re-implementation, labelled a model. Hop-count proxies time only under a
fixed separation rule (see §4) — the standing assumption of this node
(uncertainty 0.3).

### barrier

A barrier is a structure the propagation should stop at or be absorbed by.
Three candidates, each with a pinned readout — all *candidate* barriers, none
assumed effective until E91-F tests them against control:

1. **Salience barrier** — `SalienceStore` entries with high decayed
   `Accrued.score` (`src/salience.rs:84`, read at `:188` via `score()`).
   Entrenched interpretations resist perturbation. Readout: `score(tenant,
   ctx, entry, interp)`; bounded influence via `bias()` (`:211`) and cell
   mean via `cell_bias()` (`:225`). Per-tenant/context keying means a barrier
   in one tenant is not a barrier in another.
2. **Structural barrier** — `compare_structures` / `compare_node_lists`
   (`src/structsim.rs:289,:301`) matches scoring at/above threshold
   (`best_a`, `:325`). Stable structural correspondence across a boundary
   should absorb change. Readout: match set + similarity at a stated
   threshold.
3. **Community barrier** — same-community membership from
   `label_propagation` assignment equality (`src/communities.rs:87-133`,
   assignment compared at `:141`, `:155` via `detect_communities` `:139`).
   Intra-community edges may bound spread. Readout: community assignment +
   cohesion; the detector is read-only (oversized communities reported, not
   split).

---

## 2. Provenance-arm definitions (from the causal ledger)

A different object than a propagation avalanche. The ledger
(`src/ledger.rs:36`) records *object←input* provenance per tenant, persisted
to sled rows `ledger:{uid}:{seq}` (`:207-228`). It has **no magnitude field**
(E91-A §3.1). Definitions for this arm:

- **size** = count of `descendants(object)` (`src/ledger.rs:122-145`).
- **depth** = length of `lineage(object)` chain (`:98-119`), following
  `input` links.
- **`Serves` links are excluded** from both walks (`:109`, `:131-133`) —
  purposes (`purposes()`, `:148`) are final-cause annotations, never
  derivation steps.
- **duration**: the ledger carries event times (`at`, monotonic `seq`), so
  wall-clock span is available — but it measures ingestion history, not
  propagation dynamics. No avalanche-duration claim may be built from it.

Provenance size/depth and propagation size/depth must never share a column
without an arm label. Conflating them repeats the error E91-A §3 already
rules out.

## 3. Indexer-arm (activity series, no topology)

`index_once` (`src/indexer.rs:121`) with the stat-signature fast path
(`:133-154`) yields per-pass `new` / `changed` / `deleted` counts. This is a
coarse activity time series: it says *how many files moved*, never *how far a
change propagated*. Usable as a null-rate background for E91-F, not as
avalanche sizes.

## 4. Separation rule — deferred to PREREG (E91-C)

The following are **not decided here**; E91-C must fix each before any fit:

1. What counts as one avalanche when two `propagate_delta` runs from
   different sources have overlapping trace sets (union, split by source, or
   discard overlaps).
2. Whether runs at different `threshold` values may be pooled (default: no —
   threshold sets the size scale, §1).
3. The control construction the separation rule must also satisfy
   (degree-/edge-count-matched rewired graph + shuffled deltas, same
   thresholds, same seed count — per node E91-C spec).
4. Which preflight check applies (power via `n_tail`) and that the other four
   do not map.

## 5. Invariants honoured

- No imported exponents: every term above resolves to a field, function, or
  loop counter in `src/dependency/mod.rs`, `src/ledger.rs`,
  `src/salience.rs`, `src/indexer.rs`, `src/communities.rs`, or
  `src/structsim.rs`. Branching ratios, MLE exponents, and scaling relations
  are computed in E91-F over these observables only.
- `Empedocles::regime()` (`src/empedocles.rs:386-400`) is never used as a fit:
  its own doc comment (`:374-377`) states "Not a rigorous power-law fit", and
  it classifies spatial clusters of a synthetic particle field, not change
  propagation over the dependency graph (E91-A §4).
- `propagate_delta` is named LIFO throughout; the `:55` "BFS" comment is
  wrong and is not quoted as authority.

## 6. Handoff to E91-C

E91-C receives: size/depth/breadth/duration/barrier definitions above; the
model-vs-observation labelling rule (Rust-loop-exact = replay, anything else
= model); the open separation rule (§4); and the standing assumption that
hop-count proxies time only under a fixed separation rule.
