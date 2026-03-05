# AGENTS.md

## Purpose
This file defines how coding agents and contributors should work in this repository.
It combines workflow guidance with implementation-aware project conventions.

## Project Summary
- `headless-rss` is a self-hosted RSS/Atom aggregator.
- It exposes Nextcloud News compatible APIs for versions `v1-2` and `v1-3`.
- It includes email newsletter ingestion over IMAP.
- It supports optional AI-assisted summarization and newsletter parsing via OpenAI.

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
  - `uv run --dev python -m src.cli update`
  - `uv run --dev python -m src.cli add-email-credentials --server ... --port ... --username ... --password ...`
- `add-email-credentials` must validate mailbox connectivity before persisting credentials.

## Required Workflow
1. Bootstrap
- Ensure `uv` is available.
- Run migrations first.

2. Develop
- Prefer small, focused changes.
- Preserve existing API behavior and response contracts.

3. Validate after changes
- Run `Execute Migrations` task.
- Run `Lint` task.
- Run `Run All Tests` task.
- Validate CLI:
  - `uv run --dev python -m src.cli --help`
  - `uv run --dev python -m src.cli update`

4. Optional end-to-end smoke test
- Start server.
- Verify:
  - `curl http://localhost:8000/status`
  - `curl http://localhost:8000/index.php/apps/news/api/v1-3/feeds`
  - `curl http://localhost:8000/index.php/apps/news/api/v1-3/folders`
  - `curl http://localhost:8000/index.php/apps/news/api/v1-3/version`

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

## Useful Paths
- App entrypoint: `src/api/app.py`
- API version routers:
  - `src/api/nextcloud_news/v1_2/`
  - `src/api/nextcloud_news/v1_3/`
- Core domain modules:
  - `src/feed.py`
  - `src/article.py`
  - `src/folder.py`
  - `src/email.py`
  - `src/content.py`
- CLI: `src/cli.py`
- Tests: `tests/`

## Notes for Agents
- Prefer behavior-preserving edits unless the task explicitly requests behavior changes.
- Add or update tests when behavior changes.
- Keep changes readable and easy to review.
