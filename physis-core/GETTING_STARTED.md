# physis-core Getting Started

## Installation

```sh
# Install the CLI (includes studio and default features)
cargo install -- physis-core   # or use `cargo bin` from the repo

# Or embed as a library dependency
# Add to your Cargo.toml:
# physis-core = { version = "0.1", default-features = false }
#   + enable features you need:
#   - `cli` for the CLI binary
#   - `studio` for the embedded web GUI
#   - `embed-onnx` for trained embeddings
```

## Quick Start (CLI)

```sh
# 1. Build with default features (cli + studio)
cargo build --release           # or: cargo build

# 2. Classify a sentence
physis-core classify "first layer adhesion failed on the nozzle"

# 3. Open the studio UI (runs on http://127.0.0.1:3000)
physis-core studio --port 3000
```

## Quick Start (Library)

```rust
use physis_core::{
    embed::RandomProjectionEmbedder,
    ontology::OntologyLoader,
    classify::CellClassifier,
};

fn main() {
    // 1. Load the built‑in ontologies (33 JSON files)
    let loader = OntologyLoader::load_all();

    // 2. Use the deterministic random‑projection embedder (no model files)
    let embedder = RandomProjectionEmbedder::new(384);

    // 3. Build the classifier
    let clf = CellClassifier::build(&loader, &embedder);

    // 4. Classify some text
    let results = clf.classify_text("pump maintenance required");
    for r in &results {
        println!("{}×{} score={:.3} entries={:?}", r.domain, r.mode, r.score, r.entries);
    }
}
```

## Feedback Loop (Quality Tracker)

```rust
use physis_core::quality::QualityTracker;
use physis_core::embed::RandomProjectionEmbedder;

fn report_failure() {
    let tracker = QualityTracker::new(Box::new(RandomProjectionEmbedder::new(384)));
    let mut centroids = physis_core::models::HashMap::new();
    centroids.insert("FABRICATE×CREATE".to_string(), vec![0.5; 384]);
    tracker.report_failure(
        "classification was wrong",
        &centroids,
        Some("FABRICATE×CREATE"),   // optional correct domain
    );
    tracker.save("failures.json").unwrap();
}
```

## Adding an Ontology Pack

1. Create `config/govern_decide_ontology.json` following the schema.
2. Restart the CLI or studio; the loader picks up the new file automatically.
3. Classify with the new domain/mode:
   ```sh
   physis-core classify "court ruling on the patent dispute"
   ```

## Next Steps

- Read `PLANNING.md` for the domain/mode expansion roadmap.
- Experiment with the studio UI (six tabs: Classify, Semiotic grid, Ontology, Corpus, Discover, Quality).
- Swap in an ONNX embedder: `cargo build -p physis-core --features embed-onnx`.
- Contribute new ontology packs or domain/mode proposals.