# Implemented Test Cases Baseline

## Purpose
This document captures currently implemented test coverage in a self-contained, human-readable format.
Each test case has a stable ID, a short description, and an expected result so it can be understood without opening source files.

## API Test Cases
API test cases are specified in separate documents:
- `docs/api-service-test-cases.md`
- `docs/api-nextcloud-v1-2-test-cases.md`
- `docs/api-nextcloud-v1-3-test-cases.md`

## Enumerated Test Cases

### Service Runtime and CORS
Source: `tests/api/test_app.py`

| ID | Case | Description | Expected Result |
|---|---|---|---|
| TC-SVC-001 | v1.2 mount reachable | Request the v1.2 version endpoint through the full application mount path. | Endpoint responds successfully (`200`). |
| TC-SVC-002 | Health endpoint response | Call `/status` and verify health payload format. | Returns `200` with body `{"status":"ok"}`. |
| TC-SVC-003 | CORS on service endpoint | Send request to `/status` with an `Origin` header. | Response includes `access-control-allow-origin: *`. |
| TC-SVC-004 | CORS on API endpoint | Send request to Nextcloud version endpoint with an `Origin` header. | Response includes `access-control-allow-origin: *`. |
| TC-SVC-005 | CORS preflight support | Send `OPTIONS` preflight request with `Access-Control-Request-Method`. | Returns success and includes CORS method headers. |

### Feed Parsing and URL Safety
Source: `tests/test_feed_parsing.py`

| ID | Case | Description | Expected Result |
|---|---|---|---|
| TC-FEED-001 | Parse Atom 0.3/1.0 and RSS variants | Add feeds from multiple fixture formats (`atom`, `rss`, GitHub Atom, feed without explicit IDs). | Feed is stored and at least one article is ingested for each fixture. |
| TC-FEED-002 | Block dangerous URL schemes and targets | Validate unsafe URLs such as `file://`, localhost, private ranges, and metadata IPs. | URL validation rejects each unsafe URL with SSRF protection error. |
| TC-FEED-003 | Allow safe public HTTPS URL | Validate a normal public HTTPS feed URL. | URL validation succeeds without exception. |

### Feed Quality Decisioning
Source: `tests/test_feed_quality.py`

| ID | Case | Description | Expected Result |
|---|---|---|---|
| TC-FEEDQ-001 | Prefer extracted full text when clearly better | Mock extraction result as much longer than feed-provided content. | Feed quality flags set to use extracted full text; quality check timestamp updated. |
| TC-FEEDQ-002 | Keep extraction disabled when not better | Mock extraction result as short/low-value content. | Feed quality flags keep extracted full text disabled; quality check timestamp updated. |

### Article Content Extraction
Source: `tests/test_article_extraction.py`

| ID | Case | Description | Expected Result |
|---|---|---|---|
| TC-ART-001 | Extract main article body | Run article extraction on a fixture HTML page containing body and footer. | Main body text is included; footer text is excluded. |

### Media Thumbnail Extraction
Source: `tests/test_media_thumbnail.py`

| ID | Case | Description | Expected Result |
|---|---|---|---|
| TC-THUMB-001 | Parse `img src` with double quotes | Extract first image URL from HTML using `src="..."`. | Correct image URL is returned. |
| TC-THUMB-002 | Parse `img src` with single quotes | Extract first image URL from HTML using `src='...'`. | Correct image URL is returned. |
| TC-THUMB-003 | Use first image when multiple images exist | Provide HTML with two or more images. | URL of the first image is returned. |
| TC-THUMB-004 | Handle image tags with extra attributes | Provide image tag with class/size attributes. | Correct `src` URL is still extracted. |
| TC-THUMB-005 | No image present | Provide HTML with no image tag. | Result is `None`. |
| TC-THUMB-006 | Empty content | Provide empty HTML content. | Result is `None`. |
| TC-THUMB-007 | Null content | Provide `None` as content. | Result is `None`. |
| TC-THUMB-008 | Relative image URL | Provide image with relative path URL. | Relative URL is returned unchanged. |
| TC-THUMB-009 | Data URL image | Provide image with `data:` URL. | Data URL is returned. |
| TC-THUMB-010 | Case-insensitive `IMG` tag parsing | Provide uppercase image tag/attributes. | URL extraction still succeeds. |
| TC-THUMB-011 | Keep explicit thumbnail if provided | Create article with explicit `media_thumbnail` and image in body. | Explicit thumbnail remains unchanged. |
| TC-THUMB-012 | Fallback thumbnail from body image | Create article without explicit thumbnail but with body image. | Thumbnail is set from first body image. |
| TC-THUMB-013 | No thumbnail when body has no images | Create article with text-only body. | Thumbnail remains `None`. |
| TC-THUMB-014 | No thumbnail when body is missing | Create article with `None` content. | Thumbnail remains `None`. |
| TC-THUMB-015 | Feed integration for image extraction | Ingest feed fixture with image/no-image scenarios. | Thumbnails are extracted per article expectations (first image or none). |

