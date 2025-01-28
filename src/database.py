from pathlib import Path

from sqlalchemy import Engine, create_engine
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column, sessionmaker

_engine: Engine | None = None
_session_maker: sessionmaker | None = None


def init(file: Path | None) -> None:
    global _engine
    global _session_maker
    sqlalchemy_url = f"sqlite:///{file}" if file else "sqlite:///:memory:"
    _engine = create_engine(
        url=sqlalchemy_url, echo=True, connect_args={"check_same_thread": False}
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
    url: Mapped[str]
    title: Mapped[str]
    favicon_link: Mapped[str]
    added: Mapped[int]
    next_update_time: Mapped[int]
    folder_id: Mapped[int]
    unread_count: Mapped[int]
    ordering: Mapped[int]
    link: Mapped[str]
    pinned: Mapped[bool]
    update_error_count: Mapped[int]
    last_update_error: Mapped[str]
