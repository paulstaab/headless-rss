from fastapi import FastAPI

from . import feed, folder, item

app = FastAPI()
app.include_router(feed.router)
app.include_router(item.router)
app.include_router(folder.router)
