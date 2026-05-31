FROM rust:1.82-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY config ./config

RUN cargo build --release --no-default-features --features web --bin physis-pro-web

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/physis-pro-web .
COPY --from=builder /app/config ./config

ENV PHYSIS_HOST=0.0.0.0
ENV PHYSIS_PORT=8080

EXPOSE 8080
CMD ["./physis-pro-web"]
