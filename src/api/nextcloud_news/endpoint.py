from fastapi import FastAPI

from src import database, feed
from src.api.nextcloud_news import schema

app = FastAPI()


@app.get("/feeds/", response_model=list[schema.Feed])
def get_feeds() -> list[schema.Feed]:
    db = database.get_session()
    feeds = db.query(database.Feed).all()
    return [schema.Feed.model_validate(feed) for feed in feeds]


@app.post("/feeds/", response_model=schema.Feed)
def add_feed(input: schema.FeedCreate) -> schema.Feed:
    new_feed = feed.create(**input.model_dump())
    db = database.get_session()
    db.add(new_feed)
    db.commit()
    return schema.Feed.model_validate(new_feed)
