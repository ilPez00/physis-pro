# PH-1xx proposals — next quarter (2026-Q4)

Written from: unknowns sections of PH-024..PH-029 + PH-028 execution session;
PLAN.md hardening backlog; `mesh mem traps`; last 20 git log subjects.
Date: 2026-09-21. Author: kiro.

Each proposal is a full packet stub. A human or strong reviewer decides which
become real packets by copying the frontmatter into `packets/PH-0xx-*.md`.

---

## PH-030 — ResultState: collapse hits/rows/sel into one coherent struct

**Source**: PH-029 report "next measured bottleneck"; PH-027 report stale-hits hazard.

```
id: PH-030
track: physis-pro
phase: 0
status: todo
rung: 2
depends: [PH-029]
gate: "cargo clippy --all-targets -- -D warnings && cargo test --features tui --bin physis-hud"
```

### Why

`App` still has three independently-mutable fields that encode one concept:

```
hits: Vec<NavHit>      — the ranked list
rows: Vec<Row>         — the Ratatui projection of hits
sel:  usize            — the Ratatui cursor into rows
```

`set_results()` is the single write path today, but it is not enforced —
callers can still assign `hits` before calling it (e.g. `back()` does exactly
this). PH-029 documented this as the "next measured bottleneck".

### Steps

1. Define `ResultState { hits, rows, sel }` with a constructor that builds all three atomically.
2. Replace the three fields in `App` with one `ResultState`.
3. `set_results` becomes `ResultState::new(hits, zoomed_out, intent_bands)` — one function, no independent writes possible.
4. Add a `#[cfg(debug_assertions)]` invariant: `sel` points to a `Row::Hit` or the list is empty.
5. Measure: `grep -c 'self\.hits\s*=' src/bin/hud.rs` before/after.

### Tests

- Invariant test: constructing `ResultState` with mismatched sel panics in debug.
- `back()` round-trip still restores focus (the PH-029 regression test is the acceptance).

### Never

- Do not restructure the entire App to get this. One struct swap.

---

## PH-031 — Fleet world: merge per-node indexes into one navigable fleet index

**Source**: PH-028 execution session (fleet scan launched); this session's work.

```
id: PH-031
track: physis-pro
phase: 0
status: todo
rung: 2
depends: [PH-028]
gate: "cargo clippy --all-targets -- -D warnings && cargo test --lib nav:: && physis-hud --no-prescan --view world --snapshot=100x30 --query fleet"
```

### Why

PH-028 shipped `import_bundle` and fleet scanning is now running. The next step
is merging per-node bundles into a single navigable fleet index so `@pansa/…`
and `@feather/…` objects are reachable from one HUD session.

### Steps

1. `nav import` already exists. Add a `--prefix <node>` flag: object labels get
   `[<node>] ` prepended and `ObjectId.origin` becomes `<node>-fs` to avoid id
   collisions between nodes with identical content (same checkout).
2. After each fleet prescan completes, scp the bundle to pansa and import with
   `--prefix feather-1`, `--prefix air`, `--prefix omo`.
3. Verify: `--query "feather"` returns objects from feather's index; `--query "omo dev"` returns omo's objects.
4. Document the resync procedure: on prescan completion → scp bundle → `nav import --prefix <node> --merge` (merge = skip ids already present, update changed).

### Tests

- `import_bundle` with a `--prefix` must prepend to all labels.
- Two nodes with the same file (identical content hash) produce two objects, one per prefix.

### Never

- Do not merge embeddings across different embedder stamps: refuse, do not silently mix.

---

## PH-032 — LensRegistry: register lenses at runtime, not as a closed enum

**Source**: PH-026 unknown "Lens registry, not lens enum"; PH-027 deliberately deferred.

```
id: PH-032
track: physis-pro
phase: 0
status: todo
rung: 2
depends: [PH-027]
gate: "cargo clippy --all-targets -- -D warnings && cargo test --lib --features tui hud::"
```

### Why

