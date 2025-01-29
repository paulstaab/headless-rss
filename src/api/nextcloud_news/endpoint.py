from fastapi import FastAPI
from pydantic import BaseModel, ConfigDict
from pydantic.alias_generators import to_camel

from src import database, feed
from src.api.nextcloud_news import schema

app = FastAPI()


@app.get("/feeds/", response_model=list[schema.Feed])
def get_feeds() -> list[schema.Feed]:
    db = database.get_session()
    feeds = db.query(database.Feed).all()
    return [schema.Feed.model_validate(feed) for feed in feeds]


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
    new_feed = feed.create(**input.model_dump())
    db = database.get_session()
    db.add(new_feed)
    db.commit()
    db.refresh(new_feed)  # Refresh to get the ID of the newly created feed
    return {"feeds": get_feeds(), "newestItemId": new_feed.id}
