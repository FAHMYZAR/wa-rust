# syntax=docker/dockerfile:1
FROM rust:1.87-alpine AS builder

RUN apk add --no-cache musl-dev build-base pkgconfig openssl-dev
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN CARGO_TARGET_DIR=/build/target cargo build --release

FROM alpine:3.22

RUN apk add --no-cache \
    ffmpeg \
    imagemagick \
    libwebp-tools \
    ttf-dejavu \
    ca-certificates \
    tzdata \
    && adduser -D -h /app bot

WORKDIR /app
COPY --from=builder /build/target/release/wa-rust /app/wa-rust
RUN mkdir -p /app/data/disk/tmp && chown -R bot:bot /app

USER bot
ENV RUST_LOG=info

WORKDIR /app/data
VOLUME ["/app/data"]
ENTRYPOINT ["/app/wa-rust"]
