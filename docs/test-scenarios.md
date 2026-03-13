# Implemented Test Scenarios Baseline

## Purpose
This document defines end-to-end user journeys for headless-rss.
Each scenario describes a realistic flow across multiple endpoints or command paths, including setup, execution, and expected outcomes.

## Scope
- Focuses on user-visible journeys instead of isolated unit behavior.
- Covers API-first usage with Nextcloud News compatibility.
- Includes operational flows (startup, auth).

## Scenario Format
Each scenario includes:
- Stable ID for traceability.
- Journey-oriented description.
- Preconditions and main actions.
- Expected result that confirms the user goal is met.

## Shared Test Inputs
- Each scenario must start with an empty database.
- Use these feed URLs when creating feeds in scenarios:
  - `https://www.tagesschau.de/index~rss2.xml`
  - `https://www.heise.de/rss/heise-atom.xml`
  - `https://www.heise.de/rss/heise-top-atom.xml`
  - `https://simonwillison.net/atom/everything/`
- After adding feeds, run at least one update cycle before assertions.

## End-to-End Test Scenarios

### Service Startup and Reachability

| ID | Journey | Preconditions | Actions | Expected Result |
|---|---|---|---|---|
| TS-E2E-001 | Start service and check health | Database is empty. | Start service, call `/status`, add the shared feed set, run update cycle, then call `/status` again. | Both health checks return `200` with `{"status":"ok"}` after feed ingestion and updates. |
| TS-E2E-002 | Verify Nextcloud compatibility baseline | Database is empty and service is running. | Call v1-3 `/version`, add the shared feed set, run update cycle, then call v1-3 `/feeds` and `/folders`. | Version responds `200`; feeds list contains created feeds; folders list remains valid for root-only setup. |

### First-Time Feed Onboarding

| ID | Journey | Preconditions | Actions | Expected Result |
|---|---|---|---|---|
| TS-E2E-003 | Add feeds and receive initial items | Database is empty and service is running. | Create feeds using the shared feed set, run update cycle, then query `/items` per feed. | All feeds are created and items are returned for feeds with available entries. |
| TS-E2E-004 | Add feeds into non-root folder | Database is empty and service is running. | Create folder, add most shared feeds with `folderId` set to that folder, leave at least one feed in root, run update cycle, then query feeds list. | Folder-assigned feeds appear in target folder; root feed remains root-mapped. |
| TS-E2E-005 | Prevent duplicate feed add | Database is empty and service is running. | Add the shared feed set, run update cycle, then submit add-feed request again for one existing URL. | Duplicate add fails with conflict semantics and feed count does not increase. |

### Reading Workflow (Client Sync Behavior)

| ID | Journey | Preconditions | Actions | Expected Result |
|---|---|---|---|---|
| TS-E2E-006 | Fetch unread timeline and mark one item read | Database is empty and service is running. | Add the shared feed set, run update cycle, query `/items` unread-only, mark one item read, query unread-only again. | Marked item no longer appears in unread-only results and sync metadata remains consistent. |
| TS-E2E-007 | Mark all feed items read up to newest item | Database is empty and service is running. | Add shared feeds, run update cycle, pick one feed, call feed-read endpoint with `newestItemId`, then fetch unread items for that feed. | Items up to boundary are read and excluded from unread-only results for that feed. |
| TS-E2E-008 | Incremental sync with updated-items | Database is empty and service is running. | Add shared feeds, run update cycle, record timestamp, perform read/star updates on some items, then call `/items/updated` with recorded timestamp. | Response contains only changed items since timestamp and respects ordering contract. |

### Saved Items and Stars

| ID | Journey | Preconditions | Actions | Expected Result |
|---|---|---|---|---|
| TS-E2E-009 | Star and unstar item in v1-3 flow | Database is empty and service is running. | Add the shared feed set, run update cycle, star one item via v1-3 route, verify starred list, unstar it, verify list again. | Item appears in starred results after star and disappears after unstar. |
| TS-E2E-010 | Star by guid-hash in v1-2 flow | Database is empty and service is running. | Add the shared feed set, run update cycle, choose item guid-hash, call v1-2 guid-hash star route, then fetch items. | Target item becomes starred and remains compatible with v1-2 payload/route contract. |

### Folder and Feed Management

| ID | Journey | Preconditions | Actions | Expected Result |
|---|---|---|---|---|
| TS-E2E-011 | Rename folder and keep feed associations | Database is empty and service is running. | Create folder, add two shared feeds into it, run update cycle, rename folder, query folders/feeds/items by folder selection. | Folder name changes while feed association and item retrieval stay intact. |
| TS-E2E-012 | Move feed between folders | Database is empty and service is running. | Create folders A and B, add shared feeds into A, run update cycle, move one feed to B, then query folder-scoped feeds/items. | Moved feed appears only in destination folder selection and retains items. |
| TS-E2E-013 | Delete feed and validate cascade | Database is empty and service is running. | Add the shared feed set, run update cycle, delete one feed, then query feeds and items. | Deleted feed is absent and its associated items are removed. |
| TS-E2E-014 | Delete folder and validate cascade | Database is empty and service is running. | Create folder, add two shared feeds to folder and one to root, run update cycle, delete folder, then query folders/feeds/items. | Folder is removed; feeds and items in that folder are deleted; root feed remains. |

### Authentication and Access Control

| ID | Journey | Preconditions | Actions | Expected Result |
|---|---|---|---|---|
| TS-E2E-015 | Auth disabled mode permits anonymous access | Database is empty; `USERNAME` and `PASSWORD` are not set. | Start service, add the shared feed set anonymously, run update cycle, then call protected endpoints without auth header. | Feed operations and protected endpoint requests succeed without auth challenge. |
| TS-E2E-016 | Auth enabled mode enforces credentials | Database is empty; `USERNAME` and `PASSWORD` are set. | Start service, attempt to add the shared feed set with no credentials, invalid credentials, then valid credentials; run update cycle with valid credentials. | Missing/invalid credentials return `401`; valid credentials allow feed creation and follow-up API use. |

### Newsletter and Background Update Journeys

| ID | Journey | Preconditions | Actions | Expected Result |
|---|---|---|---|---|
| TS-E2E-017 | Store email credentials and ingest newsletters | Database is empty; mock IMAP mailbox messages are available to the Rust test harness. | Add the shared feed set, run update cycle, execute add-email-credentials command with test-mode mailbox validation, run another update cycle with mocked unread emails, query feeds/items. | Feed items exist from RSS/Atom sources and newsletter feed/items are created from mocked unread mailing-list emails. |
| TS-E2E-018 | Background updater handles source failures gracefully | Database is empty and service is running. | Add the shared feed set and one intentionally invalid URL, run update cycle, inspect feed metadata. | Valid feeds update normally; invalid feed records error metadata (`updateErrorCount`, `lastUpdateError`) without affecting service health. |
