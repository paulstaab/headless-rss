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


def test_cors_headers_on_status_endpoint(client: TestClient) -> None:
    """Test that CORS headers are present on status endpoint."""
    response = client.get("/status", headers={"Origin": "https://example.com"})
    assert response.status_code == 200
    assert "access-control-allow-origin" in response.headers
    assert response.headers["access-control-allow-origin"] == "*"


def test_cors_headers_on_nextcloud_endpoint(client: TestClient) -> None:
    """Test that CORS headers are present on Nextcloud News API endpoints."""
    response = client.get(
        "/index.php/apps/news/api/v1-3/version",
        headers={"Origin": "https://example.com"},
    )
    assert response.status_code == 200
    assert "access-control-allow-origin" in response.headers
    assert response.headers["access-control-allow-origin"] == "*"


def test_cors_preflight_request(client: TestClient) -> None:
    """Test that CORS preflight requests are handled correctly."""
    response = client.options(
        "/status",
        headers={
            "Origin": "https://example.com",
            "Access-Control-Request-Method": "GET",
        },
    )
    assert response.status_code == 200
    assert "access-control-allow-origin" in response.headers
    assert response.headers["access-control-allow-origin"] == "*"
    assert "access-control-allow-methods" in response.headers
