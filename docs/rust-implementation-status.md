# Rust Implementation Status

## Purpose
This document tracks Rust implementation progress against the technology-independent requirement IDs defined in `docs/requirements.md`.

## Status Legend
- `Implemented`: Requirement behavior exists and is verified by code paths and/or tests.
- `Partial`: Some behavior exists, but scope is incomplete.
- `Not Started`: No implementation yet in Rust.
- `Unknown`: Status has not been verified yet.

## Last Updated
- 2026-03-13

## Status By Requirement

| ID | Status | Notes |
| --- | --- | --- |
| DEL-001 | Implemented | Containerized runtime and Rust service binary are in place. |
| SRV-001 | Implemented | Storage pool/migrations initialized on startup. |
| SRV-002 | Implemented | Startup update cycle runs. |
| SRV-003 | Implemented | Startup refresh forces all non-mailing-list feeds. |
| SRV-004 | Implemented | Periodic refresh loop uses `FEED_UPDATE_FREQUENCY_MIN`. |
| API-001 | Implemented | Health/version endpoints present. |
| API-002 | Implemented | v1-2 and v1-3 API routes implemented with compatibility tests. |
| API-003 | Implemented | Contract field casing is preserved in API responses. |
| FEED-001 | Implemented | Atom/RSS parsing implemented in feed ingest/update. |
| FEED-002 | Implemented | Duplicate feed URLs rejected. |
| FEED-003 | Implemented | Missing folder on feed create returns validation error. |
| FEED-004 | Implemented | Feed deletion cascades article deletion in API flow. |
| FEED-005 | Implemented | Feed move endpoints implemented. |
| FEED-006 | Implemented | Feed rename endpoints implemented. |
| FEED-007 | Implemented | `next_update_time` dynamically persisted after updates. |
| FEED-008 | Implemented | 7-day average + jitter + 12h cap implemented. |
| FEED-009 | Implemented | `update_error_count` and `last_update_error` persisted on failures. |
| FEED-010 | Implemented | Error state cleared on successful updates. |
| FEED-011 | Implemented | Rust updater now removes stale feed articles only when missing from the latest payload, older than 90 days, read, and unstarred. |
| FOL-001 | Implemented | Internal root folder bootstrap exists. |
| FOL-002 | Implemented | Root folder omitted from folder listings. |
| FOL-003 | Implemented | `folderId` root/null mapping implemented. |
| FOL-004 | Implemented | Folder create/rename/list/delete supported. |
| FOL-005 | Implemented | Empty folder names rejected. |
| FOL-006 | Implemented | Duplicate folder names rejected. |
| FOL-007 | Implemented | Deleting folder removes associated feeds/articles in API flow. |
| ITEM-001 | Implemented | GUID and GUID-hash persistence implemented. |
| ITEM-002 | Implemented | GUID-hash de-duplication implemented. |
| ITEM-003 | Implemented | New articles are inserted unread by default. |
| ITEM-004 | Implemented | Feed/folder/starred/global item selection implemented. |
| ITEM-005 | Implemented | `last_modified` item filtering implemented. |
| ITEM-006 | Implemented | Single/bulk read and unread operations implemented. |
| ITEM-007 | Implemented | Single/bulk star and unstar operations implemented. |
| ITEM-008 | Implemented | Read/star changes update `last_modified`. |
| ITEM-009 | Implemented | Boundary read operations by newest item ID implemented. |
| CNT-001 | Implemented | First image extraction from HTML content implemented. |
| CNT-002 | Implemented | Rust article ingestion now supports optional full-text extraction from article URLs using Mozilla Readability via `readability-js`. |
| CNT-003 | Implemented | Rust updater and feed-create paths now run monthly feed-quality evaluation and persist the decision flags. |
| CNT-004 | Implemented | Rust only enables `use_llm_summary` when extracted full-text quality is sufficient. |
| CNT-005 | Implemented | Rust supports optional OpenAI-backed article summary generation when `OPENAI_API_KEY` is configured. |
| CNT-006 | Implemented | Rust appends ` (AI generated)` to successful LLM summaries. |
| CNT-007 | Implemented | Rust falls back to truncation when LLM summarization is disabled and content is long. |
| EML-001 | Implemented | Email credentials storage path exists. |
| EML-002 | Implemented | IMAP connectivity/login validation before persistence exists. |
| EML-003 | Implemented | Update cycles now fetch unread IMAP messages from configured mailboxes and persist newsletter articles; journey coverage includes a mocked IMAP subprocess flow. |
| EML-004 | Implemented | Rust only processes messages identified as mailing-list emails via `List-Unsubscribe`. |
| EML-005 | Implemented | Mailing-list feeds are auto-created on first sender encounter under the root folder. |
| EML-006 | Implemented | Newsletter HTML is cleaned before persistence and stored in reader-friendly form. |
| EML-007 | Implemented | Rust supports LLM-driven newsletter parsing for `single` and `multi` modes, capped to 25 items. |
| EML-008 | Implemented | Rust cleans up stale newsletter entries only when older than 90 days, read, and unstarred. |
| SEC-001 | Implemented | Scheme allowlist validation implemented. |
| SEC-002 | Implemented | IP/DNS SSRF protections implemented. |
| SEC-003 | Implemented | Localhost allowance is limited to testing mode. |
| SEC-004 | Implemented | Shared `ssrf` module used by API and updater paths. |
| SEC-005 | Implemented | Basic auth conditional behavior implemented. |
| CFG-001 | Implemented | Environment-based runtime config exists. |
| CFG-002 | Implemented | Required configuration variables supported. |
| CFG-003 | Implemented | Defaults (`dev`, `15`, `gpt-5-nano`) configured. |
| CLI-001 | Implemented | `update` command exists and runs updater. |
| CLI-002 | Implemented | `add-email-credentials` command arguments are required. |
| CLI-003 | Implemented | Validation failures return user-visible errors. |
| DAT-001 | Implemented | Core entities (`Feed`, `Folder`, `Article`, `EmailCredential`) exist. |
| DAT-002 | Implemented | Feed URL uniqueness enforced. |
| DAT-003 | Implemented | Folder name uniqueness enforced. |

## Notes
- This file tracks Rust progress only.
- Python implementation tracking can be added in a separate status document if desired.
