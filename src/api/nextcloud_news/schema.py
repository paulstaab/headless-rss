from pydantic import BaseModel


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
    folder_id: int
    unread_count: int
    ordering: int
    link: str
    pinned: bool
    update_error_count: int
    last_update_error: str

    model_config = {"from_attributes": True}
