"""API Endpoints under /feeds/"""

import logging

from fastapi import APIRouter
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


@router.get("/", response_model=ItemGetOut)
def get_items(
    batch_size: int = 10,
    offset: int = 0,
    type: int = 1,
    id: int = 0,
    get_read: bool = True,
    oldest_first: bool = False,
) -> ItemGetOut:
    db = database.get_session()
    query = db.query(database.Article)

    if not get_read:
        query = query.filter(database.Article.unread)

    if offset > 0:
        query = query.filter(database.Article.id <= offset)

    if type == 0 or type == 1:
        query = query.filter(database.Article.feed_id == id)
    elif type == 2:
        query = query.filter(database.Article.starred)
    elif type == 3:
        pass  # No additional filter for type 3 (All)

    if oldest_first:
        query = query.order_by(database.Article.id.asc())
    else:
        query = query.order_by(database.Article.id.desc())

    if batch_size != -1:
        query = query.limit(batch_size)

    items = query.all()
    return ItemGetOut(items=[Article.model_validate(item) for item in items])
