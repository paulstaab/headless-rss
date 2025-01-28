from sqlalchemy import create_engine
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column, sessionmaker

SQLALCHEMY_DATABASE_URL = "sqlite:///./test.db"

engine = create_engine(
    SQLALCHEMY_DATABASE_URL, echo=True, connect_args={"check_same_thread": False}
)

SessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)


class Base(DeclarativeBase):
    pass


class Post(Base):
    __tablename__ = "posts"

    id: Mapped[int] = mapped_column(primary_key=True)
    title: Mapped[str]
    content: Mapped[str]


class Feed(Base):
    __tablename__ = "feeds"

    id: Mapped[int] = mapped_column(primary_key=True)
    url: Mapped[str]
    title: Mapped[str]
    faviconLink: Mapped[str]
    added: Mapped[int]
    nextUpdateTime: Mapped[int]
    folderId: Mapped[int]
    unreadCount: Mapped[int]
    ordering: Mapped[int]
    link: Mapped[str]
    pinned: Mapped[bool]
    updateErrorCount: Mapped[int]
    lastUpdateError: Mapped[str]
