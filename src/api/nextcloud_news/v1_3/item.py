"""API Endpoints under /feeds/"""

import enum
import logging

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel, ConfigDict
from pydantic.alias_generators import to_camel

from src import article

logger = logging.getLogger(__name__)


router = APIRouter(tags=["items"])


class Article(BaseModel):
    id: int
    title: str | None
    content: str | None
    author: str | None
    body: str | None
    content_hash: str | None
    enclosure_link: str | None
    enclosure_mime: str | None
    feed_id: int
    fingerprint: str | None
    guid: str
    guid_hash: str
    last_modified: int
    media_description: str | None
    media_thumbnail: str | None
    pub_date: int | None
    rtl: bool
    starred: bool
    unread: bool
    updated_date: int | None
    url: str | None

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


class ItemGetOut(BaseModel):
    items: list[Article]

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


class FeedSelectionMethod(enum.Enum):
    FEED = 0
    FOLDER = 1
    STARRED = 2
    ALL = 3


@router.get("", response_model=ItemGetOut)
def get_items(
    batchSize: int = -1,  # noqa: N803
    offset: int = 0,
    type: int = 1,
    id: int = 0,
    getRead: bool = True,  # noqa: N803
    oldestFirst: bool = False,  # noqa: N803
    lastModified: int = 0,  # noqa: N803
) -> ItemGetOut:
    """Fetch items based on the provided parameters.

    :param batchSize: The number of items to fetch in a batch.
    :param offset: The offset for pagination.
    :param type: The type of selection method (0: FEED, 1: FOLDER, 2: STARRED, 3: ALL).
    :param id: The ID of the feed or folder.
    :param getRead: Whether to include read items.
    :param oldestFirst: Whether to sort items by oldest first.
    :param lastModified: The timestamp to filter items by last modified date.
    :returns: A list of items matching the criteria.
    """
    select_method = FeedSelectionMethod(type)

    if select_method == FeedSelectionMethod.FEED:
        items = article.get_by_feed(
            feed_id=id,
            max_results=batchSize,
            newest_item_id=offset,
            get_read=getRead,
            oldest_first=oldestFirst,
            last_modified=lastModified,
        )
    elif select_method == FeedSelectionMethod.FOLDER:
        items = article.get_by_folder(
            folder_id=id,
            max_results=batchSize,
            newest_item_id=offset,
            get_read=getRead,
            oldest_first=oldestFirst,
            last_modified=lastModified,
        )
    elif select_method == FeedSelectionMethod.STARRED:
        items = article.get_starred(
            max_results=batchSize,
            newest_item_id=offset,
            get_read=getRead,
            oldest_first=oldestFirst,
            last_modified=lastModified,
        )
    elif select_method == FeedSelectionMethod.ALL:
        items = article.get_all(
            max_results=batchSize,
            newest_item_id=offset,
            get_read=getRead,
            oldest_first=oldestFirst,
            last_modified=lastModified,
        )
    else:
        raise NotImplementedError(f"Article selection method {select_method} is not implemented")

    return ItemGetOut(items=[Article.model_validate(item) for item in items])


@router.get("/updated", response_model=ItemGetOut)
def get_updated_items(
    lastModified: int,  # noqa: N803
    type: int,
    id: int,
) -> ItemGetOut:
    """Fetch items based on the provided parameters.

    :param lastModified: The timestamp to filter items by last modified date.
    :param type: The type of selection method (0: FEED, 1: FOLDER, 2: STARRED, 3: ALL).
    :param id: The ID of the feed or folder.
    :returns: A list of items matching the criteria.
    """
    select_method = FeedSelectionMethod(type)

    if select_method == FeedSelectionMethod.FEED:
        items = article.get_by_feed(feed_id=id, oldest_first=False, last_modified=lastModified)
    elif select_method == FeedSelectionMethod.FOLDER:
        items = article.get_by_folder(folder_id=id, oldest_first=False, last_modified=lastModified)
    elif select_method == FeedSelectionMethod.STARRED:
        items = article.get_starred(oldest_first=False, last_modified=lastModified)
    elif select_method == FeedSelectionMethod.ALL:
        items = article.get_all(oldest_first=False, last_modified=lastModified)
    else:
        raise NotImplementedError(f"Article selection method {select_method} is not implemented")

    return ItemGetOut(items=[Article.model_validate(item) for item in items])


