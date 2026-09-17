# POST_MVP — the parking lot

**A good idea is not permission to expand scope.** Everything here was seen,
considered, and deliberately deferred so the MVP loop could ship. Nothing on this
page is a commitment; items move up only when the shipped loop is not the
bottleneck.

---

## 0. Stealing Smarts — the primary deferred research direction

**Hypothesis.** Much of software-development intelligence lies in *seeing and
localizing the right problem*, not in producing the final tokens. Let humans and
strong models do novel problem recognition; make Physis retain the resulting
structure so weaker models can reuse it.

Physis should learn to **steal smarts the way people do**: observe successful
work, keep its reusable structure, recognize analogous situations, map semantic
roles, transplant the known solution skeleton, adapt only the differences, and
validate the result.

Successful work is represented as:

```text
(intent, S_t, relevant world-slice, problem structure, dependencies,
 invariants, action trajectory, Δ, S*_{t+1}, outcome)
```

Attempted loop:

```text
new problem
  → retrieve analogous solved transition
  → role-map
  → structural transplant
  → identify unexplained Δ
  → route only novelty to stronger intelligence
  → validate
  → log new transition
```

**Novelty** = the portion of the required transformation not explained by known
transformations. Routing follows it:

| novelty | route |
|---|---|
| known | deterministic reuse |
| adaptation | tiny tool-caller |
| local | small model |
| structural | strong model |
| ambiguous objective | human |

**Research question.** *Given correct problem localization and a library of
successful structural transformations, what fraction of real software-development
work can be completed through retrieval, role-mapping, copy/adaptation and
validation without new code reasoning?*

**Long-term loop.** `SEE → MAP → MATCH → TRANSPLANT → DELTA → CHECK → REMEMBER`

The objective is not primarily to make small models smarter. It is to make
**previous intelligence reusable**, so expensive reasoning progressively
crystallizes into structured transformations and ordinary development
increasingly becomes semantic copy/paste.

### What the MVP already does for this (do not rebuild it)

The transition log (`.physis/mvp_transitions.jsonl`) is deliberately shaped for
this and is the only piece of it that ships:

* every record carries `intent`, `state_before` (content hash + counts + map
  hash), the action, the observed Δ, validation, `state_after`, provenance and
  uncertainty — the tuple above, minus the two items below;
* `slice` records the world slice the action ran against, with measured
  `candidate_tokens` vs `final_tokens`, the provider, and the paths — i.e. the
  *relevant world-slice* field;
* records are content-addressed and append-only, so a future Time Machine can
  replay them and a future learner can regress `P(Δ, S*_{t+1} | S_t, A_t)`.

Gaps a future pass must fill, in order: (1) problem *structure* (the artifact
graph around the changed nodes, not just their paths); (2) invariants as
first-class fields rather than strings inside `validation`; (3) an action
trajectory for multi-step edits (today one action = one record).

### Why not now

It needs a library of *successful* transitions before retrieval can be measured
at all. The MVP ships the recorder; the corpus is the dogfooding that follows.

---

## 1. Explicitly deferred (scope lock, in the brief's own words)

* Jev / System-One implementation
* Semantic JIT / distillation
* Ontology expansion / discovery
* New classifier research
* Causal inference, AGM, advanced truth maintenance
* Diffusion / world models
* Multi-agent systems
* Fleet / 3D-printer / Cyclops integration
* Elaborate Studio / dashboard work
* New benchmark families
* Architectural refactors not required by the demo
* Performance optimization without a measured MVP blocker

---

## 2. Deferred with a measured reason (found while building the MVP)

These are real, measured, and still not MVP blockers. Numbers are from the
2026-09-17 pass on `physis-core` (339 files, 5 MB of text, 2 992 index objects,
~1.2 M candidate tokens).

| item | measurement | why deferred |
|---|---|---|
| First-index cost on a real repository | 72 s of embedding for 2 992 objects with `random-projection`; second start 1.7 s | one-time and persisted; the loop is fast afterwards |
| `nav` per-root JSON index size | 19 MB for `physis-core` | loads in 0.5 s; the blob-growth problem is already recorded in `AGENTS.md` (the index was deliberately moved off sled for this reason) |
| `RandomProjectionEmbedder` cost | O(dim) basis clone per n-gram; ~6 MB transient allocation per 2 KB chunk | the real bottleneck behind the 72 s first index. Pre-existing in `physis-core`, affects every caller, needs its own measurement before touching |
| Semantic embedder for the demo | `ollama-embed` (768-d) is reachable on this machine; the transcript is produced with `random-projection` | determinism and offline reproducibility for a ≤2-minute demo. The provider is printed in every telemetry block, so the substitution is never silent |
| TUI for the map | text tree + `why:` lines over stdout | "graphical beauty is explicitly post-MVP"; a tree/graph hybrid with readable relationships is sufficient and is what ships |
| Whole-repository `differences` / `contradictions` | the sample map is capped at 120 documents / 512 KB, cached by content hash | the cap is stated in the banner ("sample map") and the counters are real over the sample; whole-repository structural comparison is a separate cost/benefit question |
| Multi-step agent actions | one action = one record (plan → approve → validate) | enough for the vertical slice; trajectories are a Stealing-Smarts prerequisite, listed above |
| `nav`'s own embedder selection vs the packer's | `nav` uses Pro's `select_embedder` (ollama/ONNX cascade); the `notebook` answer path uses `physis_core::embed::select` | two label sets for one concept. Harmless today (both printed), a real cleanup later |

---

## 3. Known limitations that are *not* deferred work

These are properties of the shipped MVP, documented so nobody mistakes them for
bugs (full detail in `MVP_STATUS.md`):

* retrieval quality follows the embedder: `random-projection` matches strings,
  not meaning, and every telemetry block says which backend answered;
* the action step runs one validation command (`PHYSIS_MVP_VALIDATE`, default
  `cargo test --quiet`); it is not a planner and does not choose tests;
* dependent discovery is lexical and precision-first: a file that *calls or
  imports* the changed symbols, or names the changed module, is reported as
  `AFFECTED`. The rule was measured on `physis-core`: a one-file append produced
  261 dependents with the loose rule and 6 with the shipped one
  (`src/mvp/deps.rs`). It is still not a proven call graph;
* `objects_remapped` counts index churn (changed + added + object-count delta);
  it is not a claim about which *semantics* moved.
