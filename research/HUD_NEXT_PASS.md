# HUD Next Pass — Research Findings
**Date:** 2026-09-21  
**Branch:** agent/PH-100  
**Method:** Direct source reading + live binary timing

---

## A. Snapshot/headless rendering — why does it hang?

### Root causes (three, in priority order)

**A1 — `workspace_tokens` reads all model files: ~20 seconds (PRIMARY HANG)**

Source: `src/bin/hud.rs` `fn workspace_tokens(root: &Path)` (line ~297)  
Called from: `App::new()` unconditionally, before any rendering.

```rust
fn workspace_tokens(root: &Path) -> usize {
    for f in physis_pro::scanner::scan_project(root, None) {
        let Ok(text) = std::fs::read_to_string(root.join(&f.path))
        ...
    }
}
```

`scan_project` excludes `target/`, `.git/`, `.physis/` but **does not exclude `models/`**.  
The repo has 2.6 GB in `models/` (including a 2.3 GB `nli-deberta-v3-xsmall/` model).  
`workspace_tokens` reads every file content to count tokens — so it reads 2.3 GB of binary model data.

**Measured:** reading all non-excluded files in the repo takes **~20 seconds** (8.4 GB total including model binaries).  
With `PHYSIS_EMBEDDER=random-projection` AND the repo `cwd` (so `models/` is on local SSD path): **same hang** — the issue is `workspace_tokens`, not the embedder.

**Fix (BUILD NOW, 1 line):** Add `"models"` to `DEFAULT_EXCLUDE_DIRS` in `src/scanner.rs`.

---

**A2 — BGE-base ONNX warm-up embed: ~21 seconds (when ollama is off)**

Source: `App::new()` line ~2128:
```rust
if semantic_on {
    let _ = ws.embedder(true).map(|e| e.embed("warm"));
}
```

`select_embedder` cascade: ollama → BGE-base ONNX → … The `SelfTestCache` at `~/.physis/embedder_selftest.json` caches the self-test result (pass/fail), so the model file doesn't get reloaded on every run — but **the first actual `embed("warm")` call after selection still loads and warms the 106 MB ONNX model**.

**Measured:** `PHYSIS_OLLAMA_EMBED_OFF=1` (no ollama) → `physis-hud --no-prescan --snapshot` takes **21 seconds**. With `PHYSIS_EMBEDDER=random-projection` (once A1 is fixed): **< 1 second**.

**Fix (BUILD NOW):** Skip the warm-up embed in `App::new` when called from `snapshot_run`. Pass a `headless: bool` flag or check `PHYSIS_SNAPSHOT_MODE=1`. For CI: always set `PHYSIS_EMBEDDER=random-projection`.

---

**A3 — Ollama 120-second blocking timeout (when ollama is down)**

Source: `src/embed/ollama.rs` line ~46:
```rust
reqwest::blocking::Client::builder()
    .timeout(std::time::Duration::from_secs(120))
```

`semantic_self_test` calls `e.embed("test")` on each candidate in the cascade. If `localhost:11434` is up but slow (or ollama model is loading), this probe can block 120 seconds before falling through.

**Fix (LATER):** Short probe timeout for cascade selection (2–5s). The 120s timeout is correct for production embedding calls; the self-test probe doesn't need it. Separate `probe_timeout` from `embed_timeout`.

---

**A4 — sled is always opened in write mode (not a hang, but relevant)**

Source: `src/nav/workspace.rs` `fn open_db()`:
```rust
Ok(sled::Config::new().path(dir.join("index.sled")).open()?)
```

No read-only mode. sled 0.34 does not have a `read_only()` builder — it has `.temporary(true)` (deletes on drop) but that would destroy the index. **There is no sled read-only mode in 0.34.**

`snapshot_run` calls `App::new(root, false)` which calls `NavWorkspace::open` which calls `open_db()` — opening sled in **exclusive write mode even for a snapshot**. If another process holds the lock (e.g. a running `physis-hud`), snapshot fails with `WouldBlock`.

