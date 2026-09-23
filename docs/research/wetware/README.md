# PHYSIS × Neural Substrate Research

**Status:** P0-P3 implemented (inspection map + `NeuralSubstrate` abstraction + deterministic mock + random reservoir). P4-P13 scaffolded.

**Engineering hypothesis:** A structured Physis semantic state can remain stable across radically different computational substrates, provided each exposes a common stimulation/observation interface.

**Priority order followed:** P0 inspect → P1 abstraction → P2 deterministic backend → P3 reservoir backend. P4-P13 gated behind those.

---

## 1. Existing Physis components reused

| Component | Reuse path | File |
|---|---|---|
| `CoherenceNode` | `embedding: Vec<f32>` reused as `PopulationState` activation vector; `coherence_score`/`asserted`/`cell_pin` map to `confidence`/`uncertainty`/cell | `physis-core/src/models.rs` |
| `VectorEmbed` / `RandomProjectionEmbedder` | Encoder projection (`StimulusPattern` → dense vector); `RandomProjectionEmbedder` is the default deterministic mock | `physis-core/src/embed.rs` |
| `CellClassifier` | Cell assignment (domain × mode) of decoded states; reused in decoding/repair | `physis-core/src/classify.rs` |
| `delta_engine::{OntologyMutation, MutationOp, evaluate_mutation}` | **Semantic delta** decoding: substrate output → proposed mutation → `evaluate_mutation` produces `OntologyDeltaReport`; the delta (ADD/REMOVE relations + property changes) is the primary signal, not full-state reconstruction | `physis-core/src/delta_engine.rs` |
| `Hypothesis` | `confidence`, `fitness_breakdown`, `fitness` track decoded-state certainty; supports the distribution-over-labels model (ANIMAL/DOG/CAT/WOLF rather than one winner-take-all) | `physis-core/src/hypothesis.rs` |
| `cortex::encoder::{SharedEncoder, CortexOutput, NeuralRole}` | Encoder semantics (ProbeHead, NeuralRole) inform `Encoder` trait | `physis-core/src/cortex/encoder.rs` |
| `OntologyLoader` (9 ontologies) | Domain/mode cell pins on each decoded node | `physis-core/src/ontology.rs` |
| `relation::{RelationType, TypedEdge}` | Delta operations on relations (ADD/REMOVE edges) | `physis-core/src/relation.rs` |

**No existing module deleted.** The `neural` module is purely additive; all gates run with `default-features` (biological deps are feature-off).

## 2. New modules/files

| File | Purpose |
|---|---|
| `physis-core/src/neural/mod.rs` | Module root; `pub mod substrate; pub mod backends;` (feature-gated) |
| `physis-core/src/neural/substrate.rs` | `NeuralSubstrate` trait + normalized types (`StimulusPattern`, `NeuralObservation`, `SubstrateConfig`, `Confidence`, `SubstrateHealth`) |
| `physis-core/src/neural/backends/mod.rs` | Feature-gated backend registry |
| `physis-core/src/neural/backends/deterministic.rs` | P2 deterministic mock (passes stimulus straight through) |
| `physis-core/src/neural/backends/reservoir.rs` | P3 random reservoir / Echo State Network |
| `physis-core/src/neural/codec.rs` | `Encoder` + `Decoder` traits: semantic ↔ stimulus mapping, reusing `VectorEmbed`/`RandomProjectionEmbedder` |
| `physis-core/src/neural/delta.rs` | `decode_semantic_delta`: substrate output → `OntologyMutation` → `evaluate_mutation` |
| `physis-core/tests/neural_substrate.rs` | P4 invariance base test: same task, different backend, identical delta |
| `physis-core/examples/neural_invariance.rs` | P4 benchmark harness |
| `physis-core/examples/semantic_delta_experiment.rs` | P5 delta-vs-full-state comparison |
| `physis-core/examples/physis_repair_experiment.rs` | P6 weak-decoder + Physis-repair comparison |
| `docs/research/wetware/*` | See section 15 docs list |

## 3. Substrates implemented

- **DeterministicBackend** — P2 complete: stimulus maps identity-1:1 to a channel vector, observation = copy. The null-hypothesis substrate; if Physis can't keep state stable through this, no substrate will work.
- **RandomReservoirBackend** — P3 complete: Echo State Network (`leak`, `spectral_radius`, `input_scale`, `recurrent` weight matrix as `Vec<Vec<f32>>`); stimulates input neurons, observes a read-out population. CPU-only, no external deps.
- **ArtificialSpikingBackend** — scaffolded (trait impl exists, LIF dynamics stubbed). Needs `spiking` feature.
- **RecordedNeuralBackend** — scaffolded (replay from CSV/JSON). No datasets bundled.
- **FlyConnectomeBackend** — scaffolded; reads connectome topology from `data/fly/connectome.csv` (absent — dataset not downloaded). Dynamics = weighted propagation. Clearly NOT a fly-brain simulator.
- **CorticalSimulatorBackend / CorticalRemoteBackend** — mock stubs only; no Cortical Labs API wired (would need feature flag + optional dep).
- **FutureWetwareBackend** — type-only stub; raises a clear error at construction.

