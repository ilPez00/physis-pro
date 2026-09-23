# SUBSTRATE_API — NeuralSubstrate trait contract

## Feature gate

The trait + `DeterministicBackend` always compile (no feature). Stochastic/spiking/biological backends require `--features neural`.

## Trait

```rust
pub trait NeuralSubstrate {
    fn capabilities(&self) -> SubstrateCapabilities;
    fn configure(&mut self, config: SubstrateConfig) -> SubstrateResult<()>;
    fn stimulate(&mut self, input: StimulusPattern) -> SubstrateResult<()>;
    fn observe(&mut self, window: ObservationWindow) -> SubstrateResult<NeuralObservation>;
    fn reset(&mut self) -> SubstrateResult<()>;
    fn health(&self) -> SubstrateHealth;
}
```

## Normalized types

- `StimulusPattern { channels: Vec<(ChannelId, f32)>, dense: Vec<f32>, hint: Option<(domain, mode)> }` — sparse or dense. The encoder writes `dense`; the decoder's `hint` carries the semantic cell side-channel.
- `NeuralObservation { t: usize, populations: HashMap<String, PopulationState>, window }` — `readout` is the canonical read-out population; `membrane`/`raster` are spiking-specific extras.
- `PopulationState { values: Vec<f32>, trace: Vec<Vec<f32>> }` — `values` maps 1:1 to `CoherenceNode.embedding` dims so no copy is needed.
- `Confidence { score: [-1,1], entropy, uncertainty }` — `score` maps to `CoherenceNode::asserted`; entropy from the softmax of `values`.
- `SubstrateConfig` — `kind`, `population_size`, `seed`, `dt`, `noise`, `extra: HashMap<String, String>` for backend-specific knobs (spectral radius, leak rate, plasticity on/off).

## Invariants a backend MUST satisfy

1. `stimulate` does not block on network or tissue.
2. `observe` is pure read — no mutation.
3. `reset` clears transient state (activations, time) but preserves configured topology.
4. `health().online == false` ⇒ `observe` returns `SubstrateError::Backend`.
5. Population dimension is consistent across `configure` + `stimulate` + `observe`.

## Backends

| Backend | Feature | Topology source | Dynamics |
|---|---|---|---|
| `DeterministicBackend` | (none) | identity matrix | pass-through |
| `RandomReservoirBackend` | `neural` | seedable RNG | leaky-tanh Echo State |
| `ArtificialSpikingBackend` | `neural` | self-population | toy LIF (DECAY=0.1, THRESH=1.0) — NOT biological |
| `RecordedNeuralBackend` | `neural` | CSV/JSON file | replay |
| `FlyConnectomeBackend` | `neural` | `data/fly/connectome.csv` (absent) | rate diffusion |
| `CorticalSimulatorBackend` | `neural` | mock | stub (no device) |
| `FutureWetwareBackend` | `neural` | — | raises `LiveHardwareUnavailable` |