**Fix strategy for read-only snapshots (BUILD NOW):** Do not open sled at all for pure snapshot mode. The `NavWorkspace` only needs sled for: trail, pins, `RelInfer`. For `--snapshot` none of these need to be written. Add `NavWorkspace::open_readonly()` that skips `open_db()` and uses `None` for the `db` field (or opens with `.temporary(true)` as a scratch-only db that dies on drop without touching the live store).

---

### SNAPSHOT_ROOT_CAUSE

```
src/bin/hud.rs  fn workspace_tokens()  — reads all files in root including models/ (2.6 GB)
src/bin/hud.rs  App::new()            — calls workspace_tokens() unconditionally
src/bin/hud.rs  snapshot_run()        — calls App::new(root, false) with no skip for headless
src/embed/mod.rs select_embedder()    — warm-up embed("warm") takes 21s when BGE-base is primary
src/embed/ollama.rs                   — 120s timeout for cascade self-test probe
```

**Primary: `workspace_tokens` on a repo with `models/` present. Secondary: embedder warm-up when ONNX/ollama is selected.**

---

### FIX_STRATEGY (minimal, in order)

1. **Add `"models"` to `DEFAULT_EXCLUDE_DIRS`** (`src/scanner.rs`, 1 line). Fixes the 20-second hang for any repo with model files.
2. **Skip `workspace_tokens` in snapshot/headless mode** — or accept stale value (the token count is display-only in the startup card; a snapshot doesn't need it fresh). Cheapest: pass `ws_tokens: 0` when `prescan == false`.  
   Cleaner: add `PHYSIS_FAST_OPEN=1` env guard in `workspace_tokens` that returns 0 immediately.
3. **Skip warm-up embed in `App::new` for snapshot mode.** `snapshot_run` already passes `prescan=false`; add a `headless` parameter or env var so `semantic_on && !headless` gates the warm-up.
4. **CI matrix:** set `PHYSIS_EMBEDDER=random-projection` for all snapshot jobs. Add `PHYSIS_OLLAMA_EMBED_OFF=1` as belt-and-suspenders.

---

### READ_ONLY_PATH

**sled 0.34 has no read-only mode.** The `Config` builder exposes: `cache_capacity`, `mode`, `use_compression`, `compression_factor`, `temporary`, `create_new`, `print_profile_on_drop`. No `read_only`.

**Alternative (BUILD NOW):** `NavWorkspace::open_snapshot(root)` — opens the JSON index from disk (no sled) and constructs a workspace with `db: None`. Trail/pins/RelInfer return empty results. This is safe for snapshot rendering because `draw()` only reads `ws.index()` and a few sled reads that can fall back to empty.

**Fallback (LATER):** `.temporary(true)` on a copy of the db path creates an in-memory sled that doesn't contend with the running instance, but this is heavier than skipping sled entirely.

---

## B. Proven snapshot patterns in Ratatui apps

### What `snapshot_run` does

```
snapshot_run(root, w, h, view, query, no_semantic)
  └── App::new(root, false)          ← full App init, sled write lock, workspace_tokens
  └── open_named_view(&mut app, view)
  └── TestBackend::new(w, h)
  └── terminal.draw(|f| draw(f, &app))
  └── print buf.content line by line
```

**One code path, no special headless mode.** The snapshot exercises exactly the same `draw()` function the TUI uses — which is good (it proves what the user sees) but means it pays the full `App::new()` cost including sled, workspace_tokens, and embedder selection.

### What the tests do

Tests use an **isolated temp workspace** pattern (e.g. line 6736–6742, line 7197–7204):

```rust
let dir = tempfile::tempdir().unwrap();
let _data_dir_lock = lock_data_dir();  // serialize against parallel tests
std::env::set_var("PHYSIS_DATA_DIR", dir.join(".physis"));
std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
let mut app = App::new(dir.to_path_buf(), true).unwrap();
```

The temp dir has **no `models/` subdirectory**, so `workspace_tokens` reads only a handful of tiny fixture files — sub-millisecond. This is why the 43 HUD lib tests all pass quickly while the binary `--snapshot` hangs on the real repo.

**The tests are not a representative proxy for the `--snapshot` code path.** They accidentally avoid the hang by using empty temp dirs. A CI snapshot test that runs against the real repo root would hit the same hang.

### Are they the same code path?

Yes — both call `App::new` + `draw()`. The tests use `TestBackend` internally too (via `snapshot_state()` and direct `ListState` assertions). The difference is data, not code.

---

## C. Index/DB access pattern for read-only snapshots

### Can we open sled read-only?

**No.** sled 0.34.7 does not expose a read-only open. The `Config` builder has no such method. The only escape hatch is `.temporary(true)` which creates an in-memory/tmpfs db that is deleted on drop — this avoids contending with the main lock but wastes memory and breaks trail/pin reads (empty results).

### Can we load the JSON index without touching sled?

**Yes — and this is the right approach.**

`NavWorkspace` separates two concerns:
- **JSON index** (`src/nav/workspace.rs` `fn open`): loaded via `nav::load_index(&db, root)` — it reads from sled if present, falls back to a fresh empty index
- **Runtime sled state**: trail, pins, `RelInfer` — these are accessed via `ws.store()`

For snapshot rendering, `draw()` only needs `ws.index()` (objects, files, embeddings for query ranking). It does not write trail entries, does not read pins (or can return empty), and does not infer relations.

**BUILD NOW:** Add `NavWorkspace::open_for_snapshot(root: &Path) -> Self` that:
1. Reads `<data_dir>/nav_index_<root_hash>.json` directly from disk (skipping sled entirely)
2. Sets `db` to a sled `.temporary(true)` scratch store (so sled-reading methods don't panic)
3. Returns a workspace suitable for read-only rendering

This requires exposing `nav::load_index_from_file(path)` or similar — the index JSON is already written by `persist()` in workspace.rs.

---

## D. HUD structure audit — consolidation

### Duplicate state fields remaining

`App` struct (`src/bin/hud.rs`) still carries:

| App field | FocusState equivalent | Split-brain risk |
|---|---|---|
| `sel: usize` | `focus_state.cursor.row` | **YES — synced manually in apply_hud_effects** |
| `hits: Vec<NavHit>` | _(none — FocusState has no hits)_ | N/A |
| `rows: Vec<Row>` | _(none — FocusState has no rows)_ | N/A |
| `selected: Vec<NavHit>` | `focus_state.selection` | **YES — partial** |
| `stack: Vec<(String, Vec<NavHit>, Vec<Row>, usize)>` | `focus_state.trail` | **YES — duplicated climb logic** |
| `crumbs: Vec<String>` | `focus_state.trail.crumbs` | **YES** |

Usage counts in `src/bin/hud.rs`:
- `self.sel`: **16 write sites**, `self.focus_state.cursor`: synced in `apply_hud_effects` only
- `self.hits`: **24 write sites** (not in FocusState — FocusState doesn't own retrieval results)
- `self.rows`: **19 write sites** (same)
- `self.selected`: **15 write sites** vs `focus_state.selection`

### Is the PH-029 split-brain fully resolved?

**Partially.** PH-029 moved navigation events through `dispatch() → reduce()` for:
- `MoveCursor` → `HudEffect::SetCursor` → `apply_hud_effects` syncs both `self.sel` AND `focus_state.cursor`
- `FocusObject` → updates `focus_state.focus`
- `Descend` / `Ascend` → updates `focus_state.trail` and `focus_state.focus`
- `Select` / `ClearSelection` → updates `focus_state.selection`

**What is NOT routed through the reducer:**
1. Direct `self.sel = pos` at **3 sites** (line 3273, 3304, 4261) that do NOT also update `focus_state.cursor` — confirmed split-brain remnants
2. `self.sel = self.rows.iter().position(...)` at **2 sites** (lines 3902, 4108) after `run_query` and `apply_zoomed_out` — also set `self.sel` directly without dispatching `MoveCursor`
3. `self.selected` is updated directly in toggle/selection handlers, not via `dispatch(Select)`

**Verdict:** The primary navigation paths (keyboard Up/Down, Enter/Descend/Ascend/Back) are correctly routed through the reducer. The **secondary paths** (open-file, run-query, zoomed-out rebuild) write `self.sel` directly without syncing `focus_state.cursor`. This is a latent bug: `focus_state.cursor.row` can diverge from `self.sel` after these operations, which would matter if/when the reducer reads `focus_state.cursor` to validate bounds.

### CONSOLIDATION_NEEDED

1. **`self.sel` ↔ `focus_state.cursor.row` sync** — 3 direct-write sites not going through `dispatch(MoveCursor {...})`: lines 3273, 3304, 4261. Fix: replace with `self.dispatch(HudEvent::MoveCursor { delta: 0, absolute: Some(pos) })` pattern, or add a `self.set_sel(pos)` helper that sets both.
2. **`run_query` / `apply_zoomed_out` sel reset** — these set `self.sel = first_hit_row` directly. Fix: call `self.dispatch(HudEvent::MoveCursor { delta: 0, absolute: Some(first_hit_row) })` after rebuilding rows.
3. **`self.stack` ↔ `focus_state.trail`** — the zoom stack `Vec<(query, hits, rows, sel)>` and `FocusState.trail` both track descent. PH-029 added `FocusState.trail` for the canonical scoped trail; `self.stack` is the HUD's hit/row cache. They serve different purposes but `back()` must pop both atomically. Currently `back()` pops `self.stack` then dispatches `Ascend` — correct, but fragile.

---

## RESEARCH_USED (implementation-changing findings)

| Finding | Changes what | Action |
|---|---|---|
| `workspace_tokens` reads `models/` (2.6 GB, ~20s) | `DEFAULT_EXCLUDE_DIRS` | Add `"models"` to scanner exclusion list |
| No sled read-only in 0.34 | `NavWorkspace::open_for_snapshot` design | Use `.temporary(true)` scratch db or skip sled entirely via JSON-only path |
| Warm-up embed fires even in snapshot mode (~21s for ONNX) | `App::new` / snapshot path | Gate warm-up on `!headless` |
| `SelfTestCache` at `<data_dir>/embedder_selftest.json` already exists | CI env requirements | CI must set `PHYSIS_EMBEDDER=random-projection`, not rely on cache |
| `sel` has 3 un-dispatched write sites after PH-029 | reducer completeness | Fix before adding more navigation state |
| Test scaffold uses temp dir with no `models/` — accidentally avoids hang | Test fidelity | Tests don't prove the snapshot works on the real repo |

---

## Primitives and BUILD/LATER/REJECT

| Primitive | Source | Maps to | Decision |
|---|---|---|---|
| Add `"models"` to `DEFAULT_EXCLUDE_DIRS` | `src/scanner.rs:10` | `workspace_tokens` hang fix | **BUILD NOW** — 1 line |
| Skip `workspace_tokens` in snapshot/headless | `src/bin/hud.rs:297` | Display-only metric, safe to skip | **BUILD NOW** — guard with `prescan == false` |
| Skip warm-up embed for snapshot | `src/bin/hud.rs:2128` | 21s hang when ONNX is primary | **BUILD NOW** — add `headless` bool |
| `NavWorkspace::open_for_snapshot` (JSON-only, sled=temporary) | `src/nav/workspace.rs:81` | Sled lock contention on concurrent processes | **BUILD NOW** — needed for fleet demo |
| Reduce ollama probe timeout to 2–5s | `src/embed/ollama.rs:46` | 120s hang when ollama is slow | **LATER** — not blocking for CI |
| Route `run_query` sel reset through `MoveCursor` | `src/bin/hud.rs:3902,4108` | Sel/cursor split-brain | **BUILD NOW** — correctness, small change |
| Route 3 direct `self.sel =` sites through dispatcher | `src/bin/hud.rs:3273,3304,4261` | Sel/cursor split-brain | **BUILD NOW** — complete PH-029 |
| `NavWorkspace::open_snapshot` from JSON without sled | `src/nav/workspace.rs` | CI snapshot without sled lock | **BUILD NOW** |
