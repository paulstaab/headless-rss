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


@router.get("", response_model=FolderGetOut)
def get_folders() -> FolderGetOut:
    logger.info("Fetching all folders")
    db = database.get_session()
    folders = db.query(database.Folder).filter(database.Folder.id > 0).all()
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


@router.post("", response_model=FolderPostOut)
def create_folder(input: FolderPostIn):
    logger.info(f"Creating folder with name `{input.name}`")
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


@router.delete("/{folder_id}")
def delete_folder(folder_id: int):
    logger.info(f"Deleting folder with ID {folder_id}")
    db = database.get_session()
    folder = db.query(database.Folder).filter(database.Folder.id == folder_id).first()
    if not folder:
        raise HTTPException(status_code=404, detail="Folder not found")
    db.query(database.Feed).filter(database.Feed.folder_id == folder_id).delete()
    db.delete(folder)
    db.commit()


class FolderPutIn(BaseModel):
    name: str

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.put("/{folder_id}")
def rename_folder(folder_id: int, input: FolderPutIn):
    logger.info(f"Renaming folder with ID {folder_id} to `{input.name}`")
    db = database.get_session()
    folder = db.query(database.Folder).filter(database.Folder.id == folder_id).first()
    if not folder:
        raise HTTPException(status_code=404, detail="Folder not found")

    existing_folder = db.query(database.Folder).filter(database.Folder.name == input.name).first()
    if existing_folder:
        logger.error(f"Folder with name {input.name} already exists.")
        raise HTTPException(status_code=409, detail="Folder already exists")

    if not input.name:
        logger.error("Folder name is invalid (empty).")
        raise HTTPException(status_code=422, detail="Folder name is invalid")

    folder.name = input.name
    db.commit()
    db.refresh(folder)


class MarkItemsReadIn(BaseModel):
    newest_item_id: int

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.post("/{folder_id}/read")
def mark_items_read(folder_id: int, input: MarkItemsReadIn):
    logger.info(f"Marking items as read in folder with ID {folder_id} until item ID {input.newest_item_id}")
    db = database.get_session()
    folder = db.query(database.Folder).filter(database.Folder.id == folder_id).first()
    if not folder:
        raise HTTPException(status_code=404, detail="Folder not found")

    items = (
        db.query(database.Article)
        .join(database.Feed)
        .filter(database.Feed.folder_id == folder_id)
        .filter(database.Article.id <= input.newest_item_id)
        .all()
    )
    for item in items:
        item.unread = False
    db.commit()
