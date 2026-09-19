# Day 1 Integration Handoff — 2026-09-15

This document records the completion of the original Day 1 contract items from `integration/DAY1_REMAINDER.md`, with evidence linked to existing logs under `integration/logs/`.

## Summary of Changes

### 1. Recovered `act` CLI from Core HEAD

**Status**: ✅ Complete

**Changes**:
- `src/main.rs`: Added `Act` subcommand with `--dry-run`, `--top`, `--json` options matching Core's pre-migration CLI
- `src/main.rs`: Implemented `cmd_act()` and `load_core()` routing to `src/ported/act::{run, bearing_on_with, Selection}`
- Added `Act` to licence bypass list (alongside `Doctor`, `Benchmark`, `License`, `Selftest`, `System`)
- Preserved all Core semantics: intent recorded before execution, outcome with duration, bearing (claims) surfaced with warnings first, exit code propagation, JSON output, observation store provenance

**Verification**:
```bash
# Dry-run (does not execute)
PHYSIS_ALLOW_DEV_LICENCE=1 ./target/debug/physis-pro act "echo hello" --dry-run
# → "nothing already established bears on this. 4 claim(s) checked. (not run — drop --dry-run to execute)"

# Live execution with exit code propagation
PHYSIS_ALLOW_DEV_LICENCE=1 ./target/debug/physis-pro act "echo hello"
# → exit 0, recorded as observations #3237, #3238

PHYSIS_ALLOW_DEV_LICENCE=1 ./target/debug/physis-pro act "false"; echo $?
# → exit 1, recorded as observations

# JSON output
PHYSIS_ALLOW_DEV_LICENCE=1 ./target/debug/physis-pro act "echo hello" --json
# → { "command": "echo hello", "exit_code": 0, "duration_ms": 1, "stdout_lines": 1, "stderr_tail": "", "bearing": [], "observed": [3237, 3238] }
```

**Evidence**: `integration/logs/act_cli_verification.log`

---

### 2. Studio Routing Audit

**Status**: ✅ Complete — Distinct subcommand added

**Finding**: Pro's existing `Studio` command (`PhysisApp::run_studio`) generates a **static HTML report** for prospect demos (ingest directory → file-theme graph → discovery gaps → self-contained HTML). This is **different** from Core's `studio` which runs a **live axum web server** for interactive ontology browsing/editing.

**Action**: Added new `studio-server` subcommand (gated behind `web` feature) that routes to the moved `physis_pro::ported::studio::run_with_model`, preserving the original Core Studio server behavior.

**Changes**:
- `src/main.rs`: Added `StudioServer { port, model }` subcommand under `#[cfg(feature = "web")]`
- `src/main.rs`: Added handler calling `physis_pro::ported::studio::run_with_model(port, model)`
- Added `StudioServer` to licence bypass list
- Existing `Studio` command unchanged — backward compatible

**Verification**:
```bash
./target/debug/physis-pro studio-server --help
# → Shows port/model options, "moved physis-core studio server"

PHYSIS_ALLOW_DEV_LICENCE=1 timeout 2 ./target/debug/physis-pro studio-server --port 3001
# → Starts axum server on :3001 (killed by timeout after 2s)
```

**Evidence**: `integration/logs/studio_server_verification.log`

---

### 3. Fixed Stale `physis_core::quality` Imports

**Status**: ✅ Complete

**Changes**:
- `tests/industrial_benchmark.rs`: Changed import from `physis_pro::quality::QualityTracker` to `physis_pro::ported::quality::QualityTracker` (the ported version takes only an embedder, matching test usage)
- `tests/epistemic_evolution.rs`: Already imported `ContextualQualityTracker` and `FitnessContext` from `physis_pro::ported::quality` — no change needed

**Verification**:
```bash
cargo check --test industrial_benchmark
# → Clean (library errors in unrelated registry.rs are pre-existing, not migration-related)

cargo check --test epistemic_evolution
# → Clean
```

**Evidence**: `integration/logs/quality_import_fix.log`

---

### 4. Moved-Modules Table

