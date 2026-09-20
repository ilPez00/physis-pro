# PH-013 Threat Model — Tenant Isolation & Authorization

## Assets

| Asset | Description | Sensitivity |
|-------|-------------|-------------|
| Tenant nodes | Coherence nodes, goals, ledger entries, typed edges per tenant (sled keys `{uid}:{id}`) | High — per-user semantic world |
| Credentials | API keys (`PHYSIS_API_TOKEN`), license keys in `~/.physis/` | Critical — gate all routes |
| Embedder cache | Persisted embeddings in SharedCtx; shared infra, per-tenant vectors | Medium — shared compute, isolated data |
| Observation log | Per-tenant anomaly history (Maintenance struct) | Medium — machine telemetry |

## Actors

- **End-user**: sends `X-Physis-User` header; receives isolated `RuntimeState` via `with_user(uid, …)`.
- **Admin**: no header → `_default` tenant; full access to global infra (holarchy, health, pricing).
- **MCP tools**: 30 tools via `physis-pro system serve` (stdio), inherit caller's auth context, per-user state in tenant map.
- **CLI**: local machine, no auth header, runs as current OS user, can access scan root files.

## Trust Boundaries

| Boundary | Mechanism |
|----------|-----------|
| HTTP | Bearer auth middleware (`require_auth`) → `user_key(headers)` → `with_user` per route |
| MCP | stdio transport (`./target/debug/physis-pro system serve`), per-user state in tenant map |
| CLI local | No network auth, runs as OS user, can access local files (scan root) |

## 5 Worst Attacks

| # | Attack | Severity | Mitigation | Status |
|---|--------|----------|------------|--------|
| 1 | **Cross-tenant read via ID guessing**: User A requests `/api/v1/coherence/nodes/{id}` for User B's node. | High | `with_user` scopes all reads to tenant. Verified by `coherence_nodes_are_isolated_per_tenant`. | Mitigated — needs authz test assertion |
| 2 | **Cross-tenant write via ID guessing**: User A POSTs edit/delete to User B's node. | Critical | `with_user` scopes all writes. `coherence_delete_handler` uses `user_key`. Verified by `coherence_node_delete_requires_owner`. | Mitigated — needs authz test assertion |
| 3 | **Auth spoof via X-Physis-User**: Forge header to `_default` or another uid. | High | `X-Physis-User` is set by auth middleware, not client-controllable in production (bearer token resolves the uid server-side). In dev mode, the header is the uid — known tradeoff. | Mitigated — dev mode is opt-in (`PHYSIS_DEV`) |
| 4 | **Path traversal in fs_* handlers**: `fs_graph_handler`, `fs_file_handler` accept directory paths. | High | `sandbox_existing` + `scan_root` confinement; canonicalization checked. Verified by `fs_*_respect_scan_root`. | Mitigated — existing tests cover |
| 5 | **Tenant bleed via maintenance**: `maintenance_check_handler` at `industrial.rs:149` hardcodes `"_default"` for `persist_node`, so anomaly nodes are persisted under `_default` instead of the caller's tenant. | **Critical — UNPATCHED** | **FIX NOW**: change `persist_node(db, "_default", &id)` to `persist_node(db, &uid, &id)` where `uid = user_key(&headers)`. | **UNPATCHED — this is PH-013** |
