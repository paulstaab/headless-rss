import logging
import time
from hashlib import md5

import feedparser

from src import database

logger = logging.getLogger(__name__)


def create(url: str, folder_id: int | None = None) -> database.Feed:
    logger.info(f"Creating feed for URL: {url}")
    parsed_feed = _parse(url)

    return database.Feed(
        url=url,
        title=parsed_feed.feed.title,
        favicon_link=parsed_feed.feed.get("favicon"),
        link=parsed_feed.feed.get("link"),
        folder_id=folder_id,
        added=int(time.time()),
    )


def _parse(url: str):
    parsed_feed = feedparser.parse(url)
    if parsed_feed.bozo:
        raise ValueError(f"Error parsing feed: {parsed_feed.bozo_exception}")
    return parsed_feed


def update(feed_id: int) -> None:
    db = database.get_session()
    feed = db.query(database.Feed).get(feed_id)
    if not feed:
        raise ValueError(f"Feed with ID {feed_id} does not exist")
    logger.info(f"Updating feed {feed_id} ({feed.title})")
    parsed_feed = _parse(feed.url)
    for article in parsed_feed.entries:
        db.add(_create_article(article, feed_id))
    db.commit()


def _create_article(article, feed_id: int) -> database.Article:
    return database.Article(
        title=article.get("title"),
        author=article.get("author"),
        body=article.get("summary"),
        content_hash=article.get("content_hash"),
        enclosure_link=article.get("enclosure_link"),
        enclosure_mime=article.get("enclosure_mime"),
        feed_id=feed_id,
        fingerprint=article.get("fingerprint"),
        guid=article["id"],
        guid_hash=md5(article["id"].encode()).hexdigest(),
        last_modified=article.get("last_modified"),
        media_description=article.get("media_description"),
        media_thumbnail=article.get("media_thumbnail"),
        pub_date=article.get("pub_date"),
        rtl=article.get("rtl"),
        starred=article.get("starred"),
        unread=article.get("unread"),
        updated_date=article.get("updated_date"),
        url=article.get("url"),
    )