### Newsletter HTML Cleanup
Source: `tests/test_email_html_cleanup.py`

| ID | Case | Description | Expected Result |
|---|---|---|---|
| TC-HTML-001 | Remove hidden content blocks | Input contains hidden div content used for previews/tracking. | Hidden content is removed; visible content remains. |
| TC-HTML-002 | Simplify layout tables | Input contains nested email layout tables. | Content is preserved while layout-specific table structure is stripped. |
| TC-HTML-003 | Remove meta tags | Input contains HTML `<meta>` tags. | Meta tags are removed from output. |
| TC-HTML-004 | Preserve semantic content structure | Input contains headings, paragraphs, lists. | Structural tags and text content are preserved. |
| TC-HTML-005 | Clean complex newsletter markup | Input combines hidden sections, tables, and metadata. | Core readable content remains; clutter/tracking elements removed. |
| TC-HTML-006 | Handle empty HTML input | Input is empty string. | Output is empty string. |
| TC-HTML-007 | Handle null HTML input | Input is `None`. | Output is empty string. |
| TC-HTML-008 | Remove tracking pixels | Input includes tracking pixel images and normal content. | Tracking pixel URLs are removed; newsletter content remains. |

### Email and Newsletter Processing
Source: `tests/test_email.py`

| ID | Case | Description | Expected Result |
|---|---|---|---|
| TC-EMAIL-001 | Ingest unread IMAP emails into mailing-list feeds | Mock mailbox returns unseen list emails from two senders. | Two mailing-list feeds are created/updated with expected article counts. |
| TC-EMAIL-002 | Avoid duplicates on repeated updates | Run update cycle multiple times with same mocked inputs. | Existing article GUID hashes are reused; duplicates are not inserted. |
| TC-EMAIL-003 | LLM multi-item newsletter splitting | Mock LLM response with `mode=multi` and two items. | Two separate articles are created with expected URLs and summaries. |
| TC-EMAIL-004 | LLM single-item newsletter shaping | Mock LLM response with `mode=single`, summary, and cleaned content. | One article is created with cleaned content and provided summary. |
| TC-EMAIL-005 | Remove only stale read/unstarred newsletter items | Dataset includes old/new, read/unread, starred/unstarred, and non-newsletter items. | Only old read unstarred newsletter items are deleted. |
| TC-EMAIL-006 | Run cleanup even without credentials | No mailbox credentials configured. | Cleanup function is still invoked exactly once. |

## Current Automation Gaps
- No dedicated automated tests currently target CLI command invocation directly.
- CLI behavior is validated via required manual validation steps in repository workflow.

## Source of Truth
- Unit and API tests under `tests/`.
- Shared fixtures under `tests/fixtures/`.
- API-specific test-case details in the API test-case documents listed above.

## Rust Bootstrap Test Cases
Source: `rust/src/api.rs`

