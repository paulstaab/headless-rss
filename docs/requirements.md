# Requirements Baseline

## Purpose
This document defines the product requirements independent of implementation language.
The same requirements apply to all implementations.

Implementation progress is tracked separately in `docs/implementation-status.md`.

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
  - `OBS-*`: observability
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
- `ITEM-010`: v1.2 GUID-hash bulk star/unstar requests shall reject payloads with more than 10,000 items using `400 Bad Request`.

### Content Extraction And Summarization
- `CNT-001`: If feed metadata does not provide a thumbnail, the system shall extract the first image URL from HTML content when available.
- `CNT-002`: The system shall support optional full-text extraction from article URLs.
- `CNT-003`: Feed content quality evaluation:
  - shall run when `last_quality_check` is missing or older than about 30 days,
  - shall use a representative feed entry that has both a link and feed-provided content or summary,
  - shall evaluate `use_extracted_fulltext` and `use_llm_summary` in the same periodic quality-check pass,
  - shall compare normalized feed content against extracted article content from the selected article URL,
  - shall treat extracted full text as higher quality only when the normalized extracted text is non-empty and at least twice as long as the normalized feed content,
  - shall persist the quality-check timestamp after a completed evaluation.
- `CNT-004`: Full-text and LLM summary enablement for feeds during quality evaluation:
  - `use_extracted_fulltext` shall be enabled only when feed content quality evaluation judges extracted full text to be higher quality,
  - `use_llm_summary` shall be evaluated independently of `use_extracted_fulltext`, after the content-quality decision determines the final article text to assess,
  - `use_llm_summary=true` shall mean that feed-provided summaries are not good enough and LLM-generated summaries should be used for that feed,
  - when LLM support is configured, the summary-quality check shall ask the LLM whether the feed-provided summary is a good standalone summary of the final chosen article text,
  - when LLM support is configured, `use_llm_summary` shall be enabled if the feed summary is missing or judged not good enough, and disabled if the feed summary is judged good enough,
  - when LLM support is not configured, the summary-quality check shall fall back to a heuristic that enables `use_llm_summary` when the normalized feed summary is missing or closely matches the beginning of the final chosen article text.
- `CNT-005`: Optional LLM-based article summarization when loading new articles:
  - shall run only for new articles after GUID-hash de-duplication confirms the article is not already stored,
  - shall be enabled only when LLM support is configured and LLM summarization is enabled for the articles feed,
  - shall strip HTML from article content before sending it to the model,
  - shall truncate article text to the first 8000 characters before LLM summarization,
  - shall request a concise plain-text summary in structured JSON form with a top-level `summary` field.
- `CNT-006`: Successful LLM-generated summaries shall:
  - be accepted only when a non-empty `summary` value is returned,
  - accept equivalent provider responses that wrap `summary` under the configured structured-output schema name,
  - include the suffix ` (AI generated)`.
- `CNT-007`: Automatic summary generation for articles without an existing summary:
  - shall run only for new articles before they are inserted into the database,
  - shall copy the full article content into the summary when content length is below 160 characters,
  - shall use LLM summarization only when content length is at least 160 characters and LLM summarization is both requested and enabled,
  - shall fall back to the first 160 characters plus `...` when content is long and LLM summarization is not requested, not enabled, or does not return a usable summary.

### Email Newsletter Ingestion
- `EML-001`: The system shall store IMAP credentials via CLI/API-internal paths.
- `EML-002`: Credential persistence shall require successful mailbox connectivity/login validation.
- `EML-003`: Update cycles shall fetch unread emails from configured mailboxes.
- `EML-004`: Only messages identified as mailing-list emails (for example via `List-Unsubscribe`) shall be treated as newsletters.
- `EML-005`: Mailing-list feeds shall be auto-created on first encounter of a sender.
- `EML-006`: Newsletter HTML shall be cleaned before article persistence.
- `EML-007`: Optional LLM-based newsletter parsing:
  - Optional LLM-based newsletter parsing shall be enabled only when LLM support is configured.
  - Before LLM-based newsletter parsing, newsletter content shall be truncated to the first 5000 characters.
  - LLM-based newsletter parsing shall support `single` mode and `multi` mode.
  - If a newsletter is a collection of links to different articles, it shall be parsed in `multi` mode. Otherwise, it shall be parsed in `single` mode.
  - In `single` mode, the parser should return the cleaned newsletter content as article and a concise, generated summary.
  - In `multi` mode, the parser should return an list of articles linked in the newsletter, so that they can be shown as separate entries in the generated feed.
  - LLM-based multi-item parsing shall create at most 25 articles from a single newsletter email.
- `EML-008`: If LLM-based parsing is disabled, fails, returns invalid JSON, or produces no usable multi-item entries, newsletter ingestion shall fall back to creating a single article from the cleaned email content.
- `EML-009`: Stale newsletter entries shall be eligible for cleanup only when older than 90 days, read, and unstarred.

### Observability
- `OBS-001`: All incoming API requests shall be logged at `INFO` level with the matched route and query parameters.
- `OBS-002`: All outbound LLM requests shall be logged at `INFO` level with the task name and, when available, the associated `feed_id` and `article_id`.
- `OBS-003`: Extraction of article content as described in `CNT-002` shall be logged with `feed_id`, `article_id` loaded URL at `INFO` level.

### Security
- `SEC-001`: Remote URL validation shall allow only `http` and `https` schemes.
- `SEC-002`: Remote URL validation shall block loopback, private, link-local, unspecified, multicast, and cloud metadata addresses.
- `SEC-003`: Localhost access may be allowed only in testing mode.
- `SEC-004`: The same URL validation policy shall be applied consistently in all remote-fetch paths.
- `SEC-005`: HTTP Basic auth shall be enforced only when both `USERNAME` and `PASSWORD` are configured.
- `SEC-006`: The system shall not log user-provided content, including titles, feed content, mailbox identities, or externally generated content derived from user input unless explicitly required by an `OBS` requirement.

### Configuration
- `CFG-001`: Runtime configuration shall be sourced from environment variables.
- `CFG-002`: Supported variables shall include authentication settings, feed update frequency, service version, and provider-specific LLM configuration including request timeout.
- `CFG-003`: Defaults shall include `VERSION=dev`, `FEED_UPDATE_FREQUENCY_MIN=15`, `OPENAI_TIMEOUT_SECONDS=30`, and a default LLM model identifier.

### CLI
- `CLI-001`: A CLI `update` command shall initialize persistent storage access and execute a refresh cycle.
- `CLI-002`: A CLI `add-email-credentials` command shall require server, port, username, and password inputs.
- `CLI-003`: `add-email-credentials` shall return a user-visible error when credential validation fails.

### Data Model And Constraints
- `DAT-001`: Persistence shall include `Feed`, `Folder`, `Article`, and `EmailCredential` entities.
- `DAT-002`: Feed URL uniqueness shall be enforced.
- `DAT-003`: Folder name uniqueness shall be enforced.
