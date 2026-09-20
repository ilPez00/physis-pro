---
id: PH-025
track: physis-pro
phase: 0
status: review
rung: 3
depends: []
gate: "cargo clippy --all-targets -- -D warnings && cargo test --lib guided:: && cargo test --features tui --bin physis-hud && cargo run --features cli --bin physis-pro -- guided \"read the retrieval notes\" --focus src/retrieval.rs"
commit: "branch agent/PH-024; conventional commits; per PROTOCOL §6"
claimed_by: cline-pansa-2026-09-20
claimed_at: 2026-09-20
---

# Guided execution Phase A — the model chooses inside the world, not about it

## Why (the failure this prevents)

Every agent path in this repo ends the same way: the model is asked to produce
text, the text is parsed, and what does not parse is retried. That is expensive
(`agent call jev` measured 43 s–3 m 10 s per turn, `docs/plans/2026-09-19-progress-and-ship.md` §5),
fragile (a foreign JSON object used to deserialise to an empty success), and it
asks the model for things Physis already knows: which objects exist, which verbs
are legal on them, which ids are real.

The failure is structural, not a prompt problem: **lexical validity was the only
thing standing between a hallucinated id and an execution.** An invalid object
id had to be *rejected after generation* instead of being *absent from the
generation*.

## Context (measured 2026-09-20)

- `registry.rs`: objects (`fs:*`, `host:local`, `device:placeholder`, `agent:*`,
  `intent:*`) keyed by handle, plus the capability table loaded from
  `config/actions.json`. `capabilities_for(oid, verb)` already answers "may this
  verb touch this object" — the affordance layer derives from it rather than
  duplicating it.
- `capability.rs`: `Capability{object_kinds, verbs, risk, tier, cost}`,
  `TEN_VERBS`, and `Observations` — the append-only log at
  `registry::default_obs_path()`. Outcome evidence goes there.
- Executor argument vocabulary is exactly what `registry.rs` reads: `limit`
  (read), `content` (write), `cmd` + `confirm` (execute).
- `action.rs` parses `<verb> <object> [k=v …]` and dispatches; it is already the
  one executor shared by CLI and HUD.
- `mvp::log::hash_canonical` gives a deterministic world version.
- No provider, no network and no licence is needed for any of this.

## Steps (done)

1. `ObjectHandle` with a private field and world-only constructors, so a target
   that is not in the world cannot be typed into existence.
2. `WorldView::{from_registry, resolve, affordances_for_kind, language, version}`
   — objects, capabilities and the action language of the current state.
3. State gating (`state_gate`, `verb_allowed_in_state`) read from the registry's
   own `state` meta: `start` only when not running, `stop`/`restart` only when
   running, and a missing state reads as `unknown`.
4. `Access::{ReadOnly, ReadWrite}` filtering the verb set *before* compilation.
5. `Constraint` IR (`Choice`, `ObjectSet`, `FreeText`, `Sequence`) with
   `json_schema()`, `grammar()` and `token_classes()`.
6. `ConstraintCompiler::compile` → `GenerationContext` (constraint,
   `targets_by_verb`, `capability_by_verb`, provenance, world version).
7. `GuidedAction::{action_line, validate}` with enumerated `Rejection`s.
8. `MockGuidedModel` + `resolve_exact` (one named object + one read verb ⇒ zero
   model calls) + `Metrics` (model calls, bypasses, rejections).
9. `GuidedActionObservation` recorded into the registry's log with actor
   `guided`; `GuidedExecution::step` is the whole loop, plan-only by default.
10. Surfaces: `physis-pro guided <intent> [--focus --write --execute --show]`
    and HUD `guided <intent>`, both rendering the one card.

## Tests added

`src/guided/mod.rs` — 8 tests, one per required case: invalid object neither
offered nor accepted; world state changes the language (running/stopped/unknown);
access removes mutating verbs; deterministic resolution bypasses the model (and
two candidates does not); one constraint compiles to mock + JSON Schema +
grammar; token classes; capability named in the observation; provenance versions
the world slice. HUD parse tests for `guided`/`plan`/`guide`.

## Gate

```bash
cargo clippy --all-targets -- -D warnings
cargo test --lib guided::
cargo test --features tui --bin physis-hud
cargo run --features cli --bin physis-pro -- guided "read the retrieval notes" --focus src/retrieval.rs
```

## Commit strategy

One commit: `feat(guided): Phase A of guided execution (PH-025)`.

## Report

`packets/reports/PH-025.md`.

## Never

- No second object model, action model, permission model or log.
- No LLM path for an exact fact.
- Do not claim token savings without a backend to measure them against.
- Do not let a constraint render as a hand-written prompt.
