# Changelog

Ritual: bump `version` in Cargo.toml, add a `## [x.y.z]` entry here,
run `bash scripts/release-check.sh`, tag `v<version>` on green,
push tag. Every merge to main stays releasable; tags never move.

All notable changes to physis-pro are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.21] - 2026-09-19

### Added
- `physis-pro onboard [--max-sessions]`: prepopulate the store from this
  machine — shell histories, Claude sessions, opencode sessions, codex
  threads, hermes memory + plans, mesh fleet memory (traps/decisions).
  Re-runs dedupe by label.
- `history <file>` accepts shell histories and Claude session JSONL directly.
- `/health` gains `image_embedder: {kind, degraded}` mirroring the text slot.
- `scripts/bench-pack.sh` (PH-007): cold/warm pack benchmark with integrity
  check and assert-under thresholds.
- `scripts/check-test-tiers.sh` (PH-009): fast-tier floor (≥1300 tests),
  zero bare `#[ignore]`, prints the buried heavy tier.
- `scripts/release-check.sh` (PH-010): the release ritual as a gate
  (version > last tag, changelog entry, licence consistency, clippy, full
  suite, tiers, bench-pack smoke) with `--self-test`.

### Changed
- Image endpoints (`/embed`, `/classify` with image) resolve the registry
  image slot first, then the text embedder's own capability, else answer
  **503 + machine-readable reason** — never a silent text fallback (PH-008).
- `EmbedderRegistry` honors the dedicated `config.image` slot; new
  `VectorEmbed::handles_images()` probe (clip/jina).
- Test suite 790 s+ → ~105 s (PH-009): process-wide shared classifier
  (http_surface 253 s → 54 s), explicit config-RP honored without gate,
  embedder-agnostic CLI tests offline (22 tests: 185 s → 3 s).
- Confidence inversion closed (PH-005): shared `EmbedderHealth`, all surfaces
  read it; degraded embedders are reported, never hidden.
- Agentic workspace P1–P3: effective settings with origin, cancellable
  studio ask, trail-recorded nav intent + HUD resume, staged-diff handoff
  with HEAD stale-base check.
- JEV (r2-reasoning) provider: config knobs + env overlay + cascade entry,
  keys redacted in `config`/settings surfaces.

### Fixed
- Decimal-number dead branch (PH-022, verified by gate recheck).
- `PHICS_LOG_LEVEL` typo → `PHYSIS_LOG_LEVEL` (PH-001).
- Shell history import tolerates non-UTF-8 bytes (lossy read).
- UTF-8-safe truncation in all history parsers (byte-slice panic).

### Known limitations (packets, not regressions)
- PH-006 Pro-tier gating: BLOCKED on the missing PH-001 features decision.
- PH-023 NLI discriminator: gate unmet, no local NLI server in this env.
- First tagged release pending human go-ahead (PH-010 step 3).

## [0.1.20] and earlier — backfilled 2026-09-19 from git history

- MCP workspace service (`physis_system`): recall/pack/read/remember flow,
  runs/usage rollups, opt-in call audit.
- Semantic navigation (Function 3 MVP): meaning-first nav over content-hashed
  objects, HUD graph view, per-component `why:` explanations.
- Physis Studio demo TUI (offline-first): status/graph/notebook/diff/machines.
- Space·Time·Cause ledger, vault/history/praxis importers, mail import,
  purpose links, ask endpoint, quality dashboard, pulse dashboard.
- Semiotic editing (`node-edit`/`node-delete`), fixed-token search, watch
  mode, Spanner sink, facet CLI, file-theme graph + wiki export.
- Concealment probes, accuracy floor, output-quality probe (ignored tiers).
- GLIBC floor dropped so physis-hud runs on feather (2.41).
