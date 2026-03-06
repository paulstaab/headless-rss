# Implemented Requirements Baseline

## Purpose
This document captures requirements that are currently implemented in the codebase.

## API Contracts
API requirements are specified in separate contract documents:
- `docs/api-service-contract.md`
- `docs/api-nextcloud-v1-2-contract.md`
- `docs/api-nextcloud-v1-3-contract.md`

## Test Cases
Test cases are specified in separate documents:
- `docs/test-cases.md`
- `docs/api-service-test-cases.md`
- `docs/api-nextcloud-v1-2-test-cases.md`
- `docs/api-nextcloud-v1-3-test-cases.md`

## Functional Requirements

### Delivery Requirements
- The application shall be delivered as a single, self-contained Docker container image suitable for self-hosted deployment.

### Service Startup and Runtime
- On startup, the service shall initialize persistent storage connectivity.
- On startup, the service shall trigger a feed update cycle.
- The service shall periodically execute feed updates using a configurable interval (`FEED_UPDATE_FREQUENCY_MIN`, default `15`).

### Feed Aggregation
- The system shall parse and ingest Atom and RSS feeds.
- The system shall reject duplicate feed URLs.
- The system shall reject feed creation for non-existent folders.
- The system shall support deleting feeds and all associated articles.
- The system shall support moving feeds between folders.
- The system shall support renaming feeds.
- The system shall compute and persist a dynamic `next_update_time` based on recent publishing frequency.
- The system shall persist feed update errors (`update_error_count`, `last_update_error`) and clear error state after successful updates.
- The system shall remove stale feed articles that are not in the latest feed payload when they are older than 90 days, read, and unstarred.

### Folder Management
- The system shall maintain an internal root folder and create it on demand if missing.
- The system shall hide the root folder from folder listing APIs.
- The system shall support creating, renaming, listing, and deleting user folders.
- The system shall reject empty folder names.
- The system shall reject duplicate folder names.
- Deleting a folder shall delete feeds assigned to that folder.

### Article and Item Behavior
- The system shall persist articles with GUID and GUID hash identifiers.
- The system shall prevent duplicate article insertion using GUID hash.
- The system shall provide item retrieval filtered by feed, folder, starred, or all items.
- The system shall support item retrieval filtered by `last_modified`.
- The system shall support read/unread and star/unstar state transitions for single and bulk operations.
- The system shall update `last_modified` when article read/star state changes.
- The system shall support marking items read up to a provided newest item ID at feed, folder, and global levels.

### Content Extraction and Summarization
- The system shall extract first-image URLs from HTML content when no feed thumbnail is available.
- The system shall support optional full-text extraction from article URLs.
- The system shall evaluate feed quality periodically (roughly monthly) and decide whether to use extracted full text.
- The system shall mark `use_llm_summary` only when full-text extraction quality is considered sufficient.
- The system shall support optional LLM-generated summaries when `OPENAI_API_KEY` is configured.
- LLM summaries shall be appended with `" (AI generated)"`.
- If LLM summarization is not used and content is long, summaries shall fall back to truncated content.

### Email Newsletter Integration
- The system shall store IMAP credentials via CLI/API-internal calls.
- Adding IMAP credentials shall validate connectivity/login before persisting credentials.
- The system shall fetch unread emails from configured IMAP inboxes during update cycles.
- Only emails identified as mailing list messages (`List-Unsubscribe` header) shall be processed as newsletter content.
- The system shall create mailing-list feeds automatically when first encountering a sender.
- Newsletter HTML shall be cleaned before article creation.
- When LLM is enabled, newsletter parsing shall support:
  - Single-article extraction mode.
  - Multi-item splitting mode with up to 25 generated items.
- The system shall clean up stale newsletter articles older than 90 days only when they are read and unstarred.

### Security and Safety
- The system shall implement URL validation to reduce SSRF risk for feed/article fetches.
- URL validation shall allow only `http` and `https` schemes.
- URL validation shall block loopback, private, link-local, unspecified, multicast, and cloud metadata addresses.
- Localhost access may be allowed in testing mode.
- HTTP Basic auth enforcement shall be conditional on `USERNAME` and `PASSWORD` configuration.

### Configuration and Environment
- The system shall read runtime configuration from environment variables.
- Supported environment variables shall include:
  - `USERNAME`, `PASSWORD`
  - `FEED_UPDATE_FREQUENCY_MIN`
  - `VERSION`
  - `OPENAI_API_KEY`, `OPENAI_MODEL`
- Default values shall include:
  - version: `dev`
  - update frequency: `15` minutes
  - OpenAI model: `gpt-5-mini`

### CLI Requirements
- The CLI shall provide an `update` command that initializes persistent storage access and runs feed updates.
- The CLI shall provide an `add-email-credentials` command with required server, port, username, and password options.
- `add-email-credentials` shall report a user-facing error if credential validation fails.

## Data and Persistence Requirements
- Database entities shall include `Feed`, `Folder`, `Article`, and `EmailCredential`.
- Feed URLs shall be unique.
- Folder names shall be unique.

