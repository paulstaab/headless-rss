import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient
from src.api.nextcloud_news.v1_3.app import router


@pytest.fixture(scope="module")
def client() -> TestClient:
    """Test client for the FastAPI app."""
    app = FastAPI()
    app.include_router(router)
    return TestClient(app)
