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

RUN cargo build --release --no-default-features --features web,embed-onnx,cli,wiki-embedded --bin physis-pro-web --bin physis-pro

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
RUN apt-get update && apt-get install -y ca-certificates curl ffmpeg awscli && rm -rf /var/lib/apt/lists/*

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

EXPOSE 8080
CMD ["./docker-entrypoint.sh"]
