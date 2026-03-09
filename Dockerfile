FROM rust:1.86-bookworm AS builder
WORKDIR /app

# Copy manifest files first so dependency compilation can be cached independently.
COPY rust/Cargo.toml rust/Cargo.lock /app/rust/
RUN mkdir -p /app/rust/src
RUN printf 'fn main() { println!("cache warmup"); }\n' > /app/rust/src/main.rs
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --manifest-path /app/rust/Cargo.toml --release

# Copy the actual sources after dependency compilation cache has been primed.
COPY rust/src /app/rust/src
COPY rust/migrations /app/rust/migrations
COPY docker/entrypoint /app/docker/entrypoint
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --manifest-path /app/rust/Cargo.toml --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libsqlite3-0 libssl3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/rust/target/release/headless-rss-rs /usr/local/bin/headless-rss-rs
COPY --from=builder /app/docker/entrypoint /app/docker/entrypoint
RUN mkdir -p /app/data \
    && chmod 775 /app/data \
    && chmod +x /app/docker/entrypoint
WORKDIR /app
ENTRYPOINT ["/app/docker/entrypoint"]
CMD ["start"]
