# Migration State (2026-03-06)

## Snapshot
- Migration target: Rust reimplementation of `headless-rss` with SQLite compatibility and Nextcloud News API parity.
- Stack decisions are documented in `docs/rust-techstack.md`.
- Current Rust workspace location: `rust/`.
- Current Rust test status: `cargo test` passes (`67 passed, 0 failed`).

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
  - `rust/src/api/v1_2.rs`
  - `rust/src/api/v1_3.rs`
  - `rust/src/email_credentials.rs`
  - `rust/src/updater.rs`
- Added Rust migration notes/readme:
  - `rust/README.md`

### Runtime and config
- Axum HTTP server boots from `main.rs` with `serve` command.
- SQLite pool via `sqlx` in `db.rs`.
- Rust-managed SQL migrations are now wired in `db.rs` via SQLx migrator and run automatically on pool initialization.
- Rust migration baseline file added at `rust/migrations/202603060001_baseline.sql`.
- Config/env handling in `config.rs`:
  - `USERNAME`, `PASSWORD`, `VERSION`, `DATABASE_PATH`, `FEED_UPDATE_FREQUENCY_MIN`
  - default DB path supports both repo root and `rust/` working directory layouts.
- CLI `update` command now executes Rust feed update workflow via `rust/src/updater.rs`.
- Server runtime now runs feed updates at startup and periodically in the background based on `FEED_UPDATE_FREQUENCY_MIN`.
- API version route wiring is split into version-specific files: `rust/src/api/v1_2.rs` and `rust/src/api/v1_3.rs`.

### Implemented Rust API endpoints
- Service:
  - `GET /status`
- Nextcloud v1-2 and v1-3 (implemented in lockstep):
  - `GET /version`
  - Feed endpoints:
    - `GET /feeds`
    - `POST /feeds`
    - `DELETE /feeds/{feed_id}`
    - `v1-2: PUT /feeds/{feed_id}/move`
    - `v1-3: POST /feeds/{feed_id}/move`
    - `v1-2: PUT /feeds/{feed_id}/rename`
    - `v1-3: POST /feeds/{feed_id}/rename`
    - `v1-2: PUT /feeds/{feed_id}/read`
    - `v1-3: POST /feeds/{feed_id}/read`
  - Folder endpoints:
    - `GET /folders`
    - `POST /folders`
    - `DELETE /folders/{folder_id}`
    - `PUT /folders/{folder_id}`
    - `POST /folders/{folder_id}/read`
  - `GET /items` (read-only)
  - `GET /items/updated` (read-only)
  - `GET /items/{item_id}/content` (read-only)
  - `v1-2` write endpoints for items:
    - `POST /items/{item_id}/read`
    - `PUT /items/read/multiple`
    - `PUT /items/{item_id}/unread`
    - `PUT /items/unread/multiple`
    - `PUT /items/{feed_id}/{guid_hash}/star`
    - `PUT /items/star/multiple`
    - `PUT /items/{feed_id}/{guid_hash}/unstar`
    - `PUT /items/unstar/multiple`
    - `PUT /items/read`
  - `v1-3` write endpoints for items:
    - `POST /items/{item_id}/read`
    - `POST /items/read/multiple`
    - `POST /items/{item_id}/unread`
    - `POST /items/unread/multiple`
    - `POST /items/{item_id}/star`
    - `POST /items/star/multiple`
    - `POST /items/{item_id}/unstar`
    - `POST /items/unstar/multiple`
    - `POST /items/read`

### Compatibility behavior currently implemented
- CamelCase response fields where expected.
- Root folder feed mapping: root `folder_id` -> `folderId: null` in feed responses.
- Folder listing excludes root folder (`is_root = 1`).
- Basic auth behavior on protected endpoints matches Python app rules when both `USERNAME` and `PASSWORD` are set.
- Item `body` field behavior matches Python: prefer `summary`, fall back to `content`.
- Missing item content returns `404` with `{"detail":"Item not found"}`.
- `v1-2` and `v1-3` now preserve item write method/payload differences:
  - `v1-2` uses guid-hash star/unstar routes and `PUT` for most item write actions.
  - `v1-3` uses item-id star/unstar routes and `POST` write actions with `itemIds` payload where required.
- Feed and folder writes are now implemented with version-specific method differences for feed move/rename/read (`PUT` in `v1-2`, `POST` in `v1-3`).
- Feed creation now performs HTTP fetch + feed parsing and inserts initial feed metadata and entries.
- Rust feed creation now includes SSRF URL validation and returns `400` for blocked hosts/schemes.

## Validation Completed
- Rust tests:
  - `cd rust && cargo test` -> pass (67 tests)
- Runtime smoke:
  - `cargo run -- serve --host 127.0.0.1 --port 18004`
  - `curl http://127.0.0.1:18004/status` -> `{"status":"ok"}`
