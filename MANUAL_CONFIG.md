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
Physis Pro is best secured using an authentication proxy in front of the web server (port `8080`).
- **Recommended Proxy:** `oauth2-proxy`
- **Steps:**
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
