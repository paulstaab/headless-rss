from fastapi.testclient import TestClient
import base64
import os
import pytest
from src.api.nextcloud_news.app import app

@pytest.fixture
def client() -> TestClient:
    return TestClient(app)

def test_no_auth(client: TestClient) -> None:
    response = client.get("/feeds")
    assert response.status_code == 401
    assert response.json() == {"detail": "Invalid authentication credentials"}

def test_incorrect_auth(client: TestClient) -> None:
    username = "wronguser"
    password = "wrongpass"
    credentials = base64.b64encode(f"{username}:{password}".encode()).decode()
    response = client.get("/feeds", headers={"Authorization": f"Basic {credentials}"})
    assert response.status_code == 401
    assert response.json() == {"detail": "Invalid authentication credentials"}

def test_correct_auth(client: TestClient) -> None:
    username = os.getenv("USERNAME")
    password = os.getenv("PASSWORD")
    credentials = base64.b64encode(f"{username}:{password}".encode()).decode()
    response = client.get("/feeds", headers={"Authorization": f"Basic {credentials}"})
    assert response.status_code == 200
