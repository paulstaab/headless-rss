"""API Endpoints under /feeds/"""

import logging

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel, ConfigDict
from pydantic.alias_generators import to_camel

from src import database, feed

logger = logging.getLogger(__name__)


router = APIRouter(prefix="/feeds", tags=["feeds"])


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
    logger.info("Fetching all feeds")
    with database.get_session() as db:
        feeds = db.query(database.Feed).all()
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
    logger.info(f"Adding feed with URL `{input.url}` to folder {input.folder_id}")
    with database.get_session() as db:
        existing_feed = db.query(database.Feed).filter(database.Feed.url == input.url).first()
        if existing_feed:
            logger.error(f"Feed with URL `{input.url}` already exists.")
            raise HTTPException(status_code=409, detail="Feed already exists")

        if input.folder_id is not None:
            folder = db.query(database.Folder).filter(database.Folder.id == input.folder_id).first()
            if not folder:
                logger.error(f"Folder with ID {input.folder_id} does not exist.")
                raise HTTPException(status_code=422, detail=f"Folder with ID {input.folder_id} does not exist")

    try:
        new_feed = feed.create(url=input.url, folder_id=input.folder_id)
    except Exception as e:
        logger.error(f"Error parsing feed from URL {input.url}: {e}")
        raise HTTPException(status_code=422, detail="Feed cannot be read") from e

    with database.get_session() as db:
        db.add(new_feed)
        db.commit()
        db.refresh(new_feed)

    feed.update(new_feed.id)

    return {"feeds": get_feeds().feeds, "newestItemId": new_feed.id}


@router.delete("/{feed_id}")
def delete_feed(feed_id: int):
    logger.info(f"Deleting feed with ID {feed_id}")
    with database.get_session() as db:
        feed = db.query(database.Feed).filter(database.Feed.id == feed_id).first()
        if not feed:
            raise HTTPException(status_code=404, detail="Feed not found")
        db.delete(feed)
        db.commit()


class MoveFeedIn(BaseModel):
    folder_id: int | None

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.put("/{feed_id}/move")
def move_feed(feed_id: int, input: MoveFeedIn):
    logger.info(f"Moving feed with ID {feed_id} to folder {input.folder_id}")
    with database.get_session() as db:
        feed = db.query(database.Feed).filter(database.Feed.id == feed_id).first()
        if not feed:
            raise HTTPException(status_code=404, detail="Feed not found")

        if input.folder_id is not None:
            folder = db.query(database.Folder).filter(database.Folder.id == input.folder_id).first()
            if not folder:
                logger.error(f"Folder with ID {input.folder_id} does not exist.")
                raise HTTPException(status_code=422, detail=f"Folder with ID {input.folder_id} does not exist")

        feed.folder_id = input.folder_id
        db.commit()
        db.refresh(feed)


class RenameFeedIn(BaseModel):
    feed_title: str

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.put("/{feed_id}/rename")
def rename_feed(feed_id: int, input: RenameFeedIn):
    logger.info(f"Renaming feed with ID {feed_id} to `{input.feed_title}`")
    with database.get_session() as db:
        feed = db.query(database.Feed).filter(database.Feed.id == feed_id).first()
        if not feed:
            raise HTTPException(status_code=404, detail="Feed not found")

        feed.title = input.feed_title
        db.commit()
        db.refresh(feed)


class MarkItemsReadIn(BaseModel):
    newest_item_id: int

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.put("/{feed_id}/read")
def mark_items_read(feed_id: int, input: MarkItemsReadIn):
    logger.info(f"Marking items as read in feed with ID {feed_id} until item ID {input.newest_item_id}")
    with database.get_session() as db:
        feed = db.query(database.Feed).filter(database.Feed.id == feed_id).first()
        if not feed:
            raise HTTPException(status_code=404, detail="Feed not found")

        items = (
            db.query(database.Article)
            .filter(database.Article.feed_id == feed_id)
            .filter(database.Article.id <= input.newest_item_id)
            .all()
        )
        for item in items:
            item.unread = False
        db.commit()
