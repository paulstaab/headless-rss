# API Test Cases: Nextcloud News v1.2

## Scope
Test cases for implemented Nextcloud News API v1.2 behavior.

## Authentication
| ID | Case | Expected Result |
|---|---|---|
| TC-NC12-001 | Missing credentials | Returns `401` with `Not authenticated` when auth is enabled. |
| TC-NC12-002 | Invalid credentials | Returns `401` with `Invalid authentication credentials`. |
| TC-NC12-003 | Valid credentials | Protected endpoint access succeeds. |

## Version
| ID | Case | Expected Result |
|---|---|---|
| TC-NC12-004 | `GET /version` | Returns `200`. |

## Feeds
| ID | Case | Expected Result |
|---|---|---|
| TC-NC12-005 | Feed creation success | Returns feed metadata plus `newestItemId`. |
| TC-NC12-006 | Duplicate feed creation | Returns conflict (`409`). |
| TC-NC12-007 | Unreadable feed URL | Returns validation failure (`422`). |
| TC-NC12-008 | Feed deletion cascade | Removes feed and associated items. |
| TC-NC12-009 | Unknown feed deletion | Returns not found (`404`). |
| TC-NC12-010 | Feed move endpoint | Updates the target folder. |
| TC-NC12-011 | Feed rename endpoint | Updates the title. |
| TC-NC12-012 | Feed read endpoint | Marks feed items as read. |
| TC-NC12-013 | Root-folder mapping | `folderId: 0` and `folderId: null` map to root semantics. |
| TC-NC12-014 | `nextUpdateTime` field | Created feed exposes `nextUpdateTime`. |

## Folders
| ID | Case | Expected Result |
|---|---|---|
| TC-NC12-015 | Folder listing | Works and excludes the internal root folder. |
| TC-NC12-016 | Folder creation | Success path works. |
| TC-NC12-017 | Duplicate folder creation | Returns conflict. |
| TC-NC12-018 | Invalid folder name | Returns validation error. |
| TC-NC12-019 | Folder deletion | Success and not-found behavior both work. |
| TC-NC12-020 | Folder rename | Success plus duplicate/invalid-name errors work. |
| TC-NC12-021 | Folder read endpoint | Marks folder items as read. |

## Items
| ID | Case | Expected Result |
|---|---|---|
| TC-NC12-022 | Item listing filters | Listing by feed and other supported query filters works. |
| TC-NC12-023 | Updated-items by `lastModified` | Updated-items endpoint filters by `lastModified`. |
| TC-NC12-024 | Item content endpoint | Success path works and missing item returns not found. |
| TC-NC12-025 | Read/unread single-item endpoints | Single-item read and unread endpoints work. |
| TC-NC12-026 | Read/unread bulk endpoints | v1.2 `items` payload shape is accepted. |
| TC-NC12-027 | Star/unstar single-item endpoints | `feedId/guidHash` route format works. |
| TC-NC12-028 | Star/unstar bulk endpoints | Guid-hash payload objects are accepted. |
| TC-NC12-029 | Bulk star/unstar request limit | Payloads larger than 10,000 items are rejected with `400`. |
| TC-NC12-030 | Mark-all-read and `lastModified` updates | Mark-all-read works and state-changing operations update `lastModified`. |

## Primary Test Files
- `tests/api/nextcloud_news/v1_2/test_auth.py`
- `tests/api/nextcloud_news/v1_2/test_feed.py`
- `tests/api/nextcloud_news/v1_2/test_folder.py`
- `tests/api/nextcloud_news/v1_2/test_item.py`
- `tests/api/nextcloud_news/v1_2/test_version.py`
