FROM rust:1.88-bookworm AS builder
WORKDIR /app

# Build from real sources only to avoid warmup-stub cache pitfalls.
COPY Cargo.toml Cargo.lock /app/
COPY src /app/src
COPY migrations /app/migrations
COPY vendor /app/vendor
COPY docker/entrypoint /app/docker/entrypoint
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo build --release

# Ensure we do not accidentally publish the dependency-cache warmup stub binary.
RUN /app/target/release/headless-rss --help | grep -q "headless-rss"

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libsqlite3-0 libssl3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/headless-rss /usr/local/bin/headless-rss
COPY --from=builder /app/docker/entrypoint /app/docker/entrypoint
RUN mkdir -p /app/data \
    && chmod 775 /app/data \
    && chmod +x /app/docker/entrypoint
WORKDIR /app
ENTRYPOINT ["/app/docker/entrypoint"]
CMD ["start"]
