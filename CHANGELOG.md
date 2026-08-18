# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- physis-core studio rebuilt as a six-tab app (Classify / Semiotic grid / Ontology / Corpus / Discover / Quality) sharing the Physis Console design language: header chips, nav tabs, status pill, toasts, `?` overlay, `1`–`6`, `Ctrl+K`, `Ctrl+Enter`
- Studio API for the coherence graph, previously CLI-only: `/api/core/snapshot`, `/api/core/nodes`, `/api/core/scan`, `/api/core/search`, `/api/core/assert`, `/api/core/dream`, plus `/api/health`
- Studio classification now returns the nearest ontology entry, per-cell facets, and the corpus passages closest to the same text
- Studio assets split out of the HTML into `app.css` / `app.js`, served from the same binary
- `PHYSIS_STUDIO_HOST` to opt the studio out of loopback-only binding
- `physis-pro doctor` command — validates config, data dir, scan root, embedder, and license
- `--version` flag on all binaries
- `ui-shell.js` — shared runtime for Console, Demo, and Study dashboards
- Welcome overlays with onboarding on first visit
- Grouped console navigation into Signal / Meaning / Work sections
- Hover tooltips on technical jargon
- Empty-state CTA buttons ("Seed demo data")
- Last-updated timestamps on status pills
- Keyboard shortcuts overlay (press `?`)
- Active tab persistence in `localStorage`

### Changed
- `data_dir` defaults to `./.physis` (current working directory) instead of `~/.physis`
- `scan_root` defaults to parent of `data_dir`
- Loopback auth bypass now requires explicit `PHYSIS_DEV=1` environment variable
- `build_app()` returns `Result` for testability
- Replaced deprecated `rand` APIs (`gen` → `random`, `gen_range` → `random_range`, `from_entropy` → `from_os_rng`)
- `ort` updated to `2.0.0-rc.13`
- `rand` updated to `0.9`, `rand_core` updated to `0.9`
- Added `tracing` + `tracing-subscriber` for structured JSON logging
- Added `tower-http::TraceLayer` for HTTP request tracing
- Added `trace_tenant` middleware to attach `X-Physis-User` to tracing spans

### Fixed
- `ui-shell.js` is now actually served (`GET /ui-shell.js`, public — a `<script src>` sends no Bearer token) and no longer collides with the page-local `$`, `S`, `save` and `toastTimer` declarations it was loaded alongside; that collision was a SyntaxError that killed the entire inline script on Console, Demo and Study
- Studio grid axes are derived from the loaded ontology instead of the `HumanDomain`/`HumanMode` enums, so entries on any other domain or mode are visible and editable rather than scoring invisibly
- Studio directory scan splits files into passages (whole files over 5000 characters were silently dropped), skips build/VCS directories, embeds outside the write lock, and caps one scan at 2000 passages
- Removed stray `#[cfg(feature = "spanner")]` and `#[cfg(feature = "mcp")]` attributes that were incorrectly gating non-feature code
- Fixed `classify_handler` brace balance corrupted during module extraction
- Fixed `heatmap_handler` placement and brace balance in wiki_semiotic module

## [0.9.0] — 2026-08-18

### Added
- Industrial backoffice intelligence: OEE tracking, maintenance baselines, inventory
- Document ingest: classify and process structured/unstructured documents
- Vault import: Obsidian/markdown, plain notes, git log → labeled core nodes
- Personal history import: browser bookmarks, history, OPML feeds, chat JSONL
- Praxis backfill: notebook entries → behavioural records with verdicts
- Spanner Graph sink: durable-memory mirror
- File theme graph: ontology-driven file browsing
- Study log + Elenchus argument trainer
- MCP server support
- Voice binary support
- PostgreSQL row-delta ingestion
- SQLite incremental ingestion
- Schema discovery for SQL databases
- Watermark scans for incremental processing
- Per-ontology interpretants
- Salience: interpretant evidence that biases classification
- Offline licensing with vendor generator wrapper

### Changed
- Split `src/web.rs` (7,195 lines) into per-domain handler modules
- Auth gating: loopback bypass requires `PHYSIS_DEV=1`
- CI matrix expanded with voice+tui feature check

### Fixed
- Scope salience by tenant
- Stop losing concurrent observations
- Resolve bundled models by search path
- Make embedding cache durable against failed writes

## [0.8.0] — 2026-07-15

### Added
- Semantic self-test for embedder at startup (P4)
- PDCA stagnation detection
- Dream loop with stochastic generation
- Coherence trie for token-bounded search
- Fixed-token node search with budget

### Changed
- Replaced llmwiki subprocess with in-process Tantivy BM25 (wiki-embedded feature)
- Embedder selection with self-test cache

## [0.7.0] — 2026-06-01

### Added
- Holarchy ingest ring
- Shared knowledge graph
- Ontology overlays (sector/client/project)
- Faceted ontology queries
- Multiple embedder support (ONNX, Jina, CLIP)

### Changed
- Multimodal sensory bus (audio, vision, acceleration, gyroscope, thought capture)
- Single ingestion path into holarchy

## [0.6.0] — 2026-05-01

### Added
- 70-cell semiotic grid (5 domains × 14 modes)
- Coherence tracking (+1 success / 0 inert / −1 failure)
- Exchange-parts semiotic editing
- Quality feedback loop
- Dream replay for corrective actions

### Changed
- Unified embedding space for text, images, video, voice
- Classification against human + machine + office ontologies

## [0.5.0] — 2026-04-01

### Added
- PDCA cycle orchestrator
- Goal tracking in vector space
- Linguistic lenses (Wenyan, Piraha, Sanskrit)

### Changed
- Ontology mapper with trie and vector search
- CLI with scan, query, dream, evaluate commands

## [0.1.0] — 2026-03-01

### Added
- Initial release
- Basic ontological mapper
- CLI interface
- Web server with REST API
