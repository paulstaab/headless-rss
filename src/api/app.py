import logging
import os
from contextlib import asynccontextmanager
from pathlib import Path

from apscheduler.schedulers.background import BackgroundScheduler  # type: ignore
from fastapi import FastAPI

from src import database, feed

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

    scheduler = BackgroundScheduler()
    feed_update_frequency = int(os.getenv("FEED_UPDATE_FREQUENCY", 15))
    scheduler.add_job(feed.update_all(), "interval", minutes=feed_update_frequency)  # type: ignore
    scheduler.start()

    yield

    scheduler.shutdown()


app = FastAPI(lifespan=lifespan)

app.include_router(nextcloud_router, prefix="/index.php/apps/news/api")


@app.get("/status")
async def status():
    """Status endpoint to check if the service is running."""
    return {"status": "ok"}
