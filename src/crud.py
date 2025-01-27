from typing import List
from sqlalchemy.orm import Session
from src import database, schemas


def get_posts(db: Session, skip: int = 0, limit: int = 10) -> List[database.Post]:
    return db.query(database.Post).offset(skip).limit(limit).all()


def create_post(db: Session, post: schemas.PostCreate) -> database.Post:
    db_post = database.Post(title=post.title, content=post.content)
    db.add(db_post)
    db.commit()
    db.refresh(db_post)
    return db_post