| ID | Case | Description | Expected Result |
|---|---|---|---|
| TC-RUST-001 | Rust health endpoint | Call Rust `/status`. | Returns `200` with `{"status":"ok"}`. |
| TC-RUST-002 | Root folder feed mapping | Query Rust feeds endpoint with feed assigned to root folder. | Feed object returns `folderId: null`. |
| TC-RUST-003 | Conditional auth enforcement | Query protected Rust endpoint when auth env vars are set and no credentials are provided. | Returns `401` with `{"detail":"Not authenticated"}` and Basic auth challenge header. |
| TC-RUST-004 | Rust items read endpoint | Query Rust `/items` with feed selection parameters. | Returns `200` with `items` payload and `body` preferring summary over content. |
| TC-RUST-005 | Rust updated-items filtering | Query Rust `/items/updated` with `lastModified` filter. | Returns `200` and only items meeting `lastModified` criteria. |
| TC-RUST-006 | Rust item content missing | Query Rust `/items/{item_id}/content` for a missing item ID. | Returns `404` with `{"detail":"Item not found"}`. |
| TC-RUST-007 | v1-2 star by guid-hash route | Call `PUT /index.php/apps/news/api/v1-2/items/{feed_id}/{guid_hash}/star` for an existing item. | Returns `200` and updates item to starred. |
| TC-RUST-008 | v1-3 read-multiple payload shape | Call `POST /index.php/apps/news/api/v1-3/items/read/multiple` with `{"itemIds": [...]}`. | Returns `200` and marks targeted items as read. |
| TC-RUST-009 | Rust folder creation endpoint | Call `POST /index.php/apps/news/api/v1-3/folders` with a valid non-empty name. | Returns `200` with created folder in `folders` payload. |
| TC-RUST-010 | v1-2 feed move method contract | Call `PUT /index.php/apps/news/api/v1-2/feeds/{feed_id}/move` with `{"folderId": ...}`. | Returns `200` and updates feed folder assignment. |
| TC-RUST-011 | v1-3 feed read method contract | Call `POST /index.php/apps/news/api/v1-3/feeds/{feed_id}/read` with `{"newestItemId": ...}`. | Returns `200` and marks matching feed items as read. |
| TC-RUST-012 | Rust feed creation SSRF localhost block | Call `POST /index.php/apps/news/api/v1-3/feeds` with URL `http://127.0.0.1:...` when testing mode is disabled. | Returns `400` with SSRF protection error detail. |
| TC-RUST-013 | Rust folder duplicate-create conflict | Call `POST /index.php/apps/news/api/v1-3/folders` for an existing folder name. | Returns `409` with `{"detail":"Folder already exists"}`. |
| TC-RUST-014 | Rust folder delete missing | Call `DELETE /index.php/apps/news/api/v1-3/folders/{folder_id}` for a missing folder ID. | Returns `404` with `{"detail":"Folder not found"}`. |
| TC-RUST-015 | Rust feed duplicate-create conflict | Call `POST /index.php/apps/news/api/v1-3/feeds` for an already existing feed URL. | Returns `409` conflict. |
| TC-RUST-016 | Rust feed create invalid folder | Call `POST /index.php/apps/news/api/v1-3/feeds` with a non-existent `folderId`. | Returns `422` with `Folder with ID ... does not exist`. |
| TC-RUST-017 | Rust feed delete missing | Call `DELETE /index.php/apps/news/api/v1-3/feeds/{feed_id}` for a missing feed ID. | Returns `404` not found. |
| TC-RUST-018 | Rust feed move invalid folder | Call `POST /index.php/apps/news/api/v1-3/feeds/{feed_id}/move` with non-existent `folderId`. | Returns `422` with `Folder with ID ... does not exist`. |
| TC-RUST-019 | Rust feed create payload parity | Call `POST /index.php/apps/news/api/v1-3/feeds` against a valid fixture Atom feed with `folderId=0`. | Returns `200` and payload includes expected feed fields (`url`, `title`, `link`, `updateErrorCount`) and `newestItemId` matching created feed ID. |
| TC-RUST-020 | Rust folder read side effect | Seed a feed/article in a non-root folder and call `POST /index.php/apps/news/api/v1-3/folders/{folder_id}/read`. | Returns `200` and matching folder items are marked `unread=false`. |
| TC-RUST-021 | v1-2 feed rename success path | Call `PUT /index.php/apps/news/api/v1-2/feeds/{feed_id}/rename` with `{"feedTitle": ...}`. | Returns `200` and updates feed title in storage. |
| TC-RUST-022 | v1-3 feed rename success path | Call `POST /index.php/apps/news/api/v1-3/feeds/{feed_id}/rename` with `{"feedTitle": ...}`. | Returns `200` and updates feed title in storage. |
| TC-RUST-023 | v1-2 feed rename method mismatch | Call `POST /index.php/apps/news/api/v1-2/feeds/{feed_id}/rename` (wrong method). | Returns `405` method not allowed. |
| TC-RUST-024 | v1-3 feed rename method mismatch | Call `PUT /index.php/apps/news/api/v1-3/feeds/{feed_id}/rename` (wrong method). | Returns `405` method not allowed. |
| TC-RUST-025 | v1-2 feed read success path | Call `PUT /index.php/apps/news/api/v1-2/feeds/{feed_id}/read` with `{"newestItemId": ...}`. | Returns `200` and marks matching feed items as read. |
| TC-RUST-026 | v1-3 feed read method mismatch | Call `PUT /index.php/apps/news/api/v1-3/feeds/{feed_id}/read` (wrong method). | Returns `405` method not allowed. |
| TC-RUST-027 | Rust feed delete cascade | Call `DELETE /index.php/apps/news/api/v1-3/feeds/{feed_id}` for an existing feed with items. | Returns `200`, deletes feed, and deletes associated articles. |
| TC-RUST-028 | Rust folder delete cascade | Call `DELETE /index.php/apps/news/api/v1-3/folders/{folder_id}` for a folder containing feeds/items. | Returns `200`, deletes folder, deletes feeds in folder, and deletes their articles. |
| TC-RUST-029 | Rust updater inserts new entries | Seed a due feed row and run Rust updater cycle against a valid fixture feed URL. | Due feed is processed, new article rows are inserted, and update error count remains `0`. |
| TC-RUST-030 | Rust updater persists update errors | Seed a due feed row with an invalid/blocked URL and run Rust updater cycle. | Feed `update_error_count` increments and `last_update_error` is populated. |
| TC-RUST-031 | Rust add-email-credentials success persistence | Run add-email-credentials persistence flow with validator success. | Credentials row is inserted into `email_credentials`. |
| TC-RUST-032 | Rust add-email-credentials validation gate | Run add-email-credentials persistence flow with validator failure. | Command path fails and no credential row is persisted. |
| TC-RUST-033 | Rust folder create invalid-name validation | Call `POST /index.php/apps/news/api/v1-3/folders` with an empty name. | Returns `422` with `{"detail":"Folder name is invalid"}`. |
| TC-RUST-034 | Rust folder rename duplicate-name validation | Seed two folders and call `PUT /index.php/apps/news/api/v1-3/folders/{folder_id}` with an existing name. | Returns `409` with `{"detail":"Folder already exists"}`. |
| TC-RUST-035 | Rust folder rename invalid-name validation | Call `PUT /index.php/apps/news/api/v1-3/folders/{folder_id}` with an empty name. | Returns `422` with `{"detail":"Folder name is invalid"}`. |
| TC-RUST-036 | Rust feed create unreadable-source handling | Call `POST /index.php/apps/news/api/v1-3/feeds` with a URL returning non-success HTTP status. | Returns `422` parse/read failure response. |
| TC-RUST-037 | Rust feed create next-update field | Call `POST /index.php/apps/news/api/v1-3/feeds` for a valid fixture feed and inspect payload. | Feed payload contains non-null `nextUpdateTime`. |
| TC-RUST-038 | Rust v1-2 rename missing-feed detail | Call `PUT /index.php/apps/news/api/v1-2/feeds/{feed_id}/rename` with non-existent feed ID. | Returns `404` with `Feed {id} not found`. |
| TC-RUST-039 | Rust v1-3 read missing-feed detail | Call `POST /index.php/apps/news/api/v1-3/feeds/{feed_id}/read` with non-existent feed ID. | Returns `404` with `Feed {id} not found`. |
| TC-RUST-040 | Rust protected endpoint rejects invalid credentials | Call protected endpoint with wrong Basic credentials. | Returns `401` with `{"detail":"Invalid authentication credentials"}`. |
| TC-RUST-041 | Rust protected endpoint accepts valid credentials | Call protected endpoint with correct Basic credentials. | Returns `200`. |
| TC-RUST-042 | Rust items invalid type validation | Call `GET /index.php/apps/news/api/v1-3/items?type=99&id=0`. | Returns `400` with `{"detail":"Invalid item selection type"}`. |
| TC-RUST-043 | Rust v1-2 item read missing detail | Call `POST /index.php/apps/news/api/v1-2/items/{item_id}/read` with non-existent item ID. | Returns `404` with `{"detail":"Item not found"}`. |
| TC-RUST-044 | Rust v1-3 item star missing detail | Call `POST /index.php/apps/news/api/v1-3/items/{item_id}/star` with non-existent item ID. | Returns `404` with `{"detail":"Item not found"}`. |
| TC-RUST-045 | Rust v1-2 guid-star missing detail | Call `PUT /index.php/apps/news/api/v1-2/items/{feed_id}/{guid_hash}/star` with non-existent guid-hash. | Returns `404` with `{"detail":"Item not found"}`. |
| TC-RUST-046 | Rust v1-2 read-multiple state update | Call `PUT /index.php/apps/news/api/v1-2/items/read/multiple` with `{"items":[id]}` for an unread item. | Returns `200`; item is marked `unread=false` and `lastModified` increases. |
| TC-RUST-047 | Rust v1-2 guid-star-multiple state update | Call `PUT /index.php/apps/news/api/v1-2/items/star/multiple` with a valid guid-hash payload. | Returns `200`; item is marked `starred=true` and `lastModified` increases. |
| TC-RUST-048 | Rust v1-3 unread-multiple state update | Call `POST /index.php/apps/news/api/v1-3/items/unread/multiple` with `{"itemIds":[id]}` after marking item read. | Returns `200`; item is marked `unread=true` and `lastModified` increases. |
| TC-RUST-049 | Rust v1-3 unstar-multiple state update | Call `POST /index.php/apps/news/api/v1-3/items/unstar/multiple` with `{"itemIds":[id]}` after starring item. | Returns `200`; item is marked `starred=false` and `lastModified` increases. |
| TC-RUST-050 | Rust v1-2 mark-all-read state update | Call `PUT /index.php/apps/news/api/v1-2/items/read` with `{"newestItemId":id}`. | Returns `200`; matching items are marked `unread=false` and `lastModified` increases. |
| TC-RUST-051 | Rust v1-3 mark-all-read state update | Call `POST /index.php/apps/news/api/v1-3/items/read` with `{"newestItemId":id}`. | Returns `200`; matching items are marked `unread=false` and `lastModified` increases. |
| TC-RUST-052 | Rust updater skips mailing-list feeds | Seed a due feed row with `is_mailing_list=1` and run Rust updater cycle. | Row is excluded from web-feed updates, and update error fields remain unchanged. |