- Prior Python validation (already run earlier in this migration session):
  - `uv run alembic upgrade head`
  - `uv run --dev pre-commit run --all-files`
  - `uv run --dev pytest tests`
  - `uv run --dev python -m src.cli --help`
  - `uv run --dev python -m src.cli update`

## Known Gaps
- Rust feed creation SSRF behavior is implemented for blocked host classes; broader parity tests are still pending.
- Email/IMAP and OpenAI features intentionally deferred until after core parity.

## Newly Added Test Coverage
- Added Rust parity tests for feed/folder write error paths:
  - duplicate folder create -> `409`
  - missing folder delete -> `404`
  - duplicate feed create -> `409`
  - feed create with missing folder -> `422`
  - missing feed delete -> `404`
  - feed move to missing folder -> `422`
- Added Rust success-path parity tests:
  - feed create response payload includes expected metadata fields and `newestItemId`
  - folder read endpoint marks folder-scoped items as read
- Added method-specific parity tests for feed rename/read routes:
  - `v1-2` rename/read success paths (`PUT`)
  - `v1-3` rename success path (`POST`)
  - method-mismatch checks return `405` for route-specific wrong verbs
- Added delete-side-effect parity tests:
  - deleting a feed removes its associated articles
  - deleting a folder removes feeds in the folder and their associated articles
- Added Rust updater tests:
  - due-feed update inserts new articles from parsed feed entries
  - failed update persists `update_error_count` and `last_update_error`
- Added Rust add-email-credentials tests:
  - credentials are persisted only when mailbox validation succeeds
  - failed validation prevents persistence
- Added additional API parity tests from Python suites:
  - folder create invalid-name -> `422`
  - folder rename duplicate-name -> `409`
  - folder rename invalid-name -> `422`
  - feed create unreadable source -> `422`
  - feed create payload includes non-null `nextUpdateTime`
- Added further API/auth parity tests:
  - missing-feed detail assertions for v1-2 rename and v1-3 read routes (`404` with feed-id detail)
  - protected endpoint auth parity for invalid and valid Basic credentials
- Added item endpoint parity/error-detail tests:
  - invalid item selection type returns `400` with `Invalid item selection type`
  - v1-2 item read missing id returns `404` with `Item not found`
  - v1-3 item star missing id returns `404` with `Item not found`
  - v1-2 guid-hash star with missing entry returns `404` with `Item not found`
- Added item multiple-route state parity tests:
  - v1-2 `read/multiple` updates `unread` and bumps `last_modified`
  - v1-2 `star/multiple` (guid-hash payload) updates `starred` and bumps `last_modified`
  - v1-3 `unread/multiple` updates `unread` and bumps `last_modified`
  - v1-3 `unstar/multiple` updates `starred` and bumps `last_modified`
- Added item mark-all-read parity tests:
  - v1-2 `PUT /items/read` updates `unread` and bumps `last_modified`
  - v1-3 `POST /items/read` updates `unread` and bumps `last_modified`
- Added single-item write parity tests:
  - v1-2 single `read`, `unread`, and guid-hash `unstar` update state and bump `last_modified`
  - v1-3 single `read`, `unread`, `star`, and `unstar` update state and bump `last_modified`
- Added item query-contract parity tests:
  - folder selection (`type=1`) and starred selection (`type=2`) filtering semantics
  - unread filtering with `getRead=false`
  - ordering semantics for `oldestFirst=true`
  - batch limit behavior for `batchSize`
  - `offset` behavior matching Python's `newest_item_id` filter (`id <= offset`)
- Added updated-items query-contract parity tests:
  - feed/folder/starred/all selection behavior with `lastModified` threshold filtering
  - descending ordering behavior for updated item responses
- Added Rust migration bootstrap test:
  - `db::tests::create_pool_runs_migrations_and_bootstraps_root_folder` validates SQLx migration execution, table creation, and root-folder initialization.

## Current Working Tree Notes
- Files with migration changes include:
  - `.devcontainer/devcontainer.json`
  - `README.md`
  - `docs/requirements.md`
  - `docs/test-cases.md`
  - `docs/rust-techstack.md`
  - all new files under `rust/`
- `.github/copilot-instructions.md` is also modified in the worktree and was treated as pre-existing/unrelated during migration work.

## Planned Next Steps (Priority Order)
1. Expand Rust endpoint tests toward parity with Python API test cases:
   - mirror high-value tests from `tests/api/nextcloud_news/v1_2/` and `v1_3/`
2. Expand API parity tests for remaining content-level and payload-detail parity against Python fixtures.
3. Expand Rust CLI coverage around `add-email-credentials` runtime behavior against a controlled IMAP test endpoint.
4. After core parity: implement IMAP/newsletter and OpenAI summary features.

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
