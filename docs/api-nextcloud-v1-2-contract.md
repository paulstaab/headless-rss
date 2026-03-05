# API Contract: Nextcloud News v1.2

## Scope
Contract for the implemented Nextcloud News compatible API v1.2.

## Base Path
- In the full app: `/index.php/apps/news/api/v1-2`

## Authentication
- Mechanism: HTTP Basic
- Enabled only when both `USERNAME` and `PASSWORD` environment variables are set.
- Protected route behavior:
  - Missing credentials: `401` with `{"detail":"Not authenticated"}`
  - Invalid credentials: `401` with `{"detail":"Invalid authentication credentials"}`
- `GET /version` is not protected by auth.

## Endpoints

### Version
- `GET /version`
  - Response: `200`
  - Body: `{"version": "<string>"}`

### Feeds
- `GET /feeds`
  - Returns `{"feeds": [...]}`
- `POST /feeds`
  - Request: `{"url": "<feed-url>", "folderId": <int|null>}`
  - `folderId` of `null` or `0` maps to root folder.
  - Success: `200`, body includes `feeds` and `newestItemId`.
  - Errors:
    - `409` feed exists
    - `422` invalid folder or unreadable/invalid feed
    - `400` URL rejected by SSRF protection
- `DELETE /feeds/{feed_id}`
  - Success: `200`
  - Error: `404` feed not found
- `PUT /feeds/{feed_id}/move`
  - Request: `{"folderId": <int|null>}`
  - Errors: `404` feed missing, `422` folder invalid
- `PUT /feeds/{feed_id}/rename`
  - Request: `{"feedTitle": "<string>"}`
  - Error: `404` when feed not found
- `PUT /feeds/{feed_id}/read`
  - Request: `{"newestItemId": <int>}`

### Folders
- `GET /folders`
  - Returns non-root folders only: `{"folders": [...]}`
- `POST /folders`
  - Request: `{"name":"<string>"}`
  - Errors: `409` duplicate, `422` invalid name
- `DELETE /folders/{folder_id}`
  - Error: `404` missing folder
- `PUT /folders/{folder_id}`
  - Request: `{"name":"<string>"}`
  - Errors: `404` missing folder, `409` duplicate, `422` invalid name
- `POST /folders/{folder_id}/read`
  - Request: `{"newestItemId": <int>}`
  - Error: `404` missing folder

### Items
- `GET /items`
  - Query params:
    - `batchSize` (default `-1`)
    - `offset` (default `0`)
    - `type` (`0=feed`, `1=folder`, `2=starred`, `3=all`; default `1`)
    - `id` (default `0`)
    - `getRead` (default `true`)
    - `oldestFirst` (default `false`)
    - `lastModified` (default `0`)
  - Response: `{"items": [...]}`
- `GET /items/updated`
  - Query params: `lastModified`, `type`, `id`
  - Response: `{"items": [...]}`
- `GET /items/{item_id}/content`
  - Response: `{"content": "..."}` or `{"content": null}`
  - Error: `404` if item missing
- `POST /items/{item_id}/read`
- `PUT /items/read/multiple`
  - Request: `{"items":[<int>, ...]}`
- `PUT /items/{item_id}/unread`
- `PUT /items/unread/multiple`
  - Request: `{"items":[<int>, ...]}`
- `PUT /items/{feedId}/{guidHash}/star`
- `PUT /items/star/multiple`
  - Request: `{"items":[{"feedId":<int>,"guidHash":"<string>"}, ...]}`
  - Error: `404` if any item unresolved
- `PUT /items/{feedId}/{guidHash}/unstar`
- `PUT /items/unstar/multiple`
  - Request: same shape as star/multiple
  - Error: `404` if any item unresolved
- `PUT /items/read`
  - Request: `{"newestItemId": <int>}`

## Item Payload Shape (returned by item list APIs)
Each item uses camelCase fields and includes:
- `id`, `title`, `author`, `body`, `feedId`, `guid`, `guidHash`, `url`
- `contentHash`, `fingerprint`, `lastModified`, `pubDate`, `updatedDate`
- `unread`, `starred`, `rtl`
- `enclosureLink`, `enclosureMime`, `mediaDescription`, `mediaThumbnail`
