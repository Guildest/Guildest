FROM rust:1.94-slim-bookworm AS base-builder
WORKDIR /app

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/common/Cargo.toml crates/common/Cargo.toml
COPY crates/api/Cargo.toml crates/api/Cargo.toml
COPY crates/gateway/Cargo.toml crates/gateway/Cargo.toml
COPY crates/worker/Cargo.toml crates/worker/Cargo.toml
COPY crates/common/src crates/common/src
COPY crates/api/src crates/api/src
COPY crates/gateway/src crates/gateway/src
COPY crates/worker/src crates/worker/src

FROM base-builder AS api-builder
RUN cargo build --release -p api

FROM debian:bookworm-slim AS runtime-base
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app

FROM runtime-base AS api-runtime
COPY --from=api-builder /app/target/release/api /usr/local/bin/api
CMD ["api"]

FROM base-builder AS worker-builder
RUN cargo build --release -p worker

FROM runtime-base AS worker-runtime
COPY --from=worker-builder /app/target/release/worker /usr/local/bin/worker
CMD ["worker"]

FROM base-builder AS gateway-builder
RUN cargo build --release -p gateway

FROM runtime-base AS gateway-runtime
COPY --from=gateway-builder /app/target/release/gateway /usr/local/bin/gateway
CMD ["gateway"]
