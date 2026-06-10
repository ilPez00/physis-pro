#!/bin/sh
# Boot ollama, then load the floor model. Two sources:
#   - custom GGUF (the uncensored Gemma) via GEMMA_GGUF_URL or GEMMA_GGUF_S3_KEY
#     → imported with `ollama create` under $OLLAMA_FLOOR_MODEL (default
#     gemma-uncensored). File cached on the volume so the import happens once.
#   - else a registry pull of $OLLAMA_PULL_MODEL (e.g. an abliterated Gemma3 4b).
# Loading is backgrounded; `ollama serve` stays in the foreground as PID 1 so the
# service is up immediately (model becomes available once the import finishes).
set -e

MODEL="${OLLAMA_FLOOR_MODEL:-gemma-uncensored}"
DATA="${OLLAMA_MODELS:-/root/.ollama}"
# Source GGUF goes on EPHEMERAL disk, not the volume: `ollama create` copies the
# weights into its blob store ($DATA, the volume), so keeping the source there too
# would need ~2x the space. The volume only needs to hold the final blob.
GGUF_DIR="/tmp/gguf"
GGUF="$GGUF_DIR/floor.gguf"

load_model() {
  # wait for the server
  for _ in $(seq 1 60); do
    ollama list >/dev/null 2>&1 && break
    sleep 1
  done

  # Already imported on a prior boot (persisted on the volume)? Skip everything.
  if ollama list 2>/dev/null | awk '{print $1}' | grep -qx "$MODEL"; then
    echo "[ollama-sidecar] model already present, skipping load: $MODEL" >&2
    return
  fi

  # Reaching here means the model isn't loaded — clear any orphaned partial
  # blobs/manifests a previous failed `create`/`pull` left on the volume (else a
  # half-written blob keeps the small volume full and every retry hits ENOSPC).
  rm -rf "$DATA"/models/blobs/* "$DATA"/models/manifests/* 2>/dev/null || true

  # Registry pull takes precedence: it streams the blob once (fits a small
  # volume), vs `ollama create` from a GGUF which needs ~2x transiently. So when
  # OLLAMA_PULL_MODEL is set it wins even if a stale GEMMA_GGUF_URL is still around.
  if [ -n "${OLLAMA_PULL_MODEL:-}" ]; then
    echo "[ollama-sidecar] pulling $OLLAMA_PULL_MODEL" >&2
    ollama pull "$OLLAMA_PULL_MODEL" && echo "[ollama-sidecar] ready: $OLLAMA_PULL_MODEL" >&2 \
      || echo "[ollama-sidecar] pull failed" >&2
  elif [ -n "${GEMMA_GGUF_URL:-}" ] || [ -n "${GEMMA_GGUF_S3_KEY:-}" ]; then
    mkdir -p "$GGUF_DIR"
    if [ ! -s "$GGUF" ]; then
      if [ -n "${GEMMA_GGUF_S3_KEY:-}" ] && [ -n "${AWS_S3_BUCKET:-}" ]; then
        echo "[ollama-sidecar] fetching GGUF from s3://$AWS_S3_BUCKET/$GEMMA_GGUF_S3_KEY" >&2
        aws s3 cp "s3://$AWS_S3_BUCKET/$GEMMA_GGUF_S3_KEY" "$GGUF" \
          ${AWS_S3_ENDPOINT:+--endpoint-url "$AWS_S3_ENDPOINT"}
      else
        echo "[ollama-sidecar] fetching GGUF from $GEMMA_GGUF_URL" >&2
        curl -fL "$GEMMA_GGUF_URL" -o "$GGUF"
      fi
    fi
    if [ -s "$GGUF" ]; then
      printf 'FROM %s\nTEMPLATE """<start_of_turn>user\n{{ .Prompt }}<end_of_turn>\n<start_of_turn>model\n"""\nPARAMETER stop "<end_of_turn>"\n' \
        "$GGUF" > "$DATA/Modelfile"
      echo "[ollama-sidecar] ollama create $MODEL" >&2
      if ollama create "$MODEL" -f "$DATA/Modelfile"; then
        echo "[ollama-sidecar] ready: $MODEL" >&2
        rm -f "$GGUF"
      else
        echo "[ollama-sidecar] create failed" >&2
      fi
    else
      echo "[ollama-sidecar] GGUF download failed" >&2
    fi
  fi
}

load_model &
exec ollama serve
