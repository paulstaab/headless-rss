# API Test Cases: Nextcloud News v1.3

## Scope
Test cases for implemented Nextcloud News API v1.3 behavior.

## Authentication
- Verify missing credentials return `401` with `Not authenticated` when auth is enabled.
- Verify invalid credentials return `401` with `Invalid authentication credentials`.
- Verify valid credentials permit protected endpoint access.

## Version
- Verify `GET /version` responds with `200`.

## Feeds
- Verify feed creation succeeds and returns feed metadata plus `newestItemId`.
- Verify creating the same feed twice returns conflict (`409`).
- Verify unreadable feed URLs return validation failure (`422`).
- Verify feed creation rejects redirect targets blocked by SSRF policy.
- Verify feed deletion removes feed and associated items.
- Verify deleting unknown feeds returns not found (`404`).
- Verify feed move endpoint updates target folder.
- Verify feed rename endpoint updates title.
- Verify feed read endpoint marks feed items as read.
- Verify root-folder mapping for `folderId: 0` and `folderId: null`.
- Verify `nextUpdateTime` is set.

## Folders
- Verify folder list endpoint works and excludes internal root folder.
- Verify folder creation success path.
- Verify duplicate folder creation returns conflict.
- Verify invalid folder names return validation error.
- Verify folder deletion success and not-found behavior.
- Verify folder rename success and duplicate/invalid-name errors.
- Verify folder read endpoint marks folder items as read.

## Items
- Verify item listing by feed and other supported query filters.
- Verify updated-items endpoint by `lastModified`.
- Verify item content endpoint success and missing-item not-found behavior.
- Verify read/unread single-item endpoints.
- Verify read/unread bulk endpoints with v1.3 payload shape (`itemIds`).
- Verify star/unstar single-item endpoints using `itemId` route format.
- Verify star/unstar bulk endpoints with item-id list payload.
- Verify mark-all-read endpoint behavior.
- Verify state-changing operations update `lastModified`.

## Primary Test Files
- `tests/api/nextcloud_news/v1_3/test_auth.py`
- `tests/api/nextcloud_news/v1_3/test_feed.py`
- `tests/api/nextcloud_news/v1_3/test_folder.py`
- `tests/api/nextcloud_news/v1_3/test_item.py`
- `tests/api/nextcloud_news/v1_3/test_version.py`
