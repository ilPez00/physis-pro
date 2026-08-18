# Contributing to Physis Pro

Thank you for your interest. This document explains the build, test, and
review conventions so contributions land cleanly.

## Prerequisites

- Rust 1.75+ (edition 2021)
- Node.js 18+ (for UI work)
- `cargo-watch` (optional, for `cargo watch`)
- `just` (optional, see `justfile`)

## Quick Start

```bash
# Clone and build
git clone https://github.com/ilPez00/physis-pro.git
cd physis-pro
cargo build --bin physis-pro

# Run tests
cargo test

# Run with features
cargo run --bin physis-pro --features web,industrial,voice -- scan /path/to/dir
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
src/bin/ui/        — Console, Demo, Study dashboards (HTML/JS)
src/bin/voice.rs   — Voice binary (features: voice,web)
```

## Feature Flags

| Feature | What it enables |
|---------|-----------------|
| `web` | HTTP server + REST API |
| `industrial` | OEE, maintenance, inventory, MQTT/Modbus adapters |
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

# With industrial feature
cargo test --features industrial

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

## License

Physis Pro requires a valid license to run. See `src/license.rs` for the
enforcement model. For evaluation, set `PHYSIS_DEV=1` to bypass the loopback
auth gate.

## Questions?

- Architecture: `docs/PUBLIC_ARCHITECTURE.md`
- Semiotic editing: `docs/SEMIOTIC_EDITING.md`
- Pipeline diagrams: `docs/PIPELINE_DIAGRAMS.md`
- Commercial model: `docs/COMMERCIAL_MODEL.md`
