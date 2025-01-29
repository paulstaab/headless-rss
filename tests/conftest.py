from pathlib import Path

import pytest
from fastapi.testclient import TestClient
from src import database
from src.api.nextcloud_news.endpoint import app


@pytest.fixture
def client() -> TestClient:
    """Test client for the FastAPI app."""
    return TestClient(app)


@pytest.fixture(autouse=True)
def db(tmp_path: Path):
    """Create a new database for each test."""
    database.init(file=tmp_path / "test.db")
    db = database.get_session()
    database.Base.metadata.create_all(bind=db.get_bind())
    yield db
    db.close()
