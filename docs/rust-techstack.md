# Rust Tech Stack

## Purpose
This document lists the technology and tooling choices for the Rust implementation of headless-rss.

## Language And Runtime
- Rust stable toolchain
- Rust edition: 2024
- Async runtime: `tokio`

## Service And API
- Web framework: `axum`
- Middleware/utilities: `tower`, `tower-http`
- Serialization: `serde`, `serde_json`

## Data And Persistence
- Database: SQLite
- Access layer: `sqlx` (SQLite driver)
- Migration system: SQLx migrations under `rust/migrations/`

## Feed Ingestion
- HTTP client: `reqwest`
- Feed parsing: `feed-rs`
- HTML image extraction: `regex` (for thumbnail fallback)
- Deduplication hash: `md5`

## Email Integration
- IMAP client: `imap`
- TLS for IMAP: `native-tls`

## Security
- Basic auth handling in API layer
- Shared SSRF validation module for remote URL fetch paths

## CLI
- Command-line parser: `clap`

## Observability
- Structured logging: `tracing`, `tracing-subscriber`

## Testing
- Rust test framework: `cargo test`
- API and integration tests: `axum` test utilities + `reqwest`
- End-to-end journey tests: `rust/tests/journey_tests.rs`

## Build And Packaging
- Build tooling: `cargo`
- Containerization target: single self-hosted image
