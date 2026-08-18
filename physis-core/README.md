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
| `studio` | Embedded web GUI (axum) for the ontology, the semiotic-grid heatmap, live classification, corpus scan/recall/judge/dream, gap discovery, and quality feedback. |

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

`physis-core studio` serves a single-page app over six tabs — every capability
the CLI has, plus the feedback loops, in one place. It shares the visual
language and shell conventions (header chips, nav tabs, status pill, `?`
overlay, `Ctrl+K` / `Ctrl+Enter`) of the physis-pro dashboards.

| Tab | What it does |
|-----|--------------|
| **Classify** | Score text against every populated cell. Shows the raw score next to the quality-adjusted one, the entries and facets behind each cell, the nearest single ontology entry, and the corpus passages closest to the same text. `✓` / `✕` on a result feed the quality tracker directly. |
| **Semiotic grid** | The grid as a heatmap, with axes derived from the loaded ontology — not from a fixed 5×14 block. Rows/columns marked ✦ came from a config, an edited entry, or a promoted proposal. Click a cell to see its entries or add one. |
| **Ontology** | Search across names, cells and hints; edit any entry (cell, axes, unit, hints, facets). Domain and mode are free text with suggestions, so a new axis can be introduced from the editor. Edits persist to `custom_ontology.json` and rebuild the classifier immediately. |
| **Corpus** | Scan a directory into coherence nodes, recall them by meaning, judge them (`worked` / `inert` / `failed`), and dream over the failures. Density and verdict stay separate axes, as in the engine. |
| **Discover** | Gap analysis over a corpus: coverage, unmapped count, tuned threshold, and proposed new domains — promote one and it becomes a real entry. |
| **Quality** | Every penalized and boosted cell with its weight, plus the failure log. Boosting a cell here undoes a penalty. |

The studio and the CLI share one state directory, so `physis-core scan` and the
Corpus tab operate on the same graph.

Scanning notes: files are split into ~1200-character passages (a whole-file
vector averages away everything specific), build and dependency directories
(`target`, `.git`, `node_modules`, …) are skipped, and one scan registers at
most 2000 passages — registering a node re-scores it against every existing
node, so ingestion is quadratic and an unbounded scan would wedge the process.

The studio binds `127.0.0.1` by default: its scan and ingest routes read
arbitrary local paths, so it must not be reachable off-box unless you opt in
with `PHYSIS_STUDIO_HOST=0.0.0.0` (e.g. inside a container).

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
