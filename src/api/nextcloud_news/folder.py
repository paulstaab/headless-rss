import logging

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel, ConfigDict
from pydantic.alias_generators import to_camel

from src import database

router = APIRouter(prefix="/folders", tags=["folders"])
logger = logging.getLogger(__name__)


class Folder(BaseModel):
    id: int
    name: str

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


class FolderGetOut(BaseModel):
    folders: list[Folder]

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.get("/", response_model=FolderGetOut)
def get_folders() -> FolderGetOut:
    db = database.get_session()
    folders = db.query(database.Folder).all()
    return FolderGetOut(folders=[Folder.model_validate(folder) for folder in folders])


class FolderPostIn(BaseModel):
    name: str

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


class FolderPostOut(BaseModel):
    folders: list[Folder]

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.post("/", response_model=FolderPostOut)
def create_folder(input: FolderPostIn):
    db = database.get_session()
    existing_folder = db.query(database.Folder).filter(database.Folder.name == input.name).first()
    if existing_folder:
        logger.error(f"Folder with name {input.name} already exists.")
        raise HTTPException(status_code=409, detail="Folder already exists")

    if not input.name:
        logger.error("Folder name is invalid (empty).")
        raise HTTPException(status_code=422, detail="Folder name is invalid")

    new_folder = database.Folder(name=input.name)
    db.add(new_folder)
    db.commit()
    db.refresh(new_folder)

    return FolderPostOut(folders=[Folder.model_validate(new_folder)])
