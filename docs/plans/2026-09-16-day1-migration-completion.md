# Day 1 completion — finishing the Core→Pro migration (2026-09-16)

Continues `docs/plans/2026-09-15-sonnet-day1-handoff.md`, which declared the Day 1
gate passed while listing moved-but-unrouted surfaces. This session routed them,
fixed the feature gating that made the product library unbuildable without
default features, and proved each route by running the shipped binary.

The instruction governing this session: **continue the Core→Pro migration; move
features, do not delete them.** Nothing here deletes a feature.

## 1. What was still missing when this session started

`physis-core/MOVED_TO_PRODUCT.md` listed the moved files that no product code
path reached. Re-checked by grep, then by execution:

| moved module | state at start |
|---|---|
| `ported::world`, `ported::tui` | zero callers; each still contained Core's **private `fn main()`**, so both were flagged dead by `cargo check --lib` |
| `ported::ground` | zero references anywhere in `src/`, `tests/`, `examples/` |
| `ported::act_recall` | reachable only from `examples/{floor_spread,claim_identity}.rs` |
| `ported::nlq` | zero references (Core never wired it either) |
| `ported::config_run` | zero references |
| `ported::{vault,history,praxis}` | duplicates of live Pro-native importers |
| `ported::quality` | already unified behind `src/quality.rs` — the 2026-09-15 note was stale here |

A `--no-default-features` build of the product library failed on `axum`,
`crossterm` and `clap` imports coming from the moved modules: the split had left
optional-dependency surfaces ungated.

## 2. Changes

**Feature gating — `src/ported/mod.rs`**
`studio`, `studio_communities`, `studio_lab` behind `feature = "web"`; `tui`
behind `tui`; `system_cli` behind `cli`. Rationale in-file, per surface.
Result: `cargo check --no-default-features --lib` → exit 0, zero errors (was 29).

**Two moved binaries — `src/bin/world.rs`, `src/bin/system_tui.rs`, `Cargo.toml`**
Core shipped `physis-world` and `physis-system-tui`; their `main()`s arrived in
`ported/` intact and unreachable. `pub fn main()` on each plus one-line shims
(same pattern as `src/bin/physis.rs` → `front_door::main`) and `[[bin]]`
entries give `physis-pro-world` and `physis-pro-system-tui` (the latter requires
the `tui` feature). No licence gate was added: Core had none, and adding one
would be a new product decision rather than a relocation.

**Four moved verbs — `src/main.rs`**
`ground [--json]` (→ `ported::ground::read`, Core's verb including its
`project_all_revisions` + persist step and Core's private `PersistCore` trait,
same bytes to the same path), `act-recall [--top --seed --json]` (→
`ported::act_recall::run`, same `benchmarks/results/act-recall.json` artifact),
`nlq <question> [--json]` (→ `ported::nlq::parse`), and `run --config <json>`
(→ `ported::config_run::load_config` composed with Core's public pipeline:
`ngram_table`, `model_provider`, `map`, `tokenizer`). All four are dispatched
before `PhysisApp::new` — none needs the ontology — and all four bypass the
licence gate for the same reason `system`/`act` do: local state and local
mechanisms only.

**One moved string — `src/ported/nlq.rs`**
`Proposal::command()` emitted `physis-world …`; it now emits
`physis-pro-world …`, the binary this repository ships. Its unit test was
updated with it.

**`physis-core/MOVED_TO_PRODUCT.md`**
A dated "Routing completed (2026-09-16)" section with the route table, the
binaries-vs-verbs reasoning, and the deliberate decision to leave
`vault`/`history`/`praxis` as two implementations (different stores, no parity
test, and the plan forbids moving data paths in this gate).

## 3. Evidence

