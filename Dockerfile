FROM rust:1-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY config ./config
# NOTE: models/ is git-ignored (~1.5GB) and is NOT part of the build context.
# Weights are fetched at boot by docker-entrypoint.sh from MODEL_BASE_URL.
# See PREMORTEM.md P0.

RUN cargo build --release --no-default-features --features web --bin physis-pro-web

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/physis-pro-web .
COPY --from=builder /app/config ./config
COPY docker-entrypoint.sh .
RUN chmod +x docker-entrypoint.sh

ENV PHYSIS_HOST=0.0.0.0
ENV PHYSIS_PORT=8080
# Override to pull weights from your own bucket (Supabase Storage / S3). Defaults to HF.
# ENV MODEL_BASE_URL=https://<project>.supabase.co/storage/v1/object/public/models/jina-clip-v2

EXPOSE 8080
CMD ["./docker-entrypoint.sh"]
