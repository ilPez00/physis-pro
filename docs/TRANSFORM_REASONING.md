# Transform Reasoning — Symbolic Homomorphism Engine

Status: **experimental**. physis-core 0.1.23. Source: `physis-core/src/transform.rs`.
Deterministic, no embeddings. All behavior below quotes real code.

## 1. Ninety-degree rotation

Classification asks *what* a thing is (which cell, which domain).
Transformation asks *how* one world becomes another (which op, which bindings).
Same triples, orthogonal question. Classify labels the world; transform rewrites it.
Gates judge the rewrite, not the label. This doc covers the rewrite axis only.

## 2. Homomorphism core

Patterns match by exact token equality with variable binding (`PatElem::Var` /
`Const`); multi-pattern queries join by backtracking (`find_homomorphisms`).
A transform carries structure across worlds while preserving predicates —
tokens rebind, relations hold. No cosine, no threshold, no nearest neighbor:
a binding either matches exactly or it does not, so transfer is provable
instead of probable. Precedent: `transplant.rs` weights shape over name —
"a module that was renamed but kept its place must still align."

Matching semantics, exactly as coded: a `TriplePattern` binds per element
(`Var` captures the token, `Const` requires equality, `p = None` matches any
predicate without binding). `find_homomorphisms` backtracks over multi-pattern
sources, returning every consistent binding map. `TriplePattern::instantiate`
renders a pattern concrete under one map. Ops consume these maps; they never
fuzz them. A token one character off is a different token — that strictness is
the point, and the gates exist to reject what strictness alone lets through.

Confidence ladder (hardcoded in `apply`, not tuned): Projected hypotheses 0.5,
Generalized/Analogized 0.6, Substituted/Inverted/Composed (Inferred) 0.8,
Observed facts carry their intake confidence. New claims never overwrite: 
`push_new` skips duplicates by claim equality, and `apply` starts from
`world.facts.clone()` so the past is monotonic within one step.

## 3. Operator catalog

Derived from `Transform::spec()` → `(op, input, output, gates)` plus `apply()`.
All six ops report gate summary `"logical+empirical must hold"`.

| op | input | output | invariants | failure conditions | coherence gates |
|----|-------|--------|------------|--------------------|-----------------|
| Generalize | `pattern={pat:?}` (a `TriplePattern`) | `abstract role={role} to Any` | existing facts preserved; Observed never dropped (`observed_kept` → empirical 0 on loss); novel `Any`-claims appended as Hypothesized @0.6 | `role {role} never bound` → `op_valid=false` | logical (valid + no violated constraints), empirical (Observed kept), procedural (valid) |
| Substitute | `from={from}` | `to={to}` | triples mentioning neither side survive (`substitution_invariants`); new claims Inferred @0.8, predicate unchanged | no matching triple → vacuous success, nothing added, still accepted (no validity flag) | logical + empirical; `substitution_preserves_invariants` test pins this |
| Invert | `predicate={p}` | `inverse={p.invert()}` | subject/object swapped; Requires↔Produces flip, rest self-inverse (`Predicate::invert`) | unknown predicate untouched; `Custom` inverts to itself | logical + empirical |
| Compose | `{first};{second}` | `yields {result}` | join on `a.o == b.s` only; result claim Inferred @0.8 | no chain → vacuous success, nothing added | logical + empirical; temporal 0 if self-`Precedes` loop created |
| Project | `hypothesis` | `{s} {p} {o}` of predicted triple | single Hypothesized @0.5 claim appended, nothing else touched | never fails structurally — gates alone decide (weakest confidence by design) | logical + empirical; causal 0 if claim contradicts a causal triple via `Contradicts` |
| Analogize | `{n} patterns` | `{m} mappings` | source homomorphisms remapped var-by-var through `map`; claims Hypothesized @0.6 | `no source homomorphism` → rejected; `unbound map keys: [...]` → rejected (`invalid_analogy_rejected` test) | logical 0 on either failure; procedural mirrors validity |

Accept rule (`apply`, line ~636): `gates.composite() >= ACCEPT_FLOOR (0.5)`
**and** `logical == 1.0` **and** `empirical == 1.0`. Temporal/causal can only
veto via the composite, never accept alone.

Gate dimensions, each scored 0/1 inside `apply()`:

| dimension | 1.0 when | 0.0 when |
|-----------|----------|----------|
| logical | op structurally valid **and** `incoherent_after()` empty | unbound role, no homomorphism, unbound map keys, or any constraint violated |
| empirical | every Observed fact survives in `after` | any Observed claim dropped |
| temporal | no self-`Precedes` loop (`s == o`) in `after` | reflexive `Precedes` triple present |
| causal | no `(s, causal-p, o)` + `(s, Contradicts, o)` pair in `after` | causal claim directly contradicted on same pair |
| procedural | op structurally valid | same validity flag as logical's first half |