### Rust Single-Item Write Parity Test Cases
Source: `rust/src/api.rs`

| ID | Case | Description | Expected Result |
|---|---|---|---|
| TC-RUST-053 | Rust v1-2 single read state update | Call `POST /index.php/apps/news/api/v1-2/items/{item_id}/read` for an unread item. | Returns `200`; item is marked `unread=false` and `lastModified` increases. |
| TC-RUST-054 | Rust v1-2 single unread state update | Call `PUT /index.php/apps/news/api/v1-2/items/{item_id}/unread` for a read item. | Returns `200`; item is marked `unread=true` and `lastModified` increases. |
| TC-RUST-055 | Rust v1-2 single unstar state update | Call `PUT /index.php/apps/news/api/v1-2/items/{feed_id}/{guid_hash}/unstar` for a starred item. | Returns `200`; item is marked `starred=false` and `lastModified` increases. |
| TC-RUST-056 | Rust v1-3 single read state update | Call `POST /index.php/apps/news/api/v1-3/items/{item_id}/read` for an unread item. | Returns `200`; item is marked `unread=false` and `lastModified` increases. |
| TC-RUST-057 | Rust v1-3 single unread state update | Call `POST /index.php/apps/news/api/v1-3/items/{item_id}/unread` for a read item. | Returns `200`; item is marked `unread=true` and `lastModified` increases. |
| TC-RUST-058 | Rust v1-3 single star state update | Call `POST /index.php/apps/news/api/v1-3/items/{item_id}/star` for an unstarred item. | Returns `200`; item is marked `starred=true` and `lastModified` increases. |
| TC-RUST-059 | Rust v1-3 single unstar state update | Call `POST /index.php/apps/news/api/v1-3/items/{item_id}/unstar` for a starred item. | Returns `200`; item is marked `starred=false` and `lastModified` increases. |

