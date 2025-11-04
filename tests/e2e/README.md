# End-to-End Tests

This directory contains end-to-end tests for the headless-rss application that test against the actual Docker container image.

## Test Coverage

The E2E test suite (`test_container.py`) validates a complete user journey:

1. **Adding a feed** - Creates a new RSS/Atom feed subscription
2. **Loading articles** - Retrieves articles from the feed
3. **Marking articles as read** - Updates article read status to true
4. **Marking articles as unread** - Updates article read status to false
5. **Moving feed to a folder** - Creates a folder and moves the feed into it
6. **Deleting the feed** - Removes the feed subscription

## Running the Tests

The E2E tests will automatically:
1. Build the Docker image from the Dockerfile
2. Start a container with the image
3. Run the test scenarios against the containerized application
4. Clean up all resources (container and image)

To run the E2E tests:

```bash
pytest tests/e2e/
```

## Requirements

- Docker must be installed and accessible
- Network access to download dependencies during Docker build
- Ports 9000-9999 available for the test container

## Skipped Tests

The tests will be automatically skipped if:
- Docker build fails (e.g., in environments with SSL certificate issues)
- Docker is not available
- Container fails to start

This is expected behavior in CI environments with restricted network access or sandboxed environments.

## Test Architecture

The E2E tests use:
- `pytest` for test orchestration
- `httpx` for making HTTP requests to the containerized API
- `pytest-httpserver` for serving test RSS/Atom feeds
- Docker CLI for container management

The test fixtures in `conftest.py` handle:
- Building the Docker image (module-scoped)
- Managing container lifecycle (function-scoped)
- Serving test feeds via HTTP server
