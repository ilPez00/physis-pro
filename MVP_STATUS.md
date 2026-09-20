# MVP_STATUS — what ships, what it measures, what it does not

Final implementation pass, 2026-09-17. **Freedom now, fineries later.**
This file is the measured record: capabilities, limitations, exact commands.
If a number is here, it was produced by running the thing; nothing is projected.

---

## 0. Added 2026-09-20 — the HUD front door and guided execution

Both on branches, awaiting review (`PH-024`, `PH-025`); gates in
`packets/reports/`. Commands below were run on this checkout.

```bash
cargo clippy --all-targets -- -D warnings          # exit 0
cargo test --features tui --bin physis-hud         # 36 passed; 0 failed
cargo test --lib guided::                          # 8 passed; 0 failed

physis-hud . --setup                               # exit 0
# PHYSIS SETUP: WORKSPACE (root, index counts, semantic on/off) · FILES
# (data dir, config.json, settings.json, observations) · AGENTS (roster +
# reachability) · CHECKLIST · NEXT — each gap names the command that closes it.

physis-hud . --snapshot=100x30 --view settings      # exit 0, 31 lines
# the settings rows, the selection marker, the per-row hint, the `*` dirty
# column, and the two file paths that `s` writes

physis-pro guided "read the retrieval notes" --focus src/retrieval.rs
# VALID TRANSITIONS read → 6 target(s) · CONSTRAINT (<Verb> := "read")
# (<Target> := "fs:0e4bfb603e53" | …) (<limit> := <text:12>) · SCHEMA (json enum
# of real handles) · ENTROPY fixed 6 · choice 2 fields (7 candidates, 2.58 bits)
# · free 1 · CHOSEN read fs:0e4bfb603e53 (route: model)

physis-pro guided "main"                            # one object in the slice
# CHOSEN read intent:main (route: exact) · model calls 0 · bypasses 1 (100%)
```

Limits, stated rather than implied: guided execution's only backend is the
deterministic mock, so **no token-saving claim is made** (`Constraint::token_classes`
reports the constraint side only); provider adapters, CFG composition and
consolidation are not built (`docs/GUIDED_EXECUTION.md` lists what unblocks
each); the bypass rate depends on index granularity, because a file and its
passage share a path — to be measured, not assumed. `physis-hud` needs the
`tui` feature; `physis-pro guided` sits behind the same licence gate as every
other `physis-pro` subcommand (`PHYSIS_ALLOW_DEV_LICENCE=1` for local dev).

---

## 1. One executable entry

```bash
physis-pro <directory>          # the MVP loop (this is the whole entry point)
```

* A bare directory path with no subcommand opens the loop. Every existing
  subcommand is unchanged (`physis-pro --help` still lists them all).
* `physis <directory>` through the multi-edition front door forwards here when
  `physis-pro` is on `PATH` (the front door forwards unparsed arguments).
* No licence gate on this path: it reuses only the licence-free slices (`nav`
  index, `scanner`, `pack`, `act`, `physis_core::map`) exactly like
  `physis-pro system` and `physis-pro act`. Every other subcommand keeps its
  existing licence behaviour.

Startup reports factual state only, all of it measured:

```text
PHYSIS — physis-core

files          339
objects        2992
relations      0
sample map     repeats 9 · differences 8 · contradictions 1
index          ready
```

`sample map` is named that on purpose: it is computed over a capped sample
(120 documents / 512 KB, smallest files) and cached by content hash. The counts
are real over the sample; the label says so.

---

## 2. Repository is the world

| format | structural shapes kept |
|---|---|
| Rust | `fn` / `struct` / `enum` / `trait` / `impl` / `mod` (visibility and qualifiers stripped: `pub fn`, `pub(crate) struct`, `async fn`) |
| Markdown | `h1:` / `h2:` / `h3:` headings |
| JSON | `key:` per object key |
| TOML / YAML / INI | `table:[…]` and `config:key` |
| generic text | `document:chunk` (degrades, never fails) |

Objects come from the shared `nav` index (`ObjectId` with content hash), so
nothing is duplicated: artifacts are a *view* over it
(`src/mvp/model.rs`, `Artifact::from(&PhysisObject)`), and the existing
shape-agnostic parsers in `src/scanner.rs` are the only parsers involved.

---

## 3. Universal command box

One input line accepts commands *and* plain intent; unrecognised text is scoped
and answered. Intent classification is a keyword rule set (`src/mvp/deps.rs`,
`classify_intent`) — no architecture, as instructed.

---

## 4. Minimal navigable map

Text tree of directory → file → object, each row with its score and the exact
`why:` breakdown from the shared scorer:

```text
 └─ src
     └─ rag.rs
         └─ src/rag.rs:2624-2701  ↔ 0.55  why: lexical 0.12 × 0.30 · recency 0.97 × 0.08 · semantic 0.79 × 0.60
```