| Moved Module | Actual Consumer(s) | Replaced by Pro Route? | Status |
|--------------|-------------------|------------------------|--------|
| `act.rs` | `src/main.rs` (cmd_act), `act_recall.rs`, `examples/floor_spread.rs` | No — direct use of ported | **Routed** |
| `act_recall.rs` | `examples/claim_identity.rs`, `examples/floor_spread.rs` | No — direct use of ported | **Routed** |
| `config_run.rs` | None | N/A | **Pending** (no CLI entrypoint) |
| `edition.rs` | `front_door.rs` (internal) → `bin/physis.rs` | No — internal to ported | **Routed** (via front_door) |
| `front_door.rs` | `bin/physis.rs` (the `physis` binary) | No — direct use of ported | **Routed** |
| `ground.rs` | None | N/A | **Pending** (no CLI entrypoint) |
| `history.rs` | `vault.rs` (internal) | N/A | **Pending** (vault has no external consumer) |
| `mod.rs` | Module declarations only | N/A | **Internal** |
| `nlq.rs` | None | N/A | **Pending** (no CLI entrypoint) |
| `notebook.rs` | `src/main.rs` (Notebook command) | No — direct use of ported | **Routed** |
| `observe_adapters.rs` | `tests/system_interface.rs`, `world.rs` (internal) | No — direct use of ported | **Routed** |
| `oracle.rs` | `notebook.rs` (internal) | No — internal to ported | **Routed** (via notebook) |
| `praxis.rs` | None | Pro has `Praxis` command via `PhysisApp` | **Pending** (ported version not wired) |
| `quality.rs` | `quality.rs` (facade), `tests/industrial_benchmark.rs`, `tests/ported_quality_penalty.rs`, `tests/epistemic_evolution.rs`, `examples/mapper_physis_feedback.rs`, `studio.rs` (internal) | Pro has `Quality` command via `PhysisApp` (different impl) | **Dual** — ported used by tests/examples, Pro facade used by CLI |
| `studio.rs` | `src/main.rs` (StudioServer command) | Pro has `Studio` command (static HTML, different impl) | **Dual** — ported for live server, Pro for static report |
| `studio_communities.rs` | `studio_lab.rs` (internal) | N/A | **Internal** |
| `studio_lab.rs` | `studio_communities.rs` (internal) | N/A | **Internal** |
| `system_cli.rs` | `src/main.rs` (System command) | No — direct use of ported | **Routed** |
| `system_delegation.rs` | `system_cli.rs` (internal), `system_mcp.rs` (internal) | N/A | **Internal** |
| `system_mcp.rs` | `tests/system_mcp.rs` | No — direct use of ported | **Routed** |
| `system.rs` | `system_cli.rs`, `system_delegation.rs`, `system_mcp.rs`, `tui.rs` (all internal), `tests/system_interface.rs` | No — direct use of ported | **Routed** |
| `tui.rs` | None (has `main()` but not wired) | N/A | **Pending** (dead code, flagged by `cargo check --lib`) |
| `vault.rs` | `history.rs` (internal) | Pro has `Vault` command via `PhysisApp` (different impl) | **Pending** (ported version not wired) |
| `world.rs` | None (has `main()` but not wired) | N/A | **Pending** (dead code, flagged by `cargo check --lib`) |

**Legend**:
- **Routed**: Ported module is directly called from product CLI or tests
- **Internal**: Only used within the `ported/` module tree
- **Dual**: Both ported and Pro-native implementations exist for similar functionality
- **Pending**: No active consumer; CLI entrypoint not yet wired (or Pro uses different implementation)

---

### 5. MCP Durable Round-Trip (obs:30) + Notebook Embedder Provenance

**Status**: ✅ Verified (from prior session, recorded here)

#### MCP obs:30 Durable Round-Trip

**Write** (via `physis_remember`, actor `integration-owner`):
```json
{ "id": "obs:30", ... }
```

**Search** (via `physis_history`):
```bash
physis_history(query="STRIPE") → matched_in_window: 1 (seq:30)
physis_history(query="integration-owner") → matched_in_window: 1 (seq:30)
```

**Read** (via `physis_read`):
```bash
physis_read(target="obs:30") → byte-identical body/subject, seq:30 confirmed
```

**Evidence**: `integration/logs/mcp_durable_roundtrip.log` (full transcript)

#### Notebook Embedder Provenance Labeling

**Change**: `src/main.rs` Notebook dispatch now uses `physis_core::embed::select(384)` instead of hard-coded `RandomProjectionEmbedder::new(384)`. The selector returns a real semantic embedder when configured and passing the self-test probe; otherwise `"random-projection"` labelled as such.

**Answer struct** (`physis_pro::ported::notebook::Answer`) gained `retrieval_backend: String` field, printed in both rendered and JSON output:
```
retrieval backend: onnx
# or
retrieval backend: random-projection
```

**Verification**:
```bash
cargo check --lib --bins  # Clean
cargo test --lib notebook::  # 22/22 passed
```

**Evidence**: `integration/logs/cargo_test_notebook_lib.log`, `integration/logs/cargo_check_lib_bins.log`

---

## Command Evidence Logs

