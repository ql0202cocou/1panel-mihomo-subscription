# ─── Build Stage ──────────────────────────────────────────────────────────────
FROM rust:1.78-slim AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# Build the actual application
COPY src ./src
RUN touch src/main.rs && cargo build --release

# ─── Runtime Stage ────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update \
    && apt-get install -y ca-certificates wget \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/mihomo-subscription /app/mihomo-subscription

RUN useradd -r -s /bin/false appuser && \
    mkdir -p /data && \
    chown appuser:appuser /data

USER appuser

VOLUME ["/data"]

ENV PORT=8080
ENV DATA_DIR=/data
ENV RUST_LOG=info

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -qO- http://localhost:8080/health || exit 1

CMD ["/app/mihomo-subscription"]
