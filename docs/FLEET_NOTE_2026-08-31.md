# Fleet note — 2026-08-31

## GPU uncensored-model serving (omo)
- colibri `qwen36` engine built with **HIP/ROCm** for omo's RX 6650 XT (gfx1032) — proven with
  `backend_cuda_test` exit 0 (q8/q4/q2/f32/e8 kernels on-GPU).
- Model: `wangzhang/Qwen3.6-35B-A3B-abliterated` (uncensored/abliterated) downloading to omo
  `scambio` volume `/mnt/29F19CF06CA44E9F/models/`. Serves through colibri's OpenAI shim.
- nbommo GPU relief = route local-LLM to the GPU backend (colibri olmoe engine is CPU-only).
- pansa RX 550 (gfx803): gio needs `render` group membership (sudo) to use HIP; ROCm 5.7.1
  toolchain already present in `/home/gio/bookworm` chroot and works for gfx803.

## Cross-machine memory merge (hermes-expert)
- New skill `hermes-expert` + `tools/agent-memory-bridge/agent-memory-bridge.sh` collect +
  merge harness memories (hermes/claude/opencode) across the fleet into a durable knowledge
  corpus in the **randomesh** repo (`docs/knowledge/FLEET_BRAIN.md`).
- Source bundles: pansa, omo, feather (air unreachable at collection time).

## Randomesh knowledge
- Interactive digest committed: `github.com/ilPez00/randomesh` → `docs/knowledge/FLEET_BRAIN.md`
  (MACHINES / HOSTING / PROJECTS / THE USER / WORKFLOW), plus per-host raw memory dumps.
- Repo-scope: keeps the fleet hosting brain durable + agent-readable on any box.