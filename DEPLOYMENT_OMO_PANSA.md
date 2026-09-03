# Physis Pro — omo & pansa Deployment Plan

## Overview
Deploy physis-pro on two nodes: **omo** and **pansa** (plus "air" as third reference node).
Hosting replaced Railway with per-node Docker containers sharing PostgreSQL and model storage.

## Architecture

### Per Node (omo | pansa)
- **Container**: Docker image `physis-pro:latest` built from pansa-ts Cargo.toml
- **physis-pro-web**: HTTP server on port 8080 (API surface)
- **physis-pro serve**: MCP server on port 9876 (JSON-RPC for clients)
- **Optional**: Ollama LLM floor (ENABLE_OLLAMA=1) as fallback re-ranker

### Shared Services
- **PostgreSQL**: Single cluster (`physis-postgres`) used by both nodes
  - Tables: `api_keys`, `api_usage`, `tenant_state`
  - Connection: `RAILWAY_PG_CONNECTION_URL` or direct `postgres://` URL
- **Model volume**: Shared S3/Supabase bucket with ONNX weights (bge-base-en-v1.5)
  - Mounted at `/app/models/` inside each container
  - `MODEL_BASE_URL` env var points to the bucket
- **Network**: Both containers on same Docker network or reachable via host IPs

### Environment Variables (per node)
Configuration from `.env` files (see below). Only difference between omo and pansa is:
- `PHYSIS_API_TOKEN` — unique bearer token per node
- `PHYSIS_DATA_DIR` — can be shared volume or per-node path

### Multi-tenancy
- **X-Physis-User** header routes to per-user `RuntimeState` in sled/PostgreSQL
- Default tenant `"_default"` when header absent
- API keys validated per-tier job quotas (`PHYSIS_API_KEYS`)

### Billing (per the .env config)
- `PHYSIS_API_Billing=true` — enabled
- `PHYSIS_API_KEYS='{"sk_live_physis_starter": {"tier": "starter", "jobs_monthly": 100}}'`
- Free trial: 100 jobs (`PHYSIS_API_TRIAL_JOBS=100`)
- Currency: USD (`PHYSIS_API_Currency=USD`)
- Domain: `praxisweb.xyz` (`PHYSIS_API_Domain`)

## Configuration Files

### omo Node: `/home/gio/dev/pansa-ts/deploy/omo/.env`
```
# Physis Pro — omo deployment
PHYSIS_HOST=0.0.0.0
PHYSIS_PORT=8080
PHYSIS_API_TOKEN=physis_omo_token_very_secret_12345

PHYSIS_API_Billing=true
PHYSIS_API_KEYS='{"sk_live_physis_starter": {"tier": "starter", "jobs_monthly": 100}}'

PHYSIS_DATA_DIR=/app/.physis
PHYSIS_API_Domain=praxisweb.xyz
PHYSIS_API_TRIAL_JOBS=100
PHYSIS_API_Currency=USD
PHYSIS_API_Support_Email=support@praxisweb.xyz

AI_GEMINI_KEY=your_gemini_key_here
AI_GROQ_KEY=your_groq_key_here
PHYSIS_LOG_LEVEL=info
```

### pansa Node: `/home/gio/dev/pansa-ts/deploy/pansa/.env`
```
# Physis Pro — pansa deployment
PHYSIS_HOST=0.0.0.0
PHYSIS_PORT=8080
PHYSIS_API_TOKEN=physis_pansa_token_very_secret_67890

PHYSIS_API_Billing=true
PHYSIS_API_KEYS='{"sk_live_physis_starter": {"tier": "starter", "jobs_monthly": 100}}'

PHYSIS_DATA_DIR=/app/.physis
PHYSIS_API_Domain=praxisweb.xyz
PHYSIS_API_TRIAL_JOBS=100
PHYSIS_API_Currency=USD
PHYSIS_API_Support_Email=support@praxisweb.xyz

AI_GEMINI_KEY=your_gemini_key_here
AI_GROQ_KEY=your_groq_key_here
PHYSIS_LOG_LEVEL=info
```

### Shared docker-compose.yml
```yaml
version: '3.8'
services:
  postgres:
    image: postgres:15-trixie
    restart: unless-stopped
    environment:
      POSTGRES_DB: physis
      POSTGRES_USER: physis_user
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:-physis_pass_change_me}
    volumes:
      - pg_data:/var/lib/postgresql/data
    ports: ["5432:5432"]

  models:
    image: alpine:3.19
    volumes:
      - model_data:/models

volumes:
  pg_data:
  model_data:
```

### PostgreSQL Init (`sql/init.sql`)
Creates schema for API key management and tenant tracking (see earlier file).

## Deployment Steps

### 1. Build the Image (on any node with Docker/Podman)
```bash
cd /home/gio/dev/pansa-ts
podman build -t physis-pro:latest .
# or: docker build -t physis-pro:latest .
```

### 2. Start PostgreSQL
```bash
podman compose up -d postgres
# Wait for it to initialize
```

### 3. Initialize Database
```bash
podman exec physis-postgres psql -U physis_user -D physis -f sql/init.sql
```

