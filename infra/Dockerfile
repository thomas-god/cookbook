# syntax=docker/dockerfile:1

FROM rust:1.99-bookworm AS builder

WORKDIR /app

# The Topcoat CLI bundles the Tailwind and font assets embedded in the binary.
ARG TOPCOAT_VERSION=0.11.0
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo install topcoat-cli --version "${TOPCOAT_VERSION}" --locked

COPY . .

RUN --mount=type=cache,target=/app/target \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release -p cookbook-app \
    && topcoat asset bundle --release \
    && mkdir -p /app/dist \
    && cp target/release/cookbook-app /app/dist/ \
    && cp -r target/release/assets /app/dist/assets

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/dist/cookbook-app /app/cookbook-app
COPY --from=builder /app/dist/assets /app/assets

ENV HOST=0.0.0.0
ENV PORT=3000

EXPOSE 3000

USER 10001:10001

ENTRYPOINT ["/app/cookbook-app"]
