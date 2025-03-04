import logging

from src import database

logger = logging.getLogger(__name__)


class FolderExistsError(Exception):
    """Raised when a folder already exists in the database."""


class InvalidFolderNameError(Exception):
    """Raised when a folder name is invalid."""


def create(name: str) -> database.Folder:
    """Create a new folder.

    :param name: The name of the folder.
    :returns: The created folder.
    :raises FolderExistsError: If a folder with the same name already exists.
    :raises InvalidFolderNameError: If the folder name is invalid.
    """
    logger.info(f"Creating folder with name `{name}`")

    with database.get_session() as db:
        existing_folder = db.query(database.Folder).filter(database.Folder.name == name).first()
    if existing_folder:
        logger.error(f"Folder with name {name} already exists.")
        raise FolderExistsError("Folder already exists")

    if not name:
        logger.error("Folder name is invalid (empty).")
        raise InvalidFolderNameError("Folder name is invalid")

    with database.get_session() as db:
        new_folder = database.Folder(name=name)
        db.add(new_folder)
        db.commit()
        db.refresh(new_folder)

    return new_folder


class NoFolderError(Exception):
    """Raised when a folder is not found in the database."""


def delete(folder_id: int):
    """Delete a folder from the database.

    :param folder_id: The ID of the folder to delete.
    :raises NoFolderError: If the folder is not found.
    """
    with database.get_session() as db:
        folder = db.query(database.Folder).filter(database.Folder.id == folder_id).first()
        if not folder:
            logger.info(f"Failed to delete non-existing folder with ID `{folder_id}`")
            raise NoFolderError("Folder not found.")
        db.query(database.Feed).filter(database.Feed.folder_id == folder_id).delete()
        db.delete(folder)
        db.commit()
        logger.info(f"Successfully deleted folder with ID `{folder_id}`")


def rename(folder_id: int, new_name: str):
    """Rename a folder.

    :param folder_id: The ID of the folder to rename.
    :param
    name: The new name for the folder.
    :raises NoFolderError: If the folder is not found.
    :raises FolderExistsError: If a folder with the same name already exists.
    :raises InvalidFolderNameError: If the folder name is invalid.
    """
    logger.info(f"Renaming folder with ID {folder_id} to `{new_name}`")
    with database.get_session() as db:
        folder = db.query(database.Folder).filter(database.Folder.id == folder_id).first()
        if not folder:
            raise NoFolderError("Folder not found")

        existing_folder = db.query(database.Folder).filter(database.Folder.name == new_name).first()
        if existing_folder:
            logger.error(f"Folder with name {new_name} already exists.")
            raise FolderExistsError("Folder already exists")

        if not new_name:
            logger.error("Folder name is invalid (empty).")
            raise InvalidFolderNameError("Folder name is invalid")

        folder.name = new_name
        db.commit()