| check | command | result |
|---|---|---|
| product bins build | `cargo build --bin physis-pro --bin physis-pro-world --bin physis-pro-system-tui` | exit 0 |
| every bin builds | `cargo build --bins` | exit 0 |
| library builds without defaults | `cargo check --no-default-features --lib` | exit 0, 0 errors (was 29) |
| moved modules' own tests | `cargo test --lib ported::` | **103 passed, 0 failed** |
| the new routes | `cargo test --test ported_routes` | **10 passed, 0 failed** (41.58s) |
| existing product suites | `cargo test --test system_interface --test system_delegation --test system_mcp --test ported_quality_penalty` | **31 passed, 0 failed** (20 + 8 + 2 + 1) |
| cleared Core surface | `physis-core --help` | mechanism verbs only; no `studio`/`notebook`/`vault`/`history`/`praxis`/`quality`/`act`/`system`/`ground` |
| removed Core verb errors | `physis-core studio` | `error: unrecognized subcommand 'studio'`, exit **2** |

`tests/ported_routes.rs` starts each shipped binary with `PHYSIS_CORE_DIR`
pointed at a `TempDir`, so no test touches the operator's store, log or trees.
It asserts behaviour, not presence: an empty ledger renders *"No claims yet"*
rather than an error; `nlq` emits a pasteable `physis-pro-world` command;
`act-recall` names its embedder and writes its artifact; `world observe` leaves
the tree byte-identical and logs into the sandbox; `run` echoes the config's own
corpus and query; a missing config fails naming the file.

Manual runs, each on a sandboxed `PHYSIS_CORE_DIR`:

```bash
physis-pro ground            # ── COMMON GROUND ── 0 claim(s) … (empty ledger is a valid answer)
physis-pro ground --json     # {"claims":[],"disagreements":[],"unscored":[],"events":0,"nodes":0}
physis-pro nlq "where is the pump"
#   intent locate · term pump · command physis-pro-world locate pump · cues "where is"(+4), "where"(+1)
physis-pro act-recall --json # ledger 48 (16 contradicted) · embedder bge-base · lexical recall 1.000 vs null 0.104
physis-pro run --config /tmp/run-test/cfg.json
#   ── PHYSIS RUN (config-driven, offline, deterministic) ── demo-ngram · query the pump · compression 0% (10 → 10)
physis-pro-world observe --root /tmp/worldroot   # observed 201 new (seq 1..201) … append-only
physis-pro-world inspect                         # world: 201 observations … 264 mutual-kNN edges
physis-pro-system-tui --help                     # key map, exit 0, no TTY required
```

`physis-pro run` and `act-recall` are slow (the cascade-selected embedder loads
per process — `bge-base` here, and the two tests account for 41s of the suite).
That is the mechanism, not the wiring.

## 4. Remaining, explicitly not done

1. **`vault` / `history` / `praxis` are still two implementations each.** The
   moved copies serve the moved studio server (`ported::studio` calls
   `ported::history`, `ported::praxis`, `ported::vault`); the live CLI commands
   and their HTTP routes stay on Pro's pre-existing importers and their own
   stores. Unifying them needs a parity test and a decision about which store
   wins — outside this gate by the plan's own rule (do not move data paths).
2. **`physis-pro-system-tui` is unverified beyond `--help`.** It needs a real
   terminal; the interactive path is not exercised by any test here.
3. **No licence-gate decision was made for `physis-pro-world` /
   `physis-pro-system-tui`.** They ship ungated, matching Core. Putting them
   behind a SKU would be a deliberate product change, not a migration fix.
4. **`act-recall`'s artifact path is relative to the working directory**
   (`benchmarks/results/act-recall.json`), as it was in Core. Run from an
   installed location, it writes under that CWD.
5. **The stale standalone Core build tree remains** at
   `physis-core/target/debug/physis-core` (dated 2026-09-04). The workspace
   builds Core into the root `target/` instead, and this leftover still answers
   `studio` because it predates the removal — it misled one probe during this
   session. Left in place rather than deleted; never cite it as evidence.

## 5. Gate status

Day 1's acceptance is met as written: CLI and MCP drive the same moved workspace
service (31 focused tests), every moved verb and binary has a real dispatch
path, Core builds independently and exposes no restored application surface, and
a removed Core command fails explicitly with a nonzero status.

Next per the plan: **Day 2 — extract the remaining collection adapters from
Core's `observe.rs` into Pro while preserving the log format and reader API,
then verify Core's standalone build and restart/migration parity.**

**Actor**: continuation session (Cline)
**Date**: 2026-09-16

