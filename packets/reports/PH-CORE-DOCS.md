# PH-CORE-DOCS — physis-core documentation drift sweep

**Status:** implementation done, awaiting independent review.
**Branch/commit:** `physis-core` `agent/PH-CORE-DOCS` @ `4d18293`.

## Task

The Core/Pro split relocated the workspace service, studio, personal
importers, quality tracker, notebook, oracle, action execution and edition
detection out of `physis-core` into `physis-pro` (recorded in
`physis-core/MOVED_TO_PRODUCT.md`). The crate's own docs still advertised those
modules, features and CLI verbs. This packet removes the stale claims and points
readers at the destination map. Docs only — no Rust source changed.

## What changed (3 files, +70 / −193)

`physis-core/README.md`
- Quick start: dropped `physis-core notebook` and `physis-core studio`; kept
  `context`/`demo`/`benchmark`, added a pointer to `MOVED_TO_PRODUCT.md`.
- Table of contents: removed the "Embedded Studio Web Workbench" entry.
- Capabilities table: removed the "Vault & History Ingest" row.
- Core-vs-Pro table: "User Interface" now states Core ships none.
- Module Catalog: removed `history`, `praxis`, `quality`, `vault`, `studio`;
  added a moved-to-product note.
- Install snippet + feature table: dropped the `studio` feature and its
  `axum`/`tokio` dependencies.
- Removed the `QualityTracker` example (§7) and the stale
  "Build a Quality Prediction Agent" example, which used the removed
  `quality`, `config` and `ClassifierConfig` paths.
- Removed the "Embedded Studio Web Workbench" section and `PHYSIS_STUDIO_HOST`;
  corrected the `PHYSIS_CORE_DIR` description (no `quality.json`).
- `PhysisConfig` was removed from both discovery snippets and
  `OntologyLoader::load_all(&PhysisConfig::default())` corrected to the current
  no-argument `OntologyLoader::load_all()`.
- `physis` front-door section now records that the front door moved to the
  product.

`physis-core/CHANGELOG.md`
- Rewrote the `0.1.26 (unreleased)` entry, which advertised `system.rs`,
  `system_cli.rs`, `physis-system-tui` and `physis-world`. It now records the
  two changes actually in this tree: product modules retired from Core, and the
  mode axis reworked to nine owned modes with the 35 packs migrated.

`physis-core/.physis-capabilities`
- Removed rows whose write half no longer exists (`watch_fs`, `watch_proc`,
  `Workspace::record`/`remember`, `delegation_create`, `serve_workspace`).
  Kept only capabilities with both halves verified in `src/`.

## Verification (native)

```
grep -cE 'physis-core (studio|notebook)' README.md                        -> 0
grep -cE 'physis_core::(studio|vault|history|praxis|quality|notebook)' README.md -> 0
grep -cE '^\| `studio`' README.md                                          -> 0
grep -nE 'PhysisConfig|physis-core studio|PHYSIS_STUDIO_HOST' README.md CHANGELOG.md .physis-capabilities -> (none)
grep -nE 'physis-system-tui|physis-world|workspace interface|physis system' CHANGELOG.md -> (none)
```

The one remaining match for "workspace interface" is the `.physis-capabilities`
header comment that states the capability moved, which is the intended record.

## What I could not verify

- **No compile/test gate.** The change touches no Rust source, and the README is
  not compiled into the crate (`lib.rs` does not `include_str!` it), so its code
  snippets are not doctested. This is exactly why snippets rotted. A gate that
  compiles README examples would be a separate packet.
- **Independent review not performed.** This was executed by a single agent; the
  packet is `status: review`, not `done`.
- **Remaining stale prose not in scope:** `src/store.rs:26` still has a comment
  mentioning `physis-core studio`; the Core-vs-Pro table still says "70
  canonical semiotic grid cells (5 Domains × 14 Modes)" while the grid was
  reworked to nine owned modes. Both need a source edit and belong in a
  follow-up packet.

## Unknowns / proposed follow-ups

1. Compile README snippets (doctest harness) so docs cannot silently rot.
2. Fix `src/store.rs:26` comment and the grid-size prose in the Core-vs-Pro
   table to match `docs/GRID_AXES.md`.
3. `physis-core` is a retired-public repo with live product work; see
   `PH-CORE-RETIREMENT` node.