### Rust Item Query Contract Test Cases
Source: `rust/src/api.rs`

| ID | Case | Description | Expected Result |
|---|---|---|---|
| TC-RUST-060 | Rust items folder selection | Call `GET /index.php/apps/news/api/v1-3/items?type=1&id={folder_id}` with items in and out of that folder. | Returns only items whose feeds belong to the specified folder. |
| TC-RUST-061 | Rust items starred selection | Call `GET /index.php/apps/news/api/v1-3/items?type=2&id=0` with mixed starred/unstarred items. | Returns only starred items. |
| TC-RUST-062 | Rust items unread filtering | Call `GET /index.php/apps/news/api/v1-3/items?type=3&id=0&getRead=false` with mixed read/unread items. | Returns only unread items. |
| TC-RUST-063 | Rust items oldest-first ordering | Call `GET /index.php/apps/news/api/v1-3/items?type=3&id=0&oldestFirst=true` with multiple item IDs. | Returns items ordered by ascending item ID. |
| TC-RUST-064 | Rust items batch-size limit | Call `GET /index.php/apps/news/api/v1-3/items?type=3&id=0&batchSize=1` with multiple items. | Returns exactly one item, respecting descending default order. |
| TC-RUST-065 | Rust items offset/newest-id semantics | Call `GET /index.php/apps/news/api/v1-3/items?type=3&id=0&offset={id}` with newer and older items. | Returns only items with `id <= offset`, matching Python newest-item-id semantics. |

