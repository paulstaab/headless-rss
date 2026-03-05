# API Contract: Service Endpoints

## Scope
This contract covers non-Nextcloud service endpoints exposed by the service.

## Mounting
- Application entrypoint: `src/api/app.py`
- Public host/port are deployment-specific (default local dev: `http://localhost:8000`)

## Endpoints

### GET `/status`
- Purpose: Health probe for service availability.
- Authentication: Not required.
- Response: `200 OK`
- Response body:
```json
{"status":"ok"}
```

## Cross-Cutting API Behavior
- CORS is enabled globally with:
  - `Access-Control-Allow-Origin: *`
  - `allow_methods: *`
  - `allow_headers: *`
  - `allow_credentials: false`
- This behavior applies to `/status` and all mounted Nextcloud API routes.
