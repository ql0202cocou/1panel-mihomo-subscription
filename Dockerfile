# ─── Web Build Stage ────────────────────────────────────────────────────────
FROM node:20-slim AS web

WORKDIR /web

# Cache npm install on the lockfile.
COPY web/package.json web/package-lock.json ./
RUN npm ci

COPY web/ ./
RUN npm run build

# ─── Rust Build Stage ───────────────────────────────────────────────────────
FROM rust:1.90-slim AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Cache dependencies with a stub binary.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# Build the actual application. migrations/ must be present because
# sqlx::migrate! embeds them at compile time.
COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs && cargo build --release

# ─── Runtime Stage ────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update \
    && apt-get install -y ca-certificates wget \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/mihomo-subscription /app/mihomo-subscription
# Built SPA assets served by Axum (see WEB_DIR).
COPY --from=web /web/dist /app/web/dist

RUN useradd -r -s /bin/false appuser && \
    mkdir -p /data && \
    chown appuser:appuser /data

USER appuser

VOLUME ["/data"]

ENV PORT=8080
ENV DATA_DIR=/data
ENV RUST_LOG=info
ENV WEB_DIR=/app/web/dist

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -qO- http://localhost:8080/health || exit 1

CMD ["/app/mihomo-subscription"]
