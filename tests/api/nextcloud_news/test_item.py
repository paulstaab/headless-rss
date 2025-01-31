from fastapi.testclient import TestClient


def test_get_items(client: TestClient, feed_server) -> None:
    # given
    response = client.post(
        "/feeds/",
        json={
            "url": feed_server.url_for("/atom.xml"),
            "folderId": None,
        },
    )
    feed_id = response.json()["feeds"][0]["id"]

    # when
    response = client.get(
        "/items/",
        params={
            "batchSize": 10,
            "offset": 0,
            "type": 1,
            "id": feed_id,
            "getRead": True,
            "oldestFirst": False,
        },
    )

    # then
    assert response.status_code == 200
    items = response.json()["items"]
    assert len(items) == 1
