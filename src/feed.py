import logging
import random
import time
from time import mktime

import feedparser

from src import article, database, email
from src.folder import NoFolderError

logger = logging.getLogger(__name__)

thirty_minutes = 1_800
twelve_hours = 43_200
one_day = 86_400


class NoFeedError(Exception):
    """Raised when a feed is not found in the database."""


class FeedExistsError(Exception):
    """Raised when a feed would be duplicated."""


class FeedParsingError(Exception):
    """Raised when there is an error parsing the feed."""


def now() -> int:
    """Get the current time in seconds since the epoch.

    :returns: The current time in seconds since the epoch.
    """
    return int(time.time())


def _create(url: str, folder_id: int) -> database.Feed:
    """Create a new feed in the database.

    :param url: The URL of the feed.
    :param folder_id: The ID of the folder to associate with the feed.
    :returns: The created feed.
    :raises FeedParsingError: If there is an error parsing the feed.
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


def _parse(url: str) -> feedparser.FeedParserDict:
    """Parse the feed from the given URL.

    :param url: The URL of the feed to parse.
    :returns: The parsed feed.
    :raises FeedParsingError: If there is an error parsing the feed.
    """
    parsed_feed = feedparser.parse(url)
    if parsed_feed.bozo:
        raise FeedParsingError(f"Error parsing feed from `{url}`: {parsed_feed.bozo_exception}")
    return parsed_feed


def update(feed_id: int, max_articles: int = 50) -> None:
    """Update the feed with the given ID.

    :param feed_id: The ID of the feed to update.
    :param max_articles: The maximum number of articles to update.
    :raises NoFeedError: If the feed does not exist.
    :raises FeedParsingError: If there is an error parsing the feed.
    """
    with database.get_session() as db:
        feed = db.query(database.Feed).get(feed_id)
        if not feed:
            raise NoFeedError(f"Feed {feed_id} does not exist")
        logger.info(f"Feed {feed_id} ({feed.title}): Updating feed")

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

        feed_article_guid_hashes = []
        n_new_articles = 0

        for idx, new_article in enumerate(parsed_feed.entries):
            if idx >= max_articles:
                break
            try:
                db_article = _create_article(new_article, feed_id)
                feed_article_guid_hashes.append(db_article.guid_hash)

                existing_article = db.query(database.Article).filter_by(guid_hash=db_article.guid_hash).first()
                if not existing_article:
                    db.add(db_article)
                    db.commit()
                    n_new_articles += 1

            except Exception as e:
                logger.error(f"Error adding article from feed {feed_id}: {e}")

        logger.info(
            f"Feed {feed_id} ({feed.title}): Added {n_new_articles} new articles out of {len(parsed_feed.entries)}"
        )
        feed.next_update_time = _calculate_next_update_time(feed_id)
        db.commit()

    clean_up_old_articles(feed_id, feed_article_guid_hashes)


def _create_article(new_article, feed_id: int) -> database.Article:
    """Create a new article in the database.

    :param new_article: The article data to create.
    :param feed_id: The ID of the feed to associate with the article.
    :returns: The created article.
    """
    content: str | None = (
        new_article.get("content")[0]["value"] if "content" in new_article else new_article.get("summary")
    )
    title: str | None = new_article.get("title")
    url: str | None = new_article.get("link")
    guid: str | None = new_article.get("id") or new_article.get("link") or new_article.get("title")
    if guid is None:
        raise ValueError("Article has no ID, link, or title. Failed to create an ID.")

    try:
        updated_date = int(mktime(new_article["updated_parsed"]))
    except (TypeError, ValueError):
        updated_date = now()

    try:
        pub_date = int(mktime(new_article["published_parsed"]))
    except (TypeError, ValueError, KeyError):
        pub_date = updated_date

    return article.create(
        feed_id=feed_id,
        title=title,
        author=new_article.get("author"),
        content=content,
        enclosure_link=new_article.get("enclosure_link"),
        enclosure_mime=new_article.get("enclosure_mime"),
        guid=guid,
        pub_date=pub_date,
        updated_date=updated_date,
        url=url,
    )


def update_all() -> None:
    """Update all feeds in the database.

    This function fetches all feeds from the database and updates each one.
    """
    with database.get_session() as db:
        feeds_to_update = (
            db.query(database.Feed)
            .filter(
                (database.Feed.next_update_time == None) | (database.Feed.next_update_time <= now())  # noqa: E711
            )
            .filter(database.Feed.is_mailing_list == False)  # noqa: E712
            .all()
        )
    logger.info(f"Updating {len(feeds_to_update)} feeds")

    for feed in feeds_to_update:
        update(feed.id)

    email.fetch_emails_from_all_mailboxes()
    logger.info("Finished updating all feeds")


def _calculate_next_update_time(feed_id: int) -> int:
    """Calculate the next update time based on the frequency of the last five posts.

    Use the rolling average number of articles per day over the last week to determine the next update time.

    :param feed_id: The ID of the feed to calculate the next update time for.
    :returns: The next update time in seconds since the epoch.
    """
    with database.get_session() as db:
        avg_articles_per_day = (
            db.query(database.Article)
            .filter(database.Article.feed_id == feed_id)
            .filter(database.Article.pub_date > now() - 7 * one_day)
            .count()
            / 7
        )

    if avg_articles_per_day <= 0.1:
        # Check daily if no article was published in the last week
        # We add a bit of jitter to avoid all feeds being checked at the same time
        next_update_in = one_day + random.randint(-thirty_minutes, thirty_minutes)
    else:
        # Check feeds at 4x of the historical average daily frequency.
        # We use the 4x to account that for variations of the frequency during the day.
        next_update_in = round(one_day / avg_articles_per_day / 4)
        # But we check at least every 12h
        next_update_in = min(next_update_in, twelve_hours)

    logger.info(
        f"Feed {feed_id} has {avg_articles_per_day:.2f} articles per day on average. "
        f"Next update scheduled in {next_update_in / 60:.1f} min."
    )

    return now() + next_update_in


def get_all() -> list[database.Feed]:
    """Fetch all feeds from the database.

    :returns: A list of all feeds in the database.
    """
    with database.get_session() as db:
        return db.query(database.Feed).all()


def get_by_url(url: str) -> database.Feed:
    """Fetch a feed by its URL.

    :param url: The URL of the feed to fetch.
    :returns: The feed with the given URL, or None if not found.
    raises NoFeedError: If the feed does not exist.
    """
    with database.get_session() as db:
        feed = db.query(database.Feed).filter(database.Feed.url == url).first()
    if not feed:
        raise NoFeedError(f"Feed with URL {url} not found")
    return feed


def add(url: str, folder_id: int) -> database.Feed:
    """Add a new feed.

    :param url: The URL of the feed to add.
    :param folder_id: The ID of the folder to associate with the feed.
    :returns: The created feed.
    :raises FeedExistsError: If the feed already exists.
    :raises NoFolderError: If the folder does not exist.
    :raises FeedParsingError: If there is an error parsing the feed.
    """
    with database.get_session() as db:
        existing_feed = db.query(database.Feed).filter(database.Feed.url == url).first()
        if existing_feed:
            raise FeedExistsError("Feed already exists")

        folder = db.query(database.Folder).filter(database.Folder.id == folder_id).first()
        if not folder:
            raise NoFolderError(f"Folder with ID {folder_id} does not exist")

    new_feed = _create(url=url, folder_id=folder_id)
    with database.get_session() as db:
        db.add(new_feed)
        db.commit()
        db.refresh(new_feed)

    update(new_feed.id, max_articles=10)
    return new_feed


def add_mailing_list(from_address: str, title: str, folder_id: int) -> database.Feed:
    """Add a new mailing list feed."""
    with database.get_session() as db:
        existing_feed = db.query(database.Feed).filter(database.Feed.url == from_address).first()
        if existing_feed:
            raise FeedExistsError("Feed already exists")

        folder = db.query(database.Folder).filter(database.Folder.id == folder_id).first()
        if not folder:
            raise NoFolderError(f"Folder with ID {folder_id} does not exist")

    new_feed = database.Feed(
        url=from_address,
        title=title,
        favicon_link=None,
        link=None,
        folder_id=folder_id,
        added=now(),
        is_mailing_list=True,
    )
    with database.get_session() as db:
        db.add(new_feed)
        db.commit()
        db.refresh(new_feed)

    return new_feed


def delete(feed_id: int) -> None:
    """Delete a feed from the database.

    :param feed_id: The ID of the feed to delete.
    :raises NoFeedError: If the feed does not exist.
    """
    with database.get_session() as db:
        feed = db.query(database.Feed).filter(database.Feed.id == feed_id).first()
        if not feed:
            raise NoFeedError(f"Feed {feed_id} not found")
        db.delete(feed)
        db.commit()


def move_to_folder(feed_id: int, folder_id: int) -> None:
    """Move a feed to a different folder.

    :param feed_id: The ID of the feed to move.
    :param folder_id: The ID of the folder to move the feed to.
    :raises NoFeedError: If the feed does not exist.
    :raises NoFolderError: If the folder does not exist.
    """
    with database.get_session() as db:
        feed = db.query(database.Feed).filter(database.Feed.id == feed_id).first()
        if not feed:
            raise NoFeedError(f"Feed {feed_id} not found")

        folder = db.query(database.Folder).filter(database.Folder.id == folder_id).first()
        if not folder:
            raise NoFolderError(f"Folder with ID {folder_id} does not exist")

        feed.folder_id = folder_id
        db.commit()
        db.refresh(feed)


def rename(feed_id: int, new_title: str) -> None:
    """Rename a feed.

    :param feed_id: The ID of the feed to rename.
    :param new_title: The new title of the feed.
    :raises NoFeedError: If the feed does not exist.
    """
    with database.get_session() as db:
        feed = db.query(database.Feed).filter(database.Feed.id == feed_id).first()
        if not feed:
            raise NoFeedError(f"Feed {feed_id} not found")

        feed.title = new_title
        db.commit()
        db.refresh(feed)


def clean_up_old_articles(feed_id: int, feed_guid_hashes: list[str]) -> None:
    """Clean up old articles from the database.
    This function deletes articles that are not included in the feed anymore, read, not starred,
    and have been last updated more than 90 days ago.
    :param feed_id: The ID of the feed to clean up articles for.
    :param feed_articles: The articles from the feed.
    """
    ninety_days_ago = int(time.time()) - 90 * 24 * 60 * 60

    with database.get_session() as db:
        articles_to_delete = (
            db.query(database.Article)
            .filter(database.Article.feed_id == feed_id)
            .filter(database.Article.last_modified < ninety_days_ago)
            .filter(database.Article.unread == False)  # noqa: E712
            .filter(database.Article.starred == False)  # noqa: E712
            .filter(database.Article.guid_hash.notin_(feed_guid_hashes))
            .all()
        )

        logger.info(f"Removing {len(articles_to_delete)} old articles from database")
        for new_article in articles_to_delete:
            db.delete(new_article)

        db.commit()
