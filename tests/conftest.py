from pathlib import Path

import pytest
import werkzeug
from src import database
from src.options import Options


@pytest.fixture(autouse=True)
def db(tmp_path: Path):
    """Create a new database for each test."""
    database.init(file=tmp_path / "test.db")
    with database.get_session() as db:
        database.Base.metadata.create_all(bind=db.get_bind())
        yield db


@pytest.fixture(autouse=True, scope="session")
def disable_openai_calls() -> None:
    """Disable OpenAI API calls during tests."""
    import os

    os.environ.pop("OPENAI_API_KEY", None)
    os.environ.pop("OPENAI_MODEL", None)


@pytest.fixture(autouse=True)
def clear_options_cache() -> None:
    """Reset the options so that each test can set its own environment variables."""
    Options.clear()


def _respond_with_file(request, file_name: str, content_type: str = "application/xml") -> werkzeug.Response:
    return werkzeug.wrappers.Response(
        response=(Path(__file__).parent / file_name).read_text(),
        status=200,
        content_type=content_type,
    )


@pytest.fixture
def feed_server(httpserver):
    httpserver.expect_request("/atom.xml").respond_with_handler(
        lambda x: _respond_with_file(x, "fixtures/feeds/atom.xml")
    )
    httpserver.expect_request("/github_releases.atom").respond_with_handler(
        lambda x: _respond_with_file(x, "fixtures/feeds/github_releases.atom")
    )
    httpserver.expect_request("/feed_without_ids.xml").respond_with_handler(
        lambda x: _respond_with_file(x, "fixtures/feeds/feed_without_ids.xml")
    )
    httpserver.expect_request("/rss_2_0.xml").respond_with_handler(
        lambda x: _respond_with_file(x, "fixtures/feeds/rss_2_0.xml")
    )
    httpserver.expect_request("/atom_1_0.xml").respond_with_handler(
        lambda x: _respond_with_file(x, "fixtures/feeds/atom_1_0.xml")
    )
    httpserver.expect_request("/feed_with_images.xml").respond_with_handler(
        lambda x: _respond_with_file(x, "fixtures/feeds/feed_with_images.xml")
    )
    httpserver.expect_request("/simple_article.html").respond_with_handler(
        lambda x: _respond_with_file(x, "fixtures/pages/simple_article.html", content_type="text/html")
    )
    return httpserver
