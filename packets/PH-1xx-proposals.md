# PH-1xx proposals — second discovery round (PH-100, 2026-09-22, opencode, pansa)

Sources: `packets/reports/*` unknowns sections (PH-015 §1, PH-020 §1, PH-022
§1–2), `mesh mem traps` (no new physis traps since 2026-09-19 — nothing to
promote), PLAN.md hardening backlog H5 (H6 noted as a future source; H10
residue is PH-006's blocked human input), last 20 git subjects (PH-101 cortex
landed, MS-003 corrections applied). H11 stands: no new surface below —
hardening, measurement, removal, enablement only. Numbering starts at PH-102:
PH-101 is taken (`PH-101-cortex-hardware-semantic.md`), and the PH-030…PH-036
series was delivered on branches `agent/PH-100` + `agent/PH-030-pre`
(`759817f`), not on this tree — a merger renumbers or keeps both, never
silently overwrites.

## PH-102-generic-extract-guard

---
id: PH-102
track: physis
phase: 1
status: shipped
rung: 2
depends: [PH-000, PH-015]
gate: "cargo test --lib ingest:: && cargo clippy --lib --tests -- -D warnings"
commit: "branch agent/PH-102-generic-extract-guard; commit aaab6c4 — guard + tests"
shipped: aaab6c4
---

# Generic misclassification guard for `extract_doc`

## Why

PH-015 fixed one arm: a text file classified `VoiceTranscript` without the
`audio` feature now falls back to its bytes (`src/ingest.rs:262-277`, test
`misclassified_text_file_keeps_its_body_ph015`). The report's §1 names the
rest of the class, still open: `Invoice`/`Email`/`OcrDocument`/`Spreadsheet`
with their features off (`src/ingest.rs:241,248,255,284`), `Image` without
`ocr` (`:295`), and `Video` (`:302`) all return empty unconditionally — while
the header comment (`:225-229`) claims a real UTF-8 text file is never an
empty body "whatever the kind claims". The comment over-claims; the guard is
partial.

## Steps

1. Write one failing test per arm: a UTF-8 text file forced into each kind
   above must yield a non-empty body. Run to confirm all fail.
2. Extend the PH-015 fallback (extension check + `read_to_string`) to every
   arm that can return empty for text, keeping genuine binaries empty.
3. Update the header comment to describe the NEW coverage — never delete the
   PH-015 history.
4. Full gate green.

## Tests you must add

- `misclassified_text_body_survives_{invoice,email,ocr,spreadsheet,image,video}_ph102`
  (each fails before, passes after).

## Gate

```bash
cargo test --lib ingest::
cargo clippy --lib --tests -- -D warnings
```

## Never

- Change `classify_file` itself — extraction fallback only.

## PH-103-env-off-hot-paths

---
id: PH-103
track: physis
phase: 1
status: shipped
rung: 2
depends: [PH-000, PH-020]
gate: "cargo test --lib && cargo clippy --all-targets -- -D warnings"
commit: "PH-103: cache PHYSIS_DATA_DIR + PHYSIS_MODEL env vars behind mvp::env::Lazy (repl scope + deps::generator_state hot paths)"
shipped: on agent/PH-102-generic-extract-guard
---

# Environment reads off per-call paths

## Why

PH-020's §1: a direction-affecting `env::var` was found on a hot path by
accident, and `grep -rn 'std::env::var' src/` answers whether siblings exist.
They do: `src/ai/provider.rs` (`:482,496,504,559,561,887,892,1295-1298`),
`src/mvp/repl.rs`, `src/mvp/deps.rs`, `src/settings.rs`. Any read inside a
per-call function is the same defect class — behaviour that changes with
process environment mid-run, untestable by construction.

## Steps

1. Audit every hit: startup/config-load (keep, justify in report) vs
   per-call (move).
2. Snapshot per-call reads into config at startup; call sites read the
   snapshot. No default values change.
3. Add a test with the deterministic embedder: mutate the env var mid-process,
   assert the handler output is byte-identical.
4. Full gate green.

## Tests you must add

- `env_mutation_does_not_move_handler_output_ph103` (fails before, passes
  after).

## Gate

```bash
cargo test --lib
cargo clippy --all-targets -- -D warnings
```

