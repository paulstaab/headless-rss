# Rust Implementation Status

## Purpose
This document tracks Rust implementation progress against the technology-independent requirement IDs defined in `docs/requirements.md`.

## Status Legend
- `Implemented`: Requirement behavior exists and is verified by code paths and/or tests.
- `Partial`: Some behavior exists, but scope is incomplete.
- `Not Started`: No implementation yet in Rust.
- `Unknown`: Status has not been verified yet.

## Last Updated
- 2026-03-10

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
| CNT-002 | Not Started | Optional full-text extraction not fully ported. |
| CNT-003 | Not Started | Monthly feed-quality evaluation not fully ported. |
| CNT-004 | Not Started | `use_llm_summary` quality gating not fully ported. |
| CNT-005 | Not Started | Optional LLM summary generation not fully ported. |
| CNT-006 | Not Started | LLM summary suffix behavior not fully ported. |
| CNT-007 | Not Started | Non-LLM truncation fallback not fully ported. |
| EML-001 | Implemented | Email credentials storage path exists. |
| EML-002 | Implemented | IMAP connectivity/login validation before persistence exists. |
| EML-003 | Partial | End-to-end newsletter ingestion is not fully verified in journey tests. |
| EML-004 | Partial | Mailing-list message filtering exists but needs broader parity verification. |
| EML-005 | Partial | Auto-creation behavior requires additional parity verification. |
| EML-006 | Partial | Newsletter cleanup/transformation path needs broader parity verification. |
| EML-007 | Not Started | LLM newsletter parsing modes not fully ported. |
| EML-008 | Not Started | Newsletter stale-entry cleanup parity not fully ported. |
| SEC-001 | Implemented | Scheme allowlist validation implemented. |
| SEC-002 | Implemented | IP/DNS SSRF protections implemented. |
| SEC-003 | Implemented | Localhost allowance is limited to testing mode. |
| SEC-004 | Implemented | Shared `ssrf` module used by API and updater paths. |
| SEC-005 | Implemented | Basic auth conditional behavior implemented. |
| CFG-001 | Implemented | Environment-based runtime config exists. |
| CFG-002 | Implemented | Required configuration variables supported. |
| CFG-003 | Implemented | Defaults (`dev`, `15`, `gpt-5-mini`) configured. |
| CLI-001 | Implemented | `update` command exists and runs updater. |
| CLI-002 | Implemented | `add-email-credentials` command arguments are required. |
| CLI-003 | Implemented | Validation failures return user-visible errors. |
| DAT-001 | Implemented | Core entities (`Feed`, `Folder`, `Article`, `EmailCredential`) exist. |
| DAT-002 | Implemented | Feed URL uniqueness enforced. |
| DAT-003 | Implemented | Folder name uniqueness enforced. |

## Notes
- This file tracks Rust progress only.
- Python implementation tracking can be added in a separate status document if desired.
