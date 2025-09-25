# Feed Aggregation

## Overview
- Aggregate external RSS and Atom feeds in all common formats into the local SQLite-backed store.
- Run periodic updates to import new articles.

## Data Model
- `Feed` records store `url`, `title`, `favicon_link`, `folder_id`, `next_update_time`, `update_error_count`, `last_update_error`, `is_mailing_list` and metadata fields required by Nextcloud clients.
- `Article` records keep the parsed entry (`title`, `body`, `author`, `url`, enclosure metadata) plus deduplication helpers (`guid`, `guid_hash`, `fingerprint`, `content_hash`), state flags (`unread`, `starred`), and timestamps (`pub_date`, `updated_date`, `last_modified`).
- A folder hierarchy exists; An implicit root folder (`is_root=True`) is auto-created.

## Feed ingestion lifecycle
- **Adding feeds**
  - Validate the target URL (see Security) before parsing with `feedparser`.
  - Reject duplicates by URL and ensure the target folder exists; fall back to the root folder when none is provided.
  - Persist the feed with metadata extracted from the parsed document (title, site link, favicon) and timestamp the creation.
  - Immediately import up to 10 of the freshest entries to seed the article list.
- **Updating feeds**
  - Fetch the stored feed URL, parse it, and reset error state on success. Any exception increments `update_error_count`, records the message in `last_update_error`, and aborts the current cycle without raising outward errors.
  - Iterate entries in published order (first `max_articles`, default 50). For each entry:
    - Build an article with content preference `entry.content[0].value` → `entry.summary`.
    - Derive a GUID priority `entry.id` → `entry.link` → `entry.title`; absence of all raises an error and skips the entry.
    - Normalize `published_parsed` and `updated_parsed` to UNIX timestamps, defaulting to "now" when missing.
    - Skip persistence when another article with the same `guid_hash` already exists; otherwise insert and mark it unread.
  - Compute `next_update_time` using the average number of articles per day over the last seven days: poll four times faster than the average but at least every 12 hours, or once per day (±30 minutes jitter) when no recent articles exist.
  - After committing feed changes, prune aged articles (see Cleanup) outside the transaction.
- **Bulk updates**
  - `feed.update_all()` selects feeds whose `next_update_time` is `NULL` or due and excludes mailing list feeds.

## Article lifecycle
- Deduplicate strictly on `guid_hash`; fingerprints provide additional stability across clients.
- Articles default to `unread=True`, `starred=False`, `rtl=False` and maintain `last_modified` on each state change.
- Old articles are removed from the database at the end of update if they are older than 90 days and are read, unstarred, and absent from the current feed payload.

## Scheduling and execution
- On startup the applications performs an immediate global update and then updates all due feeds as a background task every `FEED_UPDATE_FREQUENCY_MIN` minutes (default 15).
- The CLI sub command `update` triggers a feed update for cron usage.

## Security and validation
- Feed URLs must use `http`/`https`, include a hostname, and may only resolve to public IP addresses. The validator blocks localhost, loopback, private, link-local, multicast, unspecified, and cloud metadata ranges (`169.254.169.254`).
- Localhost access is automatically allowed during automated tests but rejected in production unless explicitly overridden.
