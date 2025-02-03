import logging
import os
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Annotated

from fastapi import Depends, FastAPI, HTTPException, status
from fastapi.security import HTTPBasic, HTTPBasicCredentials

from src import database

from . import feed, folder, item

logging.getLogger("src").setLevel(logging.INFO)
if default_handler := logging.getHandlerByName("default"):
    logging.getLogger("src").addHandler(default_handler)


@asynccontextmanager
async def lifespan(app: FastAPI):
    database.init(Path("data/headless-rss.sqlite3"))
    yield


app = FastAPI(lifespan=lifespan)

security = HTTPBasic(auto_error=False)


def get_current_username(credentials: Annotated[HTTPBasicCredentials, Depends(security)]):
    username = os.getenv("USERNAME")
    password = os.getenv("PASSWORD")
    if username is None or password is None:
        return
    if credentials is None:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Not authenticated",
            headers={"WWW-Authenticate": "Basic"},
        )
    if credentials.username != username or credentials.password != password:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid authentication credentials",
            headers={"WWW-Authenticate": "Basic"},
        )
    return credentials.username


app.include_router(feed.router, dependencies=[Depends(get_current_username)])
app.include_router(feed.router, prefix="/index.php/apps/news/api/v1-2", dependencies=[Depends(get_current_username)])
app.include_router(item.router, dependencies=[Depends(get_current_username)])
app.include_router(item.router, prefix="/index.php/apps/news/api/v1-2", dependencies=[Depends(get_current_username)])
app.include_router(folder.router, dependencies=[Depends(get_current_username)])
app.include_router(folder.router, prefix="/index.php/apps/news/api/v1-2", dependencies=[Depends(get_current_username)])
