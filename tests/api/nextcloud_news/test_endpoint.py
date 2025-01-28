import pytest
from fastapi.testclient import TestClient
from sqlalchemy.orm import Session
from src import database
from src.api.nextcloud_news.endpoint import app


@pytest.fixture
def client() -> TestClient:
    return TestClient(app)


@pytest.fixture
def db():
    database.init(file=None)
    db = database.get_session()
    database.Base.metadata.create_all(bind=db.get_bind())
    yield db
    db.close()


def test_get_feeds(db: Session, client: TestClient) -> None:
    # given
    feed = database.Feed(
        url="http://feeds.feedburner.com/oatmealfeed",
        title="The Oatmeal - Comics, Quizzes, & Stories",
        favicon_link="http://theoatmeal.com/favicon.ico",
        added=1367063790,
        next_update_time=2071387335,
        folder_id=4,
        unread_count=9,
        ordering=0,
        link="http://theoatmeal.com/",
        pinned=True,
        update_error_count=0,
        last_update_error="",
    )
    db.add(feed)
    db.commit()

    # when
    response = client.get("/feeds/")

    # then
    assert response.status_code == 200

    # Assert feeds were retrieved with correct data
    assert len(feeds) == 1
    assert feeds[0].url == "http://feeds.feedburner.com/oatmealfeed"
    assert feeds[0].title == "The Oatmeal - Comics, Quizzes, & Stories"
    assert feeds[0].favicon_link == "http://theoatmeal.com/favicon.ico"
    assert feeds[0].added == 1367063790
    assert feeds[0].next_update_time == 2071387335
    assert feeds[0].folder_id == 4
    assert feeds[0].unread_count == 9
    assert feeds[0].ordering == 0
    assert feeds[0].link == "http://theoatmeal.com/"
    assert feeds[0].pinned is True
    assert feeds[0].update_error_count == 0
    assert feeds[0].last_update_error == ""
    assert feeds[0].id is not None
