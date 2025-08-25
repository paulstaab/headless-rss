# headless-rss

headless-rss is a minimal RSS feed aggregator designed for self-hosting. It provides a Nextcloud News compatible API and is built with Python 3.13+, FastAPI, and SQLite.

Always reference these instructions first and fallback to search or bash commands only when you encounter unexpected information that does not match the info here.

## Working Effectively

### Bootstrap, Build, and Test the Repository
- Install uv package manager (if not available): `pip install uv`
- Install dependencies: `uv sync --dev` -- takes <1 second with cache, up to 60 seconds on first install. NEVER CANCEL. Set timeout to 5+ minutes.
- Run linting: `uv run --dev ruff check src tests` -- takes <1 second
- Run formatting: `uv run --dev ruff format src tests` -- takes <1 second  
- Run type checking: `uv run --dev mypy src tests` -- takes <1 second with cache, up to 15 seconds on first run. NEVER CANCEL. Set timeout to 2+ minutes.
- Run tests: `uv run --dev pytest tests` -- takes 7 seconds. NEVER CANCEL. Set timeout to 2+ minutes.
- Run database migrations: `uv run --dev alembic upgrade head` -- takes <1 second

### Run the Application
- ALWAYS run the bootstrapping steps first.
- Start API server: `uv run --dev uvicorn src.api.app:app --host 0.0.0.0 --port 8000`
- For development with auto-reload: `uv run --dev uvicorn src.api.app:app --host 0.0.0.0 --port 8000 --reload`
- The server runs on http://localhost:8000
- Use CLI commands: `uv run --dev python -m src.cli --help`

### Docker (Alternative Deployment)
- Build Docker image: `docker build -t headless-rss .` -- may fail due to network limitations
- Run with Docker: `docker run -d --user 9999 --name headless_rss --volume headless-rss-data:/app/data --publish 8000:8000 --env USERNAME=myuser --env PASSWORD=mypassword ghcr.io/paulstaab/headless-rss:latest`

## Validation

### Required Manual Validation After Changes
- ALWAYS run the full build and test sequence after making changes.
- Test the API server startup: `uv run --dev uvicorn src.api.app:app --host 0.0.0.0 --port 8000`
- Verify status endpoint: `curl http://localhost:8000/status` should return `{"status":"ok"}`
- Test Nextcloud News API: `curl http://localhost:8000/index.php/apps/news/api/v1-3/feeds` should return `{"feeds":[]}`
- Test CLI functionality: `uv run --dev python -m src.cli --help` and `uv run --dev python -m src.cli update`
- Always run `uv run --dev ruff check src tests`, `uv run --dev ruff format src tests`, and `uv run --dev mypy src tests` before committing or the CI (.github/workflows/ci.yml) will fail.
- Use pre-commit hooks: `uv run --dev pre-commit run --all-files` -- takes 10 seconds on first run, <1 second after that.

### Complete End-to-End Scenario Testing
After making changes, test a complete RSS feed workflow:
1. Start the server: `uv run --dev uvicorn src.api.app:app --host 0.0.0.0 --port 8000`
2. Verify server health: `curl http://localhost:8000/status` should return `{"status":"ok"}`
3. Test the Nextcloud News API endpoints:
   - `curl http://localhost:8000/index.php/apps/news/api/v1-3/feeds` should return `{"feeds":[]}`
   - `curl http://localhost:8000/index.php/apps/news/api/v1-3/folders` should return `{"folders":[]}`
   - `curl http://localhost:8000/index.php/apps/news/api/v1-3/version` should return `{"version":"dev"}`
4. Test authentication (if enabled):
   - Start server with auth: `USERNAME=testuser PASSWORD=testpass uv run --dev uvicorn src.api.app:app --host 0.0.0.0 --port 8000`
   - Unauthenticated access should return 401: `curl -w "%{http_code}" -o /dev/null http://localhost:8000/index.php/apps/news/api/v1-3/feeds`
   - Authenticated access should work: `curl -u "testuser:testpass" http://localhost:8000/index.php/apps/news/api/v1-3/feeds`
5. Verify database migrations work: `uv run --dev alembic upgrade head`
6. Test CLI commands: `uv run --dev python -m src.cli update`

## Common Tasks

### Repository Structure
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

### Key API Endpoints
- `/status` - Health check endpoint
- `/index.php/apps/news/api/v1-3/feeds` - Nextcloud News compatible feeds API
- `/index.php/apps/news/api/v1-3/folders` - Folder management
- `/index.php/apps/news/api/v1-3/items` - Articles/items API

### Key CLI Commands
```bash
# Update all feeds
uv run --dev python -m src.cli update

# Add email credentials for newsletter integration
uv run --dev python -m src.cli add-email-credentials --server imap.example.com --port 993 --username user@example.com --password yourpassword
```

### Environment Variables
- `USERNAME` - Optional authentication username
- `PASSWORD` - Optional authentication password  
- `FEED_UPDATE_FREQUENCY_MIN` - Update frequency in minutes (default: 15)

### Database
- Uses SQLite database stored in `data/headless-rss.sqlite3`
- Alembic handles schema migrations automatically
- Database is created and migrated on first startup

### Common File Locations
- Main application: `src/api/app.py`
- Database models: `src/database.py`
- Feed processing: `src/feed.py`
- CLI interface: `src/cli.py`
- Test configuration: `tests/conftest.py`
- CI configuration: `.github/workflows/ci.yml`

### Troubleshooting
- If `uv` is not installed: `pip install uv`
- If tests fail due to database issues, delete `data/headless-rss.sqlite3*` files and re-run migrations
- If Docker build fails: Network/certificate issues are common in sandboxed environments
- For linting issues: Run `uv run --dev ruff format src tests` to auto-fix formatting