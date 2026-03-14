# AGENTS.md

## Purpose
This file defines how coding agents and contributors should work in this repository.
It combines workflow guidance with implementation-aware project conventions.

## Project Summary
- `headless-rss` is a self-hosted RSS/Atom aggregator.
- It exposes Nextcloud News compatible APIs for versions `v1-2` and `v1-3`.
- It includes email newsletter ingestion over IMAP.
- It supports optional AI-assisted summarization and newsletter parsing via OpenAI.
- The production implementation is Rust-only.

## Repository Priorities
- Keep the project minimal and stable.
- Preserve Nextcloud News API compatibility.
- Favor safe defaults and clear error handling.
- Do not introduce unnecessary complexity.

## Key Runtime Behavior
- The service exposes a health endpoint at `/status`.
- Nextcloud APIs are mounted under `/index.php/apps/news/api`.
- Feed updates run on startup and then on a periodic schedule controlled by `FEED_UPDATE_FREQUENCY_MIN` (default `15`).
- Root folder behavior:
  - Root folder is internal.
  - Root folder is omitted from folder listings.
  - `folderId: null` and `folderId: 0` map to root when creating/moving feeds.

## Authentication and Security
- Basic auth is enabled only when both `USERNAME` and `PASSWORD` are set.
- When auth is enabled:
  - Missing credentials must return `401` with `{"detail":"Not authenticated"}`.
  - Invalid credentials must return `401` with `{"detail":"Invalid authentication credentials"}`.
- URL validation is required for remote fetches and must protect against SSRF:
  - Allow only `http` and `https`.
  - Block loopback, private, link-local, unspecified, multicast, and metadata service addresses.

## API Compatibility Rules
- Keep both API versions working unless a task explicitly scopes changes.
- Preserve existing method and payload differences between versions:
  - `v1-2` uses `PUT` and guid-hash star/unstar routes.
  - `v1-3` uses `POST` and item-id star/unstar routes.
- Preserve camelCase response and request field names in API payloads.

## Data and Domain Rules
- Keep uniqueness constraints intact:
  - Feed URL unique.
  - Folder name unique.
- Deleting a feed must delete associated articles.
- Deleting a folder must delete feeds in that folder.
- Avoid creating duplicate articles (guid-hash based de-duplication).
- Preserve stale-content cleanup behavior:
  - Old read and unstarred feed/newsletter entries are eligible for cleanup.

## CLI Rules
- Keep CLI commands functional:
  - `cargo run -- update`
  - `cargo run -- add-email-credentials --server ... --port ... --username ... --password ...`
- `cargo run -- --help` should remain usable.
- `add-email-credentials` must validate mailbox connectivity before persisting credentials.

## Required Workflow
1. Bootstrap
- Ensure the Rust toolchain is available.
- Run `cargo fetch` if dependencies have not been downloaded yet.

2. Develop
- Always keep requirements and test cases updated - see Documentation Sync Policy.
- Prefer small, focused changes.
- Preserve existing API behavior and response contracts.
- When manual runtime validation is needed, start the API server with the VS Code task `Start Server` and keep it running in the background while testing.
- The local server should listen on `http://localhost:8000`.
- When finished, update the rustdoc comments for touched modules and functions if necessary. Also document reasons for implementation decisions there.

3. Validate after changes
- Run `Lint` task.
- Run `Run All Tests` task.
- Validate CLI:
  - `cargo run -- --help`
  - `cargo run -- update`

4. Optional end-to-end smoke test
- Start server.
- Verify:
  - `curl http://localhost:8000/status`
  - `curl http://localhost:8000/index.php/apps/news/api/v1-3/feeds`
  - `curl http://localhost:8000/index.php/apps/news/api/v1-3/folders`
  - `curl http://localhost:8000/index.php/apps/news/api/v1-3/version`

## Repository Structure
```
.
├── .devcontainer/          # VS Code dev container configuration
├── .github/                # GitHub workflows and config
├── .pre-commit-config.yaml # Pre-commit hooks (Rust formatting/linting)
├── Dockerfile              # Container build definition
├── README.md               # Project overview and local usage
├── Cargo.toml              # Rust project manifest
├── Cargo.lock              # Locked Rust dependencies
├── data/                   # SQLite database location
├── docker/                 # Docker-related scripts
├── docs/                   # Requirements, contracts, and test catalogs
├── migrations/             # SQLx migrations
├── src/                    # Main Rust application code
├── tests/                  # Rust test suite
└── vendor/                 # Vendored crates and assets
```

## Key API Endpoints
- `/status` for health checks.
- `/index.php/apps/news/api/v{version}/feeds` for Nextcloud News compatible feed operations (supports `v1-2` and `v1-3`).
- `/index.php/apps/news/api/v{version}/folders` for folder operations (supports `v1-2` and `v1-3`).
- `/index.php/apps/news/api/v{version}/items` for article and item operations (supports `v1-2` and `v1-3`).

## Environment and Storage
- `USERNAME` and `PASSWORD` are optional and enable HTTP Basic auth only when both are set.
- By default, SQLite data lives at `data/headless-rss.sqlite3` (or `../data/headless-rss.sqlite3` depending on the working directory). This can be overridden via the `DATABASE_PATH` environment variable.
- SQLx migrations are applied automatically on startup.

## Troubleshooting
- If tests fail due to database state, remove the SQLite database at the effective path (the value of `DATABASE_PATH` if set, otherwise the default such as `data/headless-rss.sqlite3*`) and rerun the relevant command or restart the server.

## Documentation Sync Policy
When implementing fixes, refactors, or new features, keep documentation synchronized in the same change:
- Update `docs/requirements.md` to reflect implemented requirements.
- Update `docs/test-cases.md` to reflect implemented and verified test coverage.
- Update API contracts when endpoint behavior changes:
  - `docs/api-service-contract.md`
  - `docs/api-nextcloud-v1-2-contract.md`
  - `docs/api-nextcloud-v1-3-contract.md`
- Update API test-case documents when endpoint behavior or coverage changes:
  - `docs/api-service-test-cases.md`
  - `docs/api-nextcloud-v1-2-test-cases.md`
  - `docs/api-nextcloud-v1-3-test-cases.md`
- If code behavior changes but docs are not updated, treat the task as incomplete.
- Always add or update rustdoc comments when adding or changing a function or a module.

## Useful Paths
- App entrypoint: `src/main.rs`
- API version routers:
  - `src/api/v1_2.rs`
  - `src/api/v1_3.rs`
- Core domain modules:
  - `src/article_store.rs`
  - `src/content.rs`
  - `src/email.rs`
  - `src/email_credentials.rs`
  - `src/updater.rs`
- CLI: `src/main.rs`
- Tests: `tests/`

## Notes for Agents
- Prefer behavior-preserving edits unless the task explicitly requests behavior changes.
- Add or update tests when behavior changes.
- Keep changes readable and easy to review.
