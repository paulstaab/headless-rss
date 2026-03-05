# Rust Reimplementation (WIP)

This directory contains the in-progress Rust reimplementation of `headless-rss`.

## Current Scope
- `axum` HTTP server
- `sqlx` SQLite connectivity (compatible with existing `data/headless-rss.sqlite3`)
- Implemented endpoints:
  - `GET /status`
  - `GET /index.php/apps/news/api/v1-2/version`
  - `GET /index.php/apps/news/api/v1-3/version`
  - `GET /index.php/apps/news/api/v1-2/feeds` (read-only)
  - `GET /index.php/apps/news/api/v1-3/feeds` (read-only)
  - `GET /index.php/apps/news/api/v1-2/folders` (read-only)
  - `GET /index.php/apps/news/api/v1-3/folders` (read-only)
  - `GET /index.php/apps/news/api/v1-2/items` (read-only)
  - `GET /index.php/apps/news/api/v1-3/items` (read-only)
  - `GET /index.php/apps/news/api/v1-2/items/updated` (read-only)
  - `GET /index.php/apps/news/api/v1-3/items/updated` (read-only)
  - `GET /index.php/apps/news/api/v1-2/items/{item_id}/content` (read-only)
  - `GET /index.php/apps/news/api/v1-3/items/{item_id}/content` (read-only)
- Conditional HTTP Basic auth on protected endpoints when `USERNAME` and `PASSWORD` are set.

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
- `update` and `add-email-credentials` CLI subcommands are scaffolded but not implemented yet.
- Migration source of truth is planned to move to Rust-managed SQL migrations in later steps.
