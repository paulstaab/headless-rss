from fastapi import FastAPI
from src.database import SessionLocal
from src import crud, schemas
from typing import List

app = FastAPI()


@app.get("/posts/", response_model=List[schemas.Post])
def read_posts(skip: int = 0, limit: int = 10):
    db = SessionLocal()
    posts = crud.get_posts(db, skip=skip, limit=limit)
    return posts