`TraceStep` records `op/before/after/gates/accepted/note` per application —
the full replay bundle. `note` is human-readable (`"generalized role x"`,
`"substituted Alice -> Alicia"`, `"no source homomorphism"`); gates are the
machine-checkable half.

## 4. Four layers

1. **World** — `WorldState { facts: Vec<Proposition>, constraints: Vec<Constraint> }`.
   The `world.rs`-equivalent: scored propositions plus hard `Requires`/`Forbids`
   constraints checked by `Constraint::check`.
2. **Ops** — `transform.rs` `Transform` enum + `apply()`: propose candidates,
   never mutate in place; every application returns `(WorldState, TraceStep)`.
3. **Gates** — `coherence_dimensions` (`CoherenceProfile`: logical, empirical,
   temporal, causal, procedural, each 0/1) scored inside `apply()`; accept floor 0.5.
4. **Audit + store** — `epistemic.rs` (`EpistemicAuditTrail::record`,
   `history_for`, `reconstruct_status_at`) persists each accepted `TraceStep`
   as an `EpistemicEvent`; replay rebuilds status at any point.

Worked micro-example (from the lib tests): world holds
`Alice —Exchanges→ Bob` + `Bob —Exchanges→ Alice` (both Observed).
`Generalize { pattern: x —Exchanges→ y, role: "x" }` binds `{x: Alice, y: Bob}`,
emits `Any —Exchanges→ Bob` as Hypothesized @0.6, keeps both Observed facts,
gates all 1.0 → accepted. Remove the relation instead
(`without_relation(Exchanges)`) against a `Requires(exchange)` constraint and
`incoherent_after()` returns `["exchange required"]` — the counterfactual made
visible. Feed `Analogize` a map key (`"zzz"`) no homomorphism binds and the
same machinery rejects: `logical == 0.0`, `accepted == false`.

## 5. Counterfactual API

- `without_relation(&Predicate)` — clone world minus all facts with that
  predicate. Ex: `world.without_relation(&Predicate::Exchanges)` asks
  "what breaks if exchange never happened?"
- `substitution_invariants(a, b)` — triples mentioning neither `a` nor `b`.
  Ex: `world.substitution_invariants("Alice", "Alicia")` lists facts a rename cannot touch.
- `incoherent_after()` — descriptions of currently violated constraints.
  Ex: `stripped.incoherent_after()` returns `["exchange required"]` after the
  relation it depended on was removed (`absent_relation_yields_incoherence` test).

## 6. Baselines — what beating similarity means

Similarity transfers by resemblance: nearest vector wins, no reason given.
`transfer_beats_similarity` (planned baseline test) must show a case where
cosine picks the wrong donor (surface name match) while `Analogize` picks the
right one (exact structural binding) — or the reverse is documented as a miss.
Beating similarity means: higher transfer success **with** a replayable
`TraceStep` (bindings + gates) for every win. A win without a trace is anecdote.

Bench harness: `physis-core/tests/transform_bench.rs` builds a mycorrhiza world
(`ROOT —Exchanges→ RESOURCE`, `RESOURCE —flows_to→ FUNGUS`,
`FUNGUS —Supports→ ROOT`, `ROOT —Requires→ RESOURCE`, all Observed) with two
`Requires` constraints, then pits `cosine_sim` (via `models`) against the
transform ops on the same world. Removing `Exchanges` is counterfactually
incoherent by construction (second `Requires`), so the bench measures exactly
the gap section 5 describes: similarity still scores the stripped world as
"close" while the gates reject it. Run: `cargo test -p physis-core
--test transform_bench`. The bench is a test-harness file, not criterion —
promotion still needs all five section-7 gates.

## 7. Transition criterion (experimental → supported)

All five required, in order:

1. Novel proposition — an accepted `apply()` produces a triple absent from `before`.
2. Reconstructed `TraceStep` — `op/before/after/gates/accepted/note` round-trips
   through the epistemic store (`record` → `reconstruct_status_at`).
3. Independent gate rerun — fresh `CoherenceProfile` recomputed from `after`
   triples alone reproduces `accepted`.
4. Similarity-fails case — one recorded instance where embedding retrieval picks
   wrong and homomorphism picks right (or vice versa, documented).
5. Invalid-rejected case — `invalid_analogy_rejected` pattern: unbound map keys
   score `logical == 0.0` and `accepted == false`.

## 8. Coordination

- qwen n-gram agent owns `physis-core/src/embed_ngram.rs`,
  `physis-core/src/epistemic.rs`, `physis-core/tests/test_ngram_embed.rs`.
- transform track owns `physis-core/src/transform.rs`, `physis-core/tests/transform_bench.rs`,
  `docs/TRANSFORM_REASONING.md` (this file).
- No submodule commits: each track edits only its owned paths; shared-surface
  changes (lib.rs exports, conduit types) need explicit joint review first.
