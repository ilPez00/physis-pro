# Guided execution — Physis-native constrained generation

Status: **Phase A shipped** (packet PH-025). Everything below the "not yet"
heading is not built. Nothing here calls a model provider in this build: the
only backend is the deterministic mock.

## The loop

```text
registry (objects + capabilities)        ← the world, already built
   ↓  WorldView                 src/guided/affordance.rs
   ↓  ActionLanguage            verbs × objects legal right now
   ↓  ConstraintCompiler        src/guided/compile.rs   → GenerationContext
   ↓  GuidedModel               src/guided/model.rs      mock now, adapters later
   ↓  GuidedAction::validate    src/guided/action.rs
   ↓  action::dispatch          the existing executor (one for CLI, HUD, guided)
   ↓  GuidedActionObservation   src/guided/observe.rs    → the existing log
```

The principle: **never ask a model to generate what Physis already knows.**
Object ids, legal verbs, argument names and the legal target of each verb are
compiled into the constraint, so an invalid action is not a validation failure
to retry — it is not in the generated language.

`Constraint` (`src/guided/constraint.rs`) is the internal IR: `Choice` (closed
value set), `ObjectSet` (closed set of real handles), `FreeText` (the one
generation slot) and `Sequence`. It renders to JSON Schema and to a grammar, so
provider schemas are compilation targets, never the source of truth.

## What Phase A ships

| Piece | Where |
|---|---|
| Typed world handle (`ObjectHandle`, private field, world-only constructors) | `affordance.rs` |
| Affordances derived from the registry's capability table | `affordance.rs` (`WorldView::from_registry`, `language`) |
| State-gated verbs (`start` only when not running; `stop`/`restart` only when running) | `affordance.rs` (`state_gate`, `verb_allowed_in_state`) |
| Access filter: `ReadOnly` removes mutating verbs *before* compilation | `affordance.rs` (`Access`) |
| Constraint IR + JSON Schema + grammar + entropy accounting | `constraint.rs` |
| World → constraint compiler with provenance hash | `compile.rs` |
| Deterministic mock backend | `model.rs` (`MockGuidedModel`) |
| Exact-fact bypass: one named object, one read verb, zero model calls | `model.rs` (`resolve_exact`) |
| Typed action + enumerated rejection reasons | `action.rs` |
| Outcome evidence into the registry's existing observation log | `observe.rs` |
| One renderer | `GuidedStep::render` (used by CLI and HUD) |

## Use it

```sh
physis-pro guided "<intent>" [--focus <handle-or-path>] [--write] [--execute] [--show]
```

- default: read-only, plan only. `--show` prints the constraint card alone.
- `--write` admits mutating verbs into the language (still nothing runs without
  `--execute`).
- `--execute` hands the chosen action to `action::dispatch` — the same executor
  `physis-pro action call` uses, with the same capability risk gate.

In the HUD: `guided <intent>` (aliases `plan`, `guide`) renders the same card in
the reading pane, read-only and plan-only, with the `runnable as:` line naming
the exact CLI call.

Example card (fixture workspace, PH-025 gate):

```text
VALID TRANSITIONS
  read → 6 target(s)
      fs:0e4bfb603e53 (passage) …
CONSTRAINT
  (<Verb> := "read") (<Target> := "fs:0e4bfb603e53" | …) (<limit> := <text:12>)
ENTROPY
  fixed tokens Physis emits: 6
  choice fields: 2 (7 candidates, 2.58 bits)
  free-generation fields: 1
CHOSEN
  read fs:0e4bfb603e53
  route: model — chosen within the constraint
```

## Invariants

- One object model (`nav` + `registry`), one action model (`capability` +
  `action`), one log (`capability::Observations`). This subsystem adds a
  translation layer and nothing else.
- `possible`, `permitted` and `recommended` stay separate: `Access` gates
  permission at compile time, risk travels on the branch, and nothing here
  ranks.
- A target is not a string. `ObjectHandle`'s field is private; only world
  listings construct one.
- Free text is bounded (`max_chars`), because an unbounded field is a way to
  smuggle structure back in.
- Provenance is part of the payload: `world_version`, sources, access and
  `CONSTRAINT_VERSION` ship with every context.
- The model never chooses outside reality: `validate` re-checks a reply against
  the context it was generated under, so a stale or lying backend is caught.

## Not yet (and what unblocks it)

| Missing | Needs |
|---|---|
| Provider backends (llama.cpp grammar, Ollama, provider schemas) | a compile step from `Constraint` to that backend; the IR already renders grammar + JSON Schema |
| `tokens_fixed` / `tokens_choice` / `tokens_free` / `model_forward_tokens` | a real backend to count against. Phase A reports the constraint-side classes (`TokenClasses`), not model tokens — claiming forward-token savings without a backend would be a lie |
| CFG composition, guided programs (deterministic code between generations) | Phase C; `Sequence` + per-verb branches are the seam |
| Consolidation of repeated choices into deterministic routes | a reader over `registry_observations.jsonl` (actor `guided`), and a rule that proposes, never silently overwrites |
| Service/machine objects to act on | a source adapter that registers them (fleet work); the verbs are already gated by `state` |

## Tests

`cargo test --lib guided::` — 8 tests, one per spec case: invalid object can
neither be offered nor accepted, world state changes the language, access
removes mutating verbs, exact resolution bypasses the model, structured
generation always deserialises to the typed action, one constraint compiles to
mock + JSON Schema + grammar, entropy accounting, and outcome evidence with
bypass counters.
