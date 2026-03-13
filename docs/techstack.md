# Tech Stack

## Purpose
This document lists the technology and tooling choices for headless-rss.

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
- Migration system: SQLx migrations under `migrations/`

## Feed Ingestion
- HTTP client: `reqwest`
- Feed parsing: `feed-rs`
- Article extraction: `readability-js`
- HTML image extraction: `regex` (for thumbnail fallback)
- Deduplication hash: `md5`

## Evaluation Notes
- 2026-03-13: `readability-js` was evaluated against `readability-rust` on live heise.de, tagesschau.de, Spiegel, and Simon Willison articles.
- Decision: keep `readability-js` for production extraction.
- Rationale: `readability-js` produced consistently better titles and cleaner main-content extraction, while `readability-rust` pulled JSON-LD/media metadata on tagesschau pages, regressed titles on some pages, and under-extracted some articles.

## Email Integration
- IMAP client: `imap`
- TLS for IMAP: `native-tls`

## LLM Integration
- Current LLM provider: `OpenAI`
- Provider access pattern: OpenAI-compatible chat completions API
- Environment variables:
	- `OPENAI_API_KEY`
	- `OPENAI_BASE_URL`
	- `OPENAI_MODEL`
	- `OPENAI_TIMEOUT_SECONDS`

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
- End-to-end journey tests: `tests/journey_tests.rs`

## Build And Packaging
- Build tooling: `cargo`
- Containerization target: single self-hosted image
