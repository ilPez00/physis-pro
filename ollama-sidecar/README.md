# Ollama sidecar (uncensored Gemma floor)

A standalone Railway service that runs Ollama and serves the uncensored Gemma as
the **last-resort floor** for the physis provider cascade. Kept separate so the
4B model's memory (~6GB) doesn't sit on the engine container.

## Deploy

1. New Railway service in the physis project, **root dir = `ollama-sidecar/`**,
   Dockerfile build. Give it an **8GB** plan and a **volume at `/root/.ollama`**
   (so the model is imported/pulled once).
2. Set the model source on the sidecar service:
   - **Your exact GGUF** (`models/gemma-4-E4B-it-uncensored-Q4_K_M.gguf`): host it
     once, then set ONE of
     - `GEMMA_GGUF_S3_KEY=gemma-4-E4B-it-uncensored-Q4_K_M.gguf` (+ `AWS_S3_BUCKET`,
       `AWS_S3_ENDPOINT`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`), or
     - `GEMMA_GGUF_URL=https://…/gemma-4-E4B-it-uncensored-Q4_K_M.gguf`
     - `OLLAMA_FLOOR_MODEL=gemma-uncensored`
   - **Zero-hosting alternative** (registry abliterated Gemma3 4B):
     - `OLLAMA_PULL_MODEL=huihui_ai/gemma3-abliterated:4b`
     - `OLLAMA_FLOOR_MODEL=huihui_ai/gemma3-abliterated:4b`
3. On the **physis** service, point the cascade at the sidecar:
   - `OLLAMA_FLOOR_URL=http://<sidecar-service>.railway.internal:11434/v1`
   - `OLLAMA_FLOOR_MODEL=<same tag as above>`
   - leave `ENABLE_OLLAMA` unset (no in-container Ollama).

`/v1` matters — physis talks the OpenAI-compatible API. Internal Railway DNS is
`<service>.railway.internal`; the sidecar binds `0.0.0.0:11434`.
