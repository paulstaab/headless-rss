import time

import feedparser

from src import database


def create(url: str, folder_id: int | None = None) -> database.Feed:
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
    return feedparser.parse(url)
