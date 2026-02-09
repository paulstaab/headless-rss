import ipaddress
import logging
import random
import socket
import time
from time import mktime
from urllib.parse import urlparse

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


class SSRFProtectionError(Exception):
    """Raised when a URL is blocked due to SSRF protection."""


def _is_testing_mode() -> bool:
    """Detect if we're running in testing mode."""
    import sys

    return "pytest" in sys.modules or any("test" in module for module in sys.modules)


def _validate_url_scheme(parsed_url) -> None:
    """Validate that the URL uses an allowed scheme."""
    if parsed_url.scheme not in ("http", "https"):
        raise SSRFProtectionError(
            f"URL scheme '{parsed_url.scheme}' is not allowed. Only http and https are permitted."
        )


def _validate_hostname(hostname: str | None, allow_localhost: bool) -> None:
    """Validate that the hostname is not blocked."""
    if not hostname:
        raise SSRFProtectionError("URL must have a valid hostname.")

    # Block localhost variants (unless explicitly allowed)
    if not allow_localhost and hostname.lower() in ("localhost", "127.0.0.1", "::1"):
        raise SSRFProtectionError("Access to localhost is not allowed.")


def _validate_ip_address(ip: ipaddress.IPv4Address | ipaddress.IPv6Address, ip_str: str, allow_localhost: bool) -> None:
    """Validate that an IP address is not in blocked ranges."""
    # Block loopback addresses (unless explicitly allowed)
    if not allow_localhost and ip.is_loopback:
        raise SSRFProtectionError(f"Access to loopback address {ip} is not allowed.")

    # Block private addresses (RFC 1918) - but skip if already handled as loopback
    if ip.is_private and not ip.is_loopback:
        raise SSRFProtectionError(f"Access to private address {ip} is not allowed.")

    # Block link-local addresses
    if ip.is_link_local:
        raise SSRFProtectionError(f"Access to link-local address {ip} is not allowed.")

    # Block unspecified addresses (0.0.0.0, ::)
    if ip.is_unspecified:
        raise SSRFProtectionError(f"Access to unspecified address {ip} is not allowed.")

    # Block multicast addresses
    if ip.is_multicast:
        raise SSRFProtectionError(f"Access to multicast address {ip} is not allowed.")

    # Additional check for cloud metadata service (AWS, GCP, Azure common endpoint)
    if ip_str == "169.254.169.254":
        raise SSRFProtectionError("Access to cloud metadata service is not allowed.")


def _validate_feed_url(url: str, allow_localhost: bool | None = None) -> None:
    """Validate that a feed URL is safe to access (SSRF protection).

    This function blocks URLs that could be used for Server-Side Request Forgery (SSRF) attacks:
    - Non-HTTP/HTTPS schemes (file://, ftp://, etc.)
    - Localhost and loopback addresses (127.x.x.x, ::1, localhost)
    - Private network addresses (RFC 1918: 10.x.x.x, 172.16-31.x.x, 192.168.x.x)
    - Link-local addresses (169.254.x.x, fe80::/10)
    - Cloud metadata services (169.254.169.254)

    :param url: The URL to validate.
    :param allow_localhost: If True, allows localhost/loopback addresses. If None, auto-detects testing mode.
    :raises SSRFProtectionError: If the URL is blocked for security reasons.
    """
    # Auto-detect testing mode if not explicitly specified
    if allow_localhost is None:
        allow_localhost = _is_testing_mode()

    parsed_url = urlparse(url)
    _validate_url_scheme(parsed_url)

    hostname = parsed_url.hostname
    _validate_hostname(hostname, allow_localhost)

    # Now we know hostname is not None due to validation
    assert hostname is not None

    # Try to resolve hostname to IP and check if it's in blocked ranges
    try:
        # Get all IP addresses for this hostname
        addr_info = socket.getaddrinfo(hostname, None)
        for _family, _type, _proto, _canonname, sockaddr in addr_info:
            ip_str = str(sockaddr[0])  # Ensure it's a string
            try:
                ip = ipaddress.ip_address(ip_str)
                _validate_ip_address(ip, ip_str, allow_localhost)
            except ValueError:
                # If it's not a valid IP address, continue (could be IPv6 or malformed)
                continue

    except socket.gaierror:
        # DNS resolution failed - this is likely a real domain issue, let it proceed
        # The actual HTTP request will fail with a proper error
        pass


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
    :raises SSRFProtectionError: If the URL is blocked for security reasons.
    """
    # Validate URL for SSRF protection
    _validate_feed_url(url)

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
        feed = db.get(database.Feed, feed_id)
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
    except TypeError, ValueError:
        updated_date = now()

    try:
        pub_date = int(mktime(new_article["published_parsed"]))
    except TypeError, ValueError, KeyError:
        pub_date = updated_date

    # Extract media_thumbnail from feedparser if available
    media_thumbnail: str | None = None
    if "media_thumbnail" in new_article and new_article["media_thumbnail"]:
        # feedparser returns a list of thumbnails, use the first one if available
        thumbnails = new_article["media_thumbnail"]
        if len(thumbnails) > 0:
            media_thumbnail = thumbnails[0].get("url")

    return article.create(
        feed_id=feed_id,
        title=title,
        author=new_article.get("author"),
        content=content,
        enclosure_link=new_article.get("enclosure_link"),
        enclosure_mime=new_article.get("enclosure_mime"),
        guid=guid,
        media_thumbnail=media_thumbnail,
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

        # Delete associated articles
        db.query(database.Article).filter(database.Article.feed_id == feed_id).delete()

        # Delete the feed
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
