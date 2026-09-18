# Physis Studio — one TUI demo product

> Sequencing update, 2026-09-18: [agentic workspace programme](2026-09-18-agentic-workspace.md)
> governs current scope and acceptance. This document retains original designs
> and dated reports. WORLD/OPS/PULSE are not demo prerequisites; the connected
> navigation → agent → review → outcome loop is. The original no-arbitrary-file-edit
> scope is superseded only for explicitly scoped, staged repository edits with
> conflict checks and safe undo.

Date: 2026-09-18. Status: S0–S3 partially shipped (see §11). Pairs with
[operator crystallization](2026-09-18-operator-crystallization.md) (the OPS tab
is that plan's P4/P5 surface).

Decision: **one binary, `physis-studio`, five-plus tabs, offline-first.** The
three TUI binaries that exist today each re-implement the same shell and each
see a third of the product. The demo product unifies them and adds the three
tabs that are genuinely new: graph file browsing, a NotebookLM-shaped source
notebook, and a diff reviewer whose accept/reject writes Physis semantics
(coherence edit + provenance; quality failure) instead of raw bytes.

## 1. Audit — what exists, what it costs to reuse

| Piece | File | Lines | What it is | Reuse verdict |
|---|---|---|---|---|
| Control panel | `src/bin/tui.rs` | 1775 | 4 tabs `STATUS/MACHINES/CONFIG/WIKI`; HTTP + in-process hybrid; `App` state (:77), key router (:915), per-tab draw fns (`draw_wiki` :1073, `draw_heatmap` :1191, `draw_status` :1283, `draw_machines` :1326, `draw_config` :1368), `Tabs` bar (:1020) | **Fork the shell**: tab bar, `App`, typing-mode guard, debounced live re-rank (`wiki_dirty`/`wiki_last_input`) |
| HUD | `src/bin/hud.rs` | 1897 | In-process `NavWorkspace` (:96, :306), one screen / three panes, `Row{Header,Dir,Hit}` (:154), `ContextTab{Why,Context,Delta}` (:163), state glyphs (:170), dir rollup (:200) | **Lift whole**, becomes the WORLD tab; keep glyph/rollup helpers verbatim |
| Wiki client | `src/bin/wiki.rs` | 577 | Minimal HTTP client, debounced, concurrent-safe | **Backport debounce/concurrency**; its WIKI tab is a subset of tui.rs's |
| Nav façade | `src/nav/workspace.rs` | — | `open`, `query`, `resolve`, `read`, `pins`, `add_pin`, `relations`, `relations_of`, `push_trail`, `trail`, `sem_off`, `sem_on`, `update`, `embedder` | **Sole data path** for GRAPH/WORLD; lazy embedder |
| File/theme graph | `src/graph.rs` | 810 | `FileThemeGraph`: `add_theme/add_file/add_edge`, `list_categories`, `themes_by_category`, `files_for_theme`, `find_related_themes`, `search_files`, `search_themes_by_text`, `export_to_wiki`, `save/load` (:247-556) | **GRAPH tab is a direct projection** — browse-graph CLI logic moves here |
| Notebook engine | `src/ported/notebook.rs` | 553 | `answer(docs, query, embedder, budget, backend) -> Answer{text, sources: Vec<Source{name,score,excerpt}>, context_tokens, baseline_tokens, degraded, latency_ms}`; `draft(result, query, max_tokens, confidence) -> Draft` with `Span` gaps | **NOTEBOOK tab is a direct projection** — no HTTP, no new engine |
| Diff | — | — | **Nothing.** Only `tree::diff` (dir trees) and `study::diff_snapshots` (mtime/size) | **New `src/diff.rs`** (§5) |
| Quality ledger | `src/quality.rs` | — | `report_failure`, `cell_standing`, `save/load` | reject-with-reason writes here |
| Edit semantics | `src/core.rs` + `src/cli.rs::run_node_edit` (:340) | — | `edit_node`/`delete_node`: swap content, keep id/verdict/edges/provenance; `cell_pin` | accept-hunk path |
| Purpose links | `web/ingest.rs` | — | `POST /api/v1/files/serve` (`serves(planner)`), `GET /api/v1/files/lineage` | cheap "why does this file exist" panel |
| Ops registry | (planned) | — | crystallization P4/P5 | OPS tab consumer |

## 2. Product shape

### Current settings and visual contract (supersedes original CONFIG scope)

Required: **Runtime · Memory · Primitives · Appearance** inside Settings, with
drawer/full-workspace modes and preserved focus when returning to work.
[Workspace programme §5](2026-09-18-agentic-workspace.md#5-reuse-architecture-and-ownership)
owns detailed behaviors, visual design and acceptance; this is planned, not shipped.

Memory manager projects existing notes, decisions, observations and derived caches:
search, evidence inspection, pin/exclude, revision, export and previewed retention.
Primitives editor exposes real registered data-backed definitions and recipes:
typed draft, validation, impact diff, versioned apply and rollback. Compiled code
is read-only; no fake editing or ungated operator promotion.

Use restrained charcoal/cyan styling, readable text, aligned spacing and shared
master/detail panels; light/high-contrast/monochrome variants, reduced motion and
responsive narrow-terminal layouts. Empty/error/stale/saving states deserve the
same finish as populated screens. Opening Settings must never load an embedder.

S3–S6 implementation must include memory and primitive persistence/restart tests,
invalid-edit refusal, cross-workspace isolation and snapshot + interactive checks.
The demo shows excluding stale memory and changing a bounded recipe parameter;
the extended walkthrough shows export and rollback. Generic OPS remains deferred.

The expanded tab sketch below is the original design, not the required current
navigation structure. Consolidate around the connected workflow above.

```
physis-studio [--root DIR] [--url http://127.0.0.1:8080] [--snapshot] [--demo SCRIPT]
  ┌ tabs ────────────────────────────────────────────────────────────────┐
  │ GRAPH · NOTEBOOK · DIFF · WORLD · OPS · PULSE · STATUS · MACHINES · CFG │
  └──────────────────────────────────────────────────────────────────────┘
```

Rules that keep it cheap and honest:

1. **Offline-first.** GRAPH, NOTEBOOK, DIFF, WORLD read the local store,
   `NavWorkspace`, and the theme graph in-process. HTTP is used only for
   tenant-scoped engine state (coherence, PDCA, dream, quality, ops metrics)
   and only when `--url` answers. No tab requires the server to be useful.
2. **Never block the draw loop.** Every tab loads through one worker +
   `crossbeam-channel` (already a dep); the frame renders last-known state and
   a `loading` flag.
3. **Never silent degradation.** Embedder off, server down, registry absent →
   a banner row in the tab, mirroring `nav`'s lexical-only reporting.
4. **No new store.** Every pane is a projection of an existing store
   (`FileThemeGraph`, `sled` nav store, `quality_failures.json`, coherence
   nodes, ops registry). Anything that would need a new table is out of scope.
5. **Old bins keep working** during S0–S6 (`physis-pro-tui`, `physis-hud`,
   `physis-pro-wiki` stay as-is); `physis-studio` is additive.

## 3. Tabs — key map, layout, data path

| Tab | Layout | Keys | Data path |
|---|---|---|---|
| **GRAPH** | left: `CATEGORY → THEME` tree; middle: files for theme; right: related themes (score) | `↑↓` move · `Enter` open theme/file · `/` filter · `r` related · `e` export-to-wiki · `s` serve-form | `graph.rs::FileThemeGraph::{list_categories, themes_by_category, files_for_theme, find_related_themes, search_*}` |
| **NOTEBOOK** | left: sources (with score + excerpt); right: answer + citations; bottom: draft-with-gaps | `/` ask · `↑↓` source · `Enter` open cited file in DIFF/WORLD · `d` draft · `y` copy citations | `ported::notebook::{answer, draft}` + `rag::retrieve_from_texts` |
| **DIFF** | side-by-side hunks, per-hunk status gutter | `n/p` hunk · `a` accept · `x` reject (prompt reason) · `u` undo · `w` toggle word/line · `o` open object | new `src/diff.rs` + `edit_node` (accept) + `quality::report_failure` (reject) |
| **WORLD** | HUD lifted whole: 3 panes + WHY/CONTEXT/DELTA | as HUD today (`g`, `p`, `sem-off/on`, `n` notebook) | `nav::workspace::NavWorkspace` |
| **OPS** | registry table (id@ver, status, support, replay/shadow rate), candidate queue with raw metric columns, `symbolic_fraction` sparkline | `↑↓` · `e` expand (print graph) · `p` promote (only when policy says yes) · `s` shadow | ops registry JSON + metrics (crystallization P3–P5) |
| **PULSE** | KPI ribbon, quality trend, PDCA line, communities, heatmap | `↑↓` · `Enter` drill | existing `/api/v1/*` (coherence snapshot, pdca/stats, quality, communities, semiotic/heatmap) |
| **STATUS / MACHINES / CONFIG** | lifted from `tui.rs` | as today | in-process + HTTP status |

## 4. Diff editor (the one genuinely new engine)

`src/diff.rs` — deterministic, dependency-free, ~200 lines + tests:

```rust
pub struct Hunk { pub old_range: (usize, usize), pub new_range: (usize, usize),
                  pub lines: Vec<DiffLine> }
pub enum DiffLine { Same(String), Add(String), Del(String) }
pub fn diff_lines(old: &str, new: &str) -> Vec<Hunk>   // LCS, Myers-lite
pub fn diff_words(old: &str, new: &str) -> Vec<DiffLine> // intra-line for Replace
pub fn apply_hunks(old: &str, hunks: &[Hunk]) -> String  // for preview only
```

Why in-repo and not `similar`: no diff crate is a dependency today
(`grep similar` empty), the algorithm is textbook, and a deterministic
in-repo implementation is testable against a golden corpus without adding
supply chain. `similar` stays the documented fallback if word-level quality
proves insufficient.

**What makes it a Physis diff, not a text diff** — the accept/reject verbs
write engine semantics:

- **accept** a hunk on a coherence node → `PhysisCore::edit_node()` (content
  swapped, id/verdict/edges/provenance preserved; `cell_pin` respected) with
  actor `studio` recorded in provenance. Accepting a file hunk holds the new
  text as the node label and links the two versions with a `Precedes` edge.
- **reject** → prompt for reason → `quality::QualityTracker::report_failure`
  (semiotic cell resolution + penalty persisted), so a rejected suggestion
  biases the same cell next time. Demo value: the dashboard's penalty bar
  visibly moves because the reviewer said no.
- **undo** = inverse edit, written as a new version, never a silent mutation
  (versioning rule from the crystallization plan §4).

Sources for a diff: (a) two nav objects (any two versions of a file),
(b) coherence node vs proposed candidate label, (c) crystallized operator
`expand()` graph vs candidate composition — the same pane that shows a diff
for a document also shows what an operator would do differently.

## 5. Cheap wins (ship because they cost a pane, not a subsystem)

| Win | Cost | Why it demos well |
|---|---|---|
| Ledger where/when/why (`place`/`cause`/`within_days` facets) | one pane over `files/query` | answers "why does this file exist" — nobody else's TUI does |
| Purpose links (`files/serve` + `lineage`) | one action + one pane | file → goal chain, live |
| Watch-mode indicator + `watch --once` sweep button | reuse `watch` CLI path | shows auto-ingest without leaving the TUI |
| Dream replay attention feed | existing `dream/replay` | "what Physis is thinking about" |
| Quality ledger panel | existing `quality` payload | rejections visibly matter (pairs with DIFF) |
| Semiotic heatmap | already in `tui.rs` (`draw_heatmap`) | move as-is |
| Wiki export → uberwiki | `export_to_wiki` | one keypress produces browsable HTML/markdown |
| Communities grid | existing endpoint | cheap structural view |
| MCP tool list + last `remember` | read-only | proves the agent loop is connected |
| Voice transcript files | browse only | multimodal without new plumbing |
| Spanner sync button | feature-gated | enterprise check-box, one call |

## 6. Performance / refinement (the boring part that makes a demo feel fast)

1. **Cold start budget < 300 ms** with the embedder off: open the nav store,
   read the theme graph JSON, render `STATUS`. The embedder initialises only
   when a tab first needs vectors (`NavWorkspace::embedder(true)` is already
   lazy) and never on the UI thread.
2. **Tab switch < 16 ms**: each tab's data is a cached `Arc<Snapshot>`; a
   switch paints the cache and *then* kicks the loader.
3. **No allocation in the draw loop**: rows are pre-formatted into `Line`s on
   load, `ratatui` widgets borrow them; `ListState` (not index arithmetic) for
   selection — `tui.rs`/`hud.rs` already do this, keep it.
4. **One debounce, one place**: `wiki.rs`'s 120 ms debounce becomes the shared
   input rule (search, filter, diff re-render).
5. **Bounded lists everywhere**: HUD already caps dir rollups (`aggregate_dirs`
   cap); apply the same cap to graph files per theme and notebook sources so a
   200k-file root cannot render a demo into a stall.
6. **Read once, share once**: `FileThemeGraph::load` and the coherence
   snapshot are process-wide `OnceCell`s refreshed explicitly, never per frame.
7. **Structured tracing on every load**: `tracing` spans per tab load with
   duration, so the perf pass is measured, not guessed.
8. **`--snapshot`**: render one frame of any tab to stdout as text (a
   deterministic golden artifact for CI and for screenshots in docs) — this is
   also what makes the demo script testable without a TTY.
9. **`--demo SCRIPT`**: a tiny key-sequence runner (`TAB,3,/,"relief valve",…`)
   so the killer demo of the crystallization plan is a one-command
   reproduction, and so the benchmark arms drive identical UI paths.
10. **Memory**: nothing unbounded is retained; `NavHit`s are `Arc`-shared; the
    diff holds both texts once (files > 2 MB are truncated with a visible
    marker rather than silently simplified).

## 7. Phases and gates

| Phase | Work | Gate |
|---|---|---|
| S0 | `physis-studio` bin: shell (tab bar, `App`, key router typing-guard, worker channel, status bar) + STATUS/CONFIG/MACHINES lifted from `tui.rs` | builds `--features tui`; `--snapshot` on STATUS; old bins untouched |
| S1 | GRAPH tab over `FileThemeGraph` (+ `browse-graph` logic moved into a shared module, CLI keeps calling it) | graph browse works offline on a root with no engine running |
| S2 | NOTEBOOK tab over `ported::notebook::{answer,draft}` | cited answer + gaps rendered from a local corpus; `degraded` surfaced |
| S3 | `src/diff.rs` + DIFF tab + accept/reject wiring | unit tests on diff; reject moves the quality ledger; accept preserves node id/verdict |
| S4 | WORLD tab (HUD lifted) | HUD feature-parity check: every key in `hud.rs` works in the tab |
| S5 | OPS tab consuming the crystallization registry + metrics | `expand()` renders a promoted operator's graph; promote only when policy allows |
| S6 | PULSE tab + cheap wins (§5) + perf pass (§6) + demo script | snapshot goldens stable; `--demo` reproduces the killer demo on pansa; budgets in §6 measured |

Nothing in S0–S6 requires the web server to be running; PULSE is the only tab
that needs it and it degrades to a banner.

## 8. Tests

- `src/diff.rs`: LCS correctness (insert/delete/replace/moves/no-op), word
  diff, `apply_hunks` round-trip, CRLF and trailing-newline cases, golden
  corpus. Property: `apply_hunks(old, diff_lines(old,new)) == new`.
- Tab data shapers are pure functions where possible (`graph_rows()`,
  `answer_rows()`, `registry_rows()`) so they unit-test without a terminal.
- Key routing: table-driven test `(tab, key) -> Action` — the `tui.rs` router
  is a `match` in the event loop today, which is untestable; extract
  `fn action_for(tab, key) -> Action` and test it.
- `--snapshot` goldens committed for GRAPH/NOTEBOOK/DIFF/OPS with a seeded
  fixture root (`tempfile`, already a dev-dep).
- Manual smoke on pansa: `ssh pansa-ts` → build → `--snapshot` all tabs →
  `--demo`.

## 11. Shipped so far (2026-09-18)

The test counts and green results below are historical reports, not rerun during
roadmap review. Source inspection found NOTEBOOK `ask()` initializes embeddings
and calls `answer()` synchronously from key handling. Background loading for
every tab is a target, not current behavior. Corpus selection takes the first
200 graph files before retrieval. The new programme addresses both limitations.

| Piece | File | State |
|---|---|---|
| `physis-studio` bin, tab bar, status bar, key router, snapshot mode, args | `src/bin/studio.rs` | **shipped** — 6 tabs, `--snapshot [TAB]` renders a real frame through the same `draw()` path (`ratatui::backend::TestBackend`), `--left/--right` open straight on DIFF |
| GRAPH tab | `src/bin/studio.rs` over `graph::FileThemeGraph` | **shipped** — category→theme rows, files for theme, related themes ≥0.55, `Enter` opens a theme (focus in/out), `d` feeds the DIFF pane, `r` reloads; missing graph names `physis-pro ingest` |
| NOTEBOOK tab | `src/bin/studio.rs` over `ported::notebook::{answer}` | **shipped** — corpus = the graph's files (capped 200 × 64 KB), tier + sources + `degraded` + backend label shown, embedder built lazily on first ask |
| DIFF tab (line + word view) | `src/bin/studio.rs` + `src/diff.rs` | **shipped (review-only)** — hunks from `diff_lines`, `w` pairs del/add runs through `diff_words`, `n/p` navigate, `c` recomputes, 2 MB/side cap announced in the header |
| `diff` engine | `src/diff.rs` | **shipped** — LCS with prefix/suffix trimming + `MAX_CELLS` degrade, `split_inclusive` for byte-exact round trip; 11 tests |
| Shared core-embed bridge | `src/embed/mod.rs::CoreEmbedAdapter` | **shipped** — `quality.rs` now uses it instead of a private copy |
| Machine demos as library code | `src/machines/demos.rs` | **shipped** — `run(name)`/`names()`, 7 machines, test that every advertised name runs; `bin/tui.rs` still carries its own copy, folded in at S6 |
| `instructor` feature + pyo3 | `Cargo.toml` | **shipped (dependency only)** — off by default, in `full` |
| STATUS / MACHINES / CONFIG tabs | `src/bin/studio.rs` | **shipped** — engine health on a background thread, machine runs in-process via `machines::demos`, config edits/save on the same five knobs and the same file as `physis-pro-tui` |
| Tests | `src/bin/studio.rs` (15) + `src/diff.rs` (11) + `src/machines/demos.rs` (1) | **green** — router table, typing swallows shortcuts, tab wrap, bounded config edits, arg parsing, every-tab snapshot frame, DIFF end-to-end (lines + word view), notebook corpus titles, GRAPH row shape + header→no-theme, missing-side reporting; `clippy -D warnings` clean on the bin; `rustfmt` clean |

Still open: **accept/reject wiring** on the DIFF pane (`edit_node` +
`quality::report_failure`, the S3 remainder — the pane is review-only today and
says so nowhere near a silent fake), the **WORLD** tab (HUD lift, S4), **OPS**
(S5, needs the crystallization registry), **PULSE** and the §5 cheap wins (S6),
the perf pass with measured budgets (§6), and the `--demo` key-script runner.

## 12. Non-goals

Not an IDE, no LSP, no terminal multiplexer, no editing of arbitrary files
outside the coherence-object path, no new persistence, no plugin system, no
mouse support in v1. Voice stays a separate bin.

## 10. Dogfooding hook (project rule)

Starting this work: `just recall "studio tui demo"`; after each phase:
`just dev-loop "<phase>"`. Log Physis-assisted lookups and their conventional
alternative (grep/read) so the workspace rule produces data, not claims.
