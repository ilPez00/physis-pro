# Contributing to Physis Pro

Thank you for your interest. This document explains the build, test, and
review conventions so contributions land cleanly.

## Prerequisites

- Rust 1.75+ (edition 2021)
- Node.js 18+ (for UI work)
- `cargo-watch` (optional, for `cargo watch`)
- `just` (optional, see `justfile`)

## GitHub CLI

Account: `ilPez00`  
Scopes: `gist`, `read:org`, `repo`, `workflow`  
Protocol: HTTPS  

```bash
gh auth status  # verify before repo operations
```

## Quick Start

```bash
# Clone and build
git clone https://github.com/ilPez00/physis-pro.git
cd physis-pro
cargo build --bin physis-pro

# Run tests
cargo test

# Run with features
cargo run --bin physis-pro --features web,voice -- scan /path/to/dir
```

## Project Layout

```
src/
  main.rs          — CLI binary (physis-pro)
  cli.rs           — CLI parser + PhysisApp (scan, query, dream, …)
  web/             — HTTP handlers (per-domain modules)
  web_server.rs    — axum server bootstrap
  embed/           — Vector embedders (ONNX, Jina, CLIP)
  core.rs          — Coherence graph + PDCA + dream
  classify.rs      — Semiotic grid classifier
  ontology.rs      — Ontology loader + discovery
  industrial.rs    — Machine adapters, OEE, maintenance
  ingest.rs        — Document ingestion pipeline
  license.rs       — Offline license enforcement
  ...
physis-core/       — Library crate (classify, models, ontology)
physis-core/config/ — THE ontology corpus (both crates read it; do not copy it back)
src/bin/ui/        — Console, Demo, Study dashboards (HTML/JS)
src/bin/voice.rs   — Voice binary (features: voice,web)
```

## Feature Flags

| Feature | What it enables |
|---------|-----------------|
| `web` | HTTP server + REST API |
| ~~`industrial`~~ | No-op alias. OEE, maintenance, inventory and the MQTT/Modbus adapters are always compiled. |
| `voice` | Whisper + Llama voice pipeline |
| `mcp` | MCP server (stdio or TCP) |
| `spanner` | Google Spanner Graph sink |
| `wiki-embedded` | In-process Tantivy BM25 wiki search |
| `rocm` | **Removed.** Dead configuration; use future MIGraphX/WebGPU EP instead |

Build with all features:
```bash
cargo build --all-features
```

## Testing

```bash
# Unit + integration tests (default features)
cargo test

# With web feature (includes http_surface)
cargo test --features web

# The industrial stack needs no feature — it is in the default build
cargo test industrial

# Specific test
cargo test test_cli_parser_scan
```

### HTTP Surface Tests

`tests/http_surface.rs` drives the real axum router in-process via
`ServiceExt::oneshot`. No port, no network. Add tests here for any new
endpoint.

### UI Tests

Open the dashboards in a browser:
```bash
cargo run --bin physis-pro-web
# Console: http://localhost:3000/
# Demo:    http://localhost:3000/demo
# Study:   http://localhost:3000/study
```

## Code Style

- **Rust:** `cargo clippy --all-features` must pass with zero warnings.
- **Formatting:** `cargo fmt` (rustfmt stable).
- **Commits:** Conventional commits (`feat:`, `fix:`, `refactor:`, `docs:`, …).
- **Comments:** Write them. The ontology, PDCA, and holarchy are non-obvious.
- **No secrets:** Never commit API keys, tokens, or license files.

## Pull Request Checklist

1. `cargo fmt`
2. `cargo clippy --all-features`
3. `cargo test --all-features`
4. Update `PLAN.md` if the change affects the roadmap
5. Update `CHANGELOG.md` under `[Unreleased]`

## Architecture Notes

- **Tenant isolation:** Web handlers that touch tenant state must use
  `AppCtx::with_user(uid, |rt| …)`. Stateless endpoints ignore the header.
- **Embedder consistency:** All nodes must be embedded with the same embedder
  that embeds queries (`SharedCtx.embedder`). Never use the mapper's internal
  `RandomProjectionEmbedder` for searchable core nodes.
- **Lock discipline:** Do not hold `Arc<Mutex<…>>` across `.await` points.
  Use `spawn_blocking` for CPU-heavy work.

## Core API stability (PH-012)

physis-core is Apache-2.0: its public surface is a promise. `tests/api_stability.rs`
snapshots all 2000+ public items (`tests/snapshots/physis_core_api.txt`) and
fails on any drift, printing the diff. Rules:

- Breaking the frozen surface requires a **minor+1 (0.x) bump** and a
  `CHANGELOG.md` `### BREAKING` entry under the new version.
- Never bless your own diff: `BLESS=1` rewrites the snapshot and is a
  human-only operation (CI never sets it). Deliberate changes go back to a
  human with the printed diff.
- New internal modules must NOT be `pub` — use `pub(crate)` or `#[doc(hidden)]`
  with an issue entry. Audit candidates are listed in `packets/reports/PH-012.md`.

## License

Physis Pro requires a valid license to run. See `src/license.rs` for the
enforcement model. For evaluation, set `PHYSIS_DEV=1` to bypass the loopback
auth gate.

## Questions?

- Architecture: `docs/PUBLIC_ARCHITECTURE.md`
- Semiotic editing: `docs/SEMIOTIC_EDITING.md`
- Pipeline diagrams: `docs/PIPELINE_DIAGRAMS.md`
- Commercial model: `docs/COMMERCIAL_MODEL.md`
