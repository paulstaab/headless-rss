import logging
import os
from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import FastAPI
from fastapi_utilities import repeat_every  # type: ignore

from src import database, feed, email

from .nextcloud_news.app import router as nextcloud_router

logging.getLogger("src").setLevel(logging.INFO)
if default_handler := logging.getHandlerByName("default"):
    logging.getLogger("src").addHandler(default_handler)


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Initialize the database connection and start the scheduler.

    :param app: The FastAPI application instance.
    """
    database.init(Path("data/headless-rss.sqlite3"))
    await update_feeds()

    yield


app = FastAPI(lifespan=lifespan)

app.include_router(nextcloud_router, prefix="/index.php/apps/news/api")


@app.get("/status")
def status():
    """Status endpoint to check if the service is running."""
    return {"status": "ok"}


@repeat_every(seconds=int(os.getenv("FEED_UPDATE_FREQUENCY_MIN", 15)) * 60)
async def update_feeds() -> None:
    """Update all feeds."""
    feed.update_all()
    email.check_known_email_accounts()
