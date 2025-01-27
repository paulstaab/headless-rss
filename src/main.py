from fastapi import FastAPI

from src import crud, schemas
from src.database import SessionLocal

app = FastAPI()


@app.get("/posts/", response_model=list[schemas.Post])
def read_posts(skip: int = 0, limit: int = 10):
    db = SessionLocal()
    posts = crud.get_posts(db, skip=skip, limit=limit)
    return posts
