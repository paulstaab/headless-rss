import time

from src import database


def create(url: str, folder_id: int | None = None) -> database.Feed:
    return database.Feed(url=url, folder_id=folder_id, added=time.time())
