# PH-013 Threat Model — Tenant Isolation & Authorization

## Assets

| Asset | Description | Sensitivity |
|-------|-------------|-------------|
| Tenant nodes | Coherence nodes, goals, ledger entries, and typed edges, keyed by tenant in sled | High — per-user semantic world |
| Credentials | API keys (`PHYSIS_API_TOKEN` / `PHYSIS_TOKENS`) and license material | Critical — gate protected routes |
| Embedder cache | Shared embedder/cache infrastructure carrying tenant vectors | Medium — shared compute, tenant-scoped data |

## Actors

- **End-user**: supplies `X-Physis-User`; `AppCtx::with_user` selects the tenant runtime.
- **Admin**: omits the user header and therefore uses `_default`; has the deployment’s admin privileges.
- **MCP tools**: 30 tools through the Physis MCP stdio server; inherit the caller’s tenant/auth context.
- **CLI local**: runs as the local OS user with no network auth; can access files allowed by that user and `scan_root`.

## Trust Boundaries

| Boundary | Mechanism |
|----------|-----------|
| HTTP | Bearer auth (`require_auth`), then `user_key(headers)` and `with_user` for tenant state |
| MCP | stdio transport configured by `.mcp.json`; local caller trust and per-user runtime state |
| CLI local | No network authentication; OS permissions and `scan_root` confinement are the boundary |

## 5 Worst Attacks

| # | Attack | Severity | Mitigation / status |
|---|--------|----------|---------------------|
| 1 | **Cross-tenant read**: User A guesses User B’s node ID and requests it. | High | Tenant-scoped reads through `with_user`; covered by `authz_cross_tenant_read_denied`. |
| 2 | **Cross-tenant write**: User A guesses User B’s node ID and edits/deletes it. | Critical | Tenant-scoped writes; covered by `authz_cross_tenant_write_denied`. |
| 3 | **Auth spoof**: Forge `X-Physis-User` to become `_default` or another user. | High | Production must bind the header to authenticated identity at the HTTP edge; dev mode intentionally treats the header as the uid. Residual deployment risk if a proxy forwards a client-controlled header. |
| 4 | **Path traversal**: `fs_*` handlers escape `scan_root`. | High | Canonicalize and require paths to remain under `scan_root`; existing filesystem confinement tests cover this boundary. |
| 5 | **Tenant bleed via maintenance**: `maintenance_check_handler` persisted anomaly nodes under hardcoded `_default`. | Critical | **Patched**: extract `uid = user_key(&headers)` and persist with `s.core.persist_node(db, &uid, &id)` (`src/web/industrial.rs:150-151`). |
