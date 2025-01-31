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


@router.get("/", response_model=ItemGetOut)
def get_items(
    batch_size: int = 10,
    offset: int = 0,
    type: int = 1,
    id: int = 0,
    get_read: bool = True,
    oldest_first: bool = False,
) -> ItemGetOut:
    select_method = FeedSelectionMethod(type)
    db = database.get_session()
    query = db.query(database.Article)

    if not get_read:
        query = query.filter(database.Article.unread)

    if offset > 0:
        query = query.filter(database.Article.id <= offset)

    if select_method == FeedSelectionMethod.FEED:
        query = query.filter(database.Article.feed_id == id)
    elif select_method == FeedSelectionMethod.FOLDER:
        query = query.join(database.Feed).filter(database.Feed.folder_id == id)
    elif select_method == FeedSelectionMethod.STARRED:
        query = query.filter(database.Article.starred)
    elif select_method == FeedSelectionMethod.ALL:
        pass

    if oldest_first:
        query = query.order_by(database.Article.id.asc())
    else:
        query = query.order_by(database.Article.id.desc())

    if batch_size != -1:
        query = query.limit(batch_size)

    items = query.all()
    return ItemGetOut(items=[Article.model_validate(item) for item in items])


@router.get("/updated", response_model=ItemGetOut)
def get_updated_items(last_modified: int, type: int, id: int) -> ItemGetOut:
    select_method = FeedSelectionMethod(type)
    db = database.get_session()
    query = db.query(database.Article).filter(database.Article.last_modified >= last_modified)

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
    db = database.get_session()
    item = db.query(database.Article).filter(database.Article.id == item_id).first()
    if not item:
        raise HTTPException(status_code=404, detail="Item not found")
    item.unread = False
    db.commit()


@router.post("/read/multiple")
def mark_multiple_items_as_read(item_ids: list[int]):
    db = database.get_session()
    items = db.query(database.Article).filter(database.Article.id.in_(item_ids)).all()
    for item in items:
        item.unread = False
    db.commit()


@router.post("/{item_id}/unread")
def mark_item_as_unread(item_id: int):
    db = database.get_session()
    item = db.query(database.Article).filter(database.Article.id == item_id).first()
    if not item:
        raise HTTPException(status_code=404, detail="Item not found")
    item.unread = True
    db.commit()


@router.post("/unread/multiple")
def mark_multiple_items_as_unread(item_ids: list[int]):
    db = database.get_session()
    items = db.query(database.Article).filter(database.Article.id.in_(item_ids)).all()
    for item in items:
        item.unread = True
    db.commit()


@router.post("/{item_id}/star")
def mark_item_as_starred(item_id: int):
    db = database.get_session()
    item = db.query(database.Article).filter(database.Article.id == item_id).first()
    if not item:
        raise HTTPException(status_code=404, detail="Item not found")
    item.starred = True
    db.commit()


@router.post("/star/multiple")
def mark_multiple_items_as_starred(item_ids: list[int]):
    db = database.get_session()
    items = db.query(database.Article).filter(database.Article.id.in_(item_ids)).all()
    for item in items:
        item.starred = True
    db.commit()


@router.post("/{item_id}/unstar")
def mark_item_as_unstarred(item_id: int):
    db = database.get_session()
    item = db.query(database.Article).filter(database.Article.id == item_id).first()
    if not item:
        raise HTTPException(status_code=404, detail="Item not found")
    item.starred = False
    db.commit()


@router.post("/unstar/multiple")
def mark_multiple_items_as_unstarred(item_ids: list[int]):
    db = database.get_session()
    items = db.query(database.Article).filter(database.Article.id.in_(item_ids)).all()
    for item in items:
        item.starred = False
    db.commit()


class MarkAllItemsReadIn(BaseModel):
    newest_item_id: int

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.post("/read")
def mark_all_items_as_read(input: MarkAllItemsReadIn):
    db = database.get_session()
    items = db.query(database.Article).filter(database.Article.id <= input.newest_item_id).all()
    for item in items:
        item.unread = False
    db.commit()
