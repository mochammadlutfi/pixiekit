# syntax=docker/dockerfile:1.7
# ---------------------------------------------------------------------------
# Pixiekit Rust workspace — multi-stage build
#
# Build all workspace binaries with cargo-chef dependency caching, then ship
# minimal Debian-slim runtime images per binary. Pick a target with:
#
#   docker build --target web-api -t pixiekit/web-api .
#   docker build --target cli     -t pixiekit/cli     .
#   docker build --target mcp     -t pixiekit/mcp     .
#
# Override Rust toolchain via:
#   docker build --build-arg RUST_VERSION=1.85 ...
# ---------------------------------------------------------------------------
ARG RUST_VERSION=1.95
ARG DEBIAN_VERSION=bookworm

# ---- chef base ------------------------------------------------------------
FROM rust:${RUST_VERSION}-slim-${DEBIAN_VERSION} AS chef
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        ca-certificates \
 && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked --version ^0.1
WORKDIR /app

# ---- planner: compute dependency recipe ----------------------------------
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
RUN sed -i '/"apps\/web\/src-tauri",/d' Cargo.toml
COPY crates ./crates
RUN cargo chef prepare --recipe-path recipe.json

# ---- builder: cache deps, then build all workspace binaries --------------
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,id=pixiekit-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=pixiekit-cargo-git,target=/usr/local/cargo/git \
    cargo chef cook --release --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
RUN sed -i '/"apps\/web\/src-tauri",/d' Cargo.toml
COPY crates ./crates
RUN --mount=type=cache,id=pixiekit-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=pixiekit-cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=pixiekit-target,target=/app/target \
    cargo build --release --workspace --bins --exclude pixiekit-gui \
 && mkdir -p /out \
 && cp /app/target/release/pixiekit-web-api /out/ \
 && cp /app/target/release/pixiekit-cli     /out/ \
 && cp /app/target/release/pixiekit-mcp     /out/

# ---- runtime base: shared minimal image with ffmpeg ----------------------
FROM debian:${DEBIAN_VERSION}-slim AS runtime-base
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
        ffmpeg \
        ca-certificates \
        curl \
        tini \
 && rm -rf /var/lib/apt/lists/* \
 && groupadd --system --gid 1001 pixiekit \
 && useradd  --system --uid 1001 --gid pixiekit --shell /usr/sbin/nologin pixiekit \
 && mkdir -p /app/data \
 && chown -R pixiekit:pixiekit /app
WORKDIR /app
ENTRYPOINT ["/usr/bin/tini", "--"]

# ---- web-api (default target) --------------------------------------------
FROM runtime-base AS web-api
COPY --from=builder /out/pixiekit-web-api /usr/local/bin/pixiekit-web-api
USER pixiekit
ENV HOST=0.0.0.0 \
    PORT=8765 \
    RUST_LOG=info
EXPOSE 8765
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS "http://127.0.0.1:${PORT:-8765}/api/health" >/dev/null || exit 1
CMD ["/usr/local/bin/pixiekit-web-api"]

# ---- cli -----------------------------------------------------------------
FROM runtime-base AS cli
COPY --from=builder /out/pixiekit-cli /usr/local/bin/pixiekit-cli
USER pixiekit
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/pixiekit-cli"]
CMD ["--help"]

# ---- mcp (stdio) ---------------------------------------------------------
FROM runtime-base AS mcp
COPY --from=builder /out/pixiekit-mcp /usr/local/bin/pixiekit-mcp
USER pixiekit
ENV RUST_LOG=info
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/pixiekit-mcp"]
