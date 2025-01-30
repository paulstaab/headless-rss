import logging

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, ConfigDict
from pydantic.alias_generators import to_camel

from src import database, feed
from src.api.nextcloud_news import schema

app = FastAPI()
logger = logging.getLogger(__name__)


class FeedGetOut(BaseModel):
    feeds: list[schema.Feed]

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@app.get("/feeds/", response_model=FeedGetOut)
def get_feeds() -> FeedGetOut:
    db = database.get_session()
    feeds = db.query(database.Feed).all()
    return FeedGetOut(feeds=[schema.Feed.model_validate(feed) for feed in feeds])


class FeedPostIn(BaseModel):
    url: str
    folder_id: int | None

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


class FeedPostOut(BaseModel):
    feeds: list[schema.Feed]
    newest_item_id: int | None

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@app.post("/feeds/", response_model=FeedPostOut)
def add_feed(input: FeedPostIn):
    db = database.get_session()
    existing_feed = db.query(database.Feed).filter(database.Feed.url == input.url).first()
    if existing_feed:
        logger.error(f"Feed with URL {input.url} already exists.")
        raise HTTPException(status_code=409, detail="Feed already exists")

    try:
        new_feed = feed.create(**input.model_dump())
    except Exception as e:
        logger.error(f"Error parsing feed from URL {input.url}: {e}")
        raise HTTPException(status_code=422, detail="Feed cannot be read") from e

    db.add(new_feed)
    db.commit()
    db.refresh(new_feed)

    feed.update(new_feed.id)

    return {"feeds": get_feeds().feeds, "newestItemId": new_feed.id}


@app.delete("/feeds/{feed_id}")
def delete_feed(feed_id: int):
    db = database.get_session()
    feed = db.query(database.Feed).filter(database.Feed.id == feed_id).first()
    if not feed:
        raise HTTPException(status_code=404, detail="Feed not found")
    db.delete(feed)
    db.commit()


class ItemGetOut(BaseModel):
    items: list[schema.Article]

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@app.get("/items/", response_model=ItemGetOut)
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
    return ItemGetOut(items=[schema.Article.model_validate(item) for item in items])
