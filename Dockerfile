# syntax=docker/dockerfile:1
# Build with glibc for broader crate compatibility. The image still runs on
# an Alpine VPS because the Docker host and container base are independent.
FROM rust:bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY disk ./disk
RUN CARGO_TARGET_DIR=/build/target cargo build --release

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ffmpeg \
        imagemagick \
        webp \
        fonts-dejavu-core \
        ca-certificates \
        tzdata \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --home-dir /app bot

WORKDIR /app
COPY --from=builder /build/target/release/wa-rust /app/wa-rust
RUN mkdir -p /app/data/disk/tmp && chown -R bot:bot /app

USER bot
ENV RUST_LOG=info

WORKDIR /app/data
VOLUME ["/app/data"]
ENTRYPOINT ["/app/wa-rust"]
