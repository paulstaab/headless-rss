from pathlib import Path

import pytest
import werkzeug
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


def _respond_with_file(request, file_name: str) -> werkzeug.Response:
    return werkzeug.wrappers.Response(
        response=(Path(__file__).parent / file_name).read_text(),
        status=200,
        content_type="application/xml",
    )


@pytest.fixture
def feed_server(httpserver):
    httpserver.expect_request("/atom.xml").respond_with_handler(
        lambda x: _respond_with_file(x, "feeds/atom.xml")
    )
    return httpserver
