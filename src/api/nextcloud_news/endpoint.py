from fastapi import FastAPI

from src import database
from src.api.nextcloud_news import schema

app = FastAPI()


@app.get("/feeds/", response_model=list[schema.Feed])
def get_feeds() -> list[schema.Feed]:
    db = database.get_session()
    feeds = db.query(database.Feed).all()
    return [schema.Feed.model_validate(feed) for feed in feeds]
