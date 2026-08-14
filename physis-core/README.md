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
