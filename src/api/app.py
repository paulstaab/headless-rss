import logging
from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import FastAPI

from src import database

from .nextcloud_news.app import router as nextcloud_router

logging.getLogger("src").setLevel(logging.INFO)
if default_handler := logging.getHandlerByName("default"):
    logging.getLogger("src").addHandler(default_handler)


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Initialize the database connection.

    :param app: The FastAPI application instance.
    """
    database.init(Path("data/headless-rss.sqlite3"))
    yield


app = FastAPI(lifespan=lifespan)

app.include_router(nextcloud_router, prefix="/index.php/apps/news/api")
