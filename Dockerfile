FROM rust:1.86-bookworm AS builder
WORKDIR /app
ADD . /app
RUN cargo build --manifest-path rust/Cargo.toml --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libsqlite3-0 libssl3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/rust/target/release/headless-rss-rs /usr/local/bin/headless-rss-rs
COPY docker/entrypoint /app/docker/entrypoint
RUN mkdir -p /app/data \
    && chmod 775 /app/data \
    && chmod +x /app/docker/entrypoint
WORKDIR /app
ENTRYPOINT ["/app/docker/entrypoint"]
CMD ["start"]