### Rust Updated-Items Query Contract Test Cases
Source: `rust/src/api.rs`

| ID | Case | Description | Expected Result |
|---|---|---|---|
| TC-RUST-066 | Rust updated-items feed selection | Call `GET /index.php/apps/news/api/v1-3/items/updated?lastModified={ts}&type=0&id={feed_id}` with mixed modification times in one feed. | Returns only feed items where `lastModified >= ts`. |
| TC-RUST-067 | Rust updated-items folder selection | Call `GET /index.php/apps/news/api/v1-3/items/updated?lastModified={ts}&type=1&id={folder_id}` with mixed modification times in one folder. | Returns only folder items where `lastModified >= ts`. |
| TC-RUST-068 | Rust updated-items starred selection | Call `GET /index.php/apps/news/api/v1-3/items/updated?lastModified={ts}&type=2&id=0` with mixed starred items and modification times. | Returns only starred items where `lastModified >= ts`. |
| TC-RUST-069 | Rust updated-items all selection threshold | Call `GET /index.php/apps/news/api/v1-3/items/updated?lastModified={ts}&type=3&id=0` with mixed modification times. | Returns all items across feeds where `lastModified >= ts`. |
| TC-RUST-070 | Rust updated-items all selection ordering | Call `GET /index.php/apps/news/api/v1-3/items/updated?lastModified={ts}&type=3&id=0` with multiple matching IDs. | Returns matching items in descending item-ID order (`oldestFirst=false`). |

### Rust Migration Bootstrap Test Cases
Source: `rust/src/db.rs`

| ID | Case | Description | Expected Result |
|---|---|---|---|
| TC-RUST-052 | Rust SQLx migration bootstrap | Create a new SQLite file and initialize Rust pool via `create_pool`. | SQLx baseline migration is applied, core tables are created, and root folder `id=0` exists with `is_root=1`. |
