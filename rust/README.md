# Rust Reimplementation

This directory contains the Rust implementation of `headless-rss`.

## Current Scope
- `axum` HTTP server
- `sqlx` SQLite connectivity compatible with `data/headless-rss.sqlite3`
- Nextcloud News v1-2 and v1-3 API compatibility paths covered by unit and journey tests
- Feed update CLI support via `cargo run -- update`
- Email credential validation and persistence via `cargo run -- add-email-credentials ...`
- Optional article full-text extraction via `readability-js`
- Optional OpenAI-backed article summaries via `OPENAI_API_KEY`
- Conditional HTTP Basic auth when both `USERNAME` and `PASSWORD` are set

## Run
From repository root:

```bash
cd rust
cargo run -- serve --host 0.0.0.0 --port 8000
```

## Test

```bash
cd rust
cargo test
```

## Notes
- Migration source of truth is planned to move to Rust-managed SQL migrations in later steps.