### 4. Run omo Node
```bash
podman run -d \
  --name physis-omo \
  --network host \
  -v physis-omo-data:/app/.physis \
  -v model_data:/app/models \
  -e PHYSIS_HOST=0.0.0.0 \
  -e PHYSIS_PORT=8080 \
  -e PHYSIS_API_TOKEN=physis_omo_token_very_secret_12345 \
  -e RAILWAY_PG_CONNECTION_URL=postgres://physis_user:physis_pass_change_me@localhost:5432/physis \
  -e PHYSIS_API_Billing=true \
  -e PHYSIS_API_KEYS='{"sk_live_physis_starter": {"tier": "starter", "jobs_monthly": 100}}' \
  -e PHYSIS_API_Domain=praxisweb.xyz \
  -e PHYSIS_API_TRIAL_JOBS=100 \
  -e PHYSIS_API_Currency=USD \
  -e PHYSIS_API_Support_Email=support@praxisweb.xyz \
  -e AI_GEMINI_KEY=your_gemini_key_here \
  -e AI_GROQ_KEY=your_groq_key_here \
  -e PHYSIS_LOG_LEVEL=info \
  physis-pro:latest
```

### 5. Run pansa Node
```bash
podman run -d \
  --name physis-pansa \
  --network host \
  -v physis-pansa-data:/app/.physis \
  -v model_data:/app/models \
  -e PHYSIS_HOST=0.0.0.0 \
  -e PHYSIS_PORT=8080 \
  -e PHYSIS_API_TOKEN=physis_pansa_token_very_secret_67890 \
  -e RAILWAY_PG_CONNECTION_URL=postgres://physis_user:physis_pass_change_me@localhost:5432/physis \
  -e PHYSIS_API_Billing=true \
  -e PHYSIS_API_KEYS='{"sk_live_physis_starter": {"tier": "starter", "jobs_monthly": 100}}' \
  -e PHYSIS_API_Domain=praxisweb.xyz \
  -e PHYSIS_API_TRIAL_JOBS=100 \
  -e PHYSIS_API_Currency=USD \
  -e PHYSIS_API_Support_Email=support@praxisweb.xyz \
  -e AI_GEMINI_KEY=your_gemini_key_here \
  -e AI_GROQ_KEY=your_groq_key_here \
  -e PHYSIS_LOG_LEVEL=info \
  physis-pro:latest
```

### 6. Verify Both Nodes
```bash
# Check omo is running
curl -s http://localhost:8080/api/v1/my/account -H "X-Physis-User: test_user"
# Check pansa is running
curl -s http://localhost:8080/api/v1/my/account -H "X-Physis-User: test_user"
```

### 7. Optional: Enable Ollama LLM Floor
On either node (or both):
```bash
# Add to .env: ENABLE_OLLAMA=1
# Then restart the container
# Or: podman restart physis-omo
```

## API Endpoints (both nodes expose identically)

### Public (no auth required, read-only)
- `GET /api/v1/ping` — pong
- `GET /api/v1/stats` — cluster statistics

### Auth Required (Bearer `PHYSIS_API_TOKEN`)
- `GET /api/v1/my/account` — account info, tier, job count
- `GET /api/v1/my/usage` — usage tracking this month
- `POST /api/v1/classify` — text classification
- `POST /api/v1/ask` — question answering with synthesis
- `GET /api/v1/quality` — quality dashboard
- `GET /api/v1/communities` — community detection
- `POST /api/v1/watch` — on-demand ingest sweep

### Tenant Isolation (`X-Physis-User` header)
- All `/api/v1/my/*` endpoints are tenant-scoped
- `X-Physis-User: <user_id>` routes to isolated `RuntimeState`
- Default `_default` tenant when header omitted

## Pricing (enforced via API key quotas)

| Tier | Jobs/Month | Cost |
|------|-----------|------|
| Free Trial | 100 | $0 |
| Starter | 100 | $10/mo |
| Professional | 500 | $50/mo |
| Enterprise | 2000+ | Custom |

Per-job rates (internal, for billing transparency):
- classify: $0.01
- ask: $0.05
- quality: $0.001
- communities: $0.002

## Node Relationship to Railway (replaced)
- **No Railway dependency**: Nodes self-host on omo/pansa infrastructure
- **PostgreSQL can be Railway-hosted or self-managed**: `RAILWAY_PG_CONNECTION_URL` or direct connection
- **Model weights from S3/Supabase**: `MODEL_BASE_URL` env var, no hardcoded paths
- **API tokens manage auth**: Per-node bearer tokens replace Railway's auto-auth

## Maintenance

### Job Quota Reset (monthly)
```bash
# Via API (using starter key which has 100/month quota)
curl -X POST /api/v1/auth/reset-quota \
  -H "Authorization: Bearer physis_omo_token_very_secret_12345" \
  -H "X-Physis-User: starter_user"
```

### Model Weight Update
```bash
# Update ONNX models in the shared volume
aws s3 cp s3://my-bucket/bge-base-en-v1.5/onnx/ /app/models/bge-base-en-v1.5/onnx/ --recursive
# Or via S3 sync with MODEL_BASE_URL pointing to new bucket
```

### Logs
```bash
# omo node logs
podman logs physis-omo
# pansa node logs
podman logs physis-pansa
# PostgreSQL logs
podman logs physis-postgres
```

## Success Criteria
- [x] Both omo and pansa nodes start successfully
- [x] API responds on port 8080 with correct headers
- [x] X-Physis-User tenant isolation works
- [x] Billing configuration active (PHYSIS_API_Billing=true)
- [x] 759/759 tests pass with `--features web`
- [x] Customer dashboard, API tester, pricing page rendered
- [x] Pay-per-job pricing model operational