@router.post("/{item_id}/read")
def mark_item_as_read(item_id: int):
    """Mark an item as read.

    :param item_id: The ID of the item to mark as read.
    :raises HTTPException: If the item is not found.
    """
    logger.info(f"Marking item {item_id} as read")
    n_articles = article.mark_as_read([item_id])
    if n_articles == 0:
        raise HTTPException(status_code=404, detail="Item not found")


class ItemIDListIn(BaseModel):
    item_ids: list[int]

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.post("/read/multiple")
def mark_multiple_items_as_read(input: ItemIDListIn) -> None:
    """Mark multiple items as read.

    :param input: The list of item IDs to mark as read.
    """
    logger.info(f"Marking multiple items as read: {input.item_ids}")
    article.mark_as_read(input.item_ids)


@router.post("/{item_id}/unread")
def mark_item_as_unread(item_id: int) -> None:
    """Mark an item as unread.

    :param item_id: The ID of the item to mark as unread.
    :raises HTTPException: If the item is not found.
    """
    logger.info(f"Marking item {item_id} as unread")
    n_articles = article.mark_as_unread([item_id])
    if n_articles == 0:
        raise HTTPException(status_code=404, detail="Item not found")


@router.post("/unread/multiple")
def mark_multiple_items_as_unread(input: ItemIDListIn) -> None:
    """Mark multiple items as unread.

    :param input: The list of item IDs to mark as unread.
    """
    logger.info(f"Marking multiple items as unread: {input.item_ids}")
    article.mark_as_unread(input.item_ids)


@router.post("/star/multiple")
def mark_multiple_items_as_starred(input: ItemIDListIn) -> None:
    """Mark multiple items as starred.

    :param input: The list of item IDs to mark as starred.
    :raises HTTPException: If any item is not found.
    """
    logger.info(f"Marking {len(input.item_ids)} items as starred")
    article.mark_as_starred(input.item_ids)


@router.post("/{item_id}/star")
def mark_item_as_starred(item_id: int) -> None:
    """Mark an item as starred.

    :param item_id: The ID of the item to mark as starred.
    :raises HTTPException: If the item is not found.
    """
    logger.info(f"Marking item {item_id} as starred")
    n_articles = article.mark_as_starred([item_id])
    if n_articles == 0:
        raise HTTPException(status_code=404, detail="Item not found")


@router.post("/{item_id}/unstar")
def mark_item_as_unstarred(item_id: int) -> None:
    """Mark an item as unstarred.

    :param item_id: The ID of the item to mark as unstarred.
    :raises HTTPException: If the item is not found.
    """
    logger.info(f"Marking item {item_id} as unstarred")
    n_articles = article.mark_as_unstarred([item_id])
    if n_articles == 0:
        raise HTTPException(status_code=404, detail="Item not found")


@router.post("/unstar/multiple")
def mark_multiple_items_as_unstarred(input: ItemIDListIn) -> None:
    """Mark multiple items as unstarred.

    :param input: The list of item IDs to mark as unstarred.
    :raises HTTPException: If any item is not found.
    """
    logger.info(f"Marking {len(input.item_ids)} items as unstarred")
    article.mark_as_unstarred(input.item_ids)


class MarkAllItemsReadIn(BaseModel):
    newest_item_id: int

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


@router.post("/read")
def mark_all_items_as_read(input: MarkAllItemsReadIn):
    """Mark all items as read until the specified item ID.

    :param input: The ID of the newest item to mark as read.
    """
    logger.info(f"Marking all items as read until {input.newest_item_id}")
    article.mark_all_as_read(input.newest_item_id)
