Physis — Studio GUI (standalone)

This is the Physis core engine + ontology Studio, packaged as a single
self-contained binary. No external config or model files are required;
the ontologies and the Studio UI are built in.

Run:
    ./physis-core studio --port 3000
Then open http://localhost:3000

The Studio lets you browse/edit the semiotic-grid ontology, see the
5x14 heatmap, classify text live, and exercise the quality-feedback loop.
State (custom entries, quality) persists under ~/.physis-core/.

Classifier: deterministic RandomProjectionEmbedder (no model download).
To use a real MiniLM embedder instead, rebuild with --features embed-onnx
and pass --model <dir> to `studio`.
