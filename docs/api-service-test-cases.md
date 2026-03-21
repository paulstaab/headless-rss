# API Test Cases: Service Endpoints

## Scope
Test cases for non-Nextcloud service endpoints.

## Endpoint Coverage

### GET `/status`
| ID | Case | Expected Result |
|---|---|---|
| TC-SVCAPI-001 | Response status | Returns `200`. |
| TC-SVCAPI-002 | Response body | Returns `{"status":"ok"}`. |

## Cross-Cutting Coverage
| ID | Case | Expected Result |
|---|---|---|
| TC-SVCAPI-003 | CORS headers on `/status` | Response includes wildcard origin. |
| TC-SVCAPI-004 | CORS preflight support | `OPTIONS` requests succeed and include allow-method headers. |
| TC-SVCAPI-005 | CORS consistency across mounted routes | CORS behavior is consistent between `/status` and mounted Nextcloud routes. |

## Primary Test Files
- `tests/api/test_app.py`
