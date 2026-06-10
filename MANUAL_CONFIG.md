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

## 4.3 LLM provider cascade + resilience floor

`/api/v1/chat` runs `ProviderCascade::from_env` — keyed providers first, then
keyless public Ollama nodes. Praxis's `AICoachingService` falls back to this when
its own ("axiom") keys are exhausted. Two always-on floors keep the cascade alive
when paid keys are down:

### Groq (recommended primary floor)
Free, fast, reliable. Set in the Railway dashboard:
- `AI_GROQ_KEY=<groq key>` — cascade **priority 2**, model `llama-3.3-70b-versatile`.

### Self-hosted Ollama (guaranteed local floor)
Cascade provider `ollama_floor` (`http://localhost:11434`). **Off by default** —
a 2B model needs ~3GB RAM, so size the service first. Two ways to run it:

**A. In-container, registry model (simplest):** set on the physis service —
- `ENABLE_OLLAMA=1`
- `OLLAMA_FLOOR_MODEL=gemma2:2b` (the tag the entrypoint pulls AND the cascade
  requests — keep them identical)

`docker-entrypoint.sh` installs Ollama, runs `ollama serve`, and pulls the model
in the background (cached on the `/app/.physis` volume, so the pull happens once).
`/health` passes during the download.

**A2. In-container, custom uncensored Gemma GGUF** (`models/gemma-4-E4B-it-
uncensored-Q4_K_M.gguf`, Gemma 3n E4B, 2.4GB — needs the **8GB** service, ~6GB
peak). Not on the Ollama registry, so ship the GGUF and `ollama create` it. Set —
- `ENABLE_OLLAMA=1`
- `OLLAMA_FLOOR_MODEL=gemma-uncensored`
- **one** GGUF source:
  - `GEMMA_GGUF_S3_KEY=gemma-4-E4B-it-uncensored-Q4_K_M.gguf` — upload the file to
    the **same bucket** the ONNX weights come from (`AWS_S3_*` build vars); the
    entrypoint `aws s3 cp`s it, **or**
  - `GEMMA_GGUF_URL=https://…/gemma-4-E4B-it-uncensored-Q4_K_M.gguf` — any HTTPS.

The entrypoint downloads the GGUF to the volume (once), writes a Gemma-templated
Modelfile, and runs `ollama create gemma-uncensored`. `/health` passes during the
download/import. The 26B "heretic" GGUF (9.9GB) does **not** fit 8GB — use E4B.

**B. Sidecar (recommended for prod):** run the official `ollama/ollama` image as
its own Railway service, pull the model there, and on the physis service set —
- `OLLAMA_FLOOR_URL=http://<ollama-service>.railway.internal:11434/v1`
- `OLLAMA_FLOOR_MODEL=gemma2:2b`

Isolates the model's memory from the engine. Leave `ENABLE_OLLAMA` unset.

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
