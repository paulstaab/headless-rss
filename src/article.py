import time

from src import database
from src.feed import NoFeedError


class NoArticleError(Exception):
    """Raised when an article is not found in the database."""


def mark_read_by_feed(feed_id: int, newest_item_id: int) -> None:
    """Mark items from a feed older than a given item as read."""
    with database.get_session() as db:
        feed = db.query(database.Feed).filter(database.Feed.id == feed_id).first()
        if not feed:
            raise NoFeedError(f"No feed with ID {feed_id} exists")

        items = (
            db.query(database.Article)
            .filter(database.Article.feed_id == feed_id)
            .filter(database.Article.id <= newest_item_id)
            .all()
        )

        for item in items:
            item.unread = False

        db.commit()


def get_all(
    max_results: int = 0,
    newest_item_id: int = 0,
    get_read: bool = True,
    oldest_first: bool = True,
    last_modified: int = 0,
) -> list[database.Article]:
    """Fetch all items based on the provided parameters."""
    with database.get_session() as db:
        query = db.query(database.Article)
        items = _filter_article_query(query, max_results, newest_item_id, get_read, oldest_first, last_modified)

    return items


def get_by_feed(
    feed_id: int,
    max_results: int = 0,
    newest_item_id: int = 0,
    get_read: bool = True,
    oldest_first: bool = True,
    last_modified: int = 0,
) -> list[database.Article]:
    """Fetch items based on the provided parameters."""
    with database.get_session() as db:
        query = db.query(database.Article).filter(database.Article.feed_id == feed_id)
        items = _filter_article_query(query, max_results, newest_item_id, get_read, oldest_first, last_modified)

    return items


def get_by_folder(
    folder_id: int,
    max_results: int = 0,
    newest_item_id: int = 0,
    get_read: bool = True,
    oldest_first: bool = True,
    last_modified: int = 0,
) -> list[database.Article]:
    """Fetch items based on the provided parameters."""
    with database.get_session() as db:
        query = db.query(database.Article).join(database.Feed).filter(database.Feed.folder_id == folder_id)
        items = _filter_article_query(query, max_results, newest_item_id, get_read, oldest_first, last_modified)

    return items


def get_starred(
    max_results: int = 0,
    newest_item_id: int = 0,
    get_read: bool = True,
    oldest_first: bool = True,
    last_modified: int = 0,
) -> list[database.Article]:
    """Fetch starred items based on the provided parameters."""
    with database.get_session() as db:
        query = db.query(database.Article).filter(database.Article.starred)
        items = _filter_article_query(query, max_results, newest_item_id, get_read, oldest_first, last_modified)

    return items


def get_by_guid_hash(feedId: int, guidHash: str) -> database.Article:  # noqa: N803
    """Fetch an item by its GUID hash."""
    with database.get_session() as db:
        item = (
            db.query(database.Article)
            .filter(database.Article.feed_id == feedId, database.Article.guid_hash == guidHash)
            .first()
        )
        if not item:
            raise NoArticleError(f"Not article with guid_hash {guidHash} found for feed {feedId}")

    return item


def _filter_article_query(
    query,
    max_results: int = 0,
    newest_item_id: int = 0,
    get_read: bool = True,
    oldest_first: bool = True,
    last_modified: int = 0,
) -> list[database.Article]:
    if not get_read:
        query = query.filter(database.Article.unread)

    if newest_item_id > 0:
        query = query.filter(database.Article.id <= newest_item_id)

    if last_modified > 0:
        query = query.filter(database.Article.last_modified >= last_modified)

    query = query.order_by(database.Article.id.asc() if oldest_first else database.Article.id.desc())

    if max_results > 0:
        query = query.limit(max_results)

    items = query.all()
    return items


def mark_as_read(item_ids: list[int]) -> int:
    """Mark aricles as read."""
    with database.get_session() as db:
        items = db.query(database.Article).filter(database.Article.id.in_(item_ids)).all()
        for item in items:
            item.unread = False
            item.last_modified = int(time.time())
        db.commit()
        return len(items)


def mark_as_unread(item_ids: list[int]) -> int:
    """Mark article as unread."""
    with database.get_session() as db:
        items = db.query(database.Article).filter(database.Article.id.in_(item_ids)).all()
        for item in items:
            item.unread = True
            item.last_modified = int(time.time())
        db.commit()
        return len(items)


def mark_as_starred(item_ids: list[int]) -> int:
    """Mark articles as starred."""
    with database.get_session() as db:
        items = db.query(database.Article).filter(database.Article.id.in_(item_ids)).all()
        for item in items:
            item.starred = True
            item.last_modified = int(time.time())
        db.commit()
        return len(items)


def mark_as_unstarred(item_ids: list[int]) -> int:
    """Mark articles as unstarred."""
    with database.get_session() as db:
        items = db.query(database.Article).filter(database.Article.id.in_(item_ids)).all()
        for item in items:
            item.starred = False
            item.last_modified = int(time.time())
        db.commit()
        return len(items)


def mark_all_as_read(newest_item_id: int) -> int:
    """Mark all items as read until the specified item ID."""
    with database.get_session() as db:
        items = db.query(database.Article).filter(database.Article.id <= newest_item_id).all()
        for item in items:
            item.unread = False
            item.last_modified = int(time.time())
        db.commit()
        return len(items)


def clean_up_old_articles(feed_id: int, feed_articles) -> None:
    """Clean up old articles from the database.

    This function deletes articles that are not included in the feed anymore, read, not starred,
    and have been last updated more than 90 days ago.

    :param feed_id: The ID of the feed to clean up articles for.
    :param feed_articles: The articles from the feed.
    """
    with database.get_session() as db:
        feed = db.query(database.Feed).get(feed_id)
        if not feed:
            raise NoFeedError(f"Feed {feed_id} does not exist")

        feed_article_guids = {article["id"] for article in feed_articles}

        ninety_days_ago = int(time.time()) - 90 * 24 * 60 * 60
        articles_to_delete = (
            db.query(database.Article)
            .filter(database.Article.feed_id == feed_id)
            .filter(database.Article.guid.notin_(feed_article_guids))
            .filter(database.Article.unread == False)  # noqa: E712
            .filter(database.Article.starred == False)  # noqa: E712
            .filter(database.Article.last_modified < ninety_days_ago)
            .all()
        )

        for article in articles_to_delete:
            db.delete(article)

        db.commit()
