# API Test Cases: Service Endpoints

## Scope
Test cases for non-Nextcloud service endpoints.

## Endpoint Coverage

### GET `/status`
- Verify response status is `200`.
- Verify response body is `{"status":"ok"}`.

## Cross-Cutting Coverage
- Verify CORS headers on `/status` include wildcard origin.
- Verify CORS preflight (`OPTIONS`) requests succeed and include allow-method headers.
- Verify CORS behavior is consistent between `/status` and mounted Nextcloud routes.

## Primary Test Files
- `tests/api/test_app.py`
