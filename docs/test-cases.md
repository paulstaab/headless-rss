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

## Expected-Failure Cases (Known Gaps)
Source: `tests/test_email.py`

| ID | Case | Description | Current Status |
|---|---|---|---|
| TC-XFAIL-001 | Subject sanitization hardening | Validate sanitization of malicious email subjects (XSS/SQL/template/control chars). | Marked `xfail` (not fully implemented). |
| TC-XFAIL-002 | Error detail redaction | Ensure mailbox connection errors do not expose internal host/port/username details. | Marked `xfail` (not fully implemented). |

## Current Automation Gaps
- No dedicated automated tests currently target CLI command invocation directly.
- CLI behavior is validated via required manual validation steps in repository workflow.

## Source of Truth
- Unit and API tests under `tests/`.
- Shared fixtures under `tests/fixtures/`.
- API-specific test-case details in the API test-case documents listed above.
