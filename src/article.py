import logging
import re
import time
from hashlib import md5

from src import database
from src.content import (
    extract_article,
    summarize_article_with_llm,
)
from src.options import Options

logger = logging.getLogger(__name__)

LLM_SUMMARY_MIN_CHARS = 160


class NoArticleError(Exception):
    """Raised when an article is not found in the database."""


def now() -> int:
    """Get the current time in seconds since the epoch.

    :returns: The current time in seconds since the epoch.
    """
    return int(time.time())


def extract_first_image_url(html_content: str | None) -> str | None:
    """Extract the URL of the first image from HTML content.

    :param html_content: The HTML content to extract the image from.
    :returns: The URL of the first image, or None if no image is found.
    """
    if not html_content:
        return None

    # Use regex to find the first img tag with src attribute
    # Pattern matches src="..." or src='...' and captures the URL
    # Uses a non-greedy match with character class to prevent capturing across quotes
    img_pattern = r'<img[^>]+src=["\']([^"\']+)["\']'
    match = re.search(img_pattern, html_content, re.IGNORECASE)

    if match:
        return match.group(1)

    return None


def create(
    feed_id: int,
    title: str | None,
    author: str | None,
    url: str | None,
    content: str | None,
    guid: str,
    summary: str | None = None,
    pub_date: int | None = None,
    updated_date: int | None = None,
    enclosure_link: str | None = None,
    enclosure_mime: str | None = None,
    media_description: str | None = None,
    media_thumbnail: str | None = None,
) -> database.Article:
    """Create a new article."""
    return database.Article(
        title=title,
        author=author,
        summary=summary,
        content=content,
        content_hash=_hash(content) if content else None,
        enclosure_link=enclosure_link,
        enclosure_mime=enclosure_mime,
        feed_id=feed_id,
        fingerprint=_create_fingerprint(content, title, url),
        guid=guid,
        guid_hash=_hash(guid),
        last_modified=now(),
        media_description=media_description,
        media_thumbnail=media_thumbnail,
        pub_date=pub_date or now(),
        rtl=False,
        starred=False,
        unread=True,
        updated_date=updated_date or now(),
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


def mark_read_by_feed(feed_id: int, newest_item_id: int) -> None:
    """Mark items from a feed older than a given item as read."""
    with database.get_session() as db:
        feed = db.query(database.Feed).filter(database.Feed.id == feed_id).first()
        if not feed:
            from src.feed import NoFeedError

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


def get_by_id(item_id: int) -> database.Article:
    """Fetch an item by its ID."""
    with database.get_session() as db:
        item = db.query(database.Article).filter(database.Article.id == item_id).first()
        if not item:
            raise NoArticleError(f"No article with id {item_id} found")

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


def enrich(article: database.Article, download_fulltext: bool, add_llm_summary: bool) -> database.Article:
    """Enrich the article with additional content extraction and LLM summary if enabled."""
    # Use provided media_thumbnail or fall back to first image in content
    if not article.media_thumbnail:
        article.media_thumbnail = extract_first_image_url(article.content)

    # Download full text if enabled and URL is available
    if download_fulltext and article.url:
        extracted_text = extract_article(article.url)
        if extracted_text:
            logger.info(f"Article {article.id}: Extracted full text of length {len(extracted_text)}")
            article.content = extracted_text
            article.content_hash = _hash(extracted_text)

    # Generate summary if not already present
    if article.content and not article.summary:
        if len(article.content) < LLM_SUMMARY_MIN_CHARS:
            article.summary = article.content

        elif add_llm_summary and Options.get().llm_enabled:
            llm_summary = summarize_article_with_llm(article.content)
            if llm_summary:
                logger.info(f"Article {article.id}: Generated LLM summary of length {len(llm_summary)}")
                article.summary = llm_summary

        else:
            article.summary = article.content[:LLM_SUMMARY_MIN_CHARS] + "..."

    return article
