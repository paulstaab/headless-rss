from sqlalchemy.orm import Session

from src import database, schemas


def get_posts(db: Session, skip: int = 0, limit: int = 10) -> list[database.Post]:
    return db.query(database.Post).offset(skip).limit(limit).all()


def create_post(db: Session, post: schemas.PostCreate) -> database.Post:
    db_post = database.Post(title=post.title, content=post.content)
    db.add(db_post)
    db.commit()
    db.refresh(db_post)
    return db_post


def get_feeds(db: Session, skip: int = 0, limit: int = 10) -> list[database.Feed]:
    return db.query(database.Feed).offset(skip).limit(limit).all()


def create_feed(db: Session, feed: schemas.FeedCreate) -> database.Feed:
    db_feed = database.Feed(
        url=feed.url,
        title=feed.title,
        faviconLink=feed.faviconLink,
        added=feed.added,
        nextUpdateTime=feed.nextUpdateTime,
        folderId=feed.folderId,
        unreadCount=feed.unreadCount,
        ordering=feed.ordering,
        link=feed.link,
        pinned=feed.pinned,
        updateErrorCount=feed.updateErrorCount,
        lastUpdateError=feed.lastUpdateError,
    )
    db.add(db_feed)
    db.commit()
    db.refresh(db_feed)
    return db_feed