`ProjectionId` is a closed 8-variant enum. Adding a new lens (project, schema,
content, dependency, temporal, activity, fleet) requires touching the enum, the
switch in `switch_projection`, the snapshot matrix, and the help text. A registry
— keyed by `&'static str` id — lets each lens module register itself.

PH-027 deliberately waited for "a lens that cannot be expressed as data". The
fleet lens (PH-031) is the trigger: it needs a dynamic source (per-node bundle)
rather than a fixed data shape.

### Steps

1. `LensRegistry: BTreeMap<&'static str, Box<dyn LensBuilder>>` — `LensBuilder` takes a `NavWorkspace` and returns a `Projection`.
2. Keep `ProjectionId` as an alias for `&'static str`. The existing eight lens ids become string constants.
3. Migrate `switch_projection` to look up by id. The projection-focus invariant (`FocusState::switch_projection`) is unchanged.
4. Register the two existing lenses (`"graph"`, `"semantic"`) as the first registered entries to prove the seam.
5. Gate: the headless matrix covers both via `--view graph` and `--view semantic`.

### Tests

- Registering a lens by id and querying it returns the registered projection.
- An unknown id falls back to the world projection, not a panic.
- The focus-preservation invariant holds for registered lenses (reuse the existing `projection_switch_keeps_focus` test shape).

### Never

- No `Lens` trait with one implementation before this packet — PH-027 was right to wait.

---

## PH-033 — nav clear-embeddings CLI + dim-switch self-service

**Source**: PH-028 execution session ("clearing embeddings is required for a dim switch").

```
id: PH-033
track: physis-pro
phase: 0
status: todo
rung: 2
depends: [PH-028]
gate: "cargo clippy --all-targets -- -D warnings && cargo test --lib nav::"
```

### Why

Switching embedders (e.g. from random-projection 384-dim to bge-base 768-dim)
currently requires a Python one-liner against the JSON index. During PH-028
execution this was the actual blocker that made pansa "still" — the index had
mixed 384+768 dim vectors and the new embedder only filled the empty slots.

### Steps

1. `NavAction::ClearEmbeddings` CLI subcommand: `physis-pro nav clear-embeddings [--root R]`.
2. Calls `NavIndex::clear_embeddings()` (already exists) + clears `embedder_stamp`.
3. Persists the cleared index.
4. Prints: `cleared N embeddings (was <old-stamp>). Run prescan to re-embed.`
5. Add a guard in `embed_objects`: if the existing stamp differs from `embedder_kind`, emit a warning and offer `nav clear-embeddings` as the fix path rather than silently mixing dimensions.

### Tests

- After `clear-embeddings`, all objects have empty embeddings and stamp is `None`.
- Guard warning fires when stamp is `bge-base:768` and new embedder kind is `random-projection:384`.

### Never

- Do not auto-clear. The operator must invoke this explicitly — it destroys work.

---

## PH-034 — Guided execution Phase B: provider backends and fast-forward tokens

**Source**: PH-025 unknown "guided B/C/D — provider backends compile from the IR that now exists".

```
id: PH-034
track: physis-pro
phase: 1
status: todo
rung: 3
depends: [PH-025]
gate: "cargo clippy --all-targets -- -D warnings && cargo test --lib guided::"
```

### Why

PH-025 shipped the constraint IR (`Choice`/`ObjectSet`/`FreeText`/`Sequence`)
and its JSON Schema + grammar renderers. Phase A deliberately reported only
constraint-side classes. The token savings claim needs measurement: how many
model calls does Phase A's `resolve_exact` actually bypass on a real session?

### Steps

1. **Measure first**: read `registry_observations.jsonl` lines with `actor=guided`, tally `route=exact-object` vs `route=intent-ir`. Report the bypass rate.
2. Phase B: wire at least one real provider backend (the deterministic mock is Phase A). `JsonSchemaBackend` sends the schema to a provider, receives a structured choice, validates against the IR.
3. `VerbBranch` in the Sequence IR: a constraint that forks on a chosen verb, enabling guided programs (`if read → show; if edit → open inline editor`).
4. Add `Sequence + VerbBranch` to `grammar()` output (CFG composition seam).

