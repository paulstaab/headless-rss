# Nextcloud News compatible API

headless-rss should provide an API that follows the Nextcloud News API specification so that clients
that support this API specification can use headless-rss as their backend service.

It should support both versions [1.2](https://github.com/nextcloud/news/blob/master/docs/api/api-v1-2.md)
and [1.3](https://github.com/nextcloud/news/blob/master/docs/api/api-v1-3.md) of the API spec.

## Nextcloud News API v1.2

Base path: `/index.php/apps/news/api/v1-2`

### Authentication
- HTTP Basic. Required for every endpoint when `USERNAME` and `PASSWORD` environment variables are set. Missing or wrong credentials return `401` with `{"detail": "Not authenticated"}` or `{"detail": "Invalid authentication credentials"}`.

### Feeds
- **GET `/feeds`** – Lists all feeds. Returns `{ "feeds": [ { "id", "url", "title", "faviconLink", "added", "nextUpdateTime", "folderId" (root reported as `null`), "ordering", "link", "pinned", "updateErrorCount", "lastUpdateError" } ] }`.
- **POST `/feeds`** – Adds a feed. Body `{ "url": string, "folderId": int|null }` (`null`/`0` targets the root folder). Success returns `{ "feeds": [...], "newestItemId": int }`. Errors: `409` if feed exists, `422` for parsing or invalid folder, `400` for SSRF rejection.
- **DELETE `/feeds/{feedId}`** – Deletes a feed. `404` if unknown.
- **PUT `/feeds/{feedId}/move`** – Moves a feed. Body `{ "folderId": int|null }`. Errors: `404` missing feed, `422` invalid folder.
- **PUT `/feeds/{feedId}/rename`** – Renames a feed. Body `{ "feedTitle": string }`. `404` on invalid feed/title.
- **PUT `/feeds/{feedId}/read`** – Marks feed items as read up to `{ "newestItemId": int }`. `404` if feed/items missing.

### Folders
- **GET `/folders`** – Lists folders (root omitted). Returns `{ "folders": [ { "id", "name" } ] }`.
- **POST `/folders`** – Creates a folder. Body `{ "name": string }`. Success returns new folder list. Errors: `409` duplicate, `422` invalid name.
- **DELETE `/folders/{folderId}`** – Deletes a folder. `404` if missing.
- **PUT `/folders/{folderId}`** – Renames a folder. Body `{ "name": string }`. Errors: `404` missing folder, `409` duplicate name, `422` invalid name.
- **POST `/folders/{folderId}/read`** – Marks items in folder as read up to `{ "newestItemId": int }`. `404` if folder missing.

### Items
- **GET `/items`** – Retrieves items. Query parameters: `batchSize` (default `-1`), `offset` (`0`), `type` (`0` feed, `1` folder, `2` starred, `3` all), `id` (`0`), `getRead` (`true`), `oldestFirst` (`false`), `lastModified` (`0`). Returns `{ "items": [ { "id", "title", "content", "body", "author", "feedId", "guid", "guidHash", "pubDate", "updatedDate", "lastModified", "url", "unread", "starred", ... } ] }`.
- **GET `/items/updated`** – Retrieves items changed since `lastModified`. Query: `lastModified` (required), `type`, `id`. Response matches `/items`.
- **POST `/items/{itemId}/read`** – Marks one item read. `404` if missing.
- **PUT `/items/read/multiple`** – Marks multiple items read. Body `{ "items": [int] }`.
- **PUT `/items/{itemId}/unread`** – Marks one item unread. `404` if missing.
- **PUT `/items/unread/multiple`** – Marks multiple items unread. Body `{ "items": [int] }`.
- **PUT `/items/{feedId}/{guidHash}/star`** – Stars an item by feed and GUID hash. `404` if unknown.
- **PUT `/items/star/multiple`** – Stars multiple items. Body `{ "items": [ { "feedId": int, "guidHash": string } ] }`. `404` if any unresolved.
- **PUT `/items/{feedId}/{guidHash}/unstar`** – Unstars an item. `404` if unknown.
- **PUT `/items/unstar/multiple`** – Unstars multiple items. Body matches the star payload.
- **PUT `/items/read`** – Marks all items up to `{ "newestItemId": int }` as read.

### Version
- **GET `/version`** – Returns `{ "version": string }` (defaults to `"dev"` when no `VERSION` environment variable is set).

## Nextcloud News API v1.3

Base path: `/index.php/apps/news/api/v1-3`

### Authentication
- HTTP Basic. Required for every endpoint when `USERNAME` and `PASSWORD` environment variables are set. Missing or wrong credentials return `401` with `{"detail": "Not authenticated"}` or `{"detail": "Invalid authentication credentials"}`.

### Feeds
- **GET `/feeds`** – Lists all feeds. Returns `{ "feeds": [ { "id", "url", "title", "faviconLink", "added", "nextUpdateTime", "folderId" (root reported as `null`), "ordering", "link", "pinned", "updateErrorCount", "lastUpdateError" } ] }`.
- **POST `/feeds`** – Adds a feed. Body `{ "url": string, "folderId": int|null }` (`null`/`0` targets the root folder). Success returns `{ "feeds": [...], "newestItemId": int }`. Errors: `409` if feed exists, `422` for parsing or invalid folder, `400` for SSRF rejection.
- **DELETE `/feeds/{feedId}`** – Deletes a feed. `404` if unknown.
- **POST `/feeds/{feedId}/move`** – Moves a feed. Body `{ "folderId": int|null }`. Errors: `404` missing feed, `422` invalid folder.
- **POST `/feeds/{feedId}/rename`** – Renames a feed. Body `{ "feedTitle": string }`. `404` on invalid feed/title.
- **POST `/feeds/{feedId}/read`** – Marks feed items as read up to `{ "newestItemId": int }`. `404` if feed/items missing.

### Folders
- **GET `/folders`** – Lists folders (root omitted). Returns `{ "folders": [ { "id", "name" } ] }`.
- **POST `/folders`** – Creates a folder. Body `{ "name": string }`. Success returns new folder list. Errors: `409` duplicate, `422` invalid name.
- **DELETE `/folders/{folderId}`** – Deletes a folder. `404` if missing.
- **PUT `/folders/{folderId}`** – Renames a folder. Body `{ "name": string }`. Errors: `404` missing folder, `409` duplicate name, `422` invalid name.
- **POST `/folders/{folderId}/read`** – Marks items in folder as read up to `{ "newestItemId": int }`. `404` if folder missing.

### Items
- **GET `/items`** – Retrieves items. Query parameters: `batchSize` (default `-1`), `offset` (`0`), `type` (`0` feed, `1` folder, `2` starred, `3` all), `id` (`0`), `getRead` (`true`), `oldestFirst` (`false`), `lastModified` (`0`). Returns `{ "items": [ { "id", "title", "content", "body", "author", "feedId", "guid", "guidHash", "pubDate", "updatedDate", "lastModified", "url", "unread", "starred", ... } ] }`.
- **GET `/items/updated`** – Retrieves items changed since `lastModified`. Query: `lastModified` (required), `type`, `id`. Response matches `/items`.
- **POST `/items/{itemId}/read`** – Marks one item read. `404` if missing.
- **POST `/items/read/multiple`** – Marks multiple items read. Body `{ "itemIds": [int] }`.
- **POST `/items/{itemId}/unread`** – Marks one item unread. `404` if missing.
- **POST `/items/unread/multiple`** – Marks multiple items unread. Body `{ "itemIds": [int] }`.
- **POST `/items/{itemId}/star`** – Stars an item. `404` if unknown.
- **POST `/items/star/multiple`** – Stars multiple items. Body `{ "itemIds": [int] }`.
- **POST `/items/{itemId}/unstar`** – Unstars an item. `404` if unknown.
- **POST `/items/unstar/multiple`** – Unstars multiple items. Body `{ "itemIds": [int] }`.
- **POST `/items/read`** – Marks all items up to `{ "newestItemId": int }` as read.

### Version
- **GET `/version`** – Returns `{ "version": string }` (defaults to `"dev"` when no `VERSION` environment variable is set).