Report must list every residual `env::var` with its keep justification.

## Never

- Change any default value — snapshot the read, not a new policy.

## PH-104-secom-wire-or-decide

---
id: PH-104
track: physis
phase: 1
status: shipped
rung: 2
depends: [PH-000]
gate: "cargo test --lib && cargo clippy --all-targets -- -D warnings"
commit: "PH-104: SECOM marked research-only; de-cited PLAN.md/ARCHITECTURAL_REPORT.md; verdict in docs/research/PH-104-secom-verdict.md"
shipped: on agent/PH-102-generic-extract-guard
---

# H5: wire the SECOM benchmark to full data or stop citing it

## Why

PLAN.md H5, uncovered by any packet: the embedded sample is 21 observations /
10 features (real UCI SECOM is 1567 × 590); the default run splits to a
10-row test set, scoring 60% (random embedder) / 70% (ONNX) against a 61.9%
majority baseline — a number that carries no information, while the module is
still cited (`docs/ARCHITECTURAL_REPORT.md:156`, the industrial module list).
`src/industrial_benchmark/secom.rs` already has the seams: "embedded sample +
optional download" (`:4`), `parse_secom_csv` with `max_rows` (`:311`), a
`max_observations` config (`:455`).

## Steps

1. Attempt the full-data run first (download or local UCI CSV through the
   existing seams). Record accuracy, precision/recall on the rare FAIL class,
   and the majority-class baseline.
2. If full data runs: commit the numbers with the command that reproduces
   them, and update every citation to quote them.
3. If full data cannot run: remove SECOM from customer-facing claims, mark
   the module research-only in place, and say so in the report.
4. Gate green either way.

## Tests you must add

- A test pinning whichever outcome landed: full-data floor (numbers from step
  1, no tighter than measured) or the absence of customer-facing citations
  (`! grep -rin secom` over the customer pages named in the report).

## Gate

```bash
cargo test --features industrial_benchmark
```

Plus the committed numbers or the de-citation grep above.

## Never

- Invent data or quote a number not measured on the full set.

## PH-105-human-rerank-run

---
id: PH-105
track: physis
phase: 2
status: blocked
rung: 3
depends: [PH-000]
gate: "test -f research/mapper/rerank_scores.json && python3 scripts/score_rerank.py --help"
commit: "branch agent/PH-105; scores + labels commit (docs/data only)"
blocked_on: "models/wordnet + MiniLM/BGE embedders absent on pansa; human must label 75-item worklist"
---

# Run the built human-reranker worklist

## Why

Research-track open item 5 (`research/RESEARCH_STATUS.md`): the machine ranks
a worklist but cannot certify it (0.410 AUC on real misfilings, Stage 9), so
the last filter is a person.
`physis-core/examples/human_rerank_export.rs` emits a 75-item worklist
(machine head-50 + 25-item uniform tail, injected misfilings blind as gold,
machine scores hidden, labels held separately) and `scripts/score_rerank.py`
joins them back. Built, never run — the native filings this track has never
had labelled are the deliverable.

## Steps

1. Export the worklist; record the exact command and seed.
2. A human labels all 75 items blind (scores hidden, held-file discipline —
   the machine never sees labels pre-score).
3. Run the join; commit `research/mapper/rerank_scores.json`: exact recall on
   the injected, human-vs-machine AUC on the same items, precision lift from
   human filtering, anchoring check on revealed items, repair check on named
   destinations, and the flagged native filings.
4. Report which natives were flagged; propose their re-filings or a packet
   that does.

## Tests you must add

- None (measurement). The gate is the scores file existing with all six
  measures present (a schema assertion script counts; evaluator eyeballs).

## Gate

```bash
test -f research/mapper/rerank_scores.json
python3 scripts/score_rerank.py --help
```

## Never

- Let the machine see labels before scoring; break the held-file discipline
  and the AUC is void.
- Build new surface — data and scores only.

## What a human must decide (rung 3)

Who labels the 75 (domain competence required), the dispute rule on
disagreements, sign-off on the label schema before step 2, and whether the
flagged natives get re-filed (that is a follow-up packet, not this one).

---

## Round 1 (kiro, 2026-09-21, branch agent/PH-100)

