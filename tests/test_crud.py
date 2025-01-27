import pytest
from sqlalchemy import MetaData
from sqlalchemy.orm import Session
from src import crud, schemas
from src.database import Base, SessionLocal

@pytest.fixture
def db():
    db = SessionLocal()
    Base.metadata.create_all(bind=db.get_bind())
    try:
        yield db
    finally:
        db.close()

def test_create_post(db: Session) -> None:
    # Create test post data
    post_data = schemas.PostCreate(title="Test Post", content="Test Content")
    
    # Call create_post function
    created_post = crud.create_post(db=db, post=post_data)
    
    # Assert post was created with correct data
    assert created_post.title == "Test Post"
    assert created_post.content == "Test Content"
    assert created_post.id is not None