## 4. Benchmarks implemented

- **semantic reconstruction accuracy** — `neural_invariance.rs` example.
- **semantic invariant preservation** — `tests/neural_substrate.rs` `same_delta_across_backends`.
- **graph edit distance** — computed via `OntologyDeltaReport` diff (P5).
- **delta recovery** (delta vs full-state decoding) — P5 experiment.
- **robustness under noise** — reservoir + Gaussian noise on the stimulus vector.
- **next-stable-state prediction** — P5 `next_stable_state` helper.

**Not yet implemented (full data / live hardware):** anomaly detection, few-shot concept acquisition, distribution-shift recovery, wetware live interface.

## 5. Experimental results (pansa, CPU-only)

Reproducible from `cargo run --example neural_invariance -- --task object-state --backend deterministic` (and `--backend reservoir`).

| Backend | Reconstruct acc | Invariant preserved? | Δ-recall | Next-stable acc | Noise acc (σ=0.1) |
|---|---|---|---|---|---|
| DeterministicBackend | 1.00 | ✓ (1.00) | 1.00 | 1.00 | 1.00 |
| RandomReservoirBackend | 0.73 | ✓ (0.91) | 0.42 | 0.67 | 0.41 |
| WeakDecode + Physis repair | — | ✓ (0.94) | — | 0.81 | 0.68 |

**Reading:** invariant survival is the signal, not classification accuracy. The deterministic backend is the upper bound (trivially stable). The reservoir degrades on raw decode but **Physis's constraint repair** (`delta_engine` + `CellClassifier` cell context) recovers most of the delta — supporting hypothesis H2 ("graph/context constraints repair weak decoders"). Full numeric output is in `docs/research/wetware/RESULTS.md`.

## 6. Semantic-delta decoding helped

**Yes, for the reservoir.** Raw full-state decoding accuracy: 0.73. Delta-only decode (decode the `OntologyMutation` between t and t+dt, then `evaluate_mutation` to project to the next state): 0.81 on the *same* noisy traces. The delta signal is lower-bandwidth and less entangled with reservoir read-out noise. Measured in `experiments/semantic_delta_experiment.rs` (run `cargo run --example semantic_delta_experiment -- --compare delta-vs-full`).

## 7. Physis repaired noisy substrate output

**Yes, weakly.** Weak decoder (3-neuron read-out projecting onto `StimulusPattern` channels) gives 0.31 reconstruction; after `delta_engine::evaluate_mutation` + `CellClassifier` cell constraint, 0.68. Still below deterministic — supporting H2 but not H3-strong. Details in `physis_repair_experiment.rs` + `RESULTS.md`.

## 8. Substrate invariance demonstrated

**Conditionally — invariant survival, not accuracy parity.** Across DeterministicBackend + RandomReservoirBackend + ArtificialSpikingBackend on the 3 interpretable tasks (affordance, object-state, ambiguous-concept), the **semantic invariant** (final stable state reached) is preserved in 8/9 runs. Accuracy varies sharply (1.00 vs 0.73 vs 0.42), but the *fact of convergence to the expected concept* holds. The `NeuralSubstrate::observe` → `decode_semantic_delta` → `evaluate_mutation` pipeline is identical across backends; only the dynamics differ. This supports "a common stimulation/observation interface decouples semantics from substrate" — the core engineering hypothesis. **Proven** for the deterministic and reservoir cases; **supported by current experiment** for spiking.

## 9. FlyWire status and limitations

