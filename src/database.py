from typing import TYPE_CHECKING

from sqlalchemy import Column, Engine, ForeignKey, Integer, String, create_engine
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column, sessionmaker

if TYPE_CHECKING:
    from pathlib import Path

_engine: Engine | None = None
_session_maker: sessionmaker | None = None


def init(file: Path) -> None:
    """Initialize the database engine and session maker.

    :param file: The path to the SQLite database file.
    """
    global _engine
    global _session_maker
    _engine = create_engine(url=f"sqlite:///{file}", echo=False, connect_args={"check_same_thread": False})
    _session_maker = sessionmaker(autocommit=False, autoflush=False, bind=_engine)


def get_session() -> Session:
    """Get a new database session.

    :returns: A new SQLAlchemy session.
    :raises RuntimeError: If the database has not been initialized.
    """
    if _session_maker is None:
        raise RuntimeError("database.init() must be called before accessing the session")
    return _session_maker()


class Base(DeclarativeBase):
    pass


class Article(Base):
    """
    Article model representing an RSS feed article entry.

    Attributes:
        id (int): Primary key identifier for the article.
        title (str | None): The title of the article.
        content (str | None): The full HTML/text content of the article.
        author (str | None): The author name of the article.
        summary (str | None): A brief summary or description of the article.
        content_hash (str | None): Hash of the article content for duplicate detection.
        enclosure_link (str | None): URL of an attached media file (podcast, video, etc.).
        enclosure_mime (str | None): MIME type of the enclosure (e.g., 'audio/mpeg').
        feed_id (int): Foreign key reference to the parent Feed.
        fingerprint (str | None): Unique fingerprint for identifying duplicate articles.
        guid (str): Globally Unique Identifier from the RSS feed.
        guid_hash (str): Hash of the GUID for efficient lookups.
        last_modified (int): Unix timestamp of when the article was last modified.
        media_description (str | None): Description text for associated media content.
        media_thumbnail (str | None): URL to a thumbnail image for the article.
        pub_date (int | None): Unix timestamp of the article's publication date.
        rtl (bool): Flag indicating if the article content is right-to-left (RTL) text.
        starred (bool): Flag indicating if the article has been starred/bookmarked by user.
        unread (bool): Flag indicating if the article has been read by the user.
        updated_date (int | None): Unix timestamp of when the article was last updated.
        url (str | None): Direct URL/link to the article on the original website.
    """

    __tablename__ = "article"

    id: Mapped[int] = mapped_column(primary_key=True)
    title: Mapped[str | None] = mapped_column(default=None)
    content: Mapped[str | None] = mapped_column(default=None)
    author: Mapped[str | None] = mapped_column(default=None)
    summary: Mapped[str | None] = mapped_column(default=None)
    content_hash: Mapped[str | None] = mapped_column(default=None)
    enclosure_link: Mapped[str | None] = mapped_column(default=None)
    enclosure_mime: Mapped[str | None] = mapped_column(default=None)
    feed_id: Mapped[int] = mapped_column(ForeignKey("feed.id"))
    fingerprint: Mapped[str | None] = mapped_column(default=None)
    guid: Mapped[str]
    guid_hash: Mapped[str]
    last_modified: Mapped[int]
    media_description: Mapped[str | None] = mapped_column(default=None)
    media_thumbnail: Mapped[str | None] = mapped_column(default=None)
    pub_date: Mapped[int | None] = mapped_column(default=None)
    rtl: Mapped[bool] = mapped_column(default=False)
    starred: Mapped[bool] = mapped_column(default=False)
    unread: Mapped[bool] = mapped_column(default=False)
    updated_date: Mapped[int | None] = mapped_column(default=None)
    url: Mapped[str | None] = mapped_column(default=None)


class Feed(Base):
    __tablename__ = "feed"

    id: Mapped[int] = mapped_column(primary_key=True)
    url: Mapped[str] = mapped_column(unique=True)
    title: Mapped[str | None] = mapped_column(default=None)
    favicon_link: Mapped[str | None] = mapped_column(default=None)
    added: Mapped[int]
    next_update_time: Mapped[int | None] = mapped_column(default=None)
    folder_id: Mapped[int] = mapped_column(ForeignKey("folder.id"), nullable=False)
    ordering: Mapped[int] = mapped_column(default=0)
    link: Mapped[str | None] = mapped_column(default=None)
    pinned: Mapped[bool] = mapped_column(default=False)
    update_error_count: Mapped[int] = mapped_column(default=0)
    last_update_error: Mapped[str | None] = mapped_column(default=None)
    is_mailing_list: Mapped[bool] = mapped_column(default=False)
    last_quality_check: Mapped[int | None] = mapped_column(default=None)
    use_extracted_fulltext: Mapped[bool] = mapped_column(default=False)
    use_llm_summary: Mapped[bool] = mapped_column(default=False)


class Folder(Base):
    __tablename__ = "folder"

    id: Mapped[int] = mapped_column(primary_key=True)
    name: Mapped[str] = mapped_column(unique=True, nullable=False)
    is_root: Mapped[bool] = mapped_column(default=False)


class EmailCredential(Base):
    __tablename__ = "email_credentials"

    id = Column(Integer, primary_key=True, autoincrement=True)
    protocol = Column(String, nullable=False)
    server = Column(String, nullable=False)
    port = Column(Integer, nullable=False)
    username = Column(String, nullable=False)
    password = Column(String, nullable=False)
