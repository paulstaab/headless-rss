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
    db = database.get_session()
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
    db = database.get_session()
    existing_feed = db.query(database.Feed).filter(database.Feed.url == input.url).first()
    if existing_feed:
        logger.error(f"Feed with URL `{input.url}` already exists.")
        raise HTTPException(status_code=409, detail="Feed already exists")

    if input.folder_id is None or input.folder_id == 0:
        folder = db.query(database.Folder).filter(database.Folder.name == None).first()  # noqa: E711
        if not folder:
            logger.info("Creating default folder")
            folder = database.Folder(id=0, name=None)
            db.add(folder)
            db.commit()
            db.refresh(folder)
    else:
        folder = db.query(database.Folder).filter(database.Folder.id == input.folder_id).first()
        if not folder:
            logger.error(f"Folder with ID {input.folder_id} does not exist.")
            raise HTTPException(status_code=422, detail=f"Folder with ID {input.folder_id} does not exist")

    try:
        new_feed = feed.create(url=input.url, folder_id=folder.id)
    except Exception as e:
        logger.error(f"Error parsing feed from URL {input.url}: {e}")
        raise HTTPException(status_code=422, detail="Feed cannot be read") from e

    db.add(new_feed)
    db.commit()
    db.refresh(new_feed)

    feed.update(new_feed.id)

    return {"feeds": get_feeds().feeds, "newestItemId": new_feed.id}


@router.delete("/{feed_id}")
def delete_feed(feed_id: int):
    db = database.get_session()
    feed = db.query(database.Feed).filter(database.Feed.id == feed_id).first()
    if not feed:
        raise HTTPException(status_code=404, detail="Feed not found")
    db.delete(feed)
    db.commit()
