import logging
import time
from hashlib import md5
from time import mktime

import feedparser

from src import database

logger = logging.getLogger(__name__)


def now() -> int:
    return int(time.time())


def create(url: str, folder_id: int | None = None) -> database.Feed:
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
        existing_article = db.query(database.Article).filter_by(guid_hash=_hash(article["id"])).first()
        if not existing_article:
            db.add(_create_article(article, feed_id))
    db.commit()


def _create_article(article, feed_id: int) -> database.Article:
    content: str | None = article.get("content")[0]["value"] if "content" in article else article.get("summary")
    title: str | None = article.get("title")
    url: str | None = article.get("link")

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
        media_description=article.get("media_description"),
        media_thumbnail=article.get("media_thumbnail"),
        pub_date=int(mktime(article.get("published_parsed"))),
        rtl=False,
        starred=False,
        unread=True,
        updated_date=int(mktime(article.get("updated_parsed"))),
        url=url,
    )


def _hash(value: str) -> str:
    return md5(value.encode()).hexdigest()


def _create_fingerprint(content: str | None, title: str | None, url: str | None) -> str:
    return _hash(_hash(content or "") + _hash(title or "") + _hash(url or ""))


def update_all() -> None:
    db = database.get_session()
    feeds = db.query(database.Feed).all()
    logger.info(f"Updating {len(feeds)} feeds")
    for feed in feeds:
        update(feed.id)