### Tests

- `JsonSchemaBackend` round-trip: schema → valid JSON choice → resolved `ObjectHandle`.
- `VerbBranch` grammar produces a valid CFG that the tokenizer parses back.
- `resolve_exact` bypass rate is ≥ 10% on the fixture session log (non-vacuous).

### Rung note

Rung 3: needs a provider key to measure the Phase B backend. The bypass-rate
measurement (step 1) is rung 2 and should be done first.

---

## PH-035 — ExactFact: wire sensors.rs + fact.rs with a caller, test, and query

**Source**: PH-027 deliberate deferral "ExactFact needs prototype sensors.rs/fact.rs ported with a caller, a test and a genuinely useful structured question".

```
id: PH-035
track: physis-pro
phase: 1
status: todo
rung: 2
depends: [PH-027]
gate: "cargo clippy --all-targets -- -D warnings && cargo test --lib hud:: && cargo test --lib world_object::"
```

### Why

`sensors.rs` and `fact.rs` exist on `local-main-import` (commits `ee7bae7`,
`f777492`) but were not ported to the main line — PH-026 explicitly skipped
them until a caller arrived. PH-027 deleted `ExactFact` from `ResolutionRoute`
for the same reason. The feature enables answering structured questions ("what
is the mtime of this file?", "is this process running?") with zero model calls.

### Steps

1. Port `sensors.rs` + `fact.rs` from `local-main-import` to `src/`.
2. Add one genuinely useful structured question: `mtime_of(path)` → answered
   directly from the nav index (no filesystem call, no model call).
3. Wire `ExactFact` back into `ResolutionRoute` with exactly one producer:
   the HUD's textbox parser when the query matches a known fact pattern.
4. Add `ExactFact` to the intent chain `compile_bare` (after `ExactObject`,
   before `Intent`).

### Tests

- `mtime_of` on a known indexed file returns the correct mtime without touching the filesystem.
- `ExactFact` appears in the status line when the pattern matches.
- The route table (PH-027 producer table) stays complete: every variant has a caller.

---

## PH-036 — PHYSIS_OLLAMA_EMBED_OFF: document and add to MANUAL_CONFIG.md

**Source**: PH-028 execution session trap — Ollama won the cascade over bge-base ONNX silently.

```
id: PH-036
track: physis-pro
phase: 0
status: todo
rung: 1
depends: [PH-028]
gate: "grep -q 'PHYSIS_OLLAMA_EMBED_OFF' MANUAL_CONFIG.md && grep -q 'PHYSIS_OLLAMA_EMBED_OFF' README.md"
```

### Why

When an Ollama server is running, it wins the embedder cascade over bge-base
ONNX even though ONNX is the preferred semantic embedder. This is not documented
anywhere the operator looks first — it caused a wasted prescan and a mixed-dim
index during PH-028. The env var `PHYSIS_OLLAMA_EMBED_OFF=1` exists but appears
in no user-facing doc.

### Steps

1. Add `PHYSIS_OLLAMA_EMBED_OFF` to the env var table in README.md.
2. Add a note in MANUAL_CONFIG.md under "Embedder selection": "If an Ollama
   server is running, set PHYSIS_OLLAMA_EMBED_OFF=1 to force the bge-base ONNX
   cascade. Without it, Ollama wins regardless of model quality because it is
   tried first."
3. Add it to `--setup` output when an Ollama server is detected AND
   `PHYSIS_OLLAMA_EMBED_OFF` is not set: "warning: Ollama detected — ONNX
   embedder is bypassed; set PHYSIS_OLLAMA_EMBED_OFF=1 to prefer bge-base."

### Tests

- `--setup` output contains the warning when `PHYSIS_OLLAMA_EMBED_OFF` is unset and Ollama is reachable.
- README and MANUAL_CONFIG contain the env var.

### Never

- Do not change the cascade order — Ollama-first is intentional for the common case.
  The warning and the escape hatch are enough.
