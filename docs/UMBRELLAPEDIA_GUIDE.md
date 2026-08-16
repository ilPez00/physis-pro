# Umbrellapedia Integration Guide

Umbrellapedia is the wiki layer that renders physis knowledge as interlinked
pages instead of raw vectors. physis stays the brain (embed / classify / cohere
/ dream); Umbrellapedia is the surface. This guide covers how the two connect,
what physis already emits, and what a new feeder must do.

## The hand-off

```
                physis engine                       Umbrellapedia
┌──────────────────────────────┐     labelled      ┌──────────────────────────┐
│ Coherence nodes (label +     │ ──── core nodes ──▶│ pages (title + [[links]]) │
│ embedding, asserted verdict) │     (export)      │ category/facet indexes   │
│ FileThemeGraph (theme↔file)  │ ──── graph ──────▶│ uberwiki index + pages   │
│ Ontology (DOMAIN×MODE cells) │ ──── cells ──────▶│ cell landing pages       │
│ wiki-export (markdown pages) │ ──── built ──────▶│ wikilink content          │
└──────────────────────────────┘                    └──────────────────────────┘
```

Everything physis hands to Umbrellapedia is **labelled** — never a bare
embedding. A label is the handle a human (or another page) can grab. If you are
feeding new content and it has no label, register it with one first (see
below).

## What exists

### In-physis markdown export (`physis-pro wiki-export`)

`src/graph.rs` (`FileThemeGraph::export_to_wiki`) already renders the
file-theme graph as **uberwiki-compatible markdown**:

- `~/.physis/wiki/index.md` — master index of categories and themes
- `~/.physis/wiki/{category}.md` — per-category pages listing themes + linked files
- `~/.physis/wiki/files/{file}.md` — per-file pages
- `~/.physis/uberwiki_index.md` — the same master index in uberwiki form

Pages use `[[wikilink]]` syntax, so Umbrellapedia's own renderer (or any
wikilink-capable viewer) turns them into a browsable graph.

### The live engine surface your feeder posts to

Every connector below follows the same shape —
`source → embed (shared embedder) → register labeled node`:

| Feeder | CLI | Web | What it registers |
|--------|-----|-----|-------------------|
| File affinity scan | `physis-pro scan` | `POST /api/v1/scan` | one labeled node per file + per structural line (`path → fn/struct`) |
| Watch mode (auto-ingest) | `physis-pro watch <dir> [--once]` | `POST /api/v1/watch` | new/changed files as labeled nodes; drops nodes for removed files |
| Vault import (Obsidian/markdown, git log) | `physis-pro vault <dir> [--git N]` | `POST /api/v1/vault/import` | one node per note (title + `h1:`…`h3:` headings) + commits |
| Personal history | `physis-pro history <file>` | `POST /api/v1/history/import` | bookmarks, history, OPML feeds, chat messages |
| Praxis backfill | `physis-pro praxis <file>` | `POST /api/v1/praxis/backfill` | behavioral nodes with **asserted** success/inert/failure |
| Semiotic node editing | `physis-pro node-edit <id> <label> [--domain D --mode M]` | `POST /api/v1/coherence/edit` | label swap while preserving structure (id, verdict, edges) |
| Fixed-token search | `physis-pro node-search <q> [--budget N]` | `POST /api/v1/search/nodes` | — (query surface, not a feeder) |

**Embedder rule (critical):** nodes must be embedded with the **same** embedder
that embeds queries (the shared `SharedCtx.embedder`). Registered vectors carry a
provenance stamp (`embedder`, e.g. `bge-base`); on reload, a mismatched stamp
means *re-embed, don't compare*. Never register `mapper`'s internal
RandomProjection vectors into the searchable graph.

### The durable mirror (multi-device)

`--features spanner` builds a Google Spanner Graph sink:

- `POST /api/v1/spanner/sync` (upsert + `mirror` orphan-delete)
- `POST /api/v1/spanner/hydrate` (pull durable rows → re-embed → persist)

This is *memory*, not *publication*: Spanner holds the rows so a new device can
re-hydrate physis; Umbrellapedia still reads from the local engine (or its wiki
export), not from Spanner directly.

## Wiring a new feeder

1. **Produce a labelled doc.** Give it a human `title`, a `body` (the recallable
   text), and ideally headings. If the source is a file, reuse
   `FileInfo::labels()` (`path → …` lines + structural path) so it dedupes cleanly.
2. **Embed with the shared embedder**, off the hot path (the web handlers use
   `embed_batch_off_thread`).
3. **Register** via `PhysisCore::register_node_vec_labeled(emb, Some(label))`.
   Label dedupe is free: re-registering the same label returns the existing id.
4. **Persist** (`persist_node`) so a restart keeps the node.
5. **Surface it** — either a web endpoint (`POST /api/v1/…`) or a CLI command.
   Prefer both, mirroring the existing handlers.
6. **Assert a verdict** when the source carries one (Praxis backfill maps
   mood/progress → success +1 / inert 0 / failure −1). Unverified observations
   should *not* be asserted (a status line is not a judgment).

## Naming + links

Umbrellapedia pages are most useful when their titles are **stable handles**.
Prefer label prefixes that read as namespaces:

- `praxis:notebook:<title>` / `praxis:tracker:<title>`
- `bookmark <title>` · `history <url>` · `feed <title>`
- `git <short-hash>` (commits) · `src → dir → file` (scanned files)

Cross-links work best when physis emits `[[wikilink]]`s pointing at the **label**
(not the id): the label is the page title, so a link resolves without a lookup.

## Keeping it fresh

- Full re-scan: `physis-pro scan` (idempotent by label).
- Incremental: `physis-pro watch <dir> --once` in a cron (or the
  `PHYSIS_WATCH_DIR` background thread on the web server).
- Life-log: re-run `physis-pro praxis export.json` — re-applies verdicts,
  dedupes by label.
- After an embedder swap: `physis-pro spanner hydrate` (or the documented
  re-embed migration) regenerates vectors under the live model; the old-space
  nodes are flagged `stale_nodes`.

See `docs/SEMIOTIC_EDITING.md` for the structure/content invariant that makes
label edits safe while Umbrellapedia holds page links.