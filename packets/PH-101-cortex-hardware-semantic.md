---
id: PH-101
title: Physis Cortex: common CPU/ROCm/CUDA backend + LFM semantic layer
track: physis
phase: 1
status: todo
rung: 1
depends: [PH-024]
gate: "test -f physis-core/docs/plans/2026-09-20-cortex-core.md && test -f docs/plans/2026-09-20-cortex-pro.md"
commit: "branch agent/PH-101; plans commit (docs only)"
claimed_by:
claimed_at:
---

# PH-101 — Physis Cortex

## Why
Physis needs a semantic perception layer that is NOT the world model, and hardware
must be transparent: the same semantics over CPU / AMD ROCm / NVIDIA CUDA. Today
there is no backend abstraction (grep for `BackendKind` finds nothing) and CUDA is
absent; ROCm exists only as a fleet note marked "dead end" for the ONNX ROCm EP.

## Context
- `physis-core/src/embed_onnx.rs` uses `ort` 2.0.0-rc.13, which supports CPU + CUDA
  execution providers. ROCm EP via ONNX is a documented dead end
  (`packets/README.md` line 13: "ONNX Runtime GPU (ROCm EP) is a dead end — do not
  retry blind"). ROCm therefore lives in the colibri/llama.cpp stack (omo, RX 6650
  XT, HIP), not in `ort`.
- `physis-core/src/model_provider.rs` already has `Capability` — reuse for the
  escalation ladder, do not build a second capability type.
- `physis-pro/src/intent_ir.rs` already has the 9-slot typed IR — extend, do not
  replace.
- `src/agents/routing.rs` already has `RouteRequest`/`Decision` — extend.
- `src/registry.rs` + `src/capability.rs` are built but unwired (S0 of
  `2026-09-19-impossible-computer.md`) — this is their caller.

## Steps
1. Write the two plan docs (this packet's gate). No code.
2. Task 1 (core): `ComputeBackend` trait + `BackendKind{Cpu,Rocm,Cuda}` in
   `physis-core/src/backend.rs`. No `#[cfg(rocm)]`/`#[cfg(cuda)]` in semantic code.
3. Task 2 (core): `physis-core/src/devices.rs` `DeviceInfo` + `HardwareDiscovery::scan()`,
   `physis-core hardware` subcommand. NVIDIA via `nvidia-smi` + `ort` CUDA EP probe;
   AMD via `rocm-smi`/sysfs; CPU always.
4. Task 3 (core): `physis-core/src/cortex/encoder.rs` — `NeuralRole`,
   `CortexOutput`, one shared encoder invocation feeding intent/NER/relation
   heads as adapters/probes.
5. Task 4 (core): `physis-core/src/cortex/latent.rs` — canonical Physis latent with
   `W_e/W_d/W_c/W_g` projections (identity/linear/MLP), benchmarked.
6. Task 5 (core): `physis-core/src/cortex/retrieval.rs` — progressive refinement
   exact → structural → sparse → dense ANN → ColBERT → graph constraints.
7. Task 6 (core): `physis-core/src/cortex/router.rs` — `Intent` enum,
   `RoutingDecision`, deterministic-first ladder.
8. Task 7 (core): extend `bench.rs` for CPU/ROCm/CUDA per model × quantization.
9. Task 8 (core): `tests/cortex_contract.rs`.
10. Task 9 (pro): wire `PHYSIS_BACKEND` env + per-model TOML overrides into
    `src/embed/registry.rs`.
11. Task 10 (pro): `src/cortex/` module — `Intent` enum, provenance-tagged IR,
    validator → adapter (never model → shell).
12. Task 11 (pro): extend `src/agents/routing.rs` with hardware-aware routing.
13. Task 12 (pro): `src/cortex/compile.rs` — large-model result → stable rule.
14. Task 13 (pro): HUD provenance overlay in `src/bin/hud.rs`.
15. Task 14 (pro): `src/cortex/training.rs` — model-independent training signals.
16. Task 15 (pro): CPU/ROCm/CUDA microbench tables in `benchmarks/`.

## Tests you must add
- **Unit:** `DeviceInfo` parse of a synthetic `nvidia-smi`/`rocm-smi` output.
- **Integration:** `cortex_contract.rs` — text → encoder → intent/entities → IR → validator → a real `fs.read` action → HUD result, on CPU.
- **Benchmark:** one row per (model, backend, quantization) with task-success/cost as the primary metric.

## Gate
```sh
test -f physis-core/docs/plans/2026-09-20-cortex-core.md
test -f docs/plans/2026-09-20-cortex-pro.md
cargo test --features embed-onnx cortex_contract
cargo clippy --all-targets -- -D warnings
```

## Commit strategy
Branch `agent/PH-101`. Plans first (this commit), then code per task with its own
commit. Never `git add -A`.

## Report
Append `packets/reports/PH-101.md`.

## Never
- Do not fork the architecture into AMD-specific and NVIDIA-specific systems.
- Do not scatter `#[cfg(rocm)]`/`#[cfg(cuda)]` through semantic logic.
- Do not regress ROCm support, CPU operation, offline operation, deterministic
  retrieval, JEV integration, HUD/TUI, or existing model routing.
- Do not let a micro-model own shell access.
- Do not assume LFM encoder/embedder/ColBERT/decoder spaces are compatible —
  own the canonical Physis latent.
