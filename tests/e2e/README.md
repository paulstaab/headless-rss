# End-to-End Tests

This directory contains end-to-end tests for the headless-rss application that test against the actual Docker container image.

## Test Coverage

The E2E test suite validates a complete user journey:

1. **Adding a feed** - Creates a new RSS/Atom feed subscription
2. **Loading articles** - Retrieves articles from the feed
3. **Marking articles as read** - Updates article read status to true
4. **Marking articles as unread** - Updates article read status to false
5. **Moving feed to a folder** - Creates a folder and moves the feed into it
6. **Deleting the feed** - Removes the feed subscription

## Running the Tests

### In CI (Main Branch Only)

The E2E tests automatically run on the `main` branch as part of the release workflow:
1. After the Docker image is built
2. Before the image is pushed to the registry
3. Tests the actual image that will be released

### Locally

You can run the E2E test against any Docker image:

```bash
# Test a local image
python tests/e2e/run_e2e_test.py headless-rss:latest

# Test the published image
python tests/e2e/run_e2e_test.py ghcr.io/paulstaab/headless-rss:latest
```

Requirements for local testing:
- Docker must be installed and accessible
- Python 3.13+ with httpx installed (`uv run --dev python tests/e2e/run_e2e_test.py <image>`)

## Test Architecture

The E2E tests use:
- Standalone Python script (`run_e2e_test.py`) for orchestration
- `httpx` for making HTTP requests to the containerized API
- Python's built-in HTTP server for serving test RSS/Atom feeds
- Docker CLI for container management

The script:
1. Starts a local HTTP server to serve test feeds
2. Starts a container from the specified image
3. Runs the complete user journey test
4. Cleans up all resources (container, server, temp files)

## Legacy Pytest Tests

The `conftest.py` and `test_container.py` files are the original pytest-based E2E tests that build their own Docker image. These are no longer run in CI but can still be used for local development if needed:

```bash
pytest tests/e2e/
```

These tests will skip if Docker build fails (e.g., in sandboxed environments).
