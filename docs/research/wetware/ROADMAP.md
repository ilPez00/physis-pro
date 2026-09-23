# Roadmap (PH-101 priority order)

## Done (P0-P6)

- P0 — architecture inspected; reuse map written (`README.md`).
- P1 — `NeuralSubstrate` trait + normalized types (`substrate.rs`).
- P2 — `DeterministicBackend` (pass-through, no feature gate).
- P3 — `RandomReservoirBackend` (Echo State Network, `neural` feature).
- P4 — invariance harness (`tests/neural_invariance.rs` + `neural_invariance` example). DONE.
- P5 — semantic-delta vs full-state experiment. DONE (PH-106 calibration + `semantic_delta_vs_full` example).
- P6 — weak-decoder + Physis repair. DONE (`--weak-decoder` flag in same example).

## Done (P7-P11)

- **P7** artificial spiking backend — DONE (PH-106: toy LIF threshold calibrated 1.0→0.3; fires on reference task).
- **P8** recorded-neural backend — DONE (PH-107: `data/neural/sample.csv` fixture + 4 tests).
- **P9** FlyWire backend — scaffolded (`FlyConnectomeBackend`; no data committed; see FLYWIRE.md).
- **P10** Cortical simulator adapter — mock only (`CorticalSimulatorBackend`; no SDK).
- **P11** HUD substrate visualizer — DONE (PH-108: `--view neural` + tab `n`, three-pane render).

## Next

- **P12** polished mock-product/demo loop — needs a demo script (`cargo run --example neural_invariance` snapshot) + README badge.
- **P13** research report — this doc set. — this document set is the draft; the final report consolidates results.

## Non-goals

- No biological tissue culturing (§16).
- No fidelity claims about the spiking toy model.
- No GPU-required path in P0-P6.
