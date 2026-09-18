# physis-pro work packets

Read PROTOCOL.md first. Then the repo rules that override everything:

- **`cargo clippy --all-targets -- -D warnings` is CI.** A warning is a build failure.
- **Comments record the failure that motivated the code.** If you change code a
  comment describes, update the comment to describe the NEW reason — never delete it.
- Model weights live in `models/`; never download `models/jina-clip-v2` (it sits
  above bge-base in the fallback chain and silently swaps the primary embedder,
  orphaning every persisted node).
- Server must be launched from repo root (embedder paths are CWD-relative);
  confirm `GET /api/v1/embedder` → `{"degraded": false, "kind": "bge-base"}`.
- ONNX Runtime GPU (ROCm EP) is a dead end — do not retry blind.
- Pushing: HTTPS 403s. `git push git@github.com:ilPez00/physis-pro.git HEAD:main`
  (reviewers only; agents push branches).
- Full `cargo test` measured 790 s on omo: it is a Tier-1 gate (once per packet,
  never per iteration). Tier-0 is `cargo check --all-targets` (33 s) + clippy (21 s).

Suggested order = phase, then id. Phase meanings: 0 = now, 1 = next quarter,
2 = this year, 3 = later, D = discovery (writes future packets).

## Multiscale structure research

See [PLAN.md Phase 27](../PLAN.md#phase-27--measured-structure-of-change-2026-09-17).
MS-001 is the only initially eligible packet; MS-002–MS-008 are dependency-blocked.
Packets are prepared, not dispatched. Existing operational correctness work takes priority.
