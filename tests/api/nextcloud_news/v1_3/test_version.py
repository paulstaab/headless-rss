from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from fastapi.testclient import TestClient


def test_get_items(client: TestClient) -> None:
    # when
    response = client.get(
        "/version",
    )
    # then
    assert response.status_code == 200
    assert "version" in response.json()
    assert isinstance(response.json()["version"], str)