Operations: `zoom/open`, `back`, `search`, `focus`, `context`, `related`,
`dependencies`, `differences`, `contradictions`, `history`, `inspect`,
`edit`, `rescan`, `bench` (HUD: `physis-hud` adds `e` inline edit with
rescan, `:external` for `$EDITOR`, `u` rescan, `H` history, `B` bench,
`D` dependents, `C` contradictions, `●/◐/·` markers plus positional score
bars, key legend, `?` help, `1/2/3/4` why/context/delta/history tabs, and a
universal command box — plain text searches, `/text` global, `$ cmd` shell
in the workspace root, `:name` Physis commands (`:trail`, `:pins`,
`:unpin`, `:sem off|on`), `open`/`edit`/`cd` — plus
`scripts/mvp-hud-propagation-demo.sh`, the 90-second one-edit propagation
demo over `fixtures/mvp_propagation`).

`PHYSIS_MODEL=none` (default) = coherent with no generator: the notebook
degrades to extractive, `context` prints `generator: none`, and the packet
stays complete. `local` = optional offline worker via `PHYSIS_ORACLE_*`
pointed at localhost.

---

## 5. Physis-mediated context (the central property)

Every scoped interaction prints measured telemetry:

```text
repository objects considered 2992
objects selected 8
candidate/raw context tokens 1204946
final context tokens 833
model/provider used random-projection (extractive; oracle when configured)
agent calls 0
latency 1477 ms
```

`candidate_tokens` is measured inside the packer
(`PackResult::candidate_tokens`, added in this pass) — the sum of every
candidate window's real token count, by the same counter that enforces the
budget. The denominator of the compression claim is therefore not an invention.

Provenance survives retrieval: every slice chunk is printed as
`path:start-end`, and `inspect` shows the same span plus the object's origin.

---

## 6. Action loop

Reuses `ported::act` (intent recorded before the command, outcome after) and the
existing agent harness. One action = one transition:

```text
intent → scoped slice → PLAN → (--yes) → validation in the repository → Δ → remap
```

The validation command is `PHYSIS_MVP_VALIDATE` (default
`cargo test --quiet 2>&1 | tail -5`) and is executed with the repository as the
working directory. Whatever runs is printed in the Δ — a transcript can never
claim a validation that did not run.

---

## 7. Semantic Δ — the closed loop

```text
Δ

CHANGED
  src/mvp/model.rs

AFFECTED
  <files mentioning the changed file's stem or its symbols>

STRUCTURE
  - relation: <removed paths>

VALIDATION
  act: cd '<repo>' && <command> exit Some(0) in 412 ms
  file-type invariants preserved (rs/md/json/toml shapes)

MAP
  4 objects remapped (2992 → 2996, incremental)
  1 semantic neighborhood(s) changed
```

* incremental re-index is the default; a wider recomputation is used only when
  it is the correct answer, and then `full rescan` is printed;
* **actions that change nothing and actions that fail are recorded too** — a
  missing delta is an observation, not an omission.

---

## 8. Transition log (mandatory, shipped)

`<repo>/.physis/mvp_transitions.jsonl`, one JSON object per line, append-only,
never rewritten:

```json
{"v":1,"seq":3,"timestamp":"2026-09-17T17:04:41.838Z",
 "state_before":{"content_hash":"…","objects":2992,"files":339,"map_hash":"…"},
 "intent":"improve retrieval without changing its public API",
 "action":"act:cd '/repo' && cargo check --quiet 2>&1 | tail -3",
 "delta":{"changed":["src/mvp/model.rs"],"added":[],"removed":[],
          "objects_remapped":4,"neighborhoods_changed":1,
          "relations_added":[],"relations_removed":[],"no_change":false},
 "validation":["exit Some(0)"],"outcome":"exit Some(0)","state_after":{…},
 "provenance":["src/rag.rs","src/pack.rs"],
 "uncertainty":0.1,
 "slice":{"objects_considered":2992,"objects_selected":8,
          "candidate_tokens":1204946,"final_tokens":833,
          "provider":"random-projection","paths":["src/rag.rs"]}}
```

* **content-addressed snapshots + compact deltas**, never full-state copies:
  `content_hash` / `map_hash` are sha256 over the file-hash snapshot and the
  index id set; `delta` carries path lists and counts only;
* every record carries `(timestamp, S_t, intent/action A_t, Δ,
  validation/outcome, S*_{t+1}, provenance, uncertainty)`, plus the world slice
  that was in force — the fields a future Time Machine replay and a future
  world-model regressor need;
* version stamped at write (`v`), sequence dense and resumed across reopen;
* one log per repository, in `.physis/` (state, excluded from the scanned corpus
  so the observer never becomes the observed).

Neither Time Machine nor world-model training is built (deferred — see
`POST_MVP.md` §0); only the clean, versioned, replayable records are produced.

---

## 9. MVP acceptance gate — demonstrated, not just implemented

