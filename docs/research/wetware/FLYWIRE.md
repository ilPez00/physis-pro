# FlyWire / FAFB connectome backend

**Status: scaffold, not implemented.** `FlyConnectomeBackend` exists and reads an edge-list CSV, but **no connectome data is present** in this repo.

## Why scaffold only

Public Drosophila connectome datasets (FAFB, FlyWire) are multi-GB. Bundling or downloading them in-repo is out of scope for P0-P6. The backend is wired so that dropping `data/fly/connectome.csv` (columns: `source,target,weight`) activates it.

## What is NOT claimed

- This is **not** a fly-brain simulator.
- Simplified dynamics (rate neurons, graph diffusion) are used — tagged `toy dynamics` in `backends/fly.rs`.
- No synaptic plasticity, no neurotransmitter model, no anatomical layers beyond node IDs.

## Data format expected

`data/fly/connectome.csv`:
```
source,target,weight
0,1,0.823
1,4,0.154
...
```
Neurons are inferred from `max(source, target)`. Cell-type / neurotransmitter metadata, when available from a richer source, would extend the edge list (optional columns ignored until wired).

## Reproducing with real data

1. Acquire FAFB-derived adjacency (public, CC-BY): `https://github.com/fcollman/flywire` or the MICrONS public releases.
2. Convert to the 3-column edge list into `data/fly/connectome.csv`.
3. `cargo run --example neural_invariance --features neural -- --backend flywire` (example arg not yet accepted; switch the `backends` vec in `neural_invariance.rs`).

The backend currently lives behind `--features neural` and will raise `SubstrateError::Backend` if the CSV is absent.
