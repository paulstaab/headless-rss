from pydantic import BaseModel, ConfigDict
from pydantic.alias_generators import to_camel


class Feed(BaseModel):
    id: int
    url: str
    title: str | None
    favicon_link: str | None
    added: int
    next_update_time: int | None
    folder_id: int | None
    # unread_count: int
    ordering: int
    link: str | None
    pinned: bool
    update_error_count: int
    last_update_error: str | None

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )


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
    last_modified: str | None
    media_description: str | None
    media_thumbnail: str | None
    pub_date: int | None
    rtl: bool
    starred: bool
    unread: bool
    updated_date: str | None
    url: str | None

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )
