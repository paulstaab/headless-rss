import logging
import time
from hashlib import md5
from time import mktime

import feedparser

from src import database

logger = logging.getLogger(__name__)


def now() -> int:
    """Get the current time in seconds since the epoch.

    :returns: The current time in seconds since the epoch.
    """
    return int(time.time())


def create(url: str, folder_id: int | None = None) -> database.Feed:
    """Create a new feed in the database.

    :param url: The URL of the feed.
    :param folder_id: The ID of the folder to associate with the feed.
    :returns: The created feed.
    """
    logger.info(f"Creating feed for URL: {url}")
    parsed_feed = _parse(url)

    return database.Feed(
        url=url,
        title=parsed_feed.feed.title,
        favicon_link=parsed_feed.feed.get("favicon"),
        link=parsed_feed.feed.get("link"),
        folder_id=folder_id,
        added=now(),
    )


def _parse(url: str):
    """Parse the feed from the given URL.

    :param url: The URL of the feed to parse.
    :returns: The parsed feed.
    :raises ValueError: If there is an error parsing the feed.
    """
    parsed_feed = feedparser.parse(url)
    if parsed_feed.bozo:
        raise ValueError(f"Error parsing feed: {parsed_feed.bozo_exception}")
    return parsed_feed


def update(feed_id: int, max_articles: int = 50) -> None:
    """Update the feed with the given ID.

    :param feed_id: The ID of the feed to update.
    :param max_articles: The maximum number of articles to update.
    :raises ValueError: If the feed does not exist.
    """
    with database.get_session() as db:
        feed = db.query(database.Feed).get(feed_id)
    if not feed:
        raise ValueError(f"Feed with ID {feed_id} does not exist")
    logger.info(f"Updating feed {feed_id} ({feed.title})")
    with database.get_session() as db:
        try:
            parsed_feed = _parse(feed.url)
        except Exception as e:
            logger.error(f"Error updating feed {feed_id}: {e}")
            feed.update_error_count += 1
            feed.last_update_error = str(e)
            db.commit()
            return
        if feed.update_error_count > 0:
            feed.update_error_count = 0
            feed.last_update_error = None
            db.commit()

        for idx, article in enumerate(parsed_feed.entries):
            if idx >= max_articles:
                break
            try:
                existing_article = db.query(database.Article).filter_by(guid_hash=_hash(article["id"])).first()
                if not existing_article:
                    db.add(_create_article(article, feed_id))
                    db.commit()

            except Exception as e:
                logger.error(f"Error adding article from feed {feed_id}: {e}")


def _create_article(article, feed_id: int) -> database.Article:
    """Create a new article in the database.

    :param article: The article data to create.
    :param feed_id: The ID of the feed to associate with the article.
    :returns: The created article.
    """
    content: str | None = article.get("content")[0]["value"] if "content" in article else article.get("summary")
    title: str | None = article.get("title")
    url: str | None = article.get("link")

    try:
        updated_date = int(mktime(article["updated_parsed"]))
    except (TypeError, ValueError):
        updated_date = now()

    try:
        pub_date = int(mktime(article["published_parsed"]))
    except (TypeError, ValueError, KeyError):
        pub_date = updated_date

    return database.Article(
        title=title,
        author=article.get("author"),
        body=content,
        content=content,
        content_hash=_hash(content) if content else None,
        enclosure_link=article.get("enclosure_link"),
        enclosure_mime=article.get("enclosure_mime"),
        feed_id=feed_id,
        fingerprint=_create_fingerprint(content, title, url),
        guid=article["id"],
        guid_hash=md5(article["id"].encode()).hexdigest(),
        last_modified=now(),
        pub_date=pub_date,
        rtl=False,
        starred=False,
        unread=True,
        updated_date=updated_date,
        url=url,
    )


def _hash(value: str) -> str:
    """Generate an MD5 hash for the given value.

    :param value: The value to hash.
    :returns: The MD5 hash of the value.
    """
    return md5(value.encode()).hexdigest()


def _create_fingerprint(content: str | None, title: str | None, url: str | None) -> str:
    """Create a fingerprint for the given content, title, and URL.

    :param content: The content of the article.
    :param title: The title of the article.
    :param url: The URL of the article.
    :returns: The fingerprint of the article.
    """
    return _hash(_hash(content or "") + _hash(title or "") + _hash(url or ""))


def update_all() -> None:
    """Update all feeds in the database.

    This function fetches all feeds from the database and updates each one.
    """
    with database.get_session() as db:
        feeds = db.query(database.Feed).all()
    logger.info(f"Updating {len(feeds)} feeds")
    for feed in feeds:
        update(feed.id)
