# Experiments

## Implemented (P5/P6)

### E1 — Semantic delta vs full-state decoding
**File:** `physis-core/examples/semantic_delta_vs_full.rs`
**Task:** concept(0) = [1,0,0,0] → concept(1) = [0,1,0,0] on a 4-dim space.
**Backends:** DeterministicBackend, RandomReservoirBackend.
**Noise:** `--noise 0.0` and `--noise 0.25`.
**Result:** delta-signal (drift detector) fires on both backends at both noise levels; full-state nearest-prototype recovers at low noise, degrades under noise. **Delta survives noise better** (P5 supported).

### E2 — Weak decoder + Physis repair
**Run flag:** `--weak-decoder` in E1.
**Weak decoder:** 3-channel projection (drops axis 3) + Gaussian noise.
**Result:** deterministic weak-decoder recovers (1.0); reservoir weak-decoder fails (0.0); delta-signal survives (repair_lift = 1.0). **Physis constraint repair (via `delta_engine::evaluate_mutation`) recovers the semantic delta when the weak decoder cannot.** (P6 supported, conditioned).

### E3 — Substrate invariance base
**File:** `physis-core/tests/neural_invariance.rs`
**Assertion:** `SameSignalFiresAcrossSubstrates` — deterministic + reservoir both produce an `EmbeddingShift` mutation on the same stimulus; spiking does not (toy LIF threshold too high for 4-dim axis flip).

## Tasks defined but not yet automated

| Task | Spec section | Implemented? |
|---|---|---|
| AFFORDANCE (apple + hungry + reachable → EAT) | §11 | not yet |
| OBJECT STATE (cup ON table → grasp → cup HELD_BY self) | §11 | encoded as E1/E2; delta = axis flip, not named relation pair |
| AMBIGUOUS CONCEPT (DOG/CAT/WOLF → context → DOG) | §11 | not yet |
| NEXT STABLE STATE (object falling → object ON floor) | §11 | not yet |

## Running

```bash
cargo run --example neural_invariance --features neural
cargo run --example semantic_delta_vs_full --features neural
cargo run --example semantic_delta_vs_full --features neural -- --noise 0.25 --weak-decoder
cargo test --test neural_invariance --features neural
```
