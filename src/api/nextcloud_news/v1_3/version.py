from fastapi import APIRouter
from pydantic import BaseModel

from src.options import Options

router = APIRouter(tags=["feeds"])


class VersionOut(BaseModel):
    version: str


@router.get("", response_model=VersionOut)
def return_version() -> VersionOut:
    """Return the current version of the API."""
    return VersionOut(version=Options.get().version)
