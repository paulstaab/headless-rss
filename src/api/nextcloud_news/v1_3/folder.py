import logging

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel, ConfigDict
from pydantic.alias_generators import to_camel

from src import database, folder

router = APIRouter(tags=["folders"])
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
    """Fetch all folders from the database.

    :returns: A list of all folders.
    """
    logger.info("Fetching all folders")
    folders = folder.get_all(include_root=False)
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
    """Create a new folder in the database."""
    try:
        new_folder = folder.create(input.name)
    except folder.FolderExistsError:
        raise HTTPException(status_code=409, detail="Folder already exists") from None
    except folder.InvalidFolderNameError:
        raise HTTPException(status_code=422, detail="Folder name is invalid") from None

    return FolderPostOut(folders=[Folder.model_validate(new_folder)])


@router.delete("/{folder_id}")
def delete_folder(folder_id: int):
    """Delete a folder from the database."""
    try:
        folder.delete(folder_id)
    except folder.NoFolderError:
        raise HTTPException(status_code=404, detail="Folder not found") from None


class FolderPutIn(BaseModel):
    name: str

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.put("/{folder_id}")
def rename_folder(folder_id: int, input: FolderPutIn):
    """Rename a folder.

    :param folder_id: The ID of the folder to rename.
    :param input: The new name for the folder.
    :raises HTTPException: If the folder is not found or the name is invalid.
    """
    try:
        folder.rename(folder_id, input.name)
    except folder.NoFolderError:
        raise HTTPException(status_code=404, detail="Folder not found") from None
    except folder.InvalidFolderNameError:
        raise HTTPException(status_code=422, detail="Folder name is invalid") from None
    except folder.FolderExistsError:
        raise HTTPException(status_code=409, detail="Folder already exists") from None


class MarkItemsReadIn(BaseModel):
    newest_item_id: int

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.post("/{folder_id}/read")
def mark_items_read(folder_id: int, input: MarkItemsReadIn):
    """Mark items as read in a folder.

    :param folder_id: The ID of the folder.
    :param input: The ID of the newest item to mark as read.
    :raises HTTPException: If the folder is not found.
    """
    logger.info(f"Marking items as read in folder with ID {folder_id} until item ID {input.newest_item_id}")
    with database.get_session() as db:
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
