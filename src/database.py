from pathlib import Path

from sqlalchemy import Engine, create_engine
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column, sessionmaker

_engine: Engine | None = None
_session_maker: sessionmaker | None = None


def init(file: Path) -> None:
    global _engine
    global _session_maker
    _engine = create_engine(
        url=f"sqlite:///{file}", echo=True, connect_args={"check_same_thread": False}
    )
    _session_maker = sessionmaker(autocommit=False, autoflush=False, bind=_engine)


def get_session() -> Session:
    if _session_maker is None:
        raise RuntimeError("database.init() must be called before accessing the session")
    return _session_maker()


class Base(DeclarativeBase):
    pass


class Article(Base):
    __tablename__ = "article"

    id: Mapped[int] = mapped_column(primary_key=True)
    title: Mapped[str]
    content: Mapped[str]


class Feed(Base):
    __tablename__ = "feed"

    id: Mapped[int] = mapped_column(primary_key=True)
    url: Mapped[str] = mapped_column(unique=True)
    title: Mapped[str | None] = mapped_column(default=None)
    favicon_link: Mapped[str | None] = mapped_column(default=None)
    added: Mapped[int]
    next_update_time: Mapped[int | None] = mapped_column(default=None)
    folder_id: Mapped[int | None]
    ordering: Mapped[int] = mapped_column(default=0)
    link: Mapped[str | None] = mapped_column(default=None)
    pinned: Mapped[bool] = mapped_column(default=False)
    update_error_count: Mapped[int] = mapped_column(default=0)
    last_update_error: Mapped[str | None] = mapped_column(default=None)
