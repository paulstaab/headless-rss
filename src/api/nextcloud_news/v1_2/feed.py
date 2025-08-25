"""API Endpoints under /feeds/"""

import logging

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel, ConfigDict
from pydantic.alias_generators import to_camel

from src import article, feed, folder
from src.folder import NoFolderError

logger = logging.getLogger(__name__)


router = APIRouter(tags=["feeds"])


class Feed(BaseModel):
    id: int
    url: str
    title: str | None
    favicon_link: str | None
    added: int
    next_update_time: int | None
    folder_id: int | None
    # unread_count: int
    ordering: int
    link: str | None
    pinned: bool
    update_error_count: int
    last_update_error: str | None

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


class FeedGetOut(BaseModel):
    feeds: list[Feed]

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.get("", response_model=FeedGetOut)
def get_feeds() -> FeedGetOut:
    """Fetch all feeds from the database."""
    logger.info("Fetching all feeds")
    feeds = feed.get_all()
    root_folder_id = folder.get_root_folder_id()
    for f in feeds:
        if f.folder_id == root_folder_id:
            f.folder_id = None  # type: ignore
    return FeedGetOut(feeds=[Feed.model_validate(feed) for feed in feeds])


class FeedPostIn(BaseModel):
    url: str
    folder_id: int | None

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


class FeedPostOut(BaseModel):
    feeds: list[Feed]
    newest_item_id: int | None

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.post("", response_model=FeedPostOut)
def add_feed(input: FeedPostIn):
    """Add a new feed."""
    folder_id = input.folder_id or folder.get_root_folder_id()  # top-level folder can be referenced by None or 0
    logger.info(f"Adding feed with URL `{input.url}` to folder {folder_id}")
    try:
        new_feed = feed.add(url=input.url, folder_id=folder_id)
    except feed.FeedExistsError as e:
        raise HTTPException(status_code=409, detail=str(e)) from e
    except NoFolderError as e:
        raise HTTPException(status_code=422, detail=str(e)) from e
    except feed.FeedParsingError as e:
        raise HTTPException(status_code=422, detail=str(e)) from e
    except feed.SSRFProtectionError as e:
        raise HTTPException(status_code=400, detail=str(e)) from e

    return {"feeds": get_feeds().feeds, "newestItemId": new_feed.id}


@router.delete("/{feed_id}")
def delete_feed(feed_id: int):
    """Delete a feed from the database."""
    logger.info(f"Deleting feed with ID {feed_id}")
    try:
        feed.delete(feed_id)
    except feed.NoFeedError as e:
        raise HTTPException(status_code=404, detail=str(e)) from e


class MoveFeedIn(BaseModel):
    folder_id: int | None

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.put("/{feed_id}/move")
def move_feed(feed_id: int, input: MoveFeedIn):
    """Move a feed to a different folder."""
    folder_id = input.folder_id or folder.get_root_folder_id()
    logger.info(f"Moving feed with ID {feed_id} to folder {folder_id}")
    try:
        feed.move_to_folder(feed_id, folder_id)
    except feed.NoFeedError as e:
        raise HTTPException(status_code=404, detail=str(e)) from e
    except NoFolderError as e:
        raise HTTPException(status_code=422, detail=str(e)) from e


class RenameFeedIn(BaseModel):
    feed_title: str

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.put("/{feed_id}/rename")
def rename_feed(feed_id: int, input: RenameFeedIn):
    """Rename a feed."""
    logger.info(f"Renaming feed with ID {feed_id} to `{input.feed_title}`")
    try:
        feed.rename(feed_id, input.feed_title)
    except ValueError as e:
        raise HTTPException(status_code=404, detail=str(e)) from e


class MarkItemsReadIn(BaseModel):
    newest_item_id: int

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.put("/{feed_id}/read")
def mark_items_read(feed_id: int, input: MarkItemsReadIn):
    """Mark items as read in a feed."""
    logger.info(f"Marking items as read in feed with ID {feed_id} until item ID {input.newest_item_id}")
    try:
        article.mark_read_by_feed(feed_id, input.newest_item_id)
    except ValueError as e:
        raise HTTPException(status_code=404, detail=str(e)) from e
