FROM rust:1-trixie AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY config ./config
# Vendored llmwiki crate (optional `wiki-embedded` dep). In the build context so
# cargo resolves AND can compile it when wiki-embedded is enabled.
COPY vendor ./vendor
# NOTE: models/ is git-ignored (~1.5GB) and is NOT part of the build context.
# Weights are fetched at boot by docker-entrypoint.sh from MODEL_BASE_URL.
# See PREMORTEM.md P0.

RUN cargo build --release --no-default-features --features web,embed-onnx,cli,wiki-embedded,mcp --bin physis-pro-web --bin physis-pro

# Download ONNX model weights during build (avoids runtime S3 download).
# S3 creds from Railway project bucket are injected by Railway build env.
ARG AWS_S3_BUCKET
ARG AWS_S3_ENDPOINT
ARG AWS_ACCESS_KEY_ID
ARG AWS_SECRET_ACCESS_KEY
ARG AWS_DEFAULT_REGION
RUN set -e; \
    mkdir -p /app/models-build/bge-base-en-v1.5/onnx; \
    if [ -n "$AWS_S3_BUCKET" ] && [ -n "$AWS_S3_ENDPOINT" ] && [ -n "$AWS_ACCESS_KEY_ID" ]; then \
      apt-get update && apt-get install -y awscli && rm -rf /var/lib/apt/lists/*; \
      aws s3 cp "s3://$AWS_S3_BUCKET/bge-base-en-v1.5/onnx/model.onnx" \
        /app/models-build/bge-base-en-v1.5/onnx/model.onnx \
        --endpoint-url "$AWS_S3_ENDPOINT"; \
      aws s3 cp "s3://$AWS_S3_BUCKET/bge-base-en-v1.5/tokenizer.json" \
        /app/models-build/bge-base-en-v1.5/tokenizer.json \
        --endpoint-url "$AWS_S3_ENDPOINT"; \
      echo "S3 download complete"; \
      ls -lh /app/models-build/bge-base-en-v1.5/onnx/; \
    else \
      echo "No S3 creds in build env, will download at runtime"; \
    fi

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM debian:trixie-slim
ARG WITH_LEAN=0
RUN apt-get update && apt-get install -y ca-certificates curl ffmpeg awscli && rm -rf /var/lib/apt/lists/*

# Optional Lean 4 toolchain (apodeixis machine). ~500 MB; enabled with
# `docker build --build-arg WITH_LEAN=1 ...`.
RUN if [ "$WITH_LEAN" = "1" ]; then \
      echo "[runtime] installing Lean 4 toolchain (~500 MB) ..." && \
      curl https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh -sSf | sh -s -- -y --default-toolchain stable && \
      rm -rf /root/.elan/tmp && \
      echo "[runtime] Lean $(/root/.elan/bin/lean --version 2>&1 | head -1)"; \
    else \
      echo "[runtime] Lean skipped (build with --build-arg WITH_LEAN=1 to enable apodeixis)"; \
    fi

ENV LEAN_BIN=/root/.elan/bin/lean
ENV PATH="$LEAN_BIN:$PATH"

WORKDIR /app
COPY --from=builder /app/target/release/physis-pro-web .
COPY --from=builder /app/target/release/physis-pro .
COPY --from=builder /app/config ./config
COPY --from=builder /app/models-build ./models
COPY docker-entrypoint.sh .
RUN chmod +x docker-entrypoint.sh

ENV PHYSIS_HOST=0.0.0.0
ENV PHYSIS_PORT=8080
# Override to pull weights from your own bucket (Supabase Storage / S3). Defaults to HF.
# ENV MODEL_BASE_URL=https://<project>.supabase.co/storage/v1/object/public/models/jina-clip-v2
#
# Runtime env contract — set these as SECRETS in the platform (Railway/compose),
# never bake values into this image:
#   PHYSIS_API_TOKEN     required for the non-loopback bind above (P6 gate)
#   PHYSIS_RERANK=1      enable the LLM re-ranker on /classify (ambiguous cells)
#   AI_GEMINI_KEY        cascade priority-1 (gemini-2.5-flash) — the re-rank model
#   AI_GROQ_KEY          cascade priority-2 fallback (llama-3.3-70b)
# See .env.example for the full provider list. Without a provider key the
# re-ranker silently no-ops back to the vector result.

EXPOSE 8080 9876
CMD ["./docker-entrypoint.sh"]
