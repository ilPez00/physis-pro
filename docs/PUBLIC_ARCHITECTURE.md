# Physis Public Architecture (cross-repo)

How the physis engine becomes reachable on the public internet, across three
repositories and two hosts. Kept identical in both `physis-pro` and
`praxis_webapp` (`docs/PHYSIS_PUBLIC_ARCHITECTURE.md`) — update both when this
changes.

## The three hops

```
 Browser / API client
        │  https://praxisweb.xyz/api/physis/v1/<engine-path>
        ▼
┌───────────────────────────────────────────────────────────────┐
│ 1. Vercel (static SPA)            repo: praxis_webapp/client   │
│    praxisweb.xyz                                               │
│    client/vercel.json rewrites:                               │
│      /api/:path*  →  praxis backend (Railway)                 │
│      /(.*)        →  /index.html   (SPA catch-all)            │
└───────────────────────────────┬───────────────────────────────┘
        │  /api/physis/v1/<engine-path>
        ▼
┌───────────────────────────────────────────────────────────────┐
│ 2. Praxis backend (Express 5)     repo: praxis_webapp          │
│    web-production-646a4.up.railway.app                         │
│    app.use('/api/physis', generalLimiter, physisRoutes)       │
│    physisRoutes: PROXY_ALLOW allowlist + RegExp catch-all     │
│    proxyEngine() injects PHYSIS_API_TOKEN + X-Physis-User     │
└───────────────────────────────┬───────────────────────────────┘
        │  /api/v1/<engine-path>   (Bearer token, internal)
        ▼
┌───────────────────────────────────────────────────────────────┐
│ 3. Physis engine (Rust/axum)      repo: physis-pro            │
│    angelic-embrace-production-61dd.up.railway.app             │
│    binds 0.0.0.0:8080 · /health public · /api/v1/* protected │
└───────────────────────────────────────────────────────────────┘
```

## Repos & hosts

| Layer | Repo | Dir | Host | Notes |
|------|------|-----|------|-------|
| Frontend SPA | `praxis_webapp.git` | `praxis_webapp/client` | Vercel | `praxisweb.xyz`. `client/vercel.json` rewrites `/api/*` to the backend. |
| Backend API | `praxis_webapp.git` | `praxis_webapp` (own nested git) | Railway | `web-production-646a4.up.railway.app`. origin=praxis_webapp.git is live; `backend` remote (praxis-backend.git) is a **stale duplicate** — do not push there. |
| Engine | `physis-pro.git` | `physis_pro` | Railway | tracked remote is `physis-pro/main`, **not** `origin` (=physis-global). Dockerfile build. |

## Public path contract

```
praxisweb.xyz/api/physis/v1/<engine-path>   →   engine  /api/v1/<engine-path>
```

Examples:
- `GET  /api/physis/v1/oee/plant`
- `GET  /api/physis/v1/oee/press-01`
- `POST /api/physis/v1/oee/event`
- `POST /api/physis/v1/maintenance/check`
- `POST /api/physis/v1/classify`

The token is injected **server-side** by the backend (`physisProClient`), so it
is never exposed to the browser.

## Allowlist = product surface

The generic proxy forwards only `PROXY_ALLOW` prefixes
(`praxis_webapp/src/routes/physisRoutes.ts`). It deliberately **excludes**:

| Blocked | Why |
|---------|-----|
| `/chat`, `/providers` | paid LLM relay — would be an open relay / cost leak |
| `/scan` | filesystem access on the engine host |
| `/config/*` | mutates live server config |
| `/holarchy/ingest`, `/holarchy/edge` | global graph writes |

Adding a prefix to `PROXY_ALLOW` is the only step needed to make a new engine
endpoint public.

## Deploy chain

`git push` each repo → Railway auto-builds (remote compile; do **not** build the
Rust engine locally — slow). Frontend (Vercel) needs no redeploy for backend-only
changes because the `/api/*` rewrite is static.

1. **Engine** — `physis_pro$ git push physis-pro main` (Rust + ONNX, ~10 min).
2. **Backend** — `praxis_webapp$ git push origin main` (Node, ~3 min).
3. Verify — `physis_pro/scripts/deploy_verify.sh` (hits the public proxy).

Readiness signal: `praxisweb.xyz/api/physis/v1/oee/plant` returns JSON with
`machine_count` (requires **both** the new backend proxy and the new engine route).

## Monetization seam

The `/api/physis` mount is currently `generalLimiter` (public, rate-limited).
To gate + bill, swap that single mount in `praxis_webapp/src/app.ts`:

```ts
// per-user metering (infra already in the backend):
app.use('/api/physis', authenticateToken, axiomGate('physis_api'), physisRoutes);
// or subscription gate:
app.use('/api/physis', ...requirePro, physisRoutes);
```

`proxyEngine` already forwards the authenticated `req.user.id` as the engine
tenant (`X-Physis-User`), so per-user state + metering line up.
