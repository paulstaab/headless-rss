import pytest
from fastapi.testclient import TestClient
from src.api.app import app


@pytest.fixture(scope="module")
def client() -> TestClient:
    """Test client for the FastAPI app."""
    return TestClient(app)


def test_nextcloud_v1_2_access(client: TestClient) -> None:
    response = client.get("/index.php/apps/news/api/v1-2/version")
    assert response.status_code == 200


def test_status_endpoint(client: TestClient) -> None:
    response = client.get("/status")
    assert response.status_code == 200
    assert response.json() == {"status": "ok"}
