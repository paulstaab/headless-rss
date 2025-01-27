# Nano RSS

Nano RSS is a backend service for collecting posts from different RSS feeds and providing an API to access them. The API is compatible with the Freshness API.

## Features

- Collect posts from different RSS feeds
- Provide an API to access the collected posts
- Use SQLite as a database
- Use SQLAlchemy 2 as ORM
- Use Alembic to manage migrations
- Use FastAPI to provide the API
- Use uv to manage dependencies
- Use ruff and GitHub Actions to check the code
- Provide a Docker image to run the service

## Setup Instructions

1. Clone the repository:
   ```bash
   git clone https://github.com/paulstaab/nano-rss.git
   cd nano-rss
   ```

2. Create a virtual environment and activate it:
   ```bash
   python -m venv venv
   source venv/bin/activate
   ```

3. Install the dependencies:
   ```bash
   pip install -r requirements.txt
   ```

4. Set up the database:
   ```bash
   alembic upgrade head
   ```

5. Run the FastAPI server:
   ```bash
   uvicorn src.main:app --reload
   ```

## Usage Instructions

### Fetch Posts

To fetch posts from the database, send a GET request to the `/posts` endpoint.

Example:
```bash
curl -X GET "http://127.0.0.1:8000/posts"
```

## Contribution Guidelines

1. Fork the repository.
2. Create a new branch for your feature or bugfix.
3. Make your changes.
4. Ensure that your code passes the ruff checks and tests.
5. Submit a pull request.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
