from fastapi import FastAPI
from pathlib import Path
from src import database

from . import feed, folder, item

app = FastAPI()
app.include_router(feed.router)
app.include_router(item.router)
app.include_router(folder.router)

@app.on_event("startup")
async def startup_event():
    database.init(Path("data/headless-rss.sqlite3"))
