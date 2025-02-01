import logging
from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import FastAPI

from src import database

from . import feed, folder, item

logging.getLogger("src").setLevel(logging.INFO)


@asynccontextmanager
async def lifespan(app: FastAPI):
    database.init(Path("data/headless-rss.sqlite3"))
    yield


app = FastAPI(lifespan=lifespan)
app.include_router(feed.router)
app.include_router(feed.router, prefix="/index.php/apps/news/api/v1-2")
app.include_router(item.router)
app.include_router(item.router, prefix="/index.php/apps/news/api/v1-2")
app.include_router(folder.router)
app.include_router(folder.router, prefix="/index.php/apps/news/api/v1-2")
