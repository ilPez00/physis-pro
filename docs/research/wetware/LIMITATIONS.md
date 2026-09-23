# Limitations

1. **No biological data present.** `data/fly/connectome.csv`, `data/neural/`, and any Cortical Labs device handle are absent. Backends that consume these will raise `SubstrateError::Backend("not found")` until wired.
2. **Spiking backend is a toy.** `ArtificialSpikingBackend` uses `DECAY = 0.1`, `THRESHOLD = 1.0`, no plasticity, no synapse model. It will not reproduce biological dynamics. Tagged `#[cfg(feature = "neural")]` and documented inline.
3. **4-dim task space.** All experiments use a 4-axis concept space to keep runs interpretable and fast. Results do NOT generalize to full ontology scale (929 entries) without revalidation.
4. **CPU-only.** No GPU path for reservoir/spiking. Throughput is bounded; the harness reports latency per stimulus but not batch throughput.
5. **No live wetware.** `FutureWetwareBackend` is a stub returning `LiveHardwareUnavailable`. No tissue-culture support, no sterility/safety hardware.
6. **Delta decoder is a drift detector.** `decode_semantic_delta` flags "something changed" but does not by itself classify what changed into a semantic relation pair (ADD/REMOVE edge). Wiring to `OntologyMutation::RelationCreated`/`RelationSevered` is future work.
7. **PH-104 carryover:** the `--features full` ONNX embedder is unavailable on pansa (`lept` missing). The neural experiments do **not** depend on the ONNX path — they use `RandomProjectionEmbedder` (CPU, always available). The unavailability is not a blocker for P0-P6.

## What works

- The `NeuralSubstrate` trait compiles with `default-features` (core builds).
- DeterministicBackend + RandomReservoirBackend run end-to-end.
- `decode_semantic_delta` + `delta_engine::evaluate_mutation` close the loop on a 4-dim axis-flip.
- The invariance test (`tests/neural_invariance.rs`) asserts the delta signal fires on two backends.
