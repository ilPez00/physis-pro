# Physis Pro Deployment & Configuration Guide

This guide covers manual configuration and deployment for the Physis Pro engine.

## 1. Prerequisites
- **Rust/Cargo:** Required for building the engine.
- **Docker:** Required for containerized deployment.
- **Railway/Vercel/Supabase:** Configured as needed for your specific infrastructure stack.

## 2. Configuration Setup
Physis Pro relies on `PhysisProConfig` defined in `config/`.
- Ensure `config/praxis_ontology.json` (Human) and `config/machine_ontology.json` (Machine) are present.
- Default configuration is loaded from `.env` or system defaults.

## 3. Data Persistence
The engine persists ontological state and quality logs to a local directory:
- **Default path:** `~/.physis/`
- **For Docker/Railway:** Define the `PHYSIS_DATA_DIR` environment variable and mount a persistent volume to that directory.

## 4. Authentication (Railway/Production)

### 4.1 Built-in Bearer token (required for public binds)
The server is **fail-closed**: when `PHYSIS_HOST` is non-loopback (e.g. `0.0.0.0`
on Railway), it **refuses to start** unless `PHYSIS_API_TOKEN` is set. This stops
an accidental deploy from publishing an unauthenticated API that also proxies
paid LLM providers (`/api/v1/chat`) — an open relay.

- Set `PHYSIS_API_TOKEN` to a strong random value: `openssl rand -hex 32`.
- All routes except `/health` and `/api/v1/embedder` then require:
  `Authorization: Bearer <token>`. Missing/wrong token → `401`.
- `/api/v1/scan` is confined to `PHYSIS_SCAN_ROOT` (default `/app/.physis`);
  a path resolving outside it → `403` (path-traversal guard).

### 4.2 Optional auth proxy (defense in depth)
For SSO/user identity, also front the server with `oauth2-proxy`:
  1. Configure `oauth2-proxy` as a sidecar service on Railway.
  2. Map the upstream to your Physis Pro internal service URL.
  3. Provide `OAUTH_CLIENT_ID`, `OAUTH_CLIENT_SECRET`, and `OAUTH_COOKIE_SECRET` via environment variables.

## 5. Model Loading
The engine is pre-bundled with Jina CLIP v2 models.
- **Directory:** `/app/models/jina-clip-v2`
- **Entrypoint:** The `docker-entrypoint.sh` automatically detects and loads these models upon container start.

## 6. Build & Run
```bash
# Build binary
cargo build --release --features web --bin physis-pro-web

# Build docker image
docker build -t physis-pro .

# Run container
docker run -p 8080:8080 -v /path/to/persistent/data:/app/.physis physis-pro
```
