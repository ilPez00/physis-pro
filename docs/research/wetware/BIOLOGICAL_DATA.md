# Biological data status

## Recorded neural data

`RecordedNeuralBackend` reads CSV / JSON spike trains / firing-rate traces. **No recordings are bundled.** The reader accepts:

- CSV: one row per timestep, one column per channel.
- JSON: `{ "channels": N, "trace": [[...], [...]] }`.

Format is extensible; new public datasets (Allen Brain, NeuroData Without Borders) drop in by adding a reader module under `src/neural/backends/` and registering in `mod.rs`.

## Cortical simulator / live hardware

`CorticalSimulatorBackend` is a **mock** preserving the interaction model. No Cortical Labs API is wired (would require `cortical` feature + proprietary SDK). The stub raises `SubstrateError::Backend("no device connected (mock)")` on `stimulate`/`observe`.

## Live wetware

`FutureWetwareBackend` exists **only as a stub** that returns
`SubstrateError::LiveHardwareUnavailable`. **No procedures for culturing neural tissue are provided.** Wet-lab protocols, if ever added, would require laboratory supervision, institutional safety review, and legal/ethical sign-off — not the scope of this software effort.

## Summary

| Data source | Status | Notes |
|---|---|---|
| FAFB / FlyWire connectome | scaffold (no data) | CSV edge list expected at `data/fly/` |
| Recorded spikes / rates | format-ready (no data) | CSV/JSON reader |
| Allen Brain / NWB | reader stub | not implemented |
| Cortical Labs device | mock only | no SDK integration |
| Live cultures | stub error | never cultured here |
