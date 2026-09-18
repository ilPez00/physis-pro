# Operator Crystallization — evaluation and implementation plan

> Sequencing update, 2026-09-18: [agentic workspace programme](2026-09-18-agentic-workspace.md)
> is current. This is a research/design backlog, not the demo critical path.
> Compare agent traces, a handwritten recipe and a learned operator on held-out
> work first. Python/Instructor and registry infrastructure wait for that gate.
> An optional dependency does not establish a working composer gateway.

Date: 2026-09-18. Status: evaluation + plan; optional dependency landed (§9),
operator runtime and composer not implemented by this plan.

Proposal: typed LLM compositions observed, canonicalized, generalized via
anti-unification, replay/shadow-validated, promoted into a deterministic
symbolic operator registry. Intelligence paid once, structure kept forever.

Verdict: direction matches Physis doctrine (`grounding.rs`: schema ≠ truth;
`WHAT_PHYSIS_IS.md`: propose is not judge). Two-thirds of the machinery
already exists. One third must be built, and two parts of the proposal must
change: Python Instructor needs an optional embedding adapter, and open-world
predicates cannot promote. The adapter design below remains deferred.

---

## 1. Architecture audit — what already exists

### Settings editor dependency (2026-09-18 update)

The [workspace programme](2026-09-18-agentic-workspace.md#5-reuse-architecture-and-ownership)
requires a **Memory manager and Primitives editor in Settings** before learned
operators ship. Editor initially handles existing data-backed definitions and
deterministic recipes. Reuse its catalogue, type validation, impact preview,
versioned apply/rollback and evidence panel when registry candidates become real.
Do not create a second OPS editor or make settings depend on Python/Instructor.

Memory manager exposes source traces, provenance, retention and retrieval scope.
Excluding/purging evidence marks dependent candidates as needing reevaluation;
never silently keep a promotion claim whose supporting evidence was removed.
Historical runs keep their operator version and explicit evidence availability.
Editing a primitive cannot bypass replay/shadow gates, expand permissions or
mutate an in-flight run. Manual recipes and learned operators remain labelled.

| Proposal needs | Current implementation | State | Verdict |
|---|---|---|---|
| Symbolic executor + trace step | `physis-core/src/transform.rs`: `Transform` (:294), `TraceStep` (:381), `apply(world, op)` (:416) | Shipped | **Extend — never a second executor** |
| World state + constraints | `transform.rs`: `WorldState { facts, constraints }` (:252), `ConstraintKind::check`, `substitution_invariants` | Shipped | **Extend — never a second `WorldState`** |
| Deterministic pattern matcher | `transform.rs`: `TriplePattern::match_against`, `find_homomorphisms` backtracking join (:148); measured E53 | Shipped | **Reuse before any new matcher** |
| Trace source (agent tool sequences) | `src/sessions.rs`: `Event::{ToolCall,ToolResult}`, adapters (Claude Code + Codex); `decisions.rs::fold_episodes` | Shipped | **First discovery corpus; no new telemetry in v1** |
| Proposal-only-model doctrine | `src/grounding.rs::apply_gate` — ungrounded LLM fields dropped | Shipped | **Reuse: schema validity ≠ truth; replay promotes, never the model** |
| Propose-then-review shape | `src/discovery.rs` (pure propose, human reviews JSON) | Shipped | **Same shape for operators** |
| Evidence stats + persistence | `src/quality.rs` over `ported::quality`: penalties/boosts, `cell_standing`, JSON persistence | Shipped | **Mirror for operator evidence** |
| Counterfactual A/B | `src/planner.rs::simulate` — before/after diff, no live mutation | Shipped | **Template for shadow comparison** |
| Append-only trace persistence | `src/ledger.rs`: sled rows `ledger:{uid}:{seq}`, idempotent, replay-stable keys | Shipped | **Same row discipline for traces** |
| LLM entry point | `src/ai/agent.rs` ReAct loop + `provider.rs` cascade, `src/web/agent.rs` SSE | Shipped | **Composer gateway plugs in here** |
| Context packing | `rag.rs::TokenFixedRetriever`, `count_tokens` | Shipped | **Packs registry + ontology into composer context** |
| Readable persisted graph | `src/graph.rs` JSON save/load | Shipped | **Registry = versioned JSON, not sled** |

Missing: operator registry, IR executor, canonicalizer, subgraph miner,
anti-unifier, promotion/demotion, shadow runner, composer gateway, metrics.
No Python bridge yet — see §2.1: `pyo3` is added as an OPTIONAL feature so the
real Instructor library can drive the typed gateway, while every hot path
stays pure Rust.

## 2. Evaluation — accept, change, reject

**Accept.** Model proposes, only execution + deterministic checks promote
(already Physis law). Traces first-class with topology apart from literals
(same as `CausalLedger` discipline). Data-defined operators, no rebuild
(same as ontology-from-`config/` JSON). MDL + operational gain + raw metrics
kept (fits benchmark culture: pre-registered baselines, `NOT MEASURED`
legal). `expand(op)` reversibility, versioning, demotion on drift. Simple
deterministic algorithms first (E53/E55 say measure before upgrading).

**Change.**

1. **Real Instructor via `pyo3`, behind a feature flag.** `pyo3` (optional
   dep, feature `instructor`) embeds a Python interpreter that imports the
   actual `instructor` package, so the typed gateway is the library the
   proposal names — not a look-alike — and structured-output retries /
   validation semantics come from Instructor itself. Toolchain verified on
   pansa: Python 3.14.7 with headers (`/usr/include/python3.14`), rustc
   1.98.1, cargo 1.98.1. Pin `pyo3 = "0.25"` with `abi3-py311`
   (`extension-module` NOT used — the interpreter is embedded, not
   loaded), so a future Python bump does not break the build; if the crate has
   a release that supports the local interpreter natively (>= 0.26 for
   3.14), prefer it and re-pin — record whichever is used. If the build
   script refuses the 3.14 interpreter for config, set `PYO3_PYTHON` to a
   3.11+ interpreter and record that version in provenance.
   Hard rules: the feature is OFF in default CI builds; nothing on the
   request/execution hot path may take the GIL; the Python side receives
   only a registry snapshot + task + budget and may return ONLY a typed
   `CandidateComposition` (validated in Rust against the live registry);
   it never touches the executor, the store, or the registry. If the
   feature is off or Python is absent, `compose.rs` falls back to the
   native cascade path (same schema, serde-validated) and reports the
   degradation — never silently. Rust owns execution, traces, validation,
   provenance, statistics, promotion: `pyo3` is a proposal adapter, not a
   runtime dependency.
2. **IR lowers onto `transform.rs`.** New `src/ops/graph.rs` defines `OpNode`
   (Call/Sequence/Parallel/Branch/Guard/Bind/Return; Map/Filter/Reduce wait
   for a trace that demands them) and executes via `transform::apply`, so
   every run yields `TraceStep`s for free. No second `WorldState`, executor,
   or trace-step type.
3. **Predicates = closed typed DSL.** Free-text predicates would smuggle the
   model back into promotion. v1: `TypedFact | TypeCheck | CellPin |
   NumericBound`, all checkable by `Constraint::check` after replay.
   Counterexamples in v1 = replay over held-out traces + hand-authored edge
   fixtures, not theorem proving. Extension point stays open, gated on
   measurement.
4. **Anti-unify op-chains first.** Full connected-subgraph enumeration waits
   behind E92. v1 discovers motifs in canonical operator chains (the
   read→parse→find_symbol→references→rank shape is a chain);
   `find_homomorphisms` is the matcher. Equal constants stay; differing
   values lift to typed variables by common supertype from the parameter
   ontology; else split the cluster.
5. **Ship native primitives first.** `read_file`/`parse`/`find_symbol`/
   `find_references`/`rank_dependency` are not Physis ops today. P1 ships a
   `NativeOp` table (8–12 deterministic Rust fns: file read, grep/regex,
   JSON walk, classify, cell check, ledger append) — the only compiled part.

**Reject.** Scalar-only promotion (keep raw metrics; scalarize only at the
policy edge, logging the full vector). Abstraction on first sight (support +
replay + shadow required; no model-confidence fast path). A parallel
discovery stack (subsumption: exact structure + type + order first, no
embeddings unless E92 demands them).

## 3. Minimal schema (Rust + serde; JSON on the wire)

```rust
// src/ops/mod.rs — OperatorId = "name@version"
pub struct Operator {
    pub id: String, pub version: u64,
    pub inputs: Vec<TypedParameter>, pub outputs: Vec<TypeRef>,
    pub graph: OpGraph,                    // data, not compiled
    pub preconditions: Vec<Predicate>,     // closed DSL (§2.3)
    pub postconditions: Vec<Predicate>,
    pub invariants: Vec<Predicate>,
    pub provenance: Provenance,            // source traces + composer id
    pub evidence: EvidenceStats,           // raw metrics, never scalarized
    pub cost: CostModel,
    pub status: OperatorStatus,            // Experimental|Shadow|Stable|Deprecated
}
pub struct CandidateComposition {          // composer output only
    pub name_hint: Option<String>,
    pub inputs: Vec<TypedParameter>,
    pub operations: Vec<PrimitiveCall>,    // refs into live registry only
    pub outputs: Vec<Binding>,
    pub assumptions: Vec<Predicate>,
    pub expected_postconditions: Vec<Predicate>,
    pub expected_invariants: Vec<Predicate>,
    pub rationale_tags: Vec<String>,
}
```

Composer never writes the registry. Executor validates types, references,
bindings; asserts preconditions per step; records `TraceStep`s (latency /
cost / invariant results); verifies postconditions; returns the trace.

## 4. Pipeline

```
composer (typed proposal, model) ──► candidate ──► symbolic executor
    ▲                                                  │
    │ registry snapshot                        trace (sled, topology‖literals)
    │                                                  ▼
    └──── promote ◄── shadow ◄── replay ◄── canonicalize (alpha-rename,
         demote on drift         (held-out)    literal→typed-var, struct hash)
```

Canonical form: ordered op-chain, alpha-renamed bindings, literals as typed
variables, hash = `sha256(op-ids ‖ arg-types)`. Evidence per candidate:
support, success rate, stability, type consistency, invariant pass rate,
cross-context reuse, MDL compression gain, Δtokens/Δlatency/Δmodel-calls/
Δfailures, complexity, counterexample count — stored raw. Replay =
deterministic re-execution on every supporting trace's recorded inputs.
Shadow = run alongside the reference path (`planner::simulate` shape:
before/after diff, no live mutation). `Stable` ops appear in the next
composer snapshot as one primitive; `expand(op)` rebuilds the graph
(hierarchical: promoted ops may contain promoted ops). Subsumption merges
covered ops. Drift (rolling failures, invariant breakage) demotes;
implementations versioned, semantics never silently mutated.

## 5. Phases and gates

| Phase | Work | Gate |
|---|---|---|
| P0 | This doc | review; nothing below starts |
| **E92** | Falsification: one repeated task class (symbol inspection, ≥8 varied files) through the agent loop; harvest via `sessions` adapters; canonicalize; exact-structural discovery; permuted-trace null arm | motif ≥ frozen support, zero null false-positives; else embeddings required — negative goes in the module header, replan |
| P1 | `src/ops/`: `graph.rs` (IR via `transform::apply`), `native.rs` (NativeOp table), `trace.rs` (sled `ops_trace:{uid}:{seq}`, idempotent) + unit tests | `cargo test ops`, `clippy -D warnings` |
| P2 | `canonical.rs` + `discover.rs` (chain motifs, structural hash, frozen MIN_SUPPORT) | E92 reproduced on frozen corpus |
| P3 | `generalize.rs` (anti-unify), `evidence.rs` (quality-mirrored stats), `promote.rs` (replay + shadow + staged statuses), JSON registry + `expand()` | shadow beats reference on frozen set |
| P4 | `compose.rs` gateway — Rust-validation core + `instructor` feature (`pyo3`) binding the real Python Instructor; serde-validated against live registry; utility bias to highest stable op; decomposition kept; cascade fallback when feature off | composer cannot emit an unregistered op (fuzz test); build with and without `instructor` feature both green |
| P5 | Metrics `GET /api/v1/ops/*` (counts, promotions/demotions/day, tokens + model-calls avoided, `symbolic_fraction`, reuse distribution) + `/pulse` panel | zero new writes except replay archives |
| P6 | Killer demo (§6) + pre/post benchmark (§7) | script green, numbers recorded win or lose |

Deliverables 1–14 covered: assessment §1 · modules §1–2 · IR §3 ·
registry/trace/canonical/discovery/promotion/composer/replay/persistence
§3–5 · metrics P5 · tests per phase · demo §6 · benchmark §7.

## 6. Killer demo (scripted, reproducible)

Registry holds only P1 native ops. Agent runs symbol-inspection tasks across
varied files (5–7 primitive calls each; tokens/latency/model-calls
recorded). `ops discover` surfaces the chain → generalize → replay →
shadow → promote `inspect_symbol@1`. Repeat the task class: composer emits
one call, orchestration model-calls ≈ 0. `ops expand inspect_symbol@1`
prints the learned graph; dashboard shows Δtokens/Δlatency/Δmodel-calls and
`symbolic_fraction` rising. Target: orchestration tokens −90%+, model calls
for the operation → 0. Misses recorded anyway.

## 7. Benchmark (pre-registered, construction-matched)

Arms: (a) direct agent loop, no composer; (b) composer + natives,
crystallization off; (c) full crystallization. Same frozen task set, seeds
and registry versions recorded. Metrics: task success by independent check
(never exit code), tokens, latency, model calls, `symbolic_fraction =
deterministic_useful_work / total_useful_work`, `intelligence_amortization
= creating-LLM-compute / future_uses`. Controls: permuted-trace null for
discovery; identical-input pre/post for promotion. `NOT MEASURED` legal.

## 8. Standing constraints

1. No second `WorldState`, executor, or discovery stack.
2. Promotion never from model confidence or schema validity alone.
3. Every operator traces back to recorded traces, or it is a bug.
4. Predicates stay in the closed DSL until a measured need reopens them.
5. Operators are shared infra (global, like ontology); traces tenant-scoped
   (`X-Physis-User` discipline holds).
6. A negative E92 is written into the new modules' headers and kept.
7. Success = after repeated exposure to one structural problem class,
   Physis discovers the reusable computation, earns enough evidence to
   trust it, promotes it, and solves matching instances with less or zero
   model involvement — measured, not claimed.

## 9. pyo3 specifics (for P4)

Status: the dependency and feature have landed in `Cargo.toml` (`pyo3` optional,
`abi3-py311`, feature `instructor`, present in `full`, absent from `default`).
Verified on pansa: `cargo check --features instructor --bin physis-studio`
compiles `pyo3 0.25.1` against the local Python 3.14.7, and `cargo tree` shows
no `pyo3` in a default build — so the feature is real, not decorative.
Nothing calls it yet — P4 builds `compose.rs` on top.

```toml
# Cargo.toml
pyo3 = { version = "0.25", features = ["abi3-py311"], optional = true }
# feature
instructor = ["dep:pyo3"]
# include in `full`, never in `default`; CI runs one build without it
```

Python-side contract (`src/ops/py/instructor_gateway.py`, shipped inline via
`include_str!` so there is no packaging step):

```python
# in: registry snapshot JSON, task, typed state, token budget, model cfg
# out: exactly one CandidateComposition JSON (Instructor enforces the schema)
def compose(payload: str) -> str: ...
```

Rust side (`src/ops/compose.rs`): pack registry+ontology with
`TokenFixedRetriever` → call `compose` under the GIL on a blocking thread
(`spawn_blocking`, never in the executor path) → deserialize →
`validate_operator_references` against the live registry → return candidate.
Tests: (a) cargo check/test with `--no-default-features` (feature absent →
cascade fallback path), (b) with `--features instructor` and a stub
`instructor_gateway.py` that returns a fixture candidate, assert the Rust
validator rejects unknown operator ids and type-mismatched bindings, (c) assert
no GIL acquisition happens on `/api/v1/ops/*` read paths.

Front-end for this plan: [Physis Studio TUI demo](2026-09-18-studio-tui-demo.md) — OPS tab renders the registry, candidate metrics and `expand()`, and the DIFF tab reviews candidate vs promoted graph.
