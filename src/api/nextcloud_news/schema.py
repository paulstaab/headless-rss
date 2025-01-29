from pydantic import BaseModel, ConfigDict
from pydantic.alias_generators import to_camel


class PostBase(BaseModel):
    title: str
    content: str


class PostCreate(PostBase):
    pass


class Post(PostBase):
    id: int

    class Config:
        orm_mode = True


class Feed(BaseModel):
    id: int
    url: str
    title: str
    favicon_link: str
    added: int
    next_update_time: int
    folder_id: int | None
    unread_count: int
    ordering: int
    link: str
    pinned: bool
    update_error_count: int
    last_update_error: str

    model_config = {"from_attributes": True}


class FeedCreate(BaseModel):
    url: str
    folder_id: int | None

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        from_attributes=True,
    )
