# headless-rss

headless-rss is a minimal RSS feed aggregator designed for self-hosting. It provides a Nextcloud News compatible API and is built with Rust, axum, and SQLite.

Always reference these instructions first and fallback to search or bash commands only when you encounter unexpected information that does not match the info here.

## Working Effectively

### Bootstrap the Repository
- Ensure the Rust toolchain is available.
- Fetch dependencies if needed: `cargo fetch`

### Run the Application
- ALWAYS run the bootstrapping steps first.
- Start the API server with the VS Code task `Start Server`. Keep the task running in the background while you work.
- The server runs on http://localhost:8000
- Use CLI commands: `cargo run -- --help`

## Validation

### Required Manual Validation After Changes
- Run `cargo fmt --all -- --check`
- Run `cargo clippy --all-targets --all-features -- -D warnings`
- Run `cargo test --all-targets --all-features`

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
├── .pre-commit-config.yaml # Pre-commit hooks (Rust formatting/linting)
├── Dockerfile             # Container build definition
├── README.md              # Project documentation
├── Cargo.toml             # Rust project manifest
├── Cargo.lock             # Locked Rust dependencies
├── data/                 # SQLite database location
├── docker/               # Docker-related scripts
├── docs/                 # Requirements, contracts, and test catalogs
├── migrations/           # SQLx migrations
├── src/                  # Main Rust application code
├── tests/                # Rust test suite
└── vendor/               # Vendored crates and assets
```

## Key API Endpoints
- `/status` - Health check endpoint
- `/index.php/apps/news/api/v1-3/feeds` - Nextcloud News compatible feeds API
- `/index.php/apps/news/api/v1-3/folders` - Folder management
- `/index.php/apps/news/api/v1-3/items` - Articles/items API

## Key CLI Commands
```bash
# Update all feeds
cargo run -- update

# Add email credentials for newsletter integration
cargo run -- add-email-credentials --server imap.example.com --port 993 --username user@example.com --password yourpassword
```

## Environment Variables
- `USERNAME` - Optional authentication username
- `PASSWORD` - Optional authentication password
- `FEED_UPDATE_FREQUENCY_MIN` - Update frequency in minutes (default: 15)

## Database
- Uses SQLite database stored in `data/headless-rss.sqlite3`
- SQLx handles schema migrations automatically
- Database is created and migrated on first startup

## Common File Locations
- Main application: `src/main.rs`
- API router: `src/api.rs`
- Database access: `src/db.rs`
- Feed updater: `src/updater.rs`
- Email processing: `src/email.rs`
- CI configuration: `.github/workflows/ci.yml`

## Troubleshooting
- If tests fail due to database issues, delete `data/headless-rss.sqlite3*` files and restart the server or rerun the relevant command
