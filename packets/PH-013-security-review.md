---
id: PH-013
track: physis
phase: 2
status: todo
rung: 3
depends: [PH-005]
gate: "cargo test --test http_surface authz && test -f packets/reports/PH-013-threat-model.md"
commit: "branch agent/PH-013; threat model commit, then fixes, then tests"
---

# Tenant isolation & authz: threat model + enforcement tests

## Why
181 HTTP routes, 46 CLI commands, 29 MCP tools — every tenant-scoped handler
must use AppCtx::with_user(uid, |rt| …); shared immutable infra stays in
SharedCtx. One handler that reads cross-tenant is a breach. Nothing currently
proves the invariant holds on main.

## Steps
1. RUNG 3: write packets/reports/PH-013-threat-model.md — assets (tenant
   nodes, credentials, embedder cache), actors, trust boundaries (HTTP, MCP,
   CLI local), and the 5 worst attacks. Keep it one page.
2. Mechanical audit: grep every route handler; classify tenant-scoped vs
   shared; any handler not using with_user gets a finding row in the report.
3. Fix findings. Add a per-route authz test matrix in tests/http_surface.rs:
   user A cannot read/write/see user B's nodes; unauthenticated gets 401 not
   500; tenant id in path is ignored in favour of the token's.
4. Add the matrix to CI docs so future routes copy the pattern.

## Tests you must add
- **Integration (the matrix):** at minimum: cross-tenant read denied,
  cross-tenant write denied, cross-tenant LIST filtered, auth spoof via header
  denied, one positive control (same-tenant works).
- **Unit:** the AppCtx helper: with_user nesting, user-less context rejected.

## Gate
```bash
cargo clippy --all-targets -- -D warnings
cargo test --test http_surface authz
cargo test
```

## Never
- Fix an authz bug by hiding the route. Fix the check. Report anything you
  cannot fix as a finding with severity — that is a deliverable.
