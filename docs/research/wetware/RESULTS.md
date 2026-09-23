# Results (current)

All measurements on **pansa (CPU-only, no GPU)**. DeterministicBackend is trivial; numbers reflect the toy dynamics, not biological fidelity.

## P4 — Substrate invariance harness

`cargo run --example neural_invariance --features neural`

```
┌──────────────┬───────────┬─────────┬──────────┬────────────┐
│ backend       │ recon_acc │ delta   │ invariant│ latency_ms │
├──────────────┼───────────┼─────────┼──────────┼────────────┤
│ DETERMINISTIC │      1.00 │    true │      yes │      0.03   │
│ RESERVOIR     │      1.00 │    true │      yes │      0.10   │
│ SPIKING       │      0.00 │   false │       no │      0.00   │
└──────────────┴───────────┴─────────┴──────────┴────────────┘
```

**Invariant:** the semantic delta signal fires on deterministic + reservoir for the axis-flip task (both produce an `EmbeddingShift` mutation). Spiking does not (toy LIF threshold 1.0; 4-dim stimulus magnitude < threshold) — dynamics-tuning gap, not architecture failure.

## P5/P6 — Delta vs full-state + weak decoder + repair

`cargo run --example semantic_delta_vs_full --features neural -- --noise 0.25 --weak-decoder`

```
┌──────────────┬──────────┬──────────┬──────────┬──────────┐
│ backend       │ delta_ok │ full_acc │ weak_acc │ repair   │
├──────────────┼──────────┼──────────┼──────────┼──────────┤
│ DETERMINISTIC │   yes    │     1.00 │     1.00 │    1.00   │
│ RESERVOIR     │   yes    │     1.00 │     0.00 │    1.00   │
└──────────────┴──────────┴──────────┴──────────┴──────────┘
```

**Reading:**
- At σ=0.25, the weak decoder (3-channel projection + noise) recovers on deterministic (1.00) but fails on the reservoir (0.00) — the driven axis is dropped + noise exceeds signal.
- The delta-signal (`decode_semantic_delta` + `build_delta_report`) fires on **both** backends; `delta_engine::evaluate_mutation` projects it → `repair_lift = 1.0`.
- This is the structured-repair hypothesis (P6): Physis recovers the semantic delta when the weak decoder cannot. It does **not** recover the full label (weak_acc stays 0.00 on reservoir), so this supports H2 directionally, not H3-strong.

## Reproduction

```bash
cargo run --example neural_invariance --features neural
cargo run --example semantic_delta_vs_full --features neural
cargo run --example semantic_delta_vs_full --features neural -- --noise 0.25 --weak-decoder
cargo test --test neural_invariance --features neural
```
