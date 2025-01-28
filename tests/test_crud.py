import pytest
from sqlalchemy.orm import Session
from src import crud, schemas
from src.database import Base, SessionLocal


@pytest.fixture
def db():
    db = SessionLocal()
    Base.metadata.create_all(bind=db.get_bind())
    try:
        yield db
    finally:
        db.close()


def test_create_post(db: Session) -> None:
    # Create test post data
    post_data = schemas.PostCreate(title="Test Post", content="Test Content")

    # Call create_post function
    created_post = crud.create_post(db=db, post=post_data)

    # Assert post was created with correct data
    assert created_post.title == "Test Post"
    assert created_post.content == "Test Content"
    assert created_post.id is not None


def test_create_feed(db: Session) -> None:
    # Create test feed data
    feed_data = schemas.FeedCreate(
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

    # Call create_feed function
    created_feed = crud.create_feed(db=db, feed=feed_data)

    # Assert feed was created with correct data
    assert created_feed.url == "http://feeds.feedburner.com/oatmealfeed"
    assert created_feed.title == "The Oatmeal - Comics, Quizzes, & Stories"
    assert created_feed.favicon_link == "http://theoatmeal.com/favicon.ico"
    assert created_feed.added == 1367063790
    assert created_feed.next_update_time == 2071387335
    assert created_feed.folder_id == 4
    assert created_feed.unread_count == 9
    assert created_feed.ordering == 0
    assert created_feed.link == "http://theoatmeal.com/"
    assert created_feed.pinned is True
    assert created_feed.update_error_count == 0
    assert created_feed.last_update_error == ""
    assert created_feed.id is not None


def test_get_feeds(db: Session) -> None:
    # Create test feed data
    feed_data = schemas.FeedCreate(
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

    # Call create_feed function to add feed to the database
    crud.create_feed(db=db, feed=feed_data)

    # Call get_feeds function
    feeds = crud.get_feeds(db=db)

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
