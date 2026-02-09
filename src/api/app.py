import logging
from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from fastapi_utilities import repeat_every  # type: ignore

from src import database, feed
from src.options import Options

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

# Add CORS middleware to support browser clients
# Using allow_origins=["*"] to support static web apps from any origin.
# Since this is designed for self-hosting, users can restrict origins
# via a reverse proxy if needed. allow_credentials=False ensures the
# wildcard origin is properly returned in responses.
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=False,
    allow_methods=["*"],
    allow_headers=["*"],
)

app.include_router(nextcloud_router, prefix="/index.php/apps/news/api")


@app.get("/status")
def status():
    """Status endpoint to check if the service is running."""
    return {"status": "ok"}


@repeat_every(seconds=Options.get().feed_update_frequency_min * 60)
async def update_feeds() -> None:
    """Update all feeds."""
    feed.update_all()
