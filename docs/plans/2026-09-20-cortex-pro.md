# Physis Cortex — physis-pro plan

> Docs only. No code, no file edits outside plans. Ponytail: reuse what exists.

## 0. What already exists (measured)

| Need | Existing | Verdict |
|---|---|---|
| Embedder registry | `src/embed/registry.rs` `EmbedderRegistry` | reuse — add a backend field, do not rebuild |
| Intent IR | `src/intent_ir.rs` `IntentIR` (9 slots) | reuse — the typed IR already exists |
| Action registry | `src/registry.rs` + `src/capability.rs` | reuse — built, unwired (S0) |
| Routing ladder | `src/agents/routing.rs` `RouteRequest`/`Decision` | reuse — extend with hardware + confidence |
| Semantic delta | `src/semantic_delta/mod.rs` | reuse — training-signal source |
| HUD | `src/bin/hud.rs` `parse_hud_input` | reuse — provenance overlay |
| JEV | `src/ai/provider.rs` cascade | reuse — escalation leg |
| Fleet | `src/machines/`, `src/industrial.rs` | reuse — `ComputeNode` shape |
| ROCm note | packets/README.md: ONNX Runtime GPU (ROCm EP) is a dead end | preserve — do not retry blind |

## 1. Tasks

### Task 1 — backend wiring
- Modify: `src/embed/registry.rs` — add `backend: BackendKind` per slot.
- Env: `PHYSIS_BACKEND=auto|cpu|rocm|cuda`.
- Per-model override in `config/` TOML.

### Task 2 — typed resolver
- Modify: `src/intent_ir.rs` — add `compiled_from: provenance enum {D,E,R,C,mu,L,J,A,H}`.
- Create: `src/cortex/` module with `Intent` enum (Query/Action/Navigate/Edit/Remember/Compare/Explain/Escalate).
- Never let a micro-model own shell access: model → typed capability → validator → adapter.

### Task 3 — router
- Modify: `src/agents/routing.rs` — `RoutingDecision{selected_backend, confidence, estimated_cost, risk, reason}`.
- GPU-aware: CUDA/ROCm placement, memory pressure → move to CPU, mmap fallback.

### Task 4 — stable-configuration compilation
- Create: `src/cortex/compile.rs` — large-model result → validated pattern → semantic rule.
- Reuse `src/decisions.rs` `fold_episodes` + `src/discovery.rs`.

### Task 5 — HUD provenance
- Modify: `src/bin/hud.rs` — overlay showing INPUT → ENCODER → RETRIEVAL → GRAPH → RESULT with provenance letters.

### Task 6 — training signal reuse
- Create: `src/cortex/training.rs` — emit model-independent JSON from corrections/executions/undo/deltas.

### Task 7 — microbench
- Extend `benchmarks/` with CPU/ROCm/CUDA comparison.

## 2. Gate
`cargo check --all-targets` + `cargo clippy --all-targets -- -D warnings`
