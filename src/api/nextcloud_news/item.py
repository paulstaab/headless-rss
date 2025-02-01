"""API Endpoints under /feeds/"""

import enum
import logging

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel, ConfigDict
from pydantic.alias_generators import to_camel

from src import database

logger = logging.getLogger(__name__)


router = APIRouter(prefix="/items", tags=["items"])


class Article(BaseModel):
    id: int
    title: str | None
    content: str | None
    author: str | None
    body: str | None
    content_hash: str | None
    enclosure_link: str | None
    enclosure_mime: str | None
    feed_id: int
    fingerprint: str | None
    guid: str
    guid_hash: str
    last_modified: str | None
    media_description: str | None
    media_thumbnail: str | None
    pub_date: int | None
    rtl: bool
    starred: bool
    unread: bool
    updated_date: str | None
    url: str | None

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


class ItemGetOut(BaseModel):
    items: list[Article]

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


class FeedSelectionMethod(enum.Enum):
    FEED = 0
    FOLDER = 1
    STARRED = 2
    ALL = 3


@router.get("", response_model=ItemGetOut)
def get_items(
    batchSize: int = -1,  # noqa: N803
    offset: int = 0,
    type: int = 1,
    id: int = 0,
    getRead: bool = True,  # noqa: N803
    oldestFirst: bool = False,  # noqa: N803
    lastModified: int = 0,  # noqa: N803  # not official supported, by used by Fiery Feeds
) -> ItemGetOut:
    select_method = FeedSelectionMethod(type)
    logger.info(
        f"Getting items for {select_method} = {id} "
        f"({batchSize=}, {offset=}, {lastModified=}, {getRead=}, {oldestFirst=})"
    )
    db = database.get_session()
    query = db.query(database.Article)

    if not getRead:
        query = query.filter(database.Article.unread)

    if offset > 0:
        query = query.filter(database.Article.id <= offset)

    if lastModified > 0:
        query = query.filter(database.Article.last_modified >= lastModified)

    if select_method == FeedSelectionMethod.FEED:
        query = query.filter(database.Article.feed_id == id)
    elif select_method == FeedSelectionMethod.FOLDER:
        query = query.join(database.Feed).filter(database.Feed.folder_id == id)
    elif select_method == FeedSelectionMethod.STARRED:
        query = query.filter(database.Article.starred)
    elif select_method == FeedSelectionMethod.ALL:
        pass

    query = query.order_by(database.Article.id.asc() if oldestFirst else database.Article.id.desc())

    if batchSize > 0:
        query = query.limit(batchSize)

    items = query.all()
    logger.info(f"Found {len(items)} items")

    return ItemGetOut(items=[Article.model_validate(item) for item in items])


@router.get("/updated", response_model=ItemGetOut)
def get_updated_items(
    lastModified: int,  # noqa: N803
    type: int,
    id: int,
) -> ItemGetOut:
    select_method = FeedSelectionMethod(type)
    logger.info(f"Getting updated items for {select_method} = {id} (lastModified={lastModified})")
    db = database.get_session()
    query = db.query(database.Article).filter(database.Article.last_modified >= lastModified)

    if select_method == FeedSelectionMethod.FEED:
        query = query.filter(database.Article.feed_id == id)
    elif select_method == FeedSelectionMethod.FOLDER:
        query = query.join(database.Feed).filter(database.Feed.folder_id == id)
    elif select_method == FeedSelectionMethod.STARRED:
        query = query.filter(database.Article.starred)
    elif select_method == FeedSelectionMethod.ALL:
        pass

    items = query.all()
    return ItemGetOut(items=[Article.model_validate(item) for item in items])


@router.post("/{item_id}/read")
def mark_item_as_read(item_id: int):
    logger.info(f"Marking item {item_id} as read")
    db = database.get_session()
    item = db.query(database.Article).filter(database.Article.id == item_id).first()
    if not item:
        raise HTTPException(status_code=404, detail="Item not found")
    item.unread = False
    db.commit()


class ItemIDListIn(BaseModel):
    items: list[int]

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.put("/read/multiple")
def mark_multiple_items_as_read(input: ItemIDListIn) -> None:  # noqa: N803
    logger.info(f"Marking multiple items as read: {input.items}")
    db = database.get_session()
    items = db.query(database.Article).filter(database.Article.id.in_(input.items)).all()
    for item in items:
        item.unread = False
    db.commit()


@router.put("/{item_id}/unread")
def mark_item_as_unread(item_id: int) -> None:
    logger.info(f"Marking item {item_id} as unread")
    db = database.get_session()
    item = db.query(database.Article).filter(database.Article.id == item_id).first()
    if not item:
        raise HTTPException(status_code=404, detail="Item not found")
    item.unread = True
    db.commit()


@router.put("/unread/multiple")
def mark_multiple_items_as_unread(input: ItemIDListIn) -> None:
    logger.info(f"Marking multiple items as unread: {input.items}")
    db = database.get_session()
    items = db.query(database.Article).filter(database.Article.id.in_(input.items)).all()
    for item in items:
        item.unread = True
    db.commit()


@router.put("/{feedId}/{guidHash}/star")
def mark_item_as_starred(feedId: int, guidHash: str) -> None:  # noqa: N803
    logger.info(f"Marking item {guidHash} as starred")
    db = database.get_session()
    item = (
        db.query(database.Article)
        .filter(database.Article.feed_id == feedId, database.Article.guid_hash == guidHash)
        .first()
    )
    if not item:
        raise HTTPException(status_code=404, detail="Item not found")
    item.starred = True
    db.commit()


class ItemByGuidHash(BaseModel):
    feed_id: int
    guid_hash: str

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


class ItemGuidListIn(BaseModel):
    items: list[ItemByGuidHash]

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.put("/star/multiple")
def mark_multiple_items_as_starred(input: ItemGuidListIn) -> None:
    logger.info(f"Marking {len(input.items)} items as starred")
    db = database.get_session()
    for item in input.items:
        article = (
            db.query(database.Article)
            .filter(database.Article.feed_id == item.feed_id, database.Article.guid_hash == item.guid_hash)
            .first()
        )
        if not article:
            raise HTTPException(status_code=404, detail="Item not found")
        article.starred = True
    db.commit()


@router.put("/{feedId}/{guidHash}/unstar")
def mark_item_as_unstarred(feedId: int, guidHash: str) -> None:  # noqa: N803
    logger.info(f"Marking item {guidHash} as unstarred")
    db = database.get_session()
    item = (
        db.query(database.Article)
        .filter(database.Article.feed_id == feedId, database.Article.guid_hash == guidHash)
        .first()
    )
    if not item:
        raise HTTPException(status_code=404, detail="Item not found")
    item.starred = False
    db.commit()


@router.put("/unstar/multiple")
def mark_multiple_items_as_unstarred(input: ItemGuidListIn) -> None:
    logger.info(f"Marking {len(input.items)} items as unstarred")
    db = database.get_session()
    for item in input.items:
        article = (
            db.query(database.Article)
            .filter(database.Article.feed_id == item.feed_id, database.Article.guid_hash == item.guid_hash)
            .first()
        )
        if not article:
            raise HTTPException(status_code=404, detail="Item not found")
        article.starred = False
    db.commit()


class MarkAllItemsReadIn(BaseModel):
    newest_item_id: int

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.put("/read")
def mark_all_items_as_read(input: MarkAllItemsReadIn):
    logger.info(f"Marking all items as read until {input.newest_item_id}")
    db = database.get_session()
    items = db.query(database.Article).filter(database.Article.id <= input.newest_item_id).all()
    for item in items:
        item.unread = False
    db.commit()