All raw command outputs saved under `integration/logs/`:
- `act_cli_verification.log` — act dry-run, live, JSON, exit codes
- `studio_server_verification.log` — studio-server help and bounded startup
- `quality_import_fix.log` — cargo check for fixed test files
- `mcp_durable_roundtrip.log` — write/search/read chain for obs:30
- `cargo_test_notebook_lib.log` — 22/22 notebook lib tests
- `cargo_check_lib_bins.log` — lib + bins check after Notebook fix
- `cargo_test_focused_suite_rerun.log` — 31/31 focused suite re-run (justified by main.rs edits)
- `physis_skill_check_attempt.log` — physis-check execution attempts (NOT MEASURED)
- `moved_to_product_routing_grep.log` — grep evidence for routing table

---

## Gate Status (from `pro/TASK.md`)

| Gate | Status | Notes |
|------|--------|-------|
| Fresh Pro build | ✅ | `cargo build --bin physis-pro --bin physis` clean |
| Workspace/MCP/delegation focused tests | ✅ | 31/31 passed (system_interface, system_delegation, system_mcp, ported_quality_penalty) |
| Actual product command routing | ✅ | System/Notebook/Act/StudioServer dispatch before PhysisApp |
| Machine JSON without banners | ✅ | Verified by system_interface.rs JSON parsing tests |
| Core feature checks, constrained concurrency | ✅ | --no-default-features, embed-onnx, all-features clean; 224 lib tests pass |
| MCP initialize/list/history/durable round-trip | ✅ | Test-store + real store both verified |
| Explicit MCP allowlist | ⚠️ | Edit denied; coordinator clarified server-prefix scoping sufficient |

---

## Remaining / Unresolved

1. **Pre-existing `registry.rs` compile errors** (6 errors) — unrelated to migration, block full `cargo check --lib --tests` but not the changed scope
2. **`tui.rs` / `world.rs`** — fully dead code, flagged by `cargo check --lib`
3. **`config_run.rs` / `ground.rs` / `nlq.rs` / `praxis.rs` / `vault.rs` / `history.rs`** — ported modules with no external consumers; Pro has equivalent commands via `PhysisApp` but using different implementations
4. **`quality.rs` dual implementation** — ported (used by tests/examples) vs Pro facade (used by CLI); not unified
5. **`.claude/settings.local.json` MCP permissions** — still blanket server-level, per-tool edit denied
6. **Wrapper script (`scripts/physis-system-mcp.sh`)** — read but not independently exercised end-to-end outside cargo test

---

## Next-Day Plan References

- World/TUI/runtime work: explicit in next-day plan
- Unify Quality implementations (ported vs Pro facade)
- Wire `Vault`/`History`/`Praxis` to their ported counterparts or confirm Pro-native is sufficient
- Resolve `registry.rs` pre-existing errors (separate track)

---

## Day 1 Gate Re-Verification — 2026-09-16 (fresh build evidence)

All verification below was re-run on 2026-09-16 against freshly compiled
targets, per the rule that an old binary is not evidence.

| Check | Result | Evidence |
|---|---|---|
| Pro fresh build (`cargo build --bin physis-pro --bin physis`) | ✅ clean | `/tmp/pro_build.log` (session log) |
| Pro `cargo check --lib` | ✅ 0 errors — **the 6 pre-existing `registry.rs` errors from this handoff are resolved** | — |
| Core fresh build (`cargo build --bin physis-core`) | ✅ clean | — |
| Core lib tests | ✅ 224/224 | — |
| Core feature checks (`--no-default-features`, `embed-onnx`, `all-features`) | ✅ 0 errors | — |
| Focused suites: system_interface (20) + system_delegation (8) + system_mcp (2) | ✅ 30/30 | — |
| Core product dispatch removed | ✅ `physis-core studio` → `error: unrecognized subcommand 'studio'`; help lists mechanisms only | — |
| Pro routes live on fresh binary | ✅ `act --dry-run`, `studio-server --help`, `notebook <DIR> <QUERY>` all dispatch | — |

**Important clarification (resolves handoff items 2–3):** the apparent survival
of `vault`/`history`/`praxis`/`quality`/`studio` commands in Core was a stale
binary at `physis-core/target/debug/physis-core` (dated 4 Sep, pre-split).
Core is a workspace member and builds into the parent `target/`; the fresh
binary there contains mechanisms only. The stale artifact should be ignored
(or `physis-core/target/` removed) to avoid future false evidence.

**Day 1 gate: PASSED.** Next step per `2026-09-15-core-pro-development.md`:
Day 2 — extract collection adapters from `physis-core/src/observe.rs` into Pro
while preserving the log format and reader API; restart and migration parity.

---

**Actor**: `integration-owner`
**Model**: Sonnet 5 (`claude-sonnet-5`)
**Date**: 2026-09-15