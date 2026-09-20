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
Standing as of **2026-09-20 (later the same day)**: MS-001 is **done**
(independently reviewed, accepted with **amendment A1** — see below). MS-002 is
**done** (independently reviewed, report-only accepted; its runnable mutation
matrix is carried by the new **`MS-002-harness`** packet, `todo`). MS-003 was
reviewed and **returned to `todo`**: its report describes the `Transform::apply`
acceptance gate wrongly and misquotes PH-051 (see
[`reports/MS-003.md`](reports/MS-003.md) §"Verification note"). MS-004 through
MS-008 remain dependency-blocked, and MS-005 additionally waits on MS-003's
corrected relation-family baseline. Packets are prepared, not dispatched.
Existing operational correctness work takes priority.

**A1 — binding on MS-002/MS-003.** The MS-001 report called
`evaluate_mutation` "deterministic given base nodes/hypotheses/edges +
mutation". Values yes, order no: `EvaluationContext::find_neighbors`
(`physis-core/src/delta_engine.rs:252-298`) returns HashSet iteration order with
no sort, so the DFS wave's `OntologyDeltaReport.node_deltas` order — and
`net_coherence_delta`'s float summation order — is unspecified. Compare
`node_deltas` as a set sorted by `node_id`, and floats to epsilon 1e-6, never
bit-exact or positionally. Evidence in
[`reports/MS-001.md`](reports/MS-001.md) §"Verification note (2026-09-20)".
