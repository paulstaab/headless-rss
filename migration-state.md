# Migration State (2026-03-05)

## Snapshot
- Migration target: Rust reimplementation of `headless-rss` with SQLite compatibility and Nextcloud News API parity.
- Stack decisions are documented in `docs/techstack.md`.
- Current Rust workspace location: `rust/`.
- Current Rust test status: `cargo test` passes (`6 passed, 0 failed`).

## Implemented So Far

### Rust project scaffold
- Added Rust crate and lockfile:
  - `rust/Cargo.toml`
  - `rust/Cargo.lock`
- Added core modules:
  - `rust/src/main.rs`
  - `rust/src/config.rs`
  - `rust/src/db.rs`
  - `rust/src/api.rs`
- Added Rust migration notes/readme:
  - `rust/README.md`

### Runtime and config
- Axum HTTP server boots from `main.rs` with `serve` command.
- SQLite pool via `sqlx` in `db.rs`.
- Config/env handling in `config.rs`:
  - `USERNAME`, `PASSWORD`, `VERSION`, `DATABASE_PATH`
  - default DB path supports both repo root and `rust/` working directory layouts.

### Implemented Rust API endpoints
- Service:
  - `GET /status`
- Nextcloud v1-2 and v1-3 (implemented in lockstep):
  - `GET /version`
  - `GET /feeds` (read-only)
  - `GET /folders` (read-only)
  - `GET /items` (read-only)
  - `GET /items/updated` (read-only)
  - `GET /items/{item_id}/content` (read-only)

### Compatibility behavior currently implemented
- CamelCase response fields where expected.
- Root folder feed mapping: root `folder_id` -> `folderId: null` in feed responses.
- Folder listing excludes root folder (`is_root = 1`).
- Basic auth behavior on protected endpoints matches Python app rules when both `USERNAME` and `PASSWORD` are set.
- Item `body` field behavior matches Python: prefer `summary`, fall back to `content`.
- Missing item content returns `404` with `{"detail":"Item not found"}`.

## Validation Completed
- Rust tests:
  - `cd rust && cargo test` -> pass (6 tests)
- Prior Python validation (already run earlier in this migration session):
  - `uv run alembic upgrade head`
  - `uv run --dev pre-commit run --all-files`
  - `uv run --dev pytest tests`
  - `uv run --dev python -m src.cli --help`
  - `uv run --dev python -m src.cli update`

## Known Gaps
- No Rust write endpoints yet for feeds/folders/items.
- No Rust scheduler/feed update workflow yet.
- Rust CLI subcommands `update` and `add-email-credentials` are scaffolded but not implemented.
- Rust-side SQL migrations are not established yet (decision is Rust-first migrations).
- Email/IMAP and OpenAI features intentionally deferred until after core parity.

## Current Working Tree Notes
- Files with migration changes include:
  - `.devcontainer/devcontainer.json`
  - `README.md`
  - `docs/requirements.md`
  - `docs/test-cases.md`
  - `docs/techstack.md`
  - all new files under `rust/`
- `.github/copilot-instructions.md` is also modified in the worktree and was treated as pre-existing/unrelated during migration work.

## Planned Next Steps (Priority Order)
1. Implement item write endpoints in Rust for both v1-2 and v1-3:
   - read/unread single
   - read/unread multiple
   - star/unstar single
   - star/unstar multiple
   - mark-all-read
2. Implement feed and folder write endpoints in Rust:
   - create/delete/rename/move behaviors
   - preserve v1-2 vs v1-3 method/route differences
   - preserve Python-compatible status codes and error payloads
3. Expand Rust endpoint tests toward parity with Python API test cases:
   - mirror high-value tests from `tests/api/nextcloud_news/v1_2/` and `v1_3/`
4. Add Rust scheduler skeleton and feed update loop integration (without full parsing logic first).
5. Implement Rust `update` CLI parity on top of the scheduler/update service layer.
6. Introduce Rust-managed SQL migrations and baseline schema compatibility checks against existing DB.
7. After core parity: implement IMAP/newsletter and OpenAI summary features.

## Quick Resume Commands For Tomorrow
```bash
cd /workspaces/headless-rss/rust
cargo fmt
cargo test
cargo run -- serve --host 127.0.0.1 --port 18002
```

Service smoke checks against Rust server:
```bash
curl http://127.0.0.1:18002/status
curl http://127.0.0.1:18002/index.php/apps/news/api/v1-3/version
curl 'http://127.0.0.1:18002/index.php/apps/news/api/v1-3/items?type=3'
```
