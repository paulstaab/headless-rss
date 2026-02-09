"""API"""

from typing import Annotated

from fastapi import APIRouter, Depends, HTTPException, status
from fastapi.security import HTTPBasic, HTTPBasicCredentials

from src.options import Options

from . import feed, folder, item, version

router = APIRouter()

security = HTTPBasic(auto_error=False)


def get_current_username(credentials: Annotated[HTTPBasicCredentials, Depends(security)]):
    """Get the current username from the provided credentials.

    :param credentials: The HTTP basic authentication credentials.
    :returns: The username if authentication is successful.
    :raises HTTPException: If authentication fails.
    """
    options = Options.get()
    username = options.username
    password = options.password
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


router.include_router(feed.router, prefix="/feeds", dependencies=[Depends(get_current_username)])
router.include_router(item.router, prefix="/items", dependencies=[Depends(get_current_username)])
router.include_router(folder.router, prefix="/folders", dependencies=[Depends(get_current_username)])
router.include_router(version.router, prefix="/version")