Seven proposals PH-030…PH-036, verified 2026-09-22 during merge: cited symbols all resolve (PHYSIS_OLLAMA_EMBED_OFF in src/web/mod.rs + src/embed/mod.rs, undocumented in README/MANUAL_CONFIG; NavIndex::clear_embeddings at src/nav/mod.rs:429; sensors.rs/fact.rs on local-main-import; self.hits writes = 2 in src/bin/hud.rs). No id collision with existing packets. Reviewer note: PH-034 wires a provider backend — confirm it stays inside the guided programme before accepting.

## PH-030 — ResultState: collapse hits/rows/sel into one coherent struct

**Source**: PH-029 report "next measured bottleneck"; PH-027 report stale-hits hazard.

```
id: PH-030
track: physis-pro
phase: 0
status: shipped
rung: 2
depends: [PH-029]
gate: "cargo clippy --all-targets -- -D warnings && cargo test --features tui --bin physis-hud"
shipped_on: agent/PH-030-result-state (commit d30d764)
measurement: grep -c 'self\.hits\s*=' src/bin/hud.rs = 0 (was 2; the stale-hits hazard is structurally blocked; remaining self.sel= lives inside ResultState::set_sel)
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

---

## PH-037-substrate — NeuralSubstrate abstraction + reference backends

```
id: PH-037
track: physis
phase: 2
status: shipped
rung: 2
depends: [PH-029]
gate: "cargo clippy --lib --features neural -- -D warnings && cargo test --test neural_invariance --features neural"
commit: "branch agent/P-037-substrate (physis-core df09a3c + pro 930ea1b)"
shipped_on: agent/P-037-substrate
```

# Substrate-independent semantic layer (wetware interface, not wetware)

Additive `neural` module to physis-core. `NeuralSubstrate` trait + normalized
types (`StimulusPattern`, `NeuralObservation`, `PopulationState`,
`Confidence`, `SubstrateHealth`). Backends: `DeterministicBackend`
(always-on, no feature), `RandomReservoirBackend` (Echo State),
`ArtificialSpikingBackend` (toy LIF), `RecordedNeuralBackend` (CSV/JSON),
`FlyConnectomeBackend` (edge-list CSV, no data bundled),
`CorticalSimulatorBackend` (mock), `FutureWetwareBackend` (stub error).

Reuses: `CoherenceNode.embedding` → `PopulationState.values`;
`delta_engine::{OntologyMutation, MutationOp, evaluate_mutation}` for
semantic-delta decoding; `CellClassifier` + `Hypothesis` for repair/uncertainty;
`embed::{VectorEmbed, RandomProjectionEmbedder}` encoder. NO existing module
deleted. Feature gate: `neural` (default off), so core builds without biological
deps (leptonica/lept.pc absent does not block — PH-104 confirmed).

Priority order per PH-101 spec: P0 inspect→P1 trait→P2 deterministic→P3
reservoir→P4 invariance harness→P5 delta-vs-full→P6 weak-decoder+repair.
P7-P13 scaffolded (spiking tuning, recorded sample data, FlyWire CSV, Cortical
adapter, HUD viz, demo, report).

Experiments (pansa CPU, 4-dim axis-flip task):

| backend | recon | delta | invariant | weak_acc | repair |
|---|---|---|---|---|---|
| DETERMINISTIC | 1.00 | yes | yes | 1.00 | 1.00 |
| RESERVOIR | 1.00 | yes | yes | 0.00 | 1.00 |
| SPIKING | 0.00 | no | no | — | — |

Invariant (delta signal fires) survives deterministic + reservoir. Spiking toy
LIF threshold too high — dynamics tuning gap, not architecture failure.

Gate verified: `cargo clippy --lib --features neural -- -D warnings` clean;
`cargo test --test neural_invariance --features neural` 2 passed; 282 lib tests
still pass. Default `cargo check` (no neural feature) builds the trait +
DeterministicBackend only.

## Never

- Claim biological fidelity for the spiking toy model (DECAY=0.1, THRESHOLD=1.0).
- Bundle FAFB/FlyWire data (multi-GB).
- Implement tissue-culture procedures.
- Delete or restructure existing Physis semantic modules to make room.
