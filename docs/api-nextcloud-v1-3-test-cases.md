# API Test Cases: Nextcloud News v1.3

## Scope
Test cases for implemented Nextcloud News API v1.3 behavior.

## Authentication
| ID | Case | Expected Result |
|---|---|---|
| TC-NC13-001 | Missing credentials | Returns `401` with `Not authenticated` when auth is enabled. |
| TC-NC13-002 | Invalid credentials | Returns `401` with `Invalid authentication credentials`. |
| TC-NC13-003 | Valid credentials | Protected endpoint access succeeds. |

## Version
| ID | Case | Expected Result |
|---|---|---|
| TC-NC13-004 | `GET /version` | Returns `200`. |

## Feeds
| ID | Case | Expected Result |
|---|---|---|
| TC-NC13-005 | Feed creation success | Returns feed metadata plus `newestItemId`. |
| TC-NC13-006 | Duplicate feed creation | Returns conflict (`409`). |
| TC-NC13-007 | Unreadable feed URL | Returns validation failure (`422`). |
| TC-NC13-008 | SSRF redirect rejection | Feed creation rejects redirect targets blocked by SSRF policy. |
| TC-NC13-009 | Feed deletion cascade | Removes feed and associated items. |
| TC-NC13-010 | Unknown feed deletion | Returns not found (`404`). |
| TC-NC13-011 | Feed move endpoint | Updates the target folder. |
| TC-NC13-012 | Feed rename endpoint | Updates the title. |
| TC-NC13-013 | Feed read endpoint | Marks feed items as read. |
| TC-NC13-014 | Root-folder mapping | `folderId: 0` and `folderId: null` map to root semantics. |
| TC-NC13-015 | `nextUpdateTime` field | Created feed exposes `nextUpdateTime`. |

## Folders
| ID | Case | Expected Result |
|---|---|---|
| TC-NC13-016 | Folder listing | Works and excludes the internal root folder. |
| TC-NC13-017 | Folder creation | Success path works. |
| TC-NC13-018 | Duplicate folder creation | Returns conflict. |
| TC-NC13-019 | Invalid folder name | Returns validation error. |
| TC-NC13-020 | Folder deletion | Success and not-found behavior both work. |
| TC-NC13-021 | Folder rename | Success plus duplicate/invalid-name errors work. |
| TC-NC13-022 | Folder read endpoint | Marks folder items as read. |

## Items
| ID | Case | Expected Result |
|---|---|---|
| TC-NC13-023 | Item listing filters | Listing by feed and other supported query filters works. |
| TC-NC13-024 | Updated-items by `lastModified` | Updated-items endpoint filters by `lastModified`. |
| TC-NC13-025 | Item content endpoint | Success path works and missing item returns not found. |
| TC-NC13-026 | Read/unread single-item endpoints | Single-item read and unread endpoints work. |
| TC-NC13-027 | Read/unread bulk endpoints | v1.3 `itemIds` payload shape is accepted. |
| TC-NC13-028 | Star/unstar single-item endpoints | `itemId` route format works. |
| TC-NC13-029 | Star/unstar bulk endpoints | Item-id list payload is accepted. |
| TC-NC13-030 | Mark-all-read and `lastModified` updates | Mark-all-read works and state-changing operations update `lastModified`. |

## Primary Test Files
- `tests/api/nextcloud_news/v1_3/test_auth.py`
- `tests/api/nextcloud_news/v1_3/test_feed.py`
- `tests/api/nextcloud_news/v1_3/test_folder.py`
- `tests/api/nextcloud_news/v1_3/test_item.py`
- `tests/api/nextcloud_news/v1_3/test_version.py`
