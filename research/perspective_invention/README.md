# Perspective invention — can Physis discover a coordinate system, not just a class?

Mission: test whether a recursive representation system can **invert the
classifier** — observe relational residual its best organization leaves
unexplained, construct candidate re-representations from that residual,
re-organize under them, and check convergence across independent routes.

- `hypothesis.md` — pre-registered claim, tiers, falsification conditions
- `methodology.md` — pipeline, dataset, controls, ablations (frozen pre-run)
- `ARCHITECTURE.md` — full map of the existing system + gap analysis
- `results.md` — post-run verdicts (each claim OBSERVED/SUPPORTED/…)
- `failures.md` — everything that did not work, preserved
- `datasets/` — generated corpus sidecars (texts, latent/surface labels, seeds)
- `experiments/` — run logs, JSON results
- `visualizations/` — SVGs (original representation, residual graph,
  cross-perspective alignment, perspective tree, invariant structure)

Run:

```bash
cd /home/gio/dev/physis-pro
cargo run -p physis-core --features embed-onnx --release --example perspective_invention_experiment -- --seed 20260911
```

Isolation: branch `research/perspective-invention` (physis-core and
physis-pro). No production code was modified.
