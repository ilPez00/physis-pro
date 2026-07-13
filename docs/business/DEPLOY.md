# Deploying Physis on-prem (Ontology Studio server)

Physis runs as a single Rust binary (`web_server`) — no external database, no
cloud dependency. This is the deployable product surface: drop it on a box
inside the buyer's LAN and point it at their documents.

## What ships

- `web_server` binary (features: `web` + optional `pdf`/`email`)
- Role-aware bearer-token auth (viewer / editor / admin) via `src/rbac.rs`
- Studio ingestion + discovery + typed graph + provenance, all reachable over HTTP

## Build

```bash
cargo build --release --features web,pdf,email --bin web_server
```

## Tokens & RBAC

Auth is role-aware. Issue least-privilege tokens:

| Env var | Format | Use |
|---------|--------|-----|
| `PHYSIS_API_TOKEN` (legacy) | flat secret | auto-registered as **editor** (back-compat) |
| `PHYSIS_TOKENS` | `"role:secret"` space/comma list | viewer/editor/admin tokens |

Roles:
- **viewer** — GET only (read ontology, semiotic, stats). Safe to hand to a contractor.
- **editor** — POST/PUT/DELETE (ingest, scan, goals, chat). Day-to-day operator.
- **admin** — also `/api/v1/config/*` and `/api/v1/axes/promote`. The install owner.

A non-loopback bind **without any token is refused at startup** (no open relay).
Loopback (`127.0.0.1`) with no token = dev mode, warnings logged.

Example:

```bash
export PHYSIS_HOST=0.0.0.0
export PHYSIS_PORT=19876
export PHYSIS_TOKENS="admin:$(openssl rand -hex 16) editor:$(openssl rand -hex 16) viewer:$(openssl rand -hex 16)"
target/release/web_server
```

## One-shot installer

`scripts/install-physis.sh` builds, generates a token table, and installs a
systemd `--user` unit (no root needed). If `caddy` is present it also writes a
LAN-only Caddyfile (TLS on `physis.local` via mDNS — no public DNS, no egress).

```bash
./scripts/install-physis.sh
```

## Verify

```bash
# health (public)
curl -fsS http://localhost:19876/health

# viewer token: GET works
curl -fsS -H "Authorization: Bearer $VIEWER" http://localhost:19876/api/v1/ontology/list

# viewer token: POST rejected (403)
curl -s -o /dev/null -w "%{http_code}\n" -X POST -H "Authorization: Bearer $VIEWER" \
  -H 'content-type: application/json' -d '{"input":"x"}' http://localhost:19876/api/v1/goals

# editor token: POST works (2xx)
curl -s -o /dev/null -w "%{http_code}\n" -X POST -H "Authorization: Bearer $EDITOR" \
  -H 'content-type: application/json' -d '{"input":"x"}' http://localhost:19876/api/v1/goals
```

## Why this is deployable today

- Zero external services; state is local (sled on the data dir).
- Offline by default; the only outbound calls are the LLM provider cascade in
  `/api/v1/chat`, gated behind the same bearer auth (never an open relay).
- Reproducible install via one script; tokens are opaque, role-scoped, constant-time compared.
