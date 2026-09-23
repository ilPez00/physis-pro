# Roadmap (PH-101 priority order)

## Done (P0-P6)

- P0 — architecture inspected; reuse map written (`README.md`).
- P1 — `NeuralSubstrate` trait + normalized types (`substrate.rs`).
- P2 — `DeterministicBackend` (pass-through, no feature gate).
- P3 — `RandomReservoirBackend` (Echo State Network, `neural` feature).
- P4 — invariance harness (`tests/neural_invariance.rs` + `neural_invariance` example).
- P5 — semantic-delta vs full-state experiment (`semantic_delta_vs_full` example).
- P6 — weak-decoder + Physis repair experiment (same example, `--weak-decoder`).

## Next (P7-P13)

- **P7** artificial spiking backend — replace toy LIF with a real LIF crate if one is acceptable; tune threshold so the axis-flip task fires.
- **P8** recorded-neural backend — ship with a tiny sample CSV so the replay path is exercised in CI.
- **P9** FlyWire backend — acquire a small subgraph (10k edges) and commit it under `data/fly/`; verify the diffusion dynamics produce non-trivial activation patterns.
- **P10** Cortical simulator adapter — if a supported simulator/API is available, gate behind `cortical` feature.
- **P11** HUD substrate visualizer — wire `physis-hud` (tui) to show the three-pane graph / activation / decoded view. Needs the tui subsystem; stubbed here.
- **P12** polished mock-product/demo loop — the `neural_invariance` example is the text version; the HUD view is P11.
- **P13** research report — this document set is the draft; the final report consolidates results.

## Non-goals

- No biological tissue culturing (§16).
- No fidelity claims about the spiking toy model.
- No GPU-required path in P0-P6.