| gate | status | evidence |
|---|---|---|
| real repository scans | ✅ | `physis-core`: 339 files, 2 992 objects |
| structural hierarchy exists | ✅ | `file_type_invariants_are_preserved` + tree output |
| semantic retrieval works | ✅ | hybrid scorer, per-component `why:` on every hit |
| provenance survives retrieval | ✅ | `path:start-end` on every chunk and hit |
| map can be navigated | ✅ | `inspect` / `dependencies` / tree output |
| natural-language query scopes correctly | ✅ | `natural_language_query_scopes_and_reports_telemetry` |
| agent receives Physis-selected context | ✅ | telemetry: considered / selected / tokens / provider |
| agent can perform one real bounded edit | ✅ | dogfood run: append to `src/mvp/model.rs` + `cargo check` + Δ |
| validation executes | ✅ | exit code in the Δ and in the transition record |
| filesystem change is detected | ✅ | hash-snapshot diff → `CHANGED` / `AFFECTED` |
| semantic/structural Δ is shown | ✅ | `SemanticDelta::render` — all five sections |
| affected state is remapped | ✅ | incremental `nav` re-index, `MAP` counts |
| complete loop works twice from a clean checkout | ✅ | `the_loop_runs_twice_from_a_clean_state` |
| demo transcript is reproducible | ✅ | `scripts/mvp-demo.sh` (fixed input, teed output) |

---

## 10. Limitations (measured, and deliberately not fixed)

| limitation | measured / exact shape |
|---|---|
| First index of a real repository is slow | 78.7 s for `physis-core` (339 files) with `random-projection`; it persists, later starts 1.7 s. Startup says "first index …" instead of looking hung |
| First query is slow | 70.7 s — it fills the shared embedding cache; later queries 1.5 s |
| `random-projection` is not semantic | it matches strings, not meaning (measured elsewhere in this repo: paraphrase recall 0.000). Every telemetry block prints the provider, so the substitution is never silent; leave `PHYSIS_EMBEDDER` unset to use the ONNX/ollama cascade |
| Map counts are a **sample** | capped at 120 documents / 512 KB; the banner field is named `sample map` |
| Dependent discovery is lexical | a file that calls/imports the changed symbols or names the changed module; reported as `AFFECTED`, never as a proven call graph. Measured on `physis-core`: 261 dependents with the loose rule, **6** with the shipped one |
| `objects_remapped` is index churn | changed + added + \|object-count delta\|; not a claim about which semantics moved |
| Validation is one command | chosen by the operator (`PHYSIS_MVP_VALIDATE`), not by a planner |
| `history` without git | falls back to mtime order over the snapshot |
| Interface is text | tree + panes in the REPL; `physis-hud` TUI adds list + reading + why/context/delta panes, graph view, history and bench projections |
| One writer per repository | the nav index is sled-backed, so two loops on the same repository conflict (`could not acquire lock … index.sled/db`). Observed on `feather` when a killed run left a session mid-flush; retrying succeeds. Run one loop per repo |

Nonessential subsystems remain untouched and unbroken: the web console, voice,
industrial adapters, inventory, Spanner, the Studio pages. None is on the
demonstrated path.

---

## 11. What changed in this pass (small, and only where the loop needed it)

| file | change | why |
|---|---|---|
| `src/mvp/` (new) | `model.rs`, `slice.rs`, `deps.rs`, `log.rs`, `repl.rs`, `mod.rs` | the loop: artifacts, world slice, dependents, transition log, session |
| `src/lib.rs` | `pub mod mvp;` | wiring |
| `src/main.rs` | `mvp_dir` positional + `Option<Commands>`; bare-dir entry before the licence gate; existing dispatch untouched | §1 entry point |
| `src/scanner.rs` | `.physis` added to `DEFAULT_EXCLUDE_DIRS` | the observer is not the observed; without it every diff saw the log entry the last command wrote |
| `src/pack.rs` | `.physis` added to `SKIP_DIRS`; `PackResult::candidate_tokens` (measured, serde-defaulted) | same, plus a real denominator for the compression claim |
| `tests/mvp_loop.rs` (new) | 7 integration tests | protect the demo path |
| `scripts/mvp-demo.sh` (new) | scripted demo + transcript | reproducibility |
| `docs/MVP_DEMO.md`, `MVP_STATUS.md`, `POST_MVP.md` (new) | demo, status, parking lot | deliverables |

No new dependency, no new store, no new parser, no new agent framework.

---

## 12. Exact commands

```bash
# build (default features include the local embedders)
cargo build --bin physis-pro --features cli

# the loop (semantic cascade: local ollama nomic-embed-text, self-test gated)
env -u PHYSIS_EMBEDDER ./target/debug/physis-pro <directory>

# the scripted demo + transcript (prerun once to warm, then demo warm)
env -u PHYSIS_EMBEDDER scripts/mvp-demo.sh <directory>

# tests protecting the demo path
cargo test --features cli --test mvp_loop
cargo test --lib --features cli mvp::
cargo test --features tui --bin physis-hud

# regression on the slices the loop reuses
cargo test --features cli --test smoke_test
cargo test --features cli --test system_interface

# release build
cargo build --release --features cli --bin physis-pro
```