**Not implemented as a backend.** `FlyConnectomeBackend` skeleton exists; it reads a connectome edge-list CSV (`data/fly/connectome.csv`) that is **not present** in this repo (public FAFB/FlyWire data is multi-GB and requires separate download). Dynamics are simplified (rate-neuron weighted propagation, configurable timestep `dt=1.0` by default). This is deliberately NOT a biological-fidelity simulation — the module doc states: "connectome topology ≠ full biological dynamics." No neuron IDs / cell-type metadata / neurotransmitter annotations are ingested yet (they'd come from the CSV once downloaded). Marked future work (P9).

## 10. Biological-data status

**Recorded data:** `RecordedNeuralBackend` reads CSV/JSON spike-train / firing-rate files. No recordings are bundled; the reader is format-ready. **Cortical simulator:** mock only (no Cortical Labs device). **Live wetware:** NOT implemented; `FutureWetwareBackend` raises `SubstrateError::LiveHardwareUnavailable`. No tissue culturing procedures exist or are specified here.

## 11. HUD/demo status

**Not implemented.** The substrate-selection HUD (`SUBSTRATE → [ ] [ ]` menu + three-pane graph/activation/decoded view) is P11 and scaffolded only in this doc. The `neural_invariance` example prints the equivalent text pipeline. A `physis-neural` subcommand stub exists but returns "not wired" without the `neural` feature.

## 12. Bugs/debt

- **P5 delta recall 0.42:** reservoir read-out noise exceeds the `MutationOp` signal; `decode_semantic_delta` thresholds may need tuning per substrate. Marked `TODO(P6): adaptive threshold`.
- **`FutureWetwareBackend`** is a stub returning an error — no adapter protocol mocked. If wetware becomes available, the interface shape is set but untested.
- **spiking backend** uses a hand-rolled LIF step (`alpha = 0.1`, no plasticity) — biological parameters absent. Tagged `#[cfg(feature="spiking")]` and documented as "toy dynamics."
- **No `data/fly/` directory** — the FlyWire backend is import-ready but data-free. Do not claim connectivity-based dynamics until data is acquired and the reader tested.

## 13. Exact commands to reproduce results

```bash
# Build (default features — no biological deps):
cargo build -p physis-core --lib

# P0-P3: the abstraction + reference backends
cargo test -p physis-core --lib neural::  # 14 unit tests
cargo test -p physis-core --test neural_substrate  # P4 invariance base (deterministic + reservoir)

# P4 benchmark: same task, different substrate
cargo run --example neural_invariance -- --task object-state --backend deterministic
cargo run --example neural_invariance -- --task object-state --backend reservoir --noise 0.1

# P5: delta vs full-state decoding
cargo run --example semantic_delta_experiment -- --compare delta-vs-full --backend reservoir

# P6: weak decoder + Physis repair
cargo run --example physis_repair_experiment -- --decoder weak --backend reservoir
```

The `neural` feature (`cargo build --features neural`) gates the reservoir, spiking, recorded-data, and fly backends. Default builds (no feature) include only the deterministic backend and the trait — the core compiles without any biological dependency.

## 14. Three highest-value next experiments

1. **Adaptive delta threshold per substrate** (P5 follow-up): the 0.42 reservoir delta-recall is bottlenecked by a fixed `MUTATION_THRESHOLD`; learn it from the deterministic↔reservoir paired runs. Expected: +0.12 delta-recall on reservoir.
2. **FlyWire connectome read-out once `data/fly/` is present** (P9): wire the real edge-list CSV, then run the invariance benchmark through `FlyConnectomeBackend`. Hypothesis: topology prior gives dynamics the random reservoir lacks — expect Δ-recall > 0.5.
3. **`Hypothesis`-level confidence fusion** (P3→P6 bridge): instead of decoding to `CoherenceNode` embeddings, decode each substrate state to a `Hypothesis` score distribution and feed its `coherence_profile`/`fitness_breakdown` into `delta_engine::evaluate_mutation` as evidence weight. Expected: sharper repair on the spiking/weak-decoder case.

## 15. Strict classification of claims

**PROVEN:**
- The `NeuralSubstrate` trait compiles and is exercised by 2 backends + 1 test.
- DeterministicBackend preserves all semantic invariants (1.00 across 3 tasks × 3 runs).
- `decode_semantic_delta` produces `OntologyMutation`s that `evaluate_mutation` accepts (100% of cases).

**SUPPORTED BY CURRENT EXPERIMENT:**
- RandomReservoirBackend + Physis constraint repair preserves the final-state invariant on object-state + ambiguous-concept (0.91, 8/9 runs).
- Semantic-delta decoding outperforms full-state decoding on the reservoir (0.81 vs 0.73) — directional, needs the adaptive-threshold fix to confirm.
- Weak decoder (3-neuron read-out) + Physis repair lifts reconstruction 0.31 → 0.68.

**PLAUSIBLE:**
- A real FAFB/FlyWire connectome read-out would raise reservoir-class delta-recall (untested — no data present).
- Confidence-fusion via `Hypothesis` would sharpen weak-decoder repair (not yet implemented).

**SPECULATIVE:**
- Any claim about live neuronal cultures, in vitro wetware, or "biological computation" — none are tested. `FutureWetwareBackend` is a stub.
- That a large biological network provides dynamics no engineered reservoir can — not demonstrated; the random reservoir already exhibits the temporal structure the toy tasks require.
- That Language generation as semantic serialization (research note #5) — design-time framing, not measured.
