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


class FeedBase(BaseModel):
    url: str
    title: str
    faviconLink: str
    added: int
    nextUpdateTime: int
    folderId: int
    unreadCount: int
    ordering: int
    link: str
    pinned: bool
    updateErrorCount: int
    lastUpdateError: str


class FeedCreate(FeedBase):
    pass


class Feed(FeedBase):
    id: int

    class Config:
        orm_mode = True
