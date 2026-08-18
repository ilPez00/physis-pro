# physis-core

The lean core of the Physis engine, extracted as a self-contained package:
**embed → classify → cohere → learn from feedback**. No industrial adapters, no
web server in the lib, no heavy ONNX deps in the default build.

## What's in it

| Module | Purpose |
|--------|---------|
| `embed` | `VectorEmbed` trait + deterministic `RandomProjectionEmbedder` (zero model files, same input → same vector). Swap in an ONNX embedder behind the trait later. |
| `ontology` | Loads the 33 built-in ontology JSONs (praxis/human grid + machine + office + agent + extras) into domain maps. |
| `classify` | `CellClassifier` — nearest-entry cosine scoring over DOMAIN×MODE cells with a domain prior. |
| `core` | `PhysisCore` — coherence nodes, asserted-failure dream loop, certified/isolated branches, snapshot. JSON-persisted. |
| `quality` | `QualityTracker` — the feedback loop: report failures/successes → cell penalties/boosts → adjusted scores. |
| `studio` | Embedded web GUI (axum) for browsing/editing the ontology, the semiotic-grid heatmap, live classification, and quality feedback. |

State persists under `~/.physis-core/` (nodes, quality, custom ontology edits).

## Build & test

```sh
cargo build --release
cargo test --release
```

## CLI

```sh
physis-core classify "first layer adhesion failed on the nozzle"
physis-core ontology
physis-core scan /path/to/docs        # register text files as coherence nodes
physis-core search "pump maintenance" # recall nodes by similarity
physis-core snapshot                  # coherence health
physis-core assert "<label>" failure  # report a verdict (success|inert|failure)
physis-core dream
physis-core quality summary
physis-core quality fail "wrong domain"
physis-core studio --port 3000        # the ontology studio GUI
```

## Studio GUI

`physis-core studio` serves an interactive single-page app:

- **Left** — ontology browser: search, categories, click any entry to edit (rename,
  move to another grid cell, adjust hints). Custom edits persist and are layered
  over the built-ins at load.
- **Center** — the 5×14 semiotic grid as a heatmap; click a cell to see and add
  entries, or open the "New entry in this cell" form prefilled.
- **Right** — live classification (results adjusted by learned quality penalties),
  and the quality feedback loop: mark a result wrong → the cell gets penalized
  for future classifications.

Everything rebuilds the classifier in-process on edit, so the grid and rankings
update immediately.

## Design notes

- The default embedder is intentionally model-free and deterministic so the
  package builds and runs anywhere. Cosine separation is real but coarser than a
  trained model; plug a real embedder into the same `VectorEmbed` trait when
  quality matters more than zero-dependency.
- `quality.adjust_score` multiplies raw scores by `(1 - penalty + boost)`; the
  same adjustment is applied in the CLI and the GUI so learning visibly changes
  ranking.
- JSON persistence (no sled) keeps the core dependency-light.

## Features

| Feature | Default | Pulls in | Use |
|---------|---------|----------|-----|
| `cli` | yes | `clap` | The `physis-core` binary and its subcommands. |
| `studio` | yes | `axum`, `tokio` | The embedded ontology GUI (`physis-core studio`). |
| `embed-onnx` | no | `ort`, `tokenizers` | Real MiniLM embeddings; `--model <dir>` at runtime, falls back to random projection if the model is absent. |

Build the engine alone (no GUI, no ONNX) for embedding into other crates:

```sh
cargo build -p physis-core --no-default-features
```

## Publishing

`physis-core` is a workspace member of `physis-pro` (`Cargo.toml` root has
`members = ["physis-core"]`), but it is also published as an independent crate so
consumers can depend on just the engine without the industrial monolith:

```toml
[dependencies]
physis-core = { version = "0.1", default-features = false } # engine only
# physis-core = { version = "0.1" }                          # + cli + studio
# physis-core = { version = "0.1", features = ["embed-onnx"] } # + ONNX embedder
```

- The crate lives in its own subdirectory and is CI-checked in isolation via
  `.github/workflows/physis-core.yml` (no-default-features + default + all-features
  + clippy + tests).
- `default-features = false` is the documented "embed me" mode: no `axum`/`clap`/
  `ort`. Consumers re-enable what they need.
- Version is bumped independently of `physis_pro` (they share a repo but separate
  semver lines). Tag core releases as `physis-core-vX.Y.Z`.
- `cargo publish -p physis-core` (run from the repo root) publishes only the core
  package; the monolith's path dependency means a published `physis-core` will be
  picked up by downstream crates automatically.
## Free sample & 7‑day trial

A **ready‑to‑run binary** and a **quick‑start guide** are available for download at:

<https://praxisweb.xyz/physis/free-sample>

The download includes:

- `physis-core` compiled binary (`physis-core`) for Linux x86_64 (static,
  no external dependencies when `default-features = false`).
- `GETTING_STARTED.md` (the guide you are reading now).
- A **7‑day trial licence key** (`trial-key.json`) that unlocks all features
  for 7 days, after which the binary continues to work in “core‑only” mode
  (deterministic embeddings, classification, ontology loading) but the
  quality‑feedback and ONNX features are disabled.

**How to use the trial**

```sh
# 1. Download and extract the zip (or tar.gz) from the link above.
# 2. Put the binary on your PATH, e.g.
mv physis-core /usr/local/bin/
# 3. Run the classification demo
physis-core classify "first layer adhesion failed on the nozzle"
# 4. Open the embedded studio
physis-core studio --port 3000
# 5. The trial key is automatically accepted; after 7 days the binary will
#    gracefully downgrade to the free core mode.
```

If you wish to continue after the trial, purchase a commercial licence via the
Stripe flow described in `STRIPE_LICENSE_GUIDE.md`.
