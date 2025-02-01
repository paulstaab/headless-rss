from fastapi.testclient import TestClient
from src.feed import update_all


def test_update_all_feeds(client: TestClient, feed_server) -> None:
    # given
    client.post(
        "/feeds",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    # when
    update_all()
    # then
    response = client.get("/items", params={"type": 3})
    items = response.json()["items"]
    assert len(items) == 1
