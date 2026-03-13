# Requirements Baseline

## Purpose
This document defines the product requirements independent of implementation language.
The same requirements apply to all implementations.

Implementation progress is tracked separately in `docs/rust-implementation-status.md`.

## Related Contracts And Tests
- API contracts:
  - `docs/api-service-contract.md`
  - `docs/api-nextcloud-v1-2-contract.md`
  - `docs/api-nextcloud-v1-3-contract.md`
- Test case catalogs:
  - `docs/test-cases.md`
  - `docs/api-service-test-cases.md`
  - `docs/api-nextcloud-v1-2-test-cases.md`
  - `docs/api-nextcloud-v1-3-test-cases.md`

## Requirement IDs
- IDs are stable and unique.
- Prefixes indicate domain:
  - `DEL-*`: delivery
  - `SRV-*`: service runtime
  - `API-*`: API compatibility
  - `FEED-*`: feed lifecycle and refresh
  - `FOL-*`: folder behavior
  - `ITEM-*`: item/article behavior
  - `CNT-*`: content extraction and summarization
  - `EML-*`: email/newsletter ingestion
  - `SEC-*`: security
  - `CFG-*`: configuration
  - `CLI-*`: command-line behavior
  - `DAT-*`: persistence model and constraints

## Requirements

### Delivery
- `DEL-001`: The application shall be deliverable as a single self-hosted container image.

### Service Runtime
- `SRV-001`: On startup, the service shall initialize persistent storage connectivity.
- `SRV-002`: On startup, the service shall execute a feed refresh cycle.
- `SRV-003`: Startup refresh shall process all non-mailing-list feeds regardless of previous schedule.
- `SRV-004`: The service shall execute periodic feed refresh cycles based on `FEED_UPDATE_FREQUENCY_MIN`.

### API Compatibility
- `API-001`: The service shall expose health and version endpoints as defined in API contract documents.
- `API-002`: The service shall preserve Nextcloud News API compatibility for v1-2 and v1-3 contracts.
- `API-003`: API payload field naming shall preserve contract casing (including camelCase fields).

### Feed Lifecycle And Refresh
- `FEED-001`: The system shall parse and ingest both RSS and Atom feeds.
- `FEED-002`: Feed URLs shall be unique; duplicate feed creation shall be rejected.
- `FEED-003`: Feed creation shall reject non-existent target folders.
- `FEED-004`: Deleting a feed shall delete associated articles.
- `FEED-005`: The system shall support moving feeds between folders.
- `FEED-006`: The system shall support renaming feeds.
- `FEED-007`: Refresh scheduling shall persist `next_update_time` dynamically from recent publishing frequency.
- `FEED-008`: Dynamic scheduling algorithm shall:
  - use a 7-day average articles/day,
  - schedule sparse feeds (`<= 0.1/day`) at daily cadence with jitter of +/-30 minutes,
  - schedule active feeds at 4x observed daily frequency,
  - cap active-feed interval to at most 12 hours.
- `FEED-009`: Refresh failures shall increment `update_error_count` and persist `last_update_error`.
- `FEED-010`: Successful refresh shall clear persisted refresh error state.
- `FEED-011`: Stale feed articles not present in the latest payload shall be eligible for cleanup only when older than 90 days, read, and unstarred.

### Folder Behavior
- `FOL-001`: The system shall maintain an internal root folder and create it on demand if missing.
- `FOL-002`: Root folder shall be omitted from folder listing responses.
- `FOL-003`: `folderId: null` and `folderId: 0` shall map to root folder semantics where applicable.
- `FOL-004`: The system shall support creating, renaming, listing, and deleting user folders.
- `FOL-005`: Empty folder names shall be rejected.
- `FOL-006`: Duplicate folder names shall be rejected.
- `FOL-007`: Deleting a folder shall delete feeds in that folder.

