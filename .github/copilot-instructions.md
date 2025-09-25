# headless-rss

headless-rss is a minimal RSS feed aggregator designed for self-hosting. It provides a Nextcloud News compatible API and is built with Python 3.13+, FastAPI, and SQLite.

Always reference these instructions first and fallback to search or bash commands only when you encounter unexpected information that does not match the info here.

## Working Effectively

### Bootstrap the Repository
- Install uv package manager (if not available): `pip install uv`
- Run database migrations: `Execute Migrations` task -- takes <1 second

### Run the Application
- ALWAYS run the bootstrapping steps first.
- Start the API server with the VS Code task `Start Server`. Keep the task running in the background while you work.
- The server runs on http://localhost:8000
- Use CLI commands: `uv run --dev python -m src.cli --help`

## Validation

### Required Manual Validation After Changes
- ALWAYS run the full build and test sequence after making changes.
- Run the `Execute Migrations` task to execute all database migrations -- takes <10 seconds
- Run the `Lint` task to run all linters and formatters -- takes 10 seconds on first run, <1 second after that.
- Run the `Run All Tests` task to run the test suite -- takes 10 seconds. NEVER CANCEL. Set timeout to 2 minutes.
- Test CLI functionality: `uv run --dev python -m src.cli --help` and `uv run --dev python -m src.cli update`

### Complete End-to-End Scenario Testing
After making changes, test a complete RSS feed workflow:
1. Start the server: `Start Server` -- startup takes <5 seconds. Leave the server running in the background and only stop it when done testing.
2. Verify that the server started: `curl http://localhost:8000/status` should return `{"status":"ok"}`
3. Test the Nextcloud News API endpoints:
   - `curl http://localhost:8000/index.php/apps/news/api/v1-3/feeds` should return `{"feeds":[]}`
   - `curl http://localhost:8000/index.php/apps/news/api/v1-3/folders` should return `{"folders":[]}`
   - `curl http://localhost:8000/index.php/apps/news/api/v1-3/version` should return `{"version":"dev"}`
4. Stop the server

## Repository Structure
```
.
├── .devcontainer/          # VS Code dev container configuration
├── .github/               # GitHub workflows and config
├── .pre-commit-config.yaml # Pre-commit hooks (ruff, mypy)
├── Dockerfile             # Container build definition
├── README.md              # Project documentation
├── alembic/               # Database migrations
├── alembic.ini           # Alembic configuration
├── data/                 # SQLite database location
├── docker/               # Docker-related scripts
├── pyproject.toml        # Project dependencies and config
├── spec/                 # Specification documents for this application
├── src/                  # Main application code
│   ├── api/              # FastAPI application
│   ├── cli.py            # Command-line interface
│   ├── database.py       # Database models and connection
│   ├── feed.py           # RSS feed processing
│   ├── folder.py         # Feed folder management
│   ├── article.py        # Article processing
│   └── email.py          # Email newsletter integration
├── tests/                # Test suite
└── uv.lock              # Locked dependencies
```

## Key API Endpoints
- `/status` - Health check endpoint
- `/index.php/apps/news/api/v1-3/feeds` - Nextcloud News compatible feeds API
- `/index.php/apps/news/api/v1-3/folders` - Folder management
- `/index.php/apps/news/api/v1-3/items` - Articles/items API

## Key CLI Commands
```bash
# Update all feeds
uv run --dev python -m src.cli update

# Add email credentials for newsletter integration
uv run --dev python -m src.cli add-email-credentials --server imap.example.com --port 993 --username user@example.com --password yourpassword
```

## Environment Variables
- `USERNAME` - Optional authentication username
- `PASSWORD` - Optional authentication password
- `FEED_UPDATE_FREQUENCY_MIN` - Update frequency in minutes (default: 15)

## Database
- Uses SQLite database stored in `data/headless-rss.sqlite3`
- Alembic handles schema migrations automatically
- Database is created and migrated on first startup

## Common File Locations
- Main application: `src/api/app.py`
- Database models: `src/database.py`
- Feed processing: `src/feed.py`
- CLI interface: `src/cli.py`
- Test configuration: `tests/conftest.py`
- CI configuration: `.github/workflows/ci.yml`

## Troubleshooting
- If `uv` is not installed: `pip install uv`
- If tests fail due to database issues, delete `data/headless-rss.sqlite3*` files and re-run migrations