## Rust Reimplementation Progress
- A Rust implementation workspace exists under `rust/` using `axum`, `tokio`, and `sqlx` with SQLite.
- The Rust server shall expose:
  - `/status`
  - `/index.php/apps/news/api/v1-2/version`
  - `/index.php/apps/news/api/v1-3/version`
  - `/index.php/apps/news/api/v1-2/feeds`
  - `/index.php/apps/news/api/v1-3/feeds`
  - `/index.php/apps/news/api/v1-2/folders`
  - `/index.php/apps/news/api/v1-3/folders`
  - `/index.php/apps/news/api/v1-2/feeds/{feed_id}` (`DELETE`)
  - `/index.php/apps/news/api/v1-3/feeds/{feed_id}` (`DELETE`)
  - `/index.php/apps/news/api/v1-2/feeds/{feed_id}/move` (`PUT`)
  - `/index.php/apps/news/api/v1-3/feeds/{feed_id}/move` (`POST`)
  - `/index.php/apps/news/api/v1-2/feeds/{feed_id}/rename` (`PUT`)
  - `/index.php/apps/news/api/v1-3/feeds/{feed_id}/rename` (`POST`)
  - `/index.php/apps/news/api/v1-2/feeds/{feed_id}/read` (`PUT`)
  - `/index.php/apps/news/api/v1-3/feeds/{feed_id}/read` (`POST`)
  - `/index.php/apps/news/api/v1-2/folders/{folder_id}` (`DELETE`, `PUT`)
  - `/index.php/apps/news/api/v1-3/folders/{folder_id}` (`DELETE`, `PUT`)
  - `/index.php/apps/news/api/v1-2/folders/{folder_id}/read` (`POST`)
  - `/index.php/apps/news/api/v1-3/folders/{folder_id}/read` (`POST`)
  - `/index.php/apps/news/api/v1-2/items` (read-only)
  - `/index.php/apps/news/api/v1-3/items` (read-only)
  - `/index.php/apps/news/api/v1-2/items/updated` (read-only)
  - `/index.php/apps/news/api/v1-3/items/updated` (read-only)
  - `/index.php/apps/news/api/v1-2/items/{item_id}/content` (read-only)
  - `/index.php/apps/news/api/v1-3/items/{item_id}/content` (read-only)
  - `/index.php/apps/news/api/v1-2/items/{item_id}/read`
  - `/index.php/apps/news/api/v1-2/items/read/multiple`
  - `/index.php/apps/news/api/v1-2/items/{item_id}/unread`
  - `/index.php/apps/news/api/v1-2/items/unread/multiple`
  - `/index.php/apps/news/api/v1-2/items/{feed_id}/{guid_hash}/star`
  - `/index.php/apps/news/api/v1-2/items/star/multiple`
  - `/index.php/apps/news/api/v1-2/items/{feed_id}/{guid_hash}/unstar`
  - `/index.php/apps/news/api/v1-2/items/unstar/multiple`
  - `/index.php/apps/news/api/v1-2/items/read`
  - `/index.php/apps/news/api/v1-3/items/{item_id}/read`
  - `/index.php/apps/news/api/v1-3/items/read/multiple`
  - `/index.php/apps/news/api/v1-3/items/{item_id}/unread`
  - `/index.php/apps/news/api/v1-3/items/unread/multiple`
  - `/index.php/apps/news/api/v1-3/items/{item_id}/star`
  - `/index.php/apps/news/api/v1-3/items/star/multiple`
  - `/index.php/apps/news/api/v1-3/items/{item_id}/unstar`
  - `/index.php/apps/news/api/v1-3/items/unstar/multiple`
  - `/index.php/apps/news/api/v1-3/items/read`
- Rust feed responses shall preserve camelCase fields and map root folder IDs to `folderId: null`.
- Rust folder responses shall omit the internal root folder.
- Rust auth behavior shall match current API behavior for protected endpoints when `USERNAME` and `PASSWORD` are both set.
- Rust default database path handling shall support running from repository root and from `rust/`.
- Rust feed creation shall validate remote URLs with SSRF protections:
  - only `http` and `https` schemes allowed
  - block loopback, private, link-local, unspecified, multicast, and metadata service addresses
  - allow localhost only in testing mode (`TESTING_MODE=true` in Rust env)
- Rust CLI `update` command shall execute a feed update cycle for due feeds (`next_update_time` is null or in the past), insert new articles by guid-hash de-duplication, and persist feed update errors (`update_error_count`, `last_update_error`) on failures.
- Rust `serve` command shall trigger a startup update cycle and continue periodic due-feed updates based on `FEED_UPDATE_FREQUENCY_MIN`.
- Rust CLI `add-email-credentials` command shall validate IMAP connectivity/login before persisting credentials into `email_credentials`.
- Rust API version routing shall be separated into version-specific source files for maintainability (`rust/src/api/v1_2.rs` and `rust/src/api/v1_3.rs`).