### Item And Article Behavior
- `ITEM-001`: Articles shall persist stable GUID and GUID-hash identifiers.
- `ITEM-002`: Duplicate article insertion shall be prevented by GUID-hash de-duplication.
- `ITEM-003`: Newly inserted articles shall default to unread unless explicitly set otherwise.
- `ITEM-004`: Item retrieval shall support feed, folder, starred, and global selection modes.
- `ITEM-005`: Item retrieval shall support `last_modified` filtering.
- `ITEM-006`: Single and bulk read/unread operations shall be supported.
- `ITEM-007`: Single and bulk star/unstar operations shall be supported.
- `ITEM-008`: Read/star state changes shall update `last_modified`.
- `ITEM-009`: Mark-as-read operations shall support boundary behavior using newest item ID for feed, folder, and global scopes.

### Content Extraction And Summarization
- `CNT-001`: If feed metadata does not provide a thumbnail, the system shall extract the first image URL from HTML content when available.
- `CNT-002`: The system shall support optional full-text extraction from article URLs.
- `CNT-003`: Feed content quality evaluation shall run periodically (about monthly) and decide whether extracted full text should be used.
- `CNT-004`: `use_llm_summary` shall only be enabled when full-text extraction quality is sufficient.
- `CNT-005`: Optional LLM-generated summaries shall be supported when `OPENAI_API_KEY` is configured.
- `CNT-006`: LLM-generated summaries shall include the suffix ` (AI generated)`.
- `CNT-007`: If LLM summarization is disabled and content is long, summary generation shall fall back to truncation.

### Email Newsletter Ingestion
- `EML-001`: The system shall store IMAP credentials via CLI/API-internal paths.
- `EML-002`: Credential persistence shall require successful mailbox connectivity/login validation.
- `EML-003`: Update cycles shall fetch unread emails from configured mailboxes.
- `EML-004`: Only messages identified as mailing-list emails (for example via `List-Unsubscribe`) shall be treated as newsletters.
- `EML-005`: Mailing-list feeds shall be auto-created on first encounter of a sender.
- `EML-006`: Newsletter HTML shall be cleaned before article persistence.
- `EML-007`: When LLM support is enabled, newsletter parsing shall support single-article mode and multi-item mode (up to 25 items).
- `EML-008`: Stale newsletter entries shall be eligible for cleanup only when older than 90 days, read, and unstarred.

### Security
- `SEC-001`: Remote URL validation shall allow only `http` and `https` schemes.
- `SEC-002`: Remote URL validation shall block loopback, private, link-local, unspecified, multicast, and cloud metadata addresses.
- `SEC-003`: Localhost access may be allowed only in testing mode.
- `SEC-004`: The same URL validation policy shall be applied consistently in all remote-fetch paths.
- `SEC-005`: HTTP Basic auth shall be enforced only when both `USERNAME` and `PASSWORD` are configured.

### Configuration
- `CFG-001`: Runtime configuration shall be sourced from environment variables.
- `CFG-002`: Supported variables shall include `USERNAME`, `PASSWORD`, `FEED_UPDATE_FREQUENCY_MIN`, `VERSION`, `OPENAI_API_KEY`, `OPENAI_BASE_URL`, and `OPENAI_MODEL`.
- `CFG-003`: Defaults shall include `VERSION=dev`, `FEED_UPDATE_FREQUENCY_MIN=15`, and `OPENAI_MODEL=gpt-5-nano`.

### CLI
- `CLI-001`: A CLI `update` command shall initialize persistent storage access and execute a refresh cycle.
- `CLI-002`: A CLI `add-email-credentials` command shall require server, port, username, and password inputs.
- `CLI-003`: `add-email-credentials` shall return a user-visible error when credential validation fails.

### Data Model And Constraints
- `DAT-001`: Persistence shall include `Feed`, `Folder`, `Article`, and `EmailCredential` entities.
- `DAT-002`: Feed URL uniqueness shall be enforced.
- `DAT-003`: Folder name uniqueness shall be enforced.
